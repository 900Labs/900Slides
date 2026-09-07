//! Editing session that binds a loaded deck to its original PPTX bytes.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::io::{Cursor, Write};
use std::sync::Arc;

use slides_core::{Command, CommandBus};

use crate::error::Result;
use crate::ledger::LossLedger;
use crate::package::{ContentTypes, Rel};
use crate::Deck;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct ShapeIdMapping {
    pub slide_path: String,
    pub model_id: String,
    pub package_id: String,
}

/// An editing session that owns the in-memory deck and the original package
/// bytes needed for a lossless save.
#[derive(Debug)]
pub struct Session {
    /// The editable deck model.
    pub deck: Deck,
    /// Original PPTX bytes used to preserve untouched parts.
    pub(crate) original_bytes: Vec<u8>,
    /// Deleted slide XML and relationship parts retained only for session undo.
    /// They are excluded from saved decks until that slide is restored.
    retained_slide_parts: HashMap<String, Arc<[u8]>>,
    /// Package relationships from `_rels/.rels`.
    pub(crate) package_rels: Vec<Rel>,
    /// Parsed `[Content_Types].xml`.
    pub(crate) content_types: ContentTypes,
    /// Map of slide id to original part path.
    pub(crate) slide_paths: HashMap<String, String>,
    /// Stable model shape IDs mapped to schema-valid numeric OOXML IDs.
    pub(crate) shape_package_ids: HashMap<String, HashMap<String, String>>,
    /// For each slide (keyed by id), a map from a media content key (into
    /// `deck.media`) to the OOXML relationship id that resolves it. Used by the
    /// saver to emit `<a:blip r:embed="...">` for modeled images and to recognize
    /// newly inserted images.
    pub(crate) slide_media_rids: HashMap<String, HashMap<String, String>>,
    /// Path where the 900Slides manifest is (or will be) stored.
    pub(crate) manifest_path: String,
    /// Existing manifest relationship id, if any.
    pub(crate) manifest_rel_id: Option<String>,
    /// Slide ids that have been edited and need regeneration on save.
    pub(crate) dirty_slides: HashSet<String>,
    /// For each slide (keyed by id), a map from stable shape id to the chart
    /// part path that backs it. Shape positions can change after deletions.
    pub(crate) chart_source_parts: HashMap<String, HashMap<String, String>>,
    /// Original bytes of every chart part encountered during load, keyed by part
    /// path. Used by the saver to preserve unedited chart XML byte-for-byte.
    pub(crate) original_chart_bytes: HashMap<String, Vec<u8>>,
    /// For each slide (keyed by id), a map from chart part path to the
    /// relationship id that resolves it.
    pub(crate) slide_chart_rids: HashMap<String, HashMap<String, String>>,
    /// Command bus for transactional edits and undo.
    command_bus: CommandBus,
    /// Loss ledger from load.
    loss_ledger: LossLedger,
}

