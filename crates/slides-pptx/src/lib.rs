//! PPTX load and save (native format).
//!
//! This crate is the native format boundary for 900Slides. It converts a PPTX
//! package into a [`slides_core::Deck`] while preserving every unrecognized
//! OOXML part as a [`slides_core::PassthroughObject`]. Saving rebuilds only the
//! slide parts that have been edited; all other parts are copied byte-for-byte
//! from the original package.

mod error;
mod ledger;
mod load;
mod package;
mod save;
mod session;

#[cfg(test)]
mod tests;

pub use error::{Error, Result};
pub use ledger::{LossLedger, LossWarning};
pub use session::Session;

use slides_core::Deck;

/// Loads a PPTX package from bytes into an editable [`Session`].
///
/// The returned session keeps the original bytes so that a later [`save`]
/// can preserve untouched parts byte-for-byte.
pub fn load(bytes: &[u8]) -> Result<Session> {
    let result = load::load(bytes)?;
    let content_types = {
        let mut archive = load::open_and_validate(bytes)?;
        let xml = load::read_entry_to_string(&mut archive, "[Content_Types].xml")?;
        package::parse_content_types(&xml)?
    };
    Ok(Session::new(
        result.deck,
        bytes.to_vec(),
        result.package_rels,
        content_types,
        result.slide_paths,
        result.manifest_path,
        result.loss_ledger,
    ))
}

/// Saves the current [`Session`] as a PPTX package.
///
/// Only slides in the dirty set are regenerated; all other parts are copied
/// verbatim from the original package bytes.
pub fn save(session: &Session) -> Result<Vec<u8>> {
    save::save(session)
}

/// Returns the crate version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
