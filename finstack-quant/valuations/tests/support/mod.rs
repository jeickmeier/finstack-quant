#[path = "../../../calibration/tests/support/calibration.rs"]
pub mod calibration;
pub mod credit;
#[path = "../../equity/tests/support/options.rs"]
pub mod equity_options;
#[path = "../../fx/tests/support/options.rs"]
pub mod fx_options;
// The IRS builder moved to the rates crate with the IRS tests; this crate and
// the benches include the same file by path.
#[path = "../../rates/tests/support/rates.rs"]
pub mod rates;

/// Combined equity + FX option builders re-exported from the family crates.
pub mod equity_fx_options {
    pub use super::equity_options::*;
    pub use super::fx_options::*;
}
