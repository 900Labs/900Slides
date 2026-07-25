//! OOXML package helpers: content types and relationships.

use std::collections::HashMap;

use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, Event};
use quick_xml::Reader;

use crate::error::{Error, Result};

/// Office document relationship type for the presentation part.
pub const REL_TYPE_OFFICE_DOCUMENT: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";
/// Relationship type for theme parts.
pub const REL_TYPE_THEME: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme";
/// Relationship type for slide parts.
pub const REL_TYPE_SLIDE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide";
/// Relationship type for notes slide parts.
pub const REL_TYPE_NOTES_SLIDE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesSlide";
/// 900Slides manifest relationship type.
pub const REL_TYPE_MANIFEST: &str = "http://900labs.github.io/900Slides/1.0/relationships/manifest";

/// Content type for the 900Slides manifest.
pub const CT_MANIFEST: &str = "application/vnd.900labs.900slides.manifest+xml";
/// Namespace for OOXML package relationships.
pub const NS_RELATIONSHIPS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";

/// A package relationship.
#[derive(Debug, Clone, PartialEq)]
pub struct Rel {
    /// Relationship identifier.
    pub id: String,
    /// Relationship type.
    pub rel_type: String,
    /// Target part path or external URL.
    pub target: String,
    /// Whether the target is external to the package.
    pub target_mode: Option<String>,
}

impl Rel {
    /// Returns true if this relationship points to an external resource.
    pub fn is_external(&self) -> bool {
        self.target_mode
            .as_deref()
            .map(|m| m.eq_ignore_ascii_case("External"))
            .unwrap_or(false)
    }

    /// Resolves the relationship target against a base part directory.
    pub fn resolve(&self, base_dir: &str) -> Option<String> {
        if self.is_external() {
            return None;
        }
        let target = self.target.replace('\\', "/");
        if target.starts_with('/') {
            return Some(normalize_package_path(&target));
        }
        Some(normalize_package_path(&format!("{base_dir}/{target}")))
    }
}

fn normalize_package_path(path: &str) -> String {
    let mut parts = Vec::new();
    for part in path.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            parts.pop();
        } else {
            parts.push(part);
        }
    }
    parts.join("/")
}

/// Content type defaults keyed by extension.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ContentTypes {
    /// Default content types by file extension.
    pub defaults: HashMap<String, String>,
    /// Override content types by absolute part name.
    pub overrides: HashMap<String, String>,
}

impl ContentTypes {
    /// Returns the content type for a part path, falling back to defaults.
    pub fn content_type_for(&self, part: &str) -> Option<&str> {
        let normalized = normalize_part_name(part);
        if let Some(ct) = self.overrides.get(&normalized) {
            return Some(ct);
        }
        normalized
            .rsplit_once('.')
            .and_then(|(_, ext)| self.defaults.get(ext).map(String::as_str))
    }

    /// Ensures an override exists for the given part.
    pub fn ensure_override(&mut self, part: &str, content_type: &str) {
        let normalized = normalize_part_name(part);
        self.overrides.insert(normalized, content_type.to_string());
    }
}

fn normalize_part_name(name: &str) -> String {
    let mut s = name.replace('\\', "/");
    if !s.starts_with('/') {
        s.insert(0, '/');
    }
    s
}

fn attr_by_local_name(start: &BytesStart<'_>, name: &str) -> Option<String> {
    for attr in start.attributes() {
        let attr = attr.ok()?;
        if attr.key.local_name().as_ref() == name.as_bytes() {
            let value = attr.unescape_value().ok()?;
            return Some(value.into_owned());
        }
    }
    None
}

