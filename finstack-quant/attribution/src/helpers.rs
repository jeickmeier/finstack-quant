//! Helper utilities for P&L attribution.
//!
//! Provides shared functions for market context manipulation, instrument repricing,
//! and common `PnlAttribution` assembly. Currency conversion itself lives on
//! [`MarketContext::convert_money`] — call sites here use it directly.

use super::types::{AttributionFactor, AttributionMethod, CarryDetail, PnlAttribution, SourceLine};
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCountContext, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::math::Compounding;
use finstack_quant_core::money::fx::{FxConversionPolicy, FxPolicyMeta};
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;
use finstack_quant_valuations::instruments::fixed_income::bond::pricing::quote_conversions::{
    df_from_yield, YieldCompounding,
};
use finstack_quant_valuations::instruments::Bond;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::MarketDependencies;
use finstack_quant_valuations::instruments::PricingOptions;
use finstack_quant_valuations::metrics::collect_cashflows_in_period;
use finstack_quant_valuations::metrics::MetricId;
use std::sync::Arc;

/// Families the instrument declares a pricing dependency on.
///
/// [`MarketDependencies`] has no correlation field, so correlation restores
/// stay snapshot-gated. A failed dependency lookup fails open (treat every
/// family as used) so a missing declaration cannot drop a live factor.
#[derive(Debug, Clone, Copy)]
pub(crate) struct InstrumentFactorUse {
    pub rates: bool,
    pub credit: bool,
    pub inflation: bool,
    pub fx: bool,
    pub volatility: bool,
    pub scalars: bool,
}

impl InstrumentFactorUse {
    pub(crate) const fn all() -> Self {
        Self {
            rates: true,
            credit: true,
            inflation: true,
            fx: true,
            volatility: true,
            scalars: true,
        }
    }

    pub(crate) fn of(instrument: &dyn Instrument) -> Self {
        match instrument.market_dependencies() {
            Ok(deps) => Self::from_deps(&deps, instrument),
            Err(_) => Self::all(),
        }
    }

    pub(crate) fn from_deps(deps: &MarketDependencies, instrument: &dyn Instrument) -> Self {
        let rates =
            !deps.curves.discount_curves.is_empty() || !deps.curves.forward_curves.is_empty();
        let credit = !deps.curves.credit_curves.is_empty();
        let inflation = !deps.curves.inflation_curves.is_empty();
        let fx = !deps.fx_pairs.is_empty() || instrument.fx_exposure().is_some();
        let volatility = !deps.volatility_dependencies.is_empty();
        let scalars =
            !deps.market_scalar_ids.is_empty() || instrument.dividend_schedule_id().is_some();
        // An empty dependency set is "undeclared", not "uses nothing". Test
        // stubs and some custom instruments omit the declaration; fail open
        // so a live FX/scalar/credit factor is not dropped into residual.
        if !rates && !credit && !inflation && !fx && !volatility && !scalars {
            return Self::all();
        }
        Self {
            rates,
            credit,
            inflation,
            fx,
            volatility,
            scalars,
        }
    }

    pub(crate) fn uses_attribution_factor(self, factor: &AttributionFactor) -> bool {
        match factor {
            AttributionFactor::Carry
            | AttributionFactor::ModelParameters
            | AttributionFactor::Correlations => true,
            AttributionFactor::RatesCurves => self.rates,
            AttributionFactor::CreditCurves => self.credit,
            AttributionFactor::InflationCurves => self.inflation,
            AttributionFactor::Fx => self.fx,
            AttributionFactor::Volatility => self.volatility,
            AttributionFactor::MarketScalars => self.scalars,
        }
    }
}

/// Reprice an instrument at a given date with a market context.
///
/// # Arguments
///
/// * `instrument` - Instrument to price
/// * `market` - Market data context
/// * `as_of` - Valuation date
///
/// # Returns
///
/// Present value in the instrument's native currency.
///
/// # Errors
///
/// Returns error if pricing fails (missing curves, invalid parameters, etc.).
pub(crate) fn reprice_instrument(
    instrument: &Arc<dyn Instrument>,
    market: &MarketContext,
    as_of: Date,
) -> Result<Money> {
    instrument.value(market, as_of)
}

