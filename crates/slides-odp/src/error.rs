//! Error types for the ODP reader.

use thiserror::Error;

/// Errors returned by the ODP loader.
#[derive(Debug, Error)]
pub enum Error {
    /// A ZIP read/write error.
    #[error("odp zip error: {0}")]
    Zip(#[from] zip::result::ZipError),
    /// A generic I/O error.
    #[error("odp io error: {0}")]
    Io(#[from] std::io::Error),
    /// XML parsing failed.
    #[error("odp xml error: {0}")]
    Xml(String),
    /// The package does not contain the required part.
    #[error("odp missing part: {0}")]
    MissingPart(String),
    /// The package has an unsupported structure.
    #[error("odp unsupported format: {0}")]
    UnsupportedFormat(String),
}

impl From<quick_xml::Error> for Error {
    fn from(value: quick_xml::Error) -> Self {
        Error::Xml(value.to_string())
    }
}

/// Result type alias for ODP loading.
pub type Result<T> = std::result::Result<T, Error>;
