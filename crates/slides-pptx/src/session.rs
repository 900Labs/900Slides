//! Editing session that binds a loaded deck to its original PPTX bytes.

use std::collections::{HashMap, HashSet};

use slides_core::{ChartShape, Command, CommandBus};

use crate::error::Result;
use crate::ledger::LossLedger;
use crate::package::{ContentTypes, Rel};
use crate::Deck;

/// An editing session that owns the in-memory deck and the original package
/// bytes needed for a lossless save.
#[derive(Debug)]
pub struct Session {
    /// The editable deck model.
    pub deck: Deck,
    /// Original PPTX bytes used to preserve untouched parts.
    pub(crate) original_bytes: Vec<u8>,
    /// Package relationships from `_rels/.rels`.
    pub(crate) package_rels: Vec<Rel>,
    /// Parsed `[Content_Types].xml`.
    pub(crate) content_types: ContentTypes,
    /// Map of slide id to original part path.
    pub(crate) slide_paths: HashMap<String, String>,
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
    /// Chart part paths that have been edited and need patching on save.
    pub(crate) dirty_charts: HashSet<String>,
    /// For each slide (keyed by id), a map from shape index to the chart part
    /// path that backs the chart shape.
    pub(crate) chart_source_parts: HashMap<String, HashMap<usize, String>>,
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
            package_rels,
            content_types,
            slide_paths,
            slide_media_rids,
            manifest_path,
            manifest_rel_id,
            dirty_slides: HashSet::new(),
            dirty_charts: HashSet::new(),
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
        Self {
            deck,
            original_bytes,
            package_rels,
            content_types,
            slide_paths,
            slide_media_rids,
            manifest_path,
            manifest_rel_id,
            dirty_slides: HashSet::new(),
            dirty_charts: HashSet::new(),
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
        if self.slide_paths.contains_key(slide_id) {
            self.dirty_slides.insert(slide_id.to_string());
        }
    }

    /// Returns the set of dirty slide ids.
    pub fn dirty_slides(&self) -> &HashSet<String> {
        &self.dirty_slides
    }

    /// Applies a command transactionally and tracks dirty slides and chart parts.
    pub fn execute(&mut self, command: Box<dyn Command>) -> Result<()> {
        let affected = command.affected_slide_ids();
        // Snapshot chart shapes on affected slides so we can detect chart edits.
        let before: HashMap<String, Vec<Option<ChartShape>>> = affected
            .iter()
            .filter_map(|id| {
                let slide = self.deck.slides.iter().find(|s| s.id == *id)?;
                Some((
                    id.clone(),
                    slide
                        .shapes
                        .iter()
                        .map(|s| match s {
                            slides_core::Shape::Chart(c) => Some(c.clone()),
                            _ => None,
                        })
                        .collect(),
                ))
            })
            .collect();

        self.command_bus.apply(command, &mut self.deck)?;

        for id in affected {
            self.mark_slide_dirty(&id);
            // Mark chart parts dirty when chart data, title, or type changed.
            if let Some(before_shapes) = before.get(&id) {
                if let Some(after_slide) = self.deck.slides.iter().find(|s| s.id == id) {
                    for (index, before_chart) in before_shapes.iter().enumerate() {
                        let after_chart = after_slide.shapes.get(index).and_then(|s| match s {
                            slides_core::Shape::Chart(c) => Some(c),
                            _ => None,
                        });
                        if let (Some(before), Some(after)) = (before_chart, after_chart) {
                            if before.chart_type != after.chart_type
                                || before.data != after.data
                                || before.title != after.title
                            {
                                if let Some(parts) = self.chart_source_parts.get(&id) {
                                    if let Some(part) = parts.get(&index) {
                                        self.dirty_charts.insert(part.clone());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Undoes the most recent command and marks affected slides dirty.
    ///
    /// Returns `true` if a command was undone.
    pub fn undo(&mut self) -> bool {
        if let Some(affected) = self.command_bus.undo(&mut self.deck) {
            for id in affected {
                self.mark_slide_dirty(&id);
            }
            true
        } else {
            false
        }
    }

    /// Re-applies the most recently undone command.
    pub fn redo(&mut self) -> bool {
        if let Some(affected) = self.command_bus.redo(&mut self.deck) {
            for id in affected {
                self.mark_slide_dirty(&id);
            }
            true
        } else {
            false
        }
    }

    /// Returns the number of transactions available to redo.
    pub fn redo_len(&self) -> usize {
        self.command_bus.redo_len()
    }

    /// Commits a successful save by replacing the original bytes and clearing
    /// the dirty slide set.
    pub fn commit_save(&mut self, new_bytes: Vec<u8>) {
        self.original_bytes = new_bytes;
        self.dirty_slides.clear();
        self.dirty_charts.clear();
    }

    /// Returns the number of transactions available to undo.
    pub fn undo_len(&self) -> usize {
        self.command_bus.undo_len()
    }
}