/// Compute P&L between two valuations in target currency.
///
/// Converts both valuations to target currency before computing difference.
///
/// # Arguments
///
/// * `val_t0` - Opening mark-to-market amount, converted with the T₁ FX matrix
///   under this simple non-isolated convention.
/// * `val_t1` - Closing mark-to-market amount to compare with `val_t0`.
/// * `target_currency` - Currency in which the returned P&L is reported.
/// * `market_t1` - Closing market context supplying the FX matrix used to
///   convert both marks.
/// * `as_of_t1` - Closing valuation date supplied to the T₁ FX conversion.
///
/// # Returns
///
/// P&L in target currency (val_t1 - val_t0).
///
/// # Errors
///
/// Returns error if currency conversion fails.
pub(crate) fn compute_pnl(
    val_t0: Money,
    val_t1: Money,
    target_currency: Currency,
    market_t1: &MarketContext,
    as_of_t1: Date,
) -> Result<Money> {
    let val_t0_converted = market_t1.convert_money(val_t0, target_currency, as_of_t1)?;
    let val_t1_converted = market_t1.convert_money(val_t1, target_currency, as_of_t1)?;

    val_t1_converted.checked_sub(val_t0_converted)
}

/// Compute P&L with explicit FX conversion for each date.
///
/// This allows proper isolation of FX translation effects by using
/// date-appropriate FX rates for conversion.
///
/// # Arguments
///
/// * `val_t0` - Opening mark-to-market amount, converted with T₀ FX.
/// * `val_t1` - Closing mark-to-market amount, converted with T₁ FX.
/// * `target_currency` - Currency in which the returned P&L is reported.
/// * `market_fx_t0` - Opening market context supplying the FX matrix for the
///   opening-value translation.
/// * `market_fx_t1` - Closing market context supplying the FX matrix for the
///   closing-value translation.
/// * `as_of_t0` - Opening valuation date supplied to the T₀ FX conversion.
/// * `as_of_t1` - Closing valuation date supplied to the T₁ FX conversion.
///
/// # Returns
///
/// P&L in target currency with FX translation properly isolated.
///
/// # Errors
///
/// Returns error if currency conversion fails.
pub(crate) fn compute_pnl_with_fx(
    val_t0: Money,
    val_t1: Money,
    target_currency: Currency,
    market_fx_t0: &MarketContext,
    market_fx_t1: &MarketContext,
    as_of_t0: Date,
    as_of_t1: Date,
) -> Result<Money> {
    let val_t0_converted = market_fx_t0.convert_money(val_t0, target_currency, as_of_t0)?;
    let val_t1_converted = market_fx_t1.convert_money(val_t1, target_currency, as_of_t1)?;

    val_t1_converted.checked_sub(val_t0_converted)
}

pub(crate) fn init_attribution(
    total_pnl: Money,
    instrument_id: &str,
    as_of_t0: Date,
    as_of_t1: Date,
    method: AttributionMethod,
    config: Option<&FinstackConfig>,
) -> PnlAttribution {
    match config {
        Some(config) => PnlAttribution::new_with_rounding(
            total_pnl,
            instrument_id,
            as_of_t0,
            as_of_t1,
            method,
            finstack_quant_core::config::rounding_context_from(config),
        ),
        None => PnlAttribution::new(total_pnl, instrument_id, as_of_t0, as_of_t1, method),
    }
}

/// Raw, repricing-derived inputs for the full-window carry decomposition.
pub(crate) struct TotalReturnCarryInputs {
    /// Coupons whose PAYMENT date falls in `[t0, t1)` (drives carry total + total_pnl).
    pub cash_paid: Money,
    /// `accrued(t1) - accrued(t0)` (curve-independent); `None` when the instrument has no `Accrued`.
    pub delta_accrued: Option<Money>,
    /// `F_t1 - F_t0` on a flat-YTM(t0) curve (basis cancels); `None` when `Ytm`/flat pricing is unavailable.
    pub flat_window_diff: Option<Money>,
    /// Repo/funding carry over `[t0, t1)` when the instrument exposes a
    /// funding curve that is present on `market`. Overlay on the reprice
    /// path: not subtracted from `CarryDetail.total` (that total is the
    /// isolated date-roll factor). `None` when no funding curve is configured
    /// or the curve/PV/day-count lookup fails.
    pub funding_cost: Option<Money>,
    /// Diagnostics for the caller to merge into `meta.notes`.
    pub warnings: Vec<String>,
    /// True when a non-finite cashflow/metric value was zeroed; the caller
    /// must set `result_invalid` so tolerance checks refuse a clean pass.
    pub invalid: bool,
}

