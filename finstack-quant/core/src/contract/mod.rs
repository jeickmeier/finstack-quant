//! Dependency-safe persistence contract primitives.
//!
//! This module defines contract identity/version policy, resource limits, and
//! structured diagnostics without depending on domain crates. Higher-level
//! persisted artifacts can therefore share one stable loading vocabulary
//! without introducing dependency cycles.

mod descriptor;
mod diagnostics;
mod limits;
mod load;

pub use descriptor::ContractDescriptor;
pub use diagnostics::{ContractError, Diagnostic, LoadPhase, Severity, ValidationReport};
pub use limits::{DEFAULT_MAX_BYTES, LoadLimits};
pub use load::{check_json_limits, deserialize_json_value, parse_json_value};
