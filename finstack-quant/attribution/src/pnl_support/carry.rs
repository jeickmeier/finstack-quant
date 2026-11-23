//! Total-return carry decomposition, YTM pull-to-par, and funding-cost repricing.

use crate::{CarryDetail, PnlAttribution, SourceLine};
use finstack_quant_core::Result;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::math::Compounding;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::fixed_income::bond::pricing::quote_conversions::{
    YieldCompounding, df_from_yield,
};
use finstack_quant_valuations::instruments::{Instrument, PricingOptions};
use finstack_quant_valuations::metrics::{MetricId, collect_period_cash};

/// Raw, repricing-derived inputs for the full-window carry decomposition.
pub(crate) struct TotalReturnCarryInputs {
    /// All period economic cash, including principal; drives total-return addback.
    pub cash_paid: Money,
    /// Coupon, interest and fee receipts, excluding principal.
    pub income_cash_paid: Money,
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
    /// Diagnostics merged into `meta.notes` by [`apply_total_return_carry`].
    pub warnings: Vec<String>,
    /// Top-level valuation calls made by this helper, including failed attempts.
    pub num_repricings: usize,
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
) -> Result<TotalReturnCarryInputs> {
    let mut warnings = Vec::new();
    let mut num_repricings = 0;

    let cash = collect_period_cash(instrument, market, as_of_t0, as_of_t1, currency)?;

    num_repricings += 1;
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
    let mut accrued_t0 = metric(t0_metrics.as_ref(), MetricId::Accrued);
    let ytm = metric(t0_metrics.as_ref(), MetricId::Ytm);
    // Near maturity the quote-date YTM may be unavailable while accrued
    // interest remains valid. Recover accrued independently so the coupon
    // partition does not discard it with the failed yield calculation.
    if t0_metrics.is_none() {
        num_repricings += 1;
        accrued_t0 = instrument
            .price_with_metrics(
                market,
                as_of_t0,
                &[MetricId::Accrued],
                PricingOptions::default(),
            )
            .ok()
            .and_then(|result| metric(Some(&result), MetricId::Accrued));
    }
    num_repricings += 1;
    let accrued_t1 = instrument
        .price_with_metrics(
            market,
            as_of_t1,
            &[MetricId::Accrued],
            PricingOptions::default(),
        )
        .ok()
        .and_then(|result| metric(Some(&result), MetricId::Accrued));
    let delta_accrued = match (accrued_t0, accrued_t1) {
        (Some(a0), Some(a1)) => Some(Money::new(a1 - a0, currency)?),
        _ => None,
    };

    let flat_window_diff = match ytm {
        Some(ytm) => flat_window_diff_from_ytm(
            instrument,
            market,
            as_of_t0,
            as_of_t1,
            currency,
            ytm,
            &mut num_repricings,
        )?,
        None => None,
    };
    let funding_cost = reprice_funding_cost(
        instrument,
        market,
        as_of_t0,
        as_of_t1,
        currency,
        &mut warnings,
        &mut num_repricings,
    );
    if flat_window_diff.is_none() {
        warnings.push("carry pull-to-par unavailable: flat-yield repricing could not be resolved; roll_down contains the unpartitioned price-carry residual".into());
    }

    Ok(TotalReturnCarryInputs {
        cash_paid: cash.total,
        income_cash_paid: cash.income,
        delta_accrued,
        flat_window_diff,
        funding_cost,
        warnings,
        num_repricings,
    })
}