/// Gather the repricing-based pieces of the carry decomposition over `[as_of_t0, as_of_t1]`,
/// pricing on `market` (the market on which `theta` was computed: `market_t0` for the parallel
/// path, the accumulated market for the waterfall path). Accrued and YTM are read via the
/// instrument's metrics (Accrued and YTM share the T₀ `price_with_metrics` call);
/// the flat-YTM window values isolate the constant-yield aging and
/// curve-shape effects with the flat-vs-market level basis cancelled.
pub(crate) fn total_return_carry_inputs(
    instrument: &dyn Instrument,
    market: &MarketContext,
    as_of_t0: Date,
    as_of_t1: Date,
    currency: Currency,
) -> finstack_quant_core::Result<TotalReturnCarryInputs> {
    Ok({
        let mut warnings = Vec::new();
        let mut invalid = false;

        let cash_paid =
            match collect_cashflows_in_period(instrument, market, as_of_t0, as_of_t1, currency) {
                Ok(value) => factor_money_or_invalid(
                    value,
                    currency,
                    "carry cash income",
                    &mut warnings,
                    &mut invalid,
                ),
                Err(e) => {
                    warnings.push(format!("carry cash income unavailable: {e}"));
                    Money::from((0_i64, currency))
                }
            };

        let t0_metrics = instrument
            .price_with_metrics(
                market,
                as_of_t0,
                &[MetricId::Accrued, MetricId::Ytm],
                PricingOptions::default(),
            )
            .ok();
        let metric = |result: Option<&finstack_quant_valuations::results::ValuationResult>,
                      id: MetricId|
         -> Option<f64> {
            result
                .and_then(|r| r.measures.get(id.as_str()).copied())
                .filter(|v| v.is_finite())
        };
        let accrued_t0 = metric(t0_metrics.as_ref(), MetricId::Accrued);
        let ytm = metric(t0_metrics.as_ref(), MetricId::Ytm);
        let accrued_t1 = instrument
            .price_with_metrics(
                market,
                as_of_t1,
                &[MetricId::Accrued],
                PricingOptions::default(),
            )
            .ok()
            .and_then(|r| r.measures.get(MetricId::Accrued.as_str()).copied())
            .filter(|v| v.is_finite());
        let delta_accrued = match (accrued_t0, accrued_t1) {
            (Some(a0), Some(a1)) => Some(Money::new(a1 - a0, currency)?),
            _ => None,
        };

        let flat_window_diff = match ytm {
            Some(ytm) => {
                flat_window_diff_from_ytm(instrument, market, as_of_t0, as_of_t1, currency, ytm)?
            }
            None => None,
        };
        let funding_cost = reprice_funding_cost(
            instrument,
            market,
            as_of_t0,
            as_of_t1,
            currency,
            &mut warnings,
        );

        TotalReturnCarryInputs {
            cash_paid,
            delta_accrued,
            flat_window_diff,
            funding_cost,
            warnings,
            invalid,
        }
    })
}

/// `F_t1 - F_t0` on a flat-YTM(t0) curve, or `None` if flat pricing is unavailable.
fn flat_window_diff_from_ytm(
    instrument: &dyn Instrument,
    market: &MarketContext,
    as_of_t0: Date,
    as_of_t1: Date,
    currency: Currency,
    ytm: f64,
) -> Result<Option<Money>> {
    let Ok(flat) = build_flat_ytm_market(instrument, market, ytm) else {
        return Ok(None);
    };
    let Ok(f_t0) = instrument.value(&flat, as_of_t0) else {
        return Ok(None);
    };
    let f_t0 = f_t0.amount();
    let Ok(f_t1) = instrument.value(&flat, as_of_t1) else {
        return Ok(None);
    };
    let f_t1 = f_t1.amount();
    if f_t0.is_finite() && f_t1.is_finite() {
        Money::new(f_t1 - f_t0, currency).map(Some)
    } else {
        Ok(None)
    }
}

/// Compounding convention the instrument's `Ytm` metric was solved under.
///
/// Bonds use Street compounding at the coupon frequency (semi-annual US,
/// annual EUR Bunds/corporates). Other instruments that expose `Ytm`
/// (term loans via XIRR, structured credit) solve annually compounded
/// yields — matching [`finstack_quant_valuations::metrics::sensitivities::carry_decomposition`].
fn ytm_discount_convention(instrument: &dyn Instrument) -> (YieldCompounding, Tenor) {
    if let Some(bond) = instrument.as_any().downcast_ref::<Bond>() {
        (YieldCompounding::Street, bond.cashflow_spec.frequency())
    } else {
        (YieldCompounding::Annual, Tenor::annual())
    }
}

