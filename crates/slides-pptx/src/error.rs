//! Errors for the PPTX load/save layer.

use thiserror::Error;

/// Errors that can occur while loading or saving a PPTX package.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// The ZIP archive could not be read.
    #[error("invalid zip archive: {0}")]
    InvalidZip(#[from] zip::result::ZipError),
    /// A ZIP entry was larger than the allowed limit.
    #[error("zip entry exceeds size limit")]
    EntryTooLarge,
    /// The total uncompressed size of the archive exceeded the allowed limit.
    #[error("archive exceeds total size limit")]
    ArchiveTooLarge,
    /// A ZIP entry path attempted to escape the package root.
    #[error("zip entry path is unsafe: {0}")]
    UnsafePath(String),
    /// An XML part could not be parsed.
    #[error("xml error: {0}")]
    Xml(#[from] quick_xml::Error),
    /// An XML attribute value was not valid UTF-8.
    #[error("invalid xml attribute")]
    InvalidAttribute,
    /// A required package relationship was missing.
    #[error("missing package relationship: {0}")]
    MissingRelationship(String),
    /// A required package part was missing.
    #[error("missing package part: {0}")]
    MissingPart(String),
    /// The source file format was not recognized.
    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),
    /// A command from the deck model failed.
    #[error("command error: {0}")]
    Command(#[from] slides_core::CommandError),
    /// A JSON serialization error.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    /// A generic I/O error.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    /// A slide or media part could not be emitted during save.
    #[error("save error: {0}")]
    Save(String),
}

/// Result type alias for the PPTX crate.
pub type Result<T> = std::result::Result<T, Error>;
