//! ODP import / export conversion boundary.

mod error;
mod load;
mod save;

pub use load::load;
pub use save::save;

/// Returns the crate version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[test]
fn smoke_test() {
    assert!(!version().is_empty());
}