/// Build a market whose discount curve is replaced by a flat curve at the
/// instrument's quoted YTM.
///
/// Discount factors invert the same compounding convention the `Ytm` metric
/// was solved under (Street at the bond coupon frequency; annual otherwise).
/// A hardcoded Street-2 conversion (`y_cont = 2·ln(1 + y/2)`) misallocates
/// pull-to-par vs roll-down for annual-compounded bonds (Tuckman & Serrat,
/// *Fixed Income Securities*, Ch. 2–3). Knots are half-year steps out to
/// maturity plus one year, with a 100y sentinel so ultra-long queries stay
/// on the flat log-linear curve.
fn build_flat_ytm_market(
    instrument: &dyn Instrument,
    market: &MarketContext,
    ytm: f64,
) -> Result<MarketContext> {
    let curve_id = instrument
        .market_dependencies()?
        .curves
        .discount_curves
        .first()
        .cloned()
        .ok_or_else(|| finstack_quant_core::InputError::NotFound {
            id: format!("discount_curve_for:{}", instrument.id()),
        })?;
    let original = market.get_discount(curve_id.as_str())?;
    let (compounding, frequency) = ytm_discount_convention(instrument);
    let horizon_years = instrument
        .expiry()
        .map(|maturity| {
            let days = (maturity - original.base_date()).whole_days().max(1) as f64;
            (days / 365.25 + 1.0).clamp(1.0, 100.0)
        })
        .unwrap_or(100.0);
    let half_year_steps = (horizon_years * 2.0).ceil() as usize;
    let mut knots = Vec::with_capacity(half_year_steps + 2);
    for i in 0..=half_year_steps {
        let t = i as f64 * 0.5;
        knots.push((t, df_from_yield(ytm, t, compounding, frequency)?));
    }
    if knots.last().is_none_or(|(t, _)| *t < 100.0) {
        knots.push((100.0, df_from_yield(ytm, 100.0, compounding, frequency)?));
    }
    let flat_curve = DiscountCurve::builder(curve_id.as_str())
        .base_date(original.base_date())
        .day_count(original.day_count())
        .knots(knots)
        .interp(InterpStyle::LogLinear)
        .build()?;
    Ok(market.clone().insert(flat_curve))
}

/// Repo/funding cost of carrying `PV(t0)` from `as_of_t0` to `as_of_t1`
/// on the instrument's funding curve, when one is configured and present.
///
/// Accrual is `PV × (exp(r_cont × dcf) − 1)` with a continuously compounded
/// funding zero — the same formula as the valuations `FundingCost` metric.
/// This is a financing overlay: the waterfall/parallel date-roll factor
/// (`theta + cash`) stays all-in price carry.
fn reprice_funding_cost(
    instrument: &dyn Instrument,
    market: &MarketContext,
    as_of_t0: Date,
    as_of_t1: Date,
    currency: Currency,
    warnings: &mut Vec<String>,
) -> Option<Money> {
    let curve_id = instrument.funding_curve_id()?;
    if as_of_t1 <= as_of_t0 {
        return Some(Money::from((0_i64, currency)));
    }
    let funding_curve = match market.get_discount(curve_id.as_str()) {
        Ok(curve) => curve,
        Err(e) => {
            warnings.push(format!(
                "funding_cost omitted: funding curve '{curve_id}' unavailable ({e})"
            ));
            return None;
        }
    };
    let pv = match instrument.value(market, as_of_t0) {
        Ok(value) if value.amount().is_finite() => value.amount(),
        Ok(_) => {
            warnings.push("funding_cost omitted: T0 PV is non-finite".to_string());
            return None;
        }
        Err(e) => {
            warnings.push(format!("funding_cost omitted: T0 PV unavailable ({e})"));
            return None;
        }
    };
    let (day_count, frequency) = if let Some(bond) = instrument.as_any().downcast_ref::<Bond>() {
        (
            bond.cashflow_spec.day_count(),
            Some(bond.cashflow_spec.frequency()),
        )
    } else {
        (funding_curve.day_count(), None)
    };
    let dc_ctx = DayCountContext {
        frequency,
        ..DayCountContext::default()
    };
    let dcf = match day_count.year_fraction(as_of_t0, as_of_t1, dc_ctx) {
        Ok(value) if value.is_finite() => value,
        Ok(_) => {
            warnings
                .push("funding_cost omitted: day-count year fraction is non-finite".to_string());
            return None;
        }
        Err(e) => {
            warnings.push(format!(
                "funding_cost omitted: day-count year fraction unavailable ({e})"
            ));
            return None;
        }
    };
    let annual_rate = match funding_curve.zero_rate_on_date(as_of_t1, Compounding::Continuous) {
        Ok(rate) if rate.is_finite() => rate,
        Ok(_) => {
            warnings.push("funding_cost omitted: funding zero rate is non-finite".to_string());
            return None;
        }
        Err(e) => {
            warnings.push(format!(
                "funding_cost omitted: funding zero rate unavailable ({e})"
            ));
            return None;
        }
    };
    let cost = pv * ((annual_rate * dcf).exp() - 1.0);
    if cost.is_finite() {
        match Money::new(cost, currency) {
            Ok(amount) => Some(amount),
            Err(error) => {
                warnings.push(format!("funding_cost omitted: {error}"));
                None
            }
        }
    } else {
        warnings.push("funding_cost omitted: non-finite funding accrual".to_string());
        None
    }
}

