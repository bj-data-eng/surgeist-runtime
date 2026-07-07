//! App runtime orchestration contracts for Surgeist.
//!
//! This crate intentionally starts with a narrow public surface. Runtime owns
//! app-plane coordination contracts; root `surgeist` wires those contracts to
//! concrete template, CSS, style, retained, text, layout, render, window, and
//! task crates.

/// Returns the crate identity while the runtime API is being designed.
#[must_use]
pub const fn crate_name() -> &'static str {
    "surgeist-runtime"
}

#[cfg(test)]
mod tests {
    #[test]
    fn exposes_crate_identity() {
        assert_eq!(super::crate_name(), "surgeist-runtime");
    }
}
