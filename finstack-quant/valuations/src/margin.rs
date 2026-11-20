//! Initial-margin orchestration over priceable instruments.

use crate::instruments::Instrument;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::{Error, Result};
use finstack_quant_margin::SimmVersion;
use finstack_quant_margin::calculators::ImResult;
use finstack_quant_margin::calculators::im::SimmCalculator;

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