/// Assemble the carry total + the fully-labeled detail partition.
///
/// `carry_total = theta + cash_paid` (the isolated date-roll factor);
/// `total_pnl += cash_paid`. The price-carry detail:
/// `coupon_income = Δaccrued + cash`, `pull_to_par = (F_t1−F_t0) − Δaccrued`,
/// `roll_down = theta − (F_t1−F_t0)`, which sum to `carry_total`. When accrual / flat pricing is
/// unavailable (non-bonds), falls back to `coupon_income = cash`, `pull_to_par = None`, and the
/// whole price-carry residual goes to `roll_down`.
///
/// `funding_cost` is populated when the instrument exposes a funding/repo
/// curve that is present on the carry market. It is a financing overlay and
/// is **not** subtracted from `total`: the waterfall/parallel factor is
/// all-in price carry. Economic carry net of financing is `total − funding`.
/// On the metrics path, `CarryTotal` is already net of funding and the
/// PORT identity `coupon + ptp + rolldown − funding = total` holds there.
pub(crate) fn apply_total_return_carry(
    attribution: &mut PnlAttribution,
    theta: Money,
    inputs: TotalReturnCarryInputs,
) -> Result<()> {
    attribution.carry = theta.checked_add(inputs.cash_paid)?;
    if inputs.cash_paid.amount().abs() > 0.0 {
        attribution.total_pnl = attribution.total_pnl.checked_add(inputs.cash_paid)?;
    }

    let coupon_income = match inputs.delta_accrued {
        Some(da) => da.checked_add(inputs.cash_paid)?,
        None => inputs.cash_paid,
    };
    let (pull_to_par, roll_down) = match (inputs.delta_accrued, inputs.flat_window_diff) {
        (Some(da), Some(fd)) => (Some(fd.checked_sub(da)?), Some(theta.checked_sub(fd)?)),
        // Fallback (no accrual / flat split, e.g. non-bonds): the whole price-carry residual
        // goes to roll_down so `coupon_income + roll_down = total` still holds.
        _ => (None, Some(attribution.carry.checked_sub(coupon_income)?)),
    };

    attribution.carry_detail = Some(CarryDetail {
        total: attribution.carry,
        coupon_income: Some(SourceLine::scalar(coupon_income)),
        pull_to_par,
        roll_down: roll_down.map(SourceLine::scalar),
        funding_cost: inputs.funding_cost,
    });
    Ok(())
}

pub(crate) fn stamp_fx_policy(
    attribution: &mut PnlAttribution,
    target_currency: Currency,
    notes: impl Into<String>,
) {
    attribution.meta.fx_policy = Some(FxPolicyMeta {
        strategy: FxConversionPolicy::CashflowDate,
        target_currency: Some(target_currency),
        notes: notes.into(),
    });
}

pub(crate) fn note_warning(
    attribution: &mut PnlAttribution,
    message: impl Into<String>,
    instrument_id: &str,
    factor: &str,
) {
    let message = message.into();
    tracing::warn!(
        instrument_id = %instrument_id,
        factor,
        message = %message,
        "Attribution soft warning"
    );
    attribution.meta.notes.push(message);
}

