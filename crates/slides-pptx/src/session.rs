//! Editing session that binds a loaded deck to its original PPTX bytes.

use std::collections::{HashMap, HashSet};

use slides_core::{Command, CommandBus};

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
    /// Path where the 900Slides manifest is (or will be) stored.
    pub(crate) manifest_path: String,
    /// Existing manifest relationship id, if any.
    pub(crate) manifest_rel_id: Option<String>,
    /// Slide ids that have been edited and need regeneration on save.
    pub(crate) dirty_slides: HashSet<String>,
    /// Command bus for transactional edits and undo.
    command_bus: CommandBus,
    /// Loss ledger from load.
    loss_ledger: LossLedger,
}

impl Session {
    /// Creates a new session from its components.
    pub fn new(
        deck: Deck,
        original_bytes: Vec<u8>,
        package_rels: Vec<Rel>,
        content_types: ContentTypes,
        slide_paths: HashMap<String, String>,
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
            manifest_path,
            manifest_rel_id,
            dirty_slides: HashSet::new(),
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

    /// Applies a command transactionally and tracks dirty slides.
    pub fn execute(&mut self, command: Box<dyn Command>) -> Result<()> {
        let affected = command.affected_slide_ids();
        self.command_bus.apply(command, &mut self.deck)?;
        for id in affected {
            self.mark_slide_dirty(&id);
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

    /// Commits a successful save by replacing the original bytes and clearing
    /// the dirty slide set.
    pub fn commit_save(&mut self, new_bytes: Vec<u8>) {
        self.original_bytes = new_bytes;
        self.dirty_slides.clear();
    }

    /// Returns the number of transactions available to undo.
    pub fn undo_len(&self) -> usize {
        self.command_bus.undo_len()
    }
}