impl Session {
    /// Creates a new session from its components.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        deck: Deck,
        original_bytes: Vec<u8>,
        package_rels: Vec<Rel>,
        content_types: ContentTypes,
        slide_paths: HashMap<String, String>,
        slide_media_rids: HashMap<String, HashMap<String, String>>,
        manifest_path: Option<String>,
        loss_ledger: LossLedger,
    ) -> Self {
        let manifest_path = manifest_path.unwrap_or_else(|| "customXml/item1.xml".to_string());
        let manifest_rel_id = package_rels
            .iter()
            .find(|r| r.rel_type == crate::package::REL_TYPE_MANIFEST)
            .map(|r| r.id.clone());
        Self {
            deck,
            original_bytes,
            retained_slide_parts: HashMap::new(),
            package_rels,
            content_types,
            slide_paths,
            shape_package_ids: HashMap::new(),
            slide_media_rids,
            manifest_path,
            manifest_rel_id,
            dirty_slides: HashSet::new(),
            chart_source_parts: HashMap::new(),
            original_chart_bytes: HashMap::new(),
            slide_chart_rids: HashMap::new(),
            command_bus: CommandBus::default(),
            loss_ledger,
        }
    }

    /// Creates a new session with full chart metadata. Used by the loader.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_with_charts(
        deck: Deck,
        original_bytes: Vec<u8>,
        package_rels: Vec<Rel>,
        content_types: ContentTypes,
        slide_paths: HashMap<String, String>,
        shape_package_ids: HashMap<String, HashMap<String, String>>,
        slide_media_rids: HashMap<String, HashMap<String, String>>,
        chart_source_parts: HashMap<String, HashMap<usize, String>>,
        original_chart_bytes: HashMap<String, Vec<u8>>,
        slide_chart_rids: HashMap<String, HashMap<String, String>>,
        manifest_path: Option<String>,
        loss_ledger: LossLedger,
    ) -> Self {
        let manifest_path = manifest_path.unwrap_or_else(|| "customXml/item1.xml".to_string());
        let manifest_rel_id = package_rels
            .iter()
            .find(|r| r.rel_type == crate::package::REL_TYPE_MANIFEST)
            .map(|r| r.id.clone());
        let chart_source_parts = chart_source_parts
            .into_iter()
            .filter_map(|(slide_id, parts)| {
                let slide = deck.slide(&slide_id)?;
                let parts = parts
                    .into_iter()
                    .filter_map(|(index, part)| {
                        Some((slide.shapes.get(index)?.id().to_string(), part))
                    })
                    .collect();
                Some((slide_id, parts))
            })
            .collect();
        Self {
            deck,
            original_bytes,
            retained_slide_parts: HashMap::new(),
            package_rels,
            content_types,
            slide_paths,
            shape_package_ids,
            slide_media_rids,
            manifest_path,
            manifest_rel_id,
            dirty_slides: HashSet::new(),
            chart_source_parts,
            original_chart_bytes,
            slide_chart_rids,
            command_bus: CommandBus::default(),
            loss_ledger,
        }
    }

    /// Returns a reference to the deck.
    pub fn deck(&self) -> &Deck {
        &self.deck
    }

    /// Returns a mutable reference to the deck.
    pub fn deck_mut(&mut self) -> &mut Deck {
        &mut self.deck
    }

    /// Returns the loss ledger.
    pub fn loss_ledger(&self) -> &LossLedger {
        &self.loss_ledger
    }

    /// Marks a slide as dirty so its XML will be regenerated on save.
    pub fn mark_slide_dirty(&mut self, slide_id: &str) {
        // New slides do not have an OOXML part path until the first save. They
        // are still dirty model state and must be emitted by the saver.
        if self.deck.slide(slide_id).is_some() {
            self.dirty_slides.insert(slide_id.to_string());
        }
    }

    /// Returns the set of dirty slide ids.
    pub fn dirty_slides(&self) -> &HashSet<String> {
        &self.dirty_slides
    }

    /// Applies a command transactionally and tracks dirty slides. Chart content
    /// is compared with the saved package during save, including after undo.
    pub fn execute(&mut self, command: Box<dyn Command>) -> Result<()> {
        let affected = command.affected_slide_ids();
        let affects_content = command.affects_slide_content();
        self.command_bus.apply(command, &mut self.deck)?;
        self.prune_retained_sources();

        if !affects_content {
            self.dirty_slides.retain(|id| self.deck.slide(id).is_some());
            return Ok(());
        }

        if affected.is_empty() {
            // Deck-level commands (SetTemplate, SetHighContrast, etc.) return
            // empty affected_slide_ids because they mutate deck-wide state, not
            // a specific slide. Mark ALL slides dirty so the saver regenerates
            // every slide on the next save.
            for slide in &self.deck.slides {
                self.dirty_slides.insert(slide.id.clone());
            }
        } else {
            for id in affected {
                self.mark_slide_dirty(&id);
            }
        }
        Ok(())
    }

    /// Undoes the most recent command and marks affected slides dirty.
    ///
    /// Returns `true` if a command was undone.
    pub fn undo(&mut self) -> bool {
        let affects_content = self.command_bus.undo_affects_slide_content();
        if let Some(affected) = self.command_bus.undo(&mut self.deck) {
            if !affects_content {
                // Presentation order is saved independently of slide content.
            } else if affected.is_empty() {
                for slide in &self.deck.slides {
                    self.dirty_slides.insert(slide.id.clone());
                }
            } else {
                for id in affected {
                    self.mark_slide_dirty(&id);
                }
            }
            self.dirty_slides.retain(|id| self.deck.slide(id).is_some());
            self.prune_retained_sources();
            true
        } else {
            false
        }
    }

    /// Re-applies the most recently undone command.
    pub fn redo(&mut self) -> bool {
        let affects_content = self.command_bus.redo_affects_slide_content();
        if let Some(affected) = self.command_bus.redo(&mut self.deck) {
            if !affects_content {
                // Presentation order is saved independently of slide content.
            } else if affected.is_empty() {
                for slide in &self.deck.slides {
                    self.dirty_slides.insert(slide.id.clone());
                }
            } else {
                for id in affected {
                    self.mark_slide_dirty(&id);
                }
            }
            self.dirty_slides.retain(|id| self.deck.slide(id).is_some());
            self.prune_retained_sources();
            true
        } else {
            false
        }
    }

    /// Returns the number of transactions available to redo.
    pub fn redo_len(&self) -> usize {
        self.command_bus.redo_len()
    }

    fn prune_retained_sources(&mut self) {
        let mut retained_ids = self.command_bus.history_slide_ids();
        retained_ids.extend(self.deck.slides.iter().map(|slide| slide.id.clone()));
        self.slide_paths.retain(|id, _| retained_ids.contains(id));
        self.shape_package_ids
            .retain(|id, _| retained_ids.contains(id));
        self.slide_media_rids
            .retain(|id, _| retained_ids.contains(id));
        self.chart_source_parts
            .retain(|id, _| retained_ids.contains(id));
        self.slide_chart_rids
            .retain(|id, _| retained_ids.contains(id));
        let retained_paths: HashSet<String> = self
            .slide_paths
            .values()
            .flat_map(|path| [path.clone(), crate::load::rels_path_for(path)])
            .collect();
        self.retained_slide_parts
            .retain(|path, _| retained_paths.contains(path));
        let chart_paths: HashSet<&String> = self
            .chart_source_parts
            .values()
            .flat_map(|parts| parts.values())
            .collect();
        self.original_chart_bytes
            .retain(|path, _| chart_paths.contains(path));
    }

    /// Restores source parts to the save input only when their deleted slide
    /// has been undone. The actual saved package still excludes deleted slides.
    pub(crate) fn source_bytes_for_save(&self) -> Result<Cow<'_, [u8]>> {
        let mut restored = Vec::new();
        for slide in &self.deck.slides {
            if let Some(path) = self.slide_paths.get(&slide.id) {
                for part in [path.clone(), crate::load::rels_path_for(path)] {
                    if let Some(bytes) = self.retained_slide_parts.get(&part) {
                        restored.push((part, bytes));
                    }
                }
            }
        }
        if restored.is_empty() {
            return Ok(Cow::Borrowed(&self.original_bytes));
        }
        restored.sort_by(|(left, _), (right, _)| left.cmp(right));
        let mut archive = zip::ZipArchive::new(Cursor::new(self.original_bytes.as_slice()))?;
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let existing: HashSet<String> = archive.file_names().map(str::to_string).collect();
        for index in 0..archive.len() {
            writer.raw_copy_file(archive.by_index(index)?)?;
        }
        for (part, bytes) in restored {
            if !existing.contains(&part) {
                writer.start_file(
                    part,
                    zip::write::FileOptions::<()>::default()
                        .compression_method(zip::CompressionMethod::Deflated),
                )?;
                writer.write_all(bytes)?;
            }
        }
        Ok(Cow::Owned(writer.finish()?.into_inner()))
    }

    /// Commits a successful save by replacing the original bytes and clearing
    /// the dirty slide set.
    ///
    /// A saved inserted slide has a newly assigned package part path. Reload
    /// the package metadata and map it back onto the current in-memory slide
    /// ids by deck order so a later edit in the same session is saved again
    /// instead of being treated as an unpersistable new slide.
    pub fn commit_save(&mut self, new_bytes: Vec<u8>) -> Result<()> {
        let loaded = crate::load::load(&new_bytes)?;
        let (content_types, saved_part_paths) = crate::load::open_and_validate(&new_bytes)
            .and_then(|mut archive| {
                let xml = crate::load::read_entry_to_string(&mut archive, "[Content_Types].xml")?;
                let content_types = crate::package::parse_content_types(&xml)?;
                let paths: HashSet<String> = archive.file_names().map(str::to_string).collect();
                Ok((content_types, paths))
            })?;
        if loaded.deck.slides.len() != self.deck.slides.len() {
            return Err(crate::error::Error::Save(
                "saved package reopened with a different slide count; refusing to discard dirty state"
                    .to_string(),
            ));
        }
        let mut retained_slide_parts = self.retained_slide_parts.clone();
        retained_slide_parts.retain(|part, _| !saved_part_paths.contains(part));
        let mut previous_archive = crate::load::open_and_validate(&self.original_bytes)?;
        for path in self.slide_paths.values() {
            if !saved_part_paths.contains(path) {
                for part in [path.clone(), crate::load::rels_path_for(path)] {
                    if let Ok(bytes) =
                        crate::load::read_entry_to_bytes(&mut previous_archive, &part)
                    {
                        retained_slide_parts.insert(part, Arc::from(bytes));
                    }
                }
            }
        }
        let mut slide_paths = self.slide_paths.clone();
        let mut shape_package_ids = self.shape_package_ids.clone();
        let mut slide_media_rids = self.slide_media_rids.clone();
        // Deleted chart parts are preserved in the package. Keep their stable
        // associations so undo after a save can restore the original XML.
        let mut chart_source_parts = self.chart_source_parts.clone();
        for parts in chart_source_parts.values_mut() {
            parts.retain(|_, part| saved_part_paths.contains(part));
        }
        let mut slide_chart_rids = self.slide_chart_rids.clone();

        for (current, reloaded) in self.deck.slides.iter().zip(&loaded.deck.slides) {
            let path = loaded.slide_paths.get(&reloaded.id).ok_or_else(|| {
                crate::error::Error::Save(
                    "saved package is missing a slide path; refusing to discard dirty state"
                        .to_string(),
                )
            })?;
            slide_paths.insert(current.id.clone(), path.clone());
            if let Some(ids) = loaded.shape_package_ids.get(&reloaded.id) {
                shape_package_ids.insert(current.id.clone(), ids.clone());
            }
            if let Some(rids) = loaded.slide_media_rids.get(&reloaded.id) {
                slide_media_rids.insert(current.id.clone(), rids.clone());
            }
            if let Some(parts) = loaded.chart_source_parts.get(&reloaded.id) {
                let current_parts = chart_source_parts.entry(current.id.clone()).or_default();
                for (index, part) in parts {
                    let shape = current.shapes.get(*index).ok_or_else(|| {
                        crate::error::Error::Save(
                            "saved chart has no matching model shape; refusing to discard dirty state"
                                .to_string(),
                        )
                    })?;
                    current_parts.insert(shape.id().to_string(), part.clone());
                }
            }
            if let Some(rids) = loaded.slide_chart_rids.get(&reloaded.id) {
                slide_chart_rids.insert(current.id.clone(), rids.clone());
            }
        }

        self.package_rels = loaded.package_rels;
        self.content_types = content_types;
        self.slide_paths = slide_paths;
        self.shape_package_ids = shape_package_ids;
        self.slide_media_rids = slide_media_rids;
        self.chart_source_parts = chart_source_parts;
        self.original_chart_bytes
            .retain(|part, _| saved_part_paths.contains(part));
        self.original_chart_bytes
            .extend(loaded.original_chart_bytes);
        self.slide_chart_rids = slide_chart_rids;
        self.manifest_path = loaded
            .manifest_path
            .unwrap_or_else(|| "customXml/item1.xml".to_string());
        self.manifest_rel_id = self
            .package_rels
            .iter()
            .find(|rel| rel.rel_type == crate::package::REL_TYPE_MANIFEST)
            .map(|rel| rel.id.clone());
        self.original_bytes = new_bytes;
        self.retained_slide_parts = retained_slide_parts;
        self.dirty_slides.clear();
        self.prune_retained_sources();
        Ok(())
    }

    /// Returns the number of transactions available to undo.
    pub fn undo_len(&self) -> usize {
        self.command_bus.undo_len()
    }
}

#[cfg(test)]
mod history_tests {
    use super::*;
    use slides_core::{DeleteSlide, InsertSlide, SetHighContrast, Slide};

    #[test]
    fn evicted_deleted_slide_sources_are_released() {
        let mut session = crate::load(&crate::create_blank_pptx()).unwrap();
        let removed_id = session.deck.slides[0].id.clone();
        session
            .execute(Box::new(InsertSlide::new(
                1,
                Slide {
                    id: "remaining".into(),
                    ..Default::default()
                },
            )))
            .unwrap();
        session.commit_save(crate::save(&session).unwrap()).unwrap();
        session
            .execute(Box::new(DeleteSlide::new(removed_id.clone())))
            .unwrap();
        session.commit_save(crate::save(&session).unwrap()).unwrap();
        assert!(!session.retained_slide_parts.is_empty());
        for index in 0..CommandBus::MAX_TRANSACTIONS {
            session
                .execute(Box::new(SetHighContrast::new(index % 2 == 0)))
                .unwrap();
        }
        assert!(!session.slide_paths.contains_key(&removed_id));
        assert!(session.retained_slide_parts.is_empty());
        assert_eq!(session.undo_len(), CommandBus::MAX_TRANSACTIONS);
    }
}