pub(crate) fn finalize_attribution(
    attribution: &mut PnlAttribution,
    instrument_id: &str,
    method: &str,
    num_repricings: usize,
    tolerance_abs: f64,
    tolerance_pct: f64,
) {
    if let Err(e) = attribution.compute_residual() {
        tracing::warn!(
            error = %e,
            instrument_id = %instrument_id,
            method,
            "Residual computation failed; attribution may be incomplete"
        );
    }

    attribution.meta.num_repricings = num_repricings;
    attribution.meta.tolerance_abs = tolerance_abs;
    attribution.meta.tolerance_pct = tolerance_pct;
}

/// Construct a factor P&L [`Money`] from a computed `f64` amount.
///
/// If `amount` is non-finite (NaN or ±Inf), this function:
/// - Appends a diagnostic note to `notes`,
/// - Sets `*result_invalid = true` so [`crate::PnlAttribution::result_invalid`]
///   is propagated to callers, and
/// - Returns a **zero sentinel** in `currency` so the attribution can continue
///   and produce a complete (though flagged-invalid) result rather than
///   panicking inside [`Money::new`], which panics on non-finite input.
///
/// For finite amounts it delegates directly to [`Money::new`].
#[inline]
pub(crate) fn factor_money_or_invalid(
    amount: f64,
    currency: Currency,
    label: &str,
    notes: &mut Vec<String>,
    result_invalid: &mut bool,
) -> Money {
    match Money::new(amount, currency) {
        Ok(money) => money,
        Err(error) => {
            notes.push(format!(
                "Invalid factor P&L ({amount:?}) for {label}: {error}; attribution flagged invalid"
            ));
            *result_invalid = true;
            Money::from((0_i64, currency))
        }
    }
}

