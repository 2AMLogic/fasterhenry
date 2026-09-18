//! fasterhenry — clean-room PEEC (partial-element equivalent circuit)
//! resistance and inductance extraction for 3-D conductor geometries.
//!
//! This crate implements the published method (Ruehli's PEEC; Kamon, Tsuk &
//! White, *FASTHENRY: a multipole-accelerated 3-D inductance extraction
//! program*, IEEE T-MTT 1994; Grover/Rosa closed forms for filament partial
//! inductances) from the literature. It contains no code derived from MIT's
//! FastHenry/FastCap, whose license permits only internal, noncommercial use
//! and forbids redistribution. See CONTRIBUTING.md before adding anything.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// Crate version, for CLI `--version` and JSON output provenance.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
