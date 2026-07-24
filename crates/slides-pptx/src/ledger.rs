//! Per-slide loss ledger for content that could not be fully represented.

/// A single warning about content fidelity.
#[derive(Debug, Clone, PartialEq)]
pub struct LossWarning {
    /// Identifier of the affected slide.
    pub slide_id: String,
    /// Human-readable warning message.
    pub message: String,
}

impl LossWarning {
    /// Creates a new loss warning.
    pub fn new(slide_id: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            slide_id: slide_id.into(),
            message: message.into(),
        }
    }
}

/// Records per-slide warnings for content that could not be fully represented
/// in the editable model.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LossLedger {
    warnings: Vec<LossWarning>,
}

impl LossLedger {
    /// Creates an empty loss ledger.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a warning to the ledger.
    pub fn add(&mut self, warning: LossWarning) {
        self.warnings.push(warning);
    }

    /// Returns all warnings in insertion order.
    pub fn warnings(&self) -> &[LossWarning] {
        &self.warnings
    }

    /// Returns true if there are no warnings.
    pub fn is_empty(&self) -> bool {
        self.warnings.is_empty()
    }

    /// Returns the number of warnings.
    pub fn len(&self) -> usize {
        self.warnings.len()
    }
}