/// `F_t1 - F_t0` on a flat-YTM(t0) curve, or `None` if flat pricing is unavailable.
pub(super) fn flat_window_diff_from_ytm(
    instrument: &dyn Instrument,
    market: &MarketContext,
    as_of_t0: Date,
    as_of_t1: Date,
    currency: Currency,
    ytm: f64,
    num_repricings: &mut usize,
) -> Result<Option<Money>> {
    let Ok(flat) = build_flat_ytm_market(instrument, market, ytm) else {
        return Ok(None);
    };
    *num_repricings += 1;
    let Ok(f_t0) = instrument.value(&flat, as_of_t0) else {
        return Ok(None);
    };
    let f_t0 = f_t0.amount();
    *num_repricings += 1;
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
pub(super) fn ytm_discount_convention(instrument: &dyn Instrument) -> (YieldCompounding, Tenor) {
    if let Some(bond) = instrument.as_any().downcast_ref::<Bond>() {
        (YieldCompounding::Street, bond.cashflow_spec.frequency())
    } else {
        (
            YieldCompounding::Rate(Compounding::Periodic(std::num::NonZeroU32::MIN)),
            Tenor::annual(),
        )
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
pub(super) fn build_flat_ytm_market(
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
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()?;
    market.clone().try_insert(flat_curve)
}

/// Repo/funding cost of carrying `PV(t0)` from `as_of_t0` to `as_of_t1`
/// on the instrument's funding curve, when one is configured and present.
///
/// Accrual is `PV × (1 / DF(t0, t1) − 1)`, using the funding curve's
/// date-to-date discount ratio — the same formula as the valuations `FundingCost` metric.
/// This is a financing overlay: the waterfall/parallel date-roll factor
/// (`theta + cash`) stays all-in price carry.
pub(super) fn reprice_funding_cost(
    instrument: &dyn Instrument,
    market: &MarketContext,
    as_of_t0: Date,
    as_of_t1: Date,
    currency: Currency,
    warnings: &mut Vec<String>,
    num_repricings: &mut usize,
) -> Option<Money> {
    let curve_id = instrument.repo_curve_id()?;
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
    *num_repricings += 1;
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
    let horizon_df = match funding_curve.df_between_dates(as_of_t0, as_of_t1) {
        Ok(value) if value.is_finite() && value > 0.0 => value,
        Ok(_) => {
            warnings.push("funding_cost omitted: invalid horizon discount factor".to_string());
            return None;
        }
        Err(e) => {
            warnings.push(format!(
                "funding_cost omitted: horizon discount factor unavailable ({e})"
            ));
            return None;
        }
    };
    let cost = pv * (1.0 / horizon_df - 1.0);
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
/// `coupon_income = Δaccrued + income_cash`,
/// `pull_to_par = (F_t1−F_t0) + principal_cash − Δaccrued`,
/// `roll_down = theta − (F_t1−F_t0)`, which sum to `carry_total`. When accrual / flat pricing is
/// unavailable (non-bonds), falls back to `coupon_income = income_cash`, `pull_to_par = None`, and the
/// whole price-carry residual goes to `roll_down`.
///
/// `funding_cost` is populated when the instrument exposes a funding/repo
/// curve that is present on the carry market. It is a financing overlay and
/// is **not** subtracted from `total`: the waterfall/parallel factor is
/// all-in price carry. Economic carry net of financing is `total − funding`.
/// The metrics path adds funding back to the net `CarryTotal` metric to
/// preserve the same gross carry basis.
///
/// Merges `inputs.warnings` into `meta.notes` and returns the valuation calls
/// the inputs cost, for the caller's repricing count.
pub(crate) fn apply_total_return_carry(
    attribution: &mut PnlAttribution,
    theta: Money,
    inputs: TotalReturnCarryInputs,
) -> Result<usize> {
    attribution.meta.notes.extend(inputs.warnings);
    attribution.carry = theta.checked_add(inputs.cash_paid)?;
    if inputs.cash_paid.amount().abs() > 0.0 {
        attribution.total_pnl = attribution.total_pnl.checked_add(inputs.cash_paid)?;
    }

    let coupon_income = match inputs.delta_accrued {
        Some(da) => da.checked_add(inputs.income_cash_paid)?,
        None => inputs.income_cash_paid,
    };
    let principal_cash = inputs.cash_paid.checked_sub(inputs.income_cash_paid)?;
    let (pull_to_par, roll_down) = match inputs.flat_window_diff {
        Some(fd) => {
            let accrued = inputs
                .delta_accrued
                .unwrap_or(Money::from((0_i64, theta.currency())));
            (
                Some(fd.checked_add(principal_cash)?.checked_sub(accrued)?),
                Some(theta.checked_sub(fd)?),
            )
        }
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
    Ok(inputs.num_repricings)
}
