//! Sanitized generated fixtures only.

/// Returns the crate version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[test]
fn smoke_test() {
    assert!(!version().is_empty());
}