/// Validate that the attribution period is well-formed: `as_of_t1 >= as_of_t0`.
///
/// A reversed period silently flips the sign of theta / carry (`time_period_days`
/// goes negative) and produces a nonsensical decomposition, so it is rejected
/// at every attribution entry point. A zero-length period (`t1 == t0`) is
/// permitted — same-day attribution is a degenerate but valid request, with
/// theta zero over zero elapsed time.
pub(crate) fn validate_attribution_period(as_of_t0: Date, as_of_t1: Date) -> Result<()> {
    if as_of_t1 < as_of_t0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "attribution period is reversed: as_of_t1 ({as_of_t1}) precedes as_of_t0 ({as_of_t0})"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::money::fx::{FxConversionPolicy, FxMatrix, FxProvider};
    use finstack_quant_core::Error;
    use std::sync::Arc;
    use time::macros::date;

    // Simple test FX provider
    struct TestFx;
    impl FxProvider for TestFx {
        fn rate(
            &self,
            from: Currency,
            to: Currency,
            _on: Date,
            _policy: FxConversionPolicy,
        ) -> Result<f64> {
            if from == Currency::EUR && to == Currency::USD {
                Ok(1.1)
            } else if from == Currency::USD && to == Currency::EUR {
                Ok(1.0 / 1.1)
            } else if from == to {
                Ok(1.0)
            } else {
                Err(Error::Validation("FX rate not found".to_string()))
            }
        }
    }

    #[test]
    fn validate_attribution_period_accepts_forward_and_same_day() {
        assert!(
            validate_attribution_period(date!(2025 - 01 - 15), date!(2025 - 01 - 16)).is_ok(),
            "a forward period must be accepted"
        );
        // Same-day attribution is degenerate but permitted (theta over zero days).
        assert!(
            validate_attribution_period(date!(2025 - 01 - 15), date!(2025 - 01 - 15)).is_ok(),
            "a zero-length period must be accepted"
        );
    }

    #[test]
    fn validate_attribution_period_rejects_reversed_period() {
        assert!(
            validate_attribution_period(date!(2025 - 01 - 16), date!(2025 - 01 - 15)).is_err(),
            "a reversed period (t1 < t0) must be rejected"
        );
    }

    #[test]
    fn discount_only_bond_does_not_use_unused_book_families() {
        use finstack_quant_valuations::instruments::Bond;
        let bond = Bond::fixed(
            "USE-BOND",
            Money::from((1_000_000_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
            date!(2025 - 01 - 01),
            date!(2030 - 01 - 01),
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("bond");
        let factor_use = InstrumentFactorUse::of(&bond);
        assert!(factor_use.rates);
        assert!(!factor_use.credit);
        assert!(!factor_use.inflation);
        assert!(!factor_use.fx);
        assert!(!factor_use.volatility);
        assert!(!factor_use.scalars);
        assert!(factor_use.uses_attribution_factor(&AttributionFactor::RatesCurves));
        assert!(!factor_use.uses_attribution_factor(&AttributionFactor::CreditCurves));
        assert!(factor_use.uses_attribution_factor(&AttributionFactor::Correlations));
        assert!(factor_use.uses_attribution_factor(&AttributionFactor::Carry));

        // Empty declarations are undeclared, not "uses nothing".
        let undeclared = InstrumentFactorUse::from_deps(&MarketDependencies::new(), &bond);
        assert!(undeclared.fx);
        assert!(undeclared.credit);
        assert!(undeclared.scalars);
    }

    /// Flat-YTM window curve must invert Street compounding at the bond's
    /// coupon frequency. US corporate (`Bond::fixed`) is semi-annual:
    /// ytm = 5% at t = 1y → DF = (1 + 0.05/2)^(−2), NOT exp(−0.05).
    /// Knot grid must reach 100y for ultra-long bonds.
    #[test]
    fn flat_ytm_market_uses_street_semiannual_compounding() {
        use finstack_quant_valuations::instruments::Bond;

        let as_of = date!(2025 - 01 - 01);
        let bond = Bond::fixed(
            "FLAT-YTM-BOND",
            Money::from((1_000_000_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
            date!(2024 - 01 - 01),
            date!(2034 - 01 - 01),
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("bond construction");
        let base_curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (10.0, (-0.03_f64 * 10.0).exp())])
            .interp(InterpStyle::LogLinear)
            .build()
            .expect("base curve");
        let market = MarketContext::new().insert(base_curve);

        let flat = build_flat_ytm_market(&bond, &market, 0.05).expect("flat market");
        let curve = flat.get_discount("USD-OIS").expect("flat curve");

        let expected_1y = (1.0_f64 + 0.05 / 2.0).powi(-2);
        assert!(
            (curve.df(1.0) - expected_1y).abs() < 1e-12,
            "DF(1y) at 5% street (semi-annual) YTM must be (1.025)^-2 = \
             {expected_1y}, got {} (continuous exp(-0.05) = {})",
            curve.df(1.0),
            (-0.05_f64).exp()
        );

        // Grid must stay on the semi-annual flat curve out to 100y (log-linear
        // extrapolation of a flat yield is exact) without building a 201-knot
        // 0..=200 half-year grid for a 10y bond.
        let expected_100y = (1.0_f64 + 0.05 / 2.0).powi(-200);
        assert!(
            ((curve.df(100.0) - expected_100y) / expected_100y).abs() < 1e-9,
            "DF(100y) must be on the semi-annual flat curve, expected \
             {expected_100y}, got {}",
            curve.df(100.0)
        );
        assert!(
            curve.knots().len() < 50,
            "10y bond flat-YTM grid should be maturity-sized, got {} knots",
            curve.knots().len()
        );
    }

    /// Annual-pay bonds (Bund / EUR corporate) solve Street YTM at frequency 1.
    /// DF(1y) at 5% must be 1/1.05, not the hardcoded Street-2 (1.025)^-2.
    #[test]
    fn flat_ytm_market_uses_bond_coupon_frequency() {
        use finstack_quant_valuations::instruments::Bond;
        use finstack_quant_valuations::instruments::BondConvention;

        let as_of = date!(2025 - 01 - 01);
        let bond = Bond::with_convention(
            "FLAT-YTM-BUND",
            Money::from((1_000_000_i64, Currency::EUR)),
            finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
            date!(2024 - 01 - 01),
            date!(2034 - 01 - 01),
            BondConvention::GermanBund,
            "EUR-OIS",
        )
        .expect("bund construction");
        let base_curve = DiscountCurve::builder("EUR-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (10.0, (-0.03_f64 * 10.0).exp())])
            .interp(InterpStyle::LogLinear)
            .build()
            .expect("base curve");
        let market = MarketContext::new().insert(base_curve);

        let flat = build_flat_ytm_market(&bond, &market, 0.05).expect("flat market");
        let curve = flat.get_discount("EUR-OIS").expect("flat curve");

        let expected_1y = 1.0_f64 / 1.05;
        let street2_1y = (1.0_f64 + 0.05 / 2.0).powi(-2);
        assert!(
            (curve.df(1.0) - expected_1y).abs() < 1e-12,
            "DF(1y) at 5% annual Street YTM must be 1/1.05 = {expected_1y}, \
             got {} (hardcoded Street-2 would be {street2_1y})",
            curve.df(1.0)
        );
    }

    #[test]
    fn reprice_funding_cost_uses_repo_curve_when_configured() {
        use finstack_quant_core::types::CurveId;
        use finstack_quant_valuations::instruments::Bond;

        let as_of_t0 = date!(2025 - 01 - 15);
        let as_of_t1 = date!(2025 - 01 - 16);
        let mut bond = Bond::fixed(
            "FUND-BOND",
            Money::from((1_000_000_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
            date!(2025 - 01 - 15),
            date!(2030 - 01 - 15),
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("bond");
        bond.funding_curve_id = Some(CurveId::new("USD-REPO"));

        let ois = DiscountCurve::builder("USD-OIS")
            .base_date(as_of_t0)
            .knots([(0.0, 1.0), (10.0, (-0.05_f64 * 10.0).exp())])
            .interp(InterpStyle::LogLinear)
            .build()
            .expect("ois");
        let repo = DiscountCurve::builder("USD-REPO")
            .base_date(as_of_t0)
            .knots([(0.0, 1.0), (10.0, (-0.03_f64 * 10.0).exp())])
            .interp(InterpStyle::LogLinear)
            .build()
            .expect("repo");
        let market = MarketContext::new().insert(ois).insert(repo);

        let mut warnings = Vec::new();
        let funding = reprice_funding_cost(
            &bond,
            &market,
            as_of_t0,
            as_of_t1,
            Currency::USD,
            &mut warnings,
        )
        .expect("funding cost");
        assert!(
            funding.amount() > 0.0,
            "repo funding cost must be positive, got {}",
            funding.amount()
        );
        assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
    }

    #[test]
    fn test_compute_pnl() {
        let val_t0 = Money::from((1000_i64, Currency::EUR));
        let val_t1 = Money::from((1100_i64, Currency::EUR));
        let fx = FxMatrix::new(Arc::new(TestFx));
        let market = MarketContext::new().insert_fx(fx);
        let as_of = date!(2025 - 01 - 15);

        let pnl = compute_pnl(val_t0, val_t1, Currency::USD, &market, as_of)
            .expect("PNL computation should succeed in test");
        // (1100 - 1000) EUR * 1.1 = 110 USD
        assert_eq!(pnl.amount(), 110.0);
        assert_eq!(pnl.currency(), Currency::USD);
    }

    #[test]
    fn test_compute_pnl_with_fx() {
        // Test FX translation isolation
        let pv = Money::from((1000_i64, Currency::EUR));

        // T0 market: EUR/USD = 1.1
        let fx_t0 = FxMatrix::new(Arc::new(TestFx));
        let market_t0 = MarketContext::new().insert_fx(fx_t0);

        // T1 market: EUR/USD = 1.2 (10% appreciation)
        struct TestFxT1;
        impl FxProvider for TestFxT1 {
            fn rate(
                &self,
                from: Currency,
                to: Currency,
                _on: Date,
                _policy: FxConversionPolicy,
            ) -> Result<f64> {
                if from == Currency::EUR && to == Currency::USD {
                    Ok(1.2)
                } else if from == Currency::USD && to == Currency::EUR {
                    Ok(1.0 / 1.2)
                } else if from == to {
                    Ok(1.0)
                } else {
                    Err(Error::Validation("FX rate not found".to_string()))
                }
            }
        }
        let fx_t1 = FxMatrix::new(Arc::new(TestFxT1));
        let market_t1 = MarketContext::new().insert_fx(fx_t1);

        let as_of_t0 = date!(2025 - 01 - 15);
        let as_of_t1 = date!(2025 - 01 - 16);

        // PV unchanged in EUR, but FX moved
        let pnl = compute_pnl_with_fx(
            pv,
            pv,
            Currency::USD,
            &market_t0,
            &market_t1,
            as_of_t0,
            as_of_t1,
        )
        .expect("PNL computation with FX should succeed in test");

        // FX translation: 1000 EUR @ 1.2 - 1000 EUR @ 1.1 = 1200 - 1100 = 100 USD
        assert_eq!(pnl.amount(), 100.0);
        assert_eq!(pnl.currency(), Currency::USD);
    }
}