/// Parses `[Content_Types].xml`.
pub fn parse_content_types(xml: &str) -> Result<ContentTypes> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut types = ContentTypes::default();

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) | Event::Empty(e) => {
                let local_name = e.local_name();
                let local = std::str::from_utf8(local_name.as_ref())
                    .map_err(|_| Error::InvalidAttribute)?;
                match local {
                    "Default" => {
                        let ext =
                            attr_by_local_name(&e, "Extension").ok_or(Error::InvalidAttribute)?;
                        let ct =
                            attr_by_local_name(&e, "ContentType").ok_or(Error::InvalidAttribute)?;
                        types.defaults.insert(ext.to_lowercase(), ct);
                    }
                    "Override" => {
                        let part =
                            attr_by_local_name(&e, "PartName").ok_or(Error::InvalidAttribute)?;
                        let ct =
                            attr_by_local_name(&e, "ContentType").ok_or(Error::InvalidAttribute)?;
                        types.overrides.insert(normalize_part_name(&part), ct);
                    }
                    _ => {}
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(types)
}

/// Serializes `[Content_Types].xml`.
pub fn write_content_types(types: &ContentTypes) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut writer = quick_xml::Writer::new_with_indent(&mut out, b' ', 2);
        writer.write_event(Event::Decl(BytesDecl::new(
            "1.0",
            Some("UTF-8"),
            Some("yes"),
        )))?;
        let mut types_start = BytesStart::new("Types");
        types_start.push_attribute((
            "xmlns",
            "http://schemas.openxmlformats.org/package/2006/content-types",
        ));
        writer.write_event(Event::Start(types_start))?;

        for (ext, ct) in &types.defaults {
            let mut elem = BytesStart::new("Default");
            elem.push_attribute(("Extension", ext.as_str()));
            elem.push_attribute(("ContentType", ct.as_str()));
            writer.write_event(Event::Empty(elem))?;
        }
        for (part, ct) in &types.overrides {
            let mut elem = BytesStart::new("Override");
            elem.push_attribute(("PartName", part.as_str()));
            elem.push_attribute(("ContentType", ct.as_str()));
            writer.write_event(Event::Empty(elem))?;
        }
        writer.write_event(Event::End(BytesEnd::new("Types")))?;
    }
    Ok(out)
}

/// Parses a `.rels` file.
pub fn parse_rels(xml: &str) -> Result<Vec<Rel>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut rels = Vec::new();

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) | Event::Empty(e) => {
                let local_name = e.local_name();
                let local = std::str::from_utf8(local_name.as_ref())
                    .map_err(|_| Error::InvalidAttribute)?;
                if local == "Relationship" {
                    rels.push(Rel {
                        id: attr_by_local_name(&e, "Id").ok_or(Error::InvalidAttribute)?,
                        rel_type: attr_by_local_name(&e, "Type").ok_or(Error::InvalidAttribute)?,
                        target: attr_by_local_name(&e, "Target").ok_or(Error::InvalidAttribute)?,
                        target_mode: attr_by_local_name(&e, "TargetMode"),
                    });
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(rels)
}

/// Serializes a `.rels` file.
pub fn write_rels(rels: &[Rel]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut writer = quick_xml::Writer::new_with_indent(&mut out, b' ', 2);
        writer.write_event(Event::Decl(BytesDecl::new(
            "1.0",
            Some("UTF-8"),
            Some("yes"),
        )))?;
        let mut rels_start = BytesStart::new("Relationships");
        rels_start.push_attribute(("xmlns", NS_RELATIONSHIPS));
        writer.write_event(Event::Start(rels_start))?;

        for rel in rels {
            let mut elem = BytesStart::new("Relationship");
            elem.push_attribute(("Id", rel.id.as_str()));
            elem.push_attribute(("Type", rel.rel_type.as_str()));
            elem.push_attribute(("Target", rel.target.as_str()));
            if let Some(mode) = &rel.target_mode {
                elem.push_attribute(("TargetMode", mode.as_str()));
            }
            writer.write_event(Event::Empty(elem))?;
        }
        writer.write_event(Event::End(BytesEnd::new("Relationships")))?;
    }
    Ok(out)
}

/// Finds a relationship by type.
pub fn find_rel_by_type<'a>(rels: &'a [Rel], rel_type: &str) -> Option<&'a Rel> {
    rels.iter().find(|r| r.rel_type == rel_type)
}
