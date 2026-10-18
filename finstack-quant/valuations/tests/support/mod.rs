#[path = "../../../calibration/tests/support/calibration.rs"]
pub mod calibration;
pub mod credit;
pub mod equity_fx_options;
// The IRS builder moved to the rates crate with the IRS tests; this crate and
// the benches include the same file by path.
#[path = "../../rates/tests/support/rates.rs"]
pub mod rates;
