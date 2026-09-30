//! Integration tests for this crate.
//!
//! Former top-level `tests/*.rs` binaries are modules of this one target
//! so the crate links once instead of once per file.

#[path = "credit.rs"]
mod credit;
#[path = "dtsm_serde.rs"]
mod dtsm_serde;
#[path = "factor_canonical_contract.rs"]
mod factor_canonical_contract;
#[path = "factor_credit_peel_parity.rs"]
mod factor_credit_peel_parity;
#[path = "factor_multi_asset_config.rs"]
mod factor_multi_asset_config;
#[path = "factor_schema_contract.rs"]
mod factor_schema_contract;
#[path = "factor_strictness.rs"]
mod factor_strictness;
#[path = "fx_delta_vol_tests.rs"]
mod fx_delta_vol_tests;
#[path = "liability_management.rs"]
mod liability_management;
#[path = "lookback_commodity_regressions.rs"]
mod lookback_commodity_regressions;
#[path = "market_data_diff_tests.rs"]
mod market_data_diff_tests;
#[path = "model_audit_regressions.rs"]
mod model_audit_regressions;
#[cfg(feature = "json-schema")]
#[path = "model_schema_contract.rs"]
mod model_schema_contract;
#[path = "portfolio_loss.rs"]
mod portfolio_loss;
#[path = "production_audit.rs"]
mod production_audit;
#[path = "production_equity_audit.rs"]
mod production_equity_audit;
#[path = "rates_credit_tree.rs"]
mod rates_credit_tree;
#[path = "recovery_waterfall.rs"]
mod recovery_waterfall;
#[path = "sabr_golden.rs"]
mod sabr_golden;
#[path = "simplicity_parity.rs"]
mod simplicity_parity;
#[path = "vol_cube_tests.rs"]
mod vol_cube_tests;
#[path = "vol_models_quantlib_tests.rs"]
mod vol_models_quantlib_tests;
#[path = "vol_surface_tests.rs"]
mod vol_surface_tests;
