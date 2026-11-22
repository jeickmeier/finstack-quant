//! Initial-margin orchestration over priceable instruments.

use crate::instruments::Instrument;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::{Error, Result};
pub use finstack_quant_margin::Marginable;
use finstack_quant_margin::SimmVersion;
use finstack_quant_margin::calculators::ImResult;
use finstack_quant_margin::calculators::im::SimmCalculator;
use finstack_quant_margin::constants;
use finstack_quant_margin::types::{ClearingStatus, NettingSetId, OtcMarginSpec};

/// Compute approximate SIMM for an explicitly selected netting set of instruments.
///
/// Each trade must price successfully on the selected market before its
/// product-specific SIMM sensitivity estimator contributes to margin.
/// Product sensitivity approximations carry through to the result.
///
/// # Arguments
/// * `instruments` - Contracts in one netting set. Each instrument contributes
///   at its own contractual notional and direction, without position scaling.
/// * `market` - Market snapshot containing the curves and risk factors the
///   instruments' SIMM estimators require.
/// * `as_of` - Valuation date used for trade sensitivities and the result.
/// * `version` - Embedded SIMM rule set. `None` uses its registered default.
/// * `mpor_days` - Positive margin period of risk in business days. `None`
///   uses the rule set's default.
/// * `currency` - USD calculation currency; `None` resolves to USD.
///   Trades must produce sensitivities in the selected currency.
///
/// # Errors
/// Rejects an empty set, a trade without a SIMM margin implementation,
/// zero MPOR, unsupported calculation currencies or inconsistent sensitivity
/// currencies. Base-price failures and errors returned by the sensitivity
/// estimator propagate.
pub fn calculate_simm(
    instruments: &[&dyn Instrument],
    market: &MarketContext,
    as_of: Date,
    version: Option<SimmVersion>,
    mpor_days: Option<u32>,
    currency: Option<Currency>,
) -> Result<ImResult> {
    let currency = currency.unwrap_or(Currency::USD);
    let marginable = instruments
        .iter()
        .map(|instrument| {
            let trade = instrument.as_marginable().ok_or_else(|| {
                Error::Validation(format!(
                    "Instrument '{}' does not support SIMM margin",
                    instrument.id()
                ))
            })?;
            instrument.value(market, as_of)?;
            Ok(trade)
        })
        .collect::<Result<Vec<_>>>()?;
    let mut calculator = SimmCalculator::new(version.unwrap_or_default())?;
    if let Some(days) = mpor_days {
        if days == 0 {
            return Err(Error::Validation("SIMM MPOR must be positive".into()));
        }
        calculator = calculator.with_mpor(days);
    }
    calculator.calculate_netting_set(marginable, market, as_of, currency)
}

/// Map a time to maturity in years to its ISDA SIMM IR tenor bucket label.
///
/// # Arguments
///
/// * `years_to_maturity` - Remaining tenor in years using the caller's day-count convention.
#[must_use]
pub fn assign_ir_tenor_bucket(years_to_maturity: f64) -> &'static str {
    use constants::tenor_buckets::*;
    match years_to_maturity {
        y if y <= BUCKET_6M => "6M",
        y if y <= BUCKET_1Y => "1Y",
        y if y <= BUCKET_2Y => "2Y",
        y if y <= BUCKET_3Y => "3Y",
        y if y <= BUCKET_5Y => "5Y",
        y if y <= BUCKET_10Y => "10Y",
        y if y <= BUCKET_15Y => "15Y",
        y if y <= BUCKET_20Y => "20Y",
        _ => "30Y",
    }
}

/// Derive a netting set ID from an OTC margin specification.
///
/// Maps clearing status to the appropriate netting set identifier.
///
/// # Arguments
///
/// * `spec` - OTC margin specification carrying clearing status and bilateral CSA identifier.
#[must_use]
pub fn netting_set_id_from_spec(spec: &OtcMarginSpec) -> NettingSetId {
    match &spec.clearing_status {
        ClearingStatus::Cleared { ccp } => NettingSetId::cleared(ccp),
        _ => NettingSetId::bilateral(&spec.csa.id, &spec.csa.id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ir_tenor_buckets_cover_boundaries() {
        assert_eq!(assign_ir_tenor_bucket(0.25), "6M");
        assert_eq!(assign_ir_tenor_bucket(1.0), "1Y");
        assert_eq!(assign_ir_tenor_bucket(5.0), "5Y");
        assert_eq!(assign_ir_tenor_bucket(30.0), "30Y");
    }

    #[test]
    fn netting_set_id_tracks_clearing_status() {
        let bilateral = OtcMarginSpec::usd_bilateral().expect("bilateral spec");
        assert_eq!(
            netting_set_id_from_spec(&bilateral),
            NettingSetId::bilateral(&bilateral.csa.id, &bilateral.csa.id)
        );

        let cleared = OtcMarginSpec::cleared("LCH", Currency::USD).expect("cleared spec");
        assert_eq!(
            netting_set_id_from_spec(&cleared),
            NettingSetId::cleared("LCH")
        );
    }
}
