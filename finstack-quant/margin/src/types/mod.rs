//! Core margin types and specifications.
//!
//! This module defines the fundamental types for margin and collateral management:
//!
//! - [`CsaSpec`]: Credit Support Annex specification
//! - [`VmParameters`]: Variation margin parameters
//! - [`ImParameters`]: Initial margin parameters
//! - [`EligibleCollateralSchedule`]: Eligible collateral with haircuts
//! - [`MarginCall`]: Margin call event representation
//! - [`OtcMarginSpec`]: OTC derivative margin specification

mod call;
mod collateral;
mod csa;
mod enums;
/// Netting-set identifiers and per-instrument margin aggregation outputs.
pub mod netting;
mod otc;
mod repo_cashflows;
/// Repo-specific margining conventions and helper calculations.
pub mod repo_margin;
mod serde_validation;
pub(crate) mod simm_curvature;
/// SIMM risk-class and sensitivity container types.
pub mod simm_types;
mod thresholds;

fn default_margin_calendar(currency: finstack_quant_core::currency::Currency) -> &'static str {
    use finstack_quant_core::currency::Currency;

    match currency {
        Currency::USD => "usny",
        Currency::EUR => "target2",
        Currency::GBP => "gblo",
        Currency::JPY => "jpto",
        Currency::CHF => "chzh",
        Currency::CAD => "cato",
        Currency::AUD => "auce",
        _ => "weekends_only",
    }
}

pub use call::{MarginCall, MarginCallType};
pub use collateral::{
    CollateralAssetClass, CollateralEligibility, ConcentrationBreach, EligibleCollateralSchedule,
    MaturityConstraints,
};
pub use csa::{CsaSpec, MarginCallTiming};
pub use enums::{ClearingStatus, ImMethodology, MarginTenor};
pub use netting::NettingSetId;
pub use otc::{OtcMarginSpec, SimmCreditClassification};
pub use repo_cashflows::{
    generate_margin_cashflows, generate_margin_interest_cashflows, margin_calls_to_cashflows,
};
pub use repo_margin::{RepoMarginSpec, RepoMarginType};
pub use simm_curvature::SimmCurvatureSensitivity;
pub use simm_types::{
    commodity_bucket_id, ordered_credit_sector_pair, ordered_risk_class_pair, ordered_tenor_pair,
    SimmCreditSector, SimmRiskClass, SimmSensitivities, SimmSensitivitiesJson,
    SIMM_COMMODITY_BUCKET_COUNT, SIMM_TENORS,
};
pub use thresholds::{ImCollateralResult, ImParameters, VmParameters};

#[cfg(test)]
mod tests {
    use super::default_margin_calendar;
    use finstack_quant_core::currency::Currency;

    #[test]
    fn default_margin_calendars_cover_regulatory_currencies_and_fallback() {
        assert_eq!(default_margin_calendar(Currency::USD), "usny");
        assert_eq!(default_margin_calendar(Currency::EUR), "target2");
        assert_eq!(default_margin_calendar(Currency::GBP), "gblo");
        assert_eq!(default_margin_calendar(Currency::JPY), "jpto");
        assert_eq!(default_margin_calendar(Currency::CHF), "chzh");
        assert_eq!(default_margin_calendar(Currency::CAD), "cato");
        assert_eq!(default_margin_calendar(Currency::AUD), "auce");
        assert_eq!(default_margin_calendar(Currency::NZD), "weekends_only");
    }
}
