//! Calibration target construction and shared input validation.
//!
use crate::api::schema::SurfaceExtrapolationPolicy;
use crate::build::context::BuildCtx;
use crate::config::ResidualWeightingScheme;
use crate::constants::{OrderedF64, TOLERANCE_DUP_KNOTS};
use crate::prepared::CalibrationQuote;
use crate::quotes::market_quote::{ExtractQuotes, MarketQuote};
use crate::quotes::rates::RateQuote;
use crate::quotes::vol::VolQuote;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::dividends::DividendKind;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::Result;
use finstack_quant_valuations::instruments::rates::irs::FloatingLegCompounding;
use finstack_quant_valuations::market::conventions::ConventionRegistry;
use std::cell::RefCell;
use std::collections::BTreeMap;

/// Map ACT/365F calibration time to a curve's date origin and day count.
/// Fractional days are interpolated on the destination clock so synthetic
/// caplet periods and surface grids do not round to whole calendar days.
///
/// # Arguments
///
/// * `base_date` - Calibration date defining time zero on the source clock.
/// * `time` - Finite, non-negative ACT/365F years since `base_date`.
/// * `curve_base_date` - Date defining time zero on the destination curve.
/// * `day_count` - Destination curve's day-count convention for dated lookups.
pub(crate) fn calibration_time_on_curve(
    base_date: Date,
    time: f64,
    curve_base_date: Date,
    day_count: DayCount,
) -> Result<f64> {
    let days = time * 365.0;
    if !days.is_finite() || days < 0.0 || days > (Date::MAX - base_date).whole_days() as f64 {
        return Err(finstack_quant_core::Error::Validation(
            "calibration time must be finite, non-negative and within the date range".into(),
        ));
    }
    let date = base_date + time::Duration::days(days.floor() as i64);
    let curve_time =
        |date| day_count.signed_year_fraction(curve_base_date, date, DayCountContext::default());
    let mut mapped = curve_time(date)?;
    let fraction = days.fract();
    if fraction > 0.0 {
        let next = date.next_day().ok_or_else(|| {
            finstack_quant_core::Error::Validation("calibration time exceeds date range".into())
        })?;
        mapped += fraction * (curve_time(next)? - mapped);
    }
    Ok(mapped)
}

#[derive(Debug)]
pub(crate) struct EquityForwardInputs {
    base_date: Date,
    spot: f64,
    continuous_yield: Option<f64>,
    cash_dividends: Vec<(f64, f64)>,
}

impl EquityForwardInputs {
    pub(crate) fn forward(&self, discount: &DiscountCurve, expiry: f64) -> Result<f64> {
        let curve_time = |time| {
            calibration_time_on_curve(
                self.base_date,
                time,
                discount.base_date(),
                discount.day_count(),
            )
        };
        let discount_factor = discount.df_between_times(curve_time(0.0)?, curve_time(expiry)?)?;
        if !discount_factor.is_finite() || discount_factor <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "equity forward discount factor must be finite and positive at T={expiry}, got {discount_factor}"
            )));
        }
        let prepaid = if let Some(dividend_yield) = self.continuous_yield {
            self.spot * (-dividend_yield * expiry).exp()
        } else {
            let dividend_pv: f64 = self
                .cash_dividends
                .iter()
                .filter(|(time, _)| *time <= expiry)
                .map(|(_, present_value)| *present_value)
                .sum();
            self.spot - dividend_pv
        };
        let forward = prepaid / discount_factor;
        if !forward.is_finite() || forward <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "equity forward must be finite and positive at T={expiry}; spot={}, prepaid={prepaid}, df={discount_factor}",
                self.spot
            )));
        }
        Ok(forward)
    }
}

pub(crate) fn resolve_equity_forward_inputs(
    ticker: &str,
    base_date: Date,
    spot: f64,
    dividend_yield_override: Option<f64>,
    discount: &DiscountCurve,
    context: &MarketContext,
) -> Result<EquityForwardInputs> {
    if !spot.is_finite() || spot <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "equity spot must be finite and positive, got {spot}"
        )));
    }
    let scalar_key = format!("{ticker}-DIVYIELD");
    let continuous_yield = match dividend_yield_override {
        Some(value) => Some(value),
        None => match context.get_price(&scalar_key) {
            Ok(MarketScalar::Unitless(value)) => Some(*value),
            Ok(MarketScalar::Price(_)) => {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "equity carry scalar '{scalar_key}' must be unitless"
                )))
            }
            Err(_) => None,
        },
    };
    if let Some(value) = continuous_yield {
        if !value.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "equity dividend yield must be finite, got {value}"
            )));
        }
        return Ok(EquityForwardInputs {
            base_date,
            spot,
            continuous_yield: Some(value),
            cash_dividends: Vec::new(),
        });
    }

    let fallback_id = format!("{ticker}-DIVS");
    let mut schedules = context.dividends_iter().filter(|(id, schedule)| {
        schedule.get_underlying() == Some(ticker)
            || id.as_str() == ticker
            || id.as_str() == fallback_id
    });
    let (_, schedule) = schedules.next().ok_or_else(|| {
        finstack_quant_core::Error::Input(finstack_quant_core::InputError::NotFound {
            id: format!("explicit dividend yield or dividend schedule for equity '{ticker}'"),
        })
    })?;
    if schedules.next().is_some() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "multiple dividend schedules match equity '{ticker}'"
        )));
    }
    schedule.validate()?;

    let mut cash_dividends = Vec::new();
    for event in schedule.get_events() {
        if event.date <= base_date {
            continue;
        }
        match &event.kind {
            DividendKind::Cash(amount) => {
                let surface_time = DayCount::Act365F.year_fraction(
                    base_date,
                    event.date,
                    DayCountContext::default(),
                )?;
                cash_dividends.push((surface_time, amount.amount() * discount.df_between_dates(base_date, event.date)?));
            }
            DividendKind::Yield(_) | DividendKind::Stock { .. } => {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "equity '{ticker}' dividend schedule contains non-cash events; supply an explicit continuous dividend yield"
                )))
            }
        }
    }

    Ok(EquityForwardInputs {
        base_date,
        spot,
        continuous_yield: None,
        cash_dividends,
    })
}

/// Reprice the original option quotes through the published surface evaluator.
pub(crate) fn surface_quote_residuals(
    surface: &VolSurface,
    quotes: &[MarketQuote],
    underlying: &str,
    base_date: Date,
) -> Result<BTreeMap<String, f64>> {
    let mut residuals = BTreeMap::new();
    for quote in quotes {
        if let MarketQuote::Vol(VolQuote::OptionVol {
            id,
            underlying: ticker,
            expiry,
            strike,
            vol,
            ..
        }) = quote
        {
            if ticker.as_str() != underlying || *expiry <= base_date {
                continue;
            }
            let t =
                DayCount::Act365F.year_fraction(base_date, *expiry, DayCountContext::default())?;
            let fitted = finstack_quant_models::volatility::get_surface_vol(surface, t, *strike)?;
            residuals.insert(id.to_string(), fitted - vol);
        }
    }
    Ok(residuals)
}

/// Resolve the day count convention for a discount or forward curve from market conventions.
pub(crate) fn curve_day_count_from_quotes(quotes: &[RateQuote]) -> Result<DayCount> {
    let registry = ConventionRegistry::try_global()?;
    let mut curve_day_count: Option<DayCount> = None;

    for q in quotes {
        let index_id = match q {
            RateQuote::Deposit { index, .. }
            | RateQuote::Fra { index, .. }
            | RateQuote::Swap { index, .. } => index.clone(),
            RateQuote::Futures { contract, .. } => {
                registry.require_ir_future(contract)?.index_id.clone()
            }
        };

        let idx_conv = registry.require_rate_index(&index_id)?;
        match curve_day_count {
            Some(day_count) if day_count != idx_conv.day_count => {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Mixed rate index day counts for curve construction: got {:?} and {:?}",
                    day_count, idx_conv.day_count
                )));
            }
            Some(_) => {}
            None => curve_day_count = Some(idx_conv.day_count),
        }
    }

    curve_day_count.ok_or_else(|| {
        finstack_quant_core::Error::Validation(
            "Unable to resolve curve day count: no rate quotes provided".to_string(),
        )
    })
}

/// Result of preparing rate quotes for a calibration target.
pub(crate) struct PreparedRateQuotes {
    /// Prepared quotes ready for the solver.
    pub(crate) quotes: Vec<CalibrationQuote>,
    /// Day count convention used for curve time-axis.
    pub(crate) curve_day_count: DayCount,
}

/// Common preflight for rates calibration targets: extract `RateQuote`s, resolve day count,
/// build a `BuildCtx`, and convert each quote into a `PreparedQuote` wrapped in
/// [`CalibrationQuote::Rates`].
///
/// Pass `curve_ids` as the role -> id mapping the underlying instruments expect (typically
/// "discount" and, for projection-aware quotes, "forward"). Pass `explicit_curve_day_count = None`
/// to derive the curve time-axis day count from the quote indices.
pub(crate) fn prepare_rate_calibration_quotes(
    quotes: &[MarketQuote],
    base_date: Date,
    curve_ids: finstack_quant_core::HashMap<String, String>,
    explicit_curve_day_count: Option<DayCount>,
    residual_notional: f64,
) -> Result<PreparedRateQuotes> {
    prepare_rate_calibration_quotes_with_ois_override(
        quotes,
        base_date,
        curve_ids,
        explicit_curve_day_count,
        residual_notional,
        None,
    )
}

/// Variant of [`prepare_rate_calibration_quotes`] that threads an OIS compounding
/// override through `BuildCtx`. Used by `DiscountCurveTarget` to honour
/// step-level OIS-compounding selection without forcing every caller to know
/// about the override.
pub(crate) fn prepare_rate_calibration_quotes_with_ois_override(
    quotes: &[MarketQuote],
    base_date: Date,
    curve_ids: finstack_quant_core::HashMap<String, String>,
    explicit_curve_day_count: Option<DayCount>,
    residual_notional: f64,
    ois_compounding_override: Option<FloatingLegCompounding>,
) -> Result<PreparedRateQuotes> {
    let rates_quotes: Vec<RateQuote> = quotes.extract_quotes();
    if rates_quotes.is_empty() {
        return Err(finstack_quant_core::Error::Input(
            finstack_quant_core::InputError::TooFewPoints,
        ));
    }

    let curve_day_count = match explicit_curve_day_count {
        Some(day_count) => day_count,
        None => curve_day_count_from_quotes(&rates_quotes)?,
    };

    let build_ctx = BuildCtx::new(base_date, residual_notional, curve_ids)
        .with_ois_compounding_override(ois_compounding_override);

    let mut prepared = Vec::with_capacity(rates_quotes.len());
    for q in rates_quotes {
        let pq = crate::build::prepared::prepare_rate_quote(
            q,
            &build_ctx,
            curve_day_count,
            base_date,
            true,
        )?;
        prepared.push(CalibrationQuote::Rates(pq));
    }

    Ok(PreparedRateQuotes {
        quotes: prepared,
        curve_day_count,
    })
}

/// Convenience: `{ "discount" => discount_id }` curve-ids map.
pub(crate) fn discount_only_curve_ids(
    discount_id: &str,
) -> finstack_quant_core::HashMap<String, String> {
    let mut m = finstack_quant_core::HashMap::default();
    m.insert("discount".to_string(), discount_id.to_string());
    m
}

/// Convenience: `{ "discount" => discount_id, "forward" => forward_id }` curve-ids map.
pub(crate) fn discount_and_forward_curve_ids(
    discount_id: &str,
    forward_id: &str,
) -> finstack_quant_core::HashMap<String, String> {
    let mut m = finstack_quant_core::HashMap::default();
    m.insert("discount".to_string(), discount_id.to_string());
    m.insert("forward".to_string(), forward_id.to_string());
    m
}

/// Closed-form proxy for a par instrument's fixed-leg annuity (PV01) at maturity
/// `t`, used by discount and forward residual weights to put residuals on a
/// common rate-error scale.
///
/// The continuously-discounted annuity of a unit-coupon par instrument is
/// `A(t, r) = (1 − e^{−r·t}) / r`, with the well-defined limit
/// `A → t` as `r → 0`. This is exact for a continuously-paid coupon and a tight
/// proxy for the discrete fixed-leg annuity `Σ τ_i·DF_i` of swaps; for a single-
/// period instrument (deposit / FRA) it reduces to `≈ t`, which is the correct
/// PV01 there. The proxy needs only the quote's own par rate, so it works inside
/// `residual_weights` where no calibrated curve is yet available.
///
/// `r` is taken from the quote when it carries a par rate (rate quotes); other
/// quote kinds fall back to a small representative rate, for which `A ≈ t`.
pub(crate) fn quote_annuity_proxy(quote: &CalibrationQuote, t: f64) -> f64 {
    // Representative rate for the discount factor in the annuity integral.
    let r = match quote {
        CalibrationQuote::Rates(pq) => pq.quote.implied_rate(),
        // Inflation / xccy-basis quotes do not carry a comparable fixed par rate;
        // a small rate makes the proxy degrade gracefully to `A ≈ t`.
        _ => 0.0,
    };
    // Use the absolute rate: a negative-rate regime (EUR/JPY) still has a
    // well-defined positive annuity, and `(1 − e^{−r·t})/r` is symmetric in the
    // sign of `r` only to second order — `|r|` keeps the proxy stable and positive.
    let r_abs = if r.is_finite() { r.abs() } else { 0.0 };
    let t_pos = t.max(1e-6);
    let annuity = if r_abs < 1e-8 {
        // r → 0 limit: A(t, 0) = t.
        t_pos
    } else {
        (1.0 - (-r_abs * t_pos).exp()) / r_abs
    };
    // Floor strictly positive so the `1/A²` weight is always finite.
    annuity.max(1e-6)
}

/// Reusable scratch context for calibration targets.
///
/// Holds a `RefCell<MarketContext>` that is mutated in place with the trial
/// curve before each residual evaluation, avoiding a full
/// `MarketContext::clone()` per call. Targets are constructed per step and never
/// shared across threads; the `RefCell` itself is `!Sync`, which prevents
/// accidental cross-thread reuse.
pub(crate) struct ContextScratch {
    scratch: RefCell<MarketContext>,
}

impl ContextScratch {
    /// Create a scratch seeded with `base_context`.
    pub(crate) fn new(base_context: MarketContext) -> Self {
        Self {
            scratch: RefCell::new(base_context),
        }
    }

    /// Run `op` against a `MarketContext` containing `curve` plus the base context's data.
    pub(crate) fn with_curve<C, F, T>(&self, curve: &C, op: F) -> Result<T>
    where
        C: Clone + Into<finstack_quant_core::market_data::context::CurveStorage>,
        F: FnOnce(&MarketContext) -> Result<T>,
    {
        self.with_curve_then(curve, |_| {}, op)
    }

    /// Like [`Self::with_curve`], but runs `after_insert` on the scratch context
    /// once the trial curve is in place — used to sync derived market objects
    /// (e.g. a credit index holding its own copy of the curve) before pricing.
    pub(crate) fn with_curve_then<C, S, F, T>(&self, curve: &C, after_insert: S, op: F) -> Result<T>
    where
        C: Clone + Into<finstack_quant_core::market_data::context::CurveStorage>,
        S: FnOnce(&mut MarketContext),
        F: FnOnce(&MarketContext) -> Result<T>,
    {
        // `insert_mut` keeps the existing storage intact and only overwrites the
        // single curve entry, so a panic mid-insert cannot leave an empty context
        // behind for subsequent calls.
        let mut ctx = self.scratch.borrow_mut();
        ctx.insert_mut(curve.clone());
        after_insert(&mut ctx);
        op(&ctx)
    }
}

/// Relative time emphasis applied by a residual weighting scheme at pillar `t`.
///
/// # Arguments
///
/// * `scheme` - Weighting scheme selected for the curve family being solved.
/// * `t` - Pillar time in years (positive).
pub(crate) fn scheme_factor(scheme: &ResidualWeightingScheme, t: f64) -> f64 {
    match scheme {
        ResidualWeightingScheme::Equal => 1.0,
        ResidualWeightingScheme::LinearTime => t,
        ResidualWeightingScheme::SqrtTime => t.sqrt(),
        ResidualWeightingScheme::InverseDuration => 1.0 / t.max(0.1),
    }
}

/// Sort `(time, initial_guess, quote)` entries by time and split them into the
/// strictly increasing knot grid the global solver requires.
///
/// # Arguments
///
/// * `entries` - Unordered `(pillar time, initial parameter guess, quote)` triples.
/// * `label` - Curve family name used in the duplicate-knot error message.
///
/// # Errors
///
/// Returns `Error::Calibration` when two entries fall within
/// [`TOLERANCE_DUP_KNOTS`] of each other.
pub(crate) fn sorted_knot_grid<Q>(
    mut entries: Vec<(f64, f64, Q)>,
    label: &str,
) -> Result<(Vec<f64>, Vec<f64>, Vec<Q>)> {
    entries.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut times: Vec<f64> = Vec::with_capacity(entries.len());
    let mut initials = Vec::with_capacity(entries.len());
    let mut active_quotes = Vec::with_capacity(entries.len());
    for (t, guess, quote) in entries {
        if let Some(&prev) = times.last() {
            if (t - prev).abs() <= TOLERANCE_DUP_KNOTS {
                return Err(finstack_quant_core::Error::Calibration {
                    message: format!(
                        "Duplicate or unsorted {label} knot times detected (prev={prev:.10}, new={t:.10}). \
Ensure quotes map to strictly increasing year fractions."
                    ),
                    category: "global_solve".to_string(),
                });
            }
        }
        times.push(t);
        initials.push(guess);
        active_quotes.push(quote);
    }
    Ok((times, initials, active_quotes))
}

/// Interpolate implied volatility in expiry by linear interpolation of total
/// variance `w = σ²T` at a fixed absolute strike (Gatheral).
///
/// Each neighbouring slice is evaluated by `total_variance` in its own
/// forward-moneyness coordinate, so the interpolated `w` refers to the same
/// absolute strike across `T₁`, `T`, and `T₂`. Linear interpolation of `w` in
/// `T` is calendar-safe whenever the calibrated slices are calendar-monotone.
/// Extrapolation holds the nearest slice's `w` flat (`Clamp`) or rejects the
/// target (`Error`).
///
/// # Arguments
///
/// * `label` - Model name used in error messages (e.g. `"SABR"`, `"SVI"`).
/// * `target_expiry` - Target expiry in years; must be positive.
/// * `target_strike` - Absolute strike the grid cell is evaluated at.
/// * `slices` - Calibrated per-expiry slice parameters keyed by expiry.
/// * `total_variance` - `(slice_expiry, slice_params) -> w` evaluator; must
///   return a finite, non-negative total variance at `target_strike`.
/// * `extrapolation` - Policy applied when `target_expiry` lies outside the
///   calibrated expiry range.
///
/// # Errors
///
/// Returns `Error::Validation` for a non-positive target expiry, an
/// out-of-range target under `Error`, or an invalid interpolated variance, and
/// `Error::Calibration` when `slices` is empty.
pub(crate) fn interpolate_total_variance<S>(
    label: &str,
    target_expiry: f64,
    target_strike: f64,
    slices: &BTreeMap<OrderedF64, S>,
    total_variance: impl Fn(f64, &S) -> Result<f64>,
    extrapolation: SurfaceExtrapolationPolicy,
) -> Result<f64> {
    if target_expiry <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{label} interpolation target expiry must be positive; got {target_expiry:.6}"
        )));
    }

    let (Some((&min_key, _)), Some((&max_key, _))) =
        (slices.iter().next(), slices.iter().next_back())
    else {
        return Err(finstack_quant_core::Error::Calibration {
            message: format!("No calibrated {label} parameters"),
            category: "vol_surface".to_string(),
        });
    };
    let min_t = min_key.into_inner();
    let max_t = max_key.into_inner();

    if extrapolation == SurfaceExtrapolationPolicy::Error
        && (target_expiry < min_t || target_expiry > max_t)
    {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Target expiry t={target_expiry:.6} is out of bounds for calibrated expiries \
[{min_t:.6}, {max_t:.6}]. Set params.expiry_extrapolation='clamp' to allow flat \
total-variance extrapolation."
        )));
    }

    let mut before = None;
    let mut after = None;
    for (&kt, p) in slices {
        let kt_f = kt.into_inner();
        if kt_f <= target_expiry {
            before = Some((kt_f, p));
        }
        if kt_f >= target_expiry && after.is_none() {
            after = Some((kt_f, p));
        }
    }

    let w = match (before, after) {
        (Some((t1, p1)), Some((t2, p2))) if (t2 - t1).abs() > 1e-12 => {
            let w1 = total_variance(t1, p1)?;
            let w2 = total_variance(t2, p2)?;
            let tau = ((target_expiry - t1) / (t2 - t1)).clamp(0.0, 1.0);
            w1 + tau * (w2 - w1)
        }
        (Some((t, p)), _) | (_, Some((t, p))) => total_variance(t, p)?,
        (None, None) => {
            return Err(finstack_quant_core::Error::Calibration {
                message: format!("No {label} expiry neighbours for target t={target_expiry:.6}"),
                category: "vol_surface".to_string(),
            });
        }
    };
    if !w.is_finite() || w < 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{label} total-variance interpolation produced invalid w={w:.6} at \
T={target_expiry:.6}, K={target_strike:.4}"
        )));
    }
    Ok((w / target_expiry).sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::dividends::DividendSchedule;
    use finstack_quant_core::money::Money;
    use time::macros::date;

    fn discount_curve() -> DiscountCurve {
        DiscountCurve::builder("USD-OIS")
            .base_date(date!(2025 - 01 - 01))
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (2.0, (-0.06_f64).exp())])
            .build()
            .expect("discount curve")
    }

    #[test]
    fn missing_equity_carry_is_rejected() {
        let discount = discount_curve();
        let error = resolve_equity_forward_inputs(
            "SPX",
            date!(2025 - 01 - 01),
            100.0,
            None,
            &discount,
            &MarketContext::new(),
        )
        .expect_err("missing carry");
        assert!(error.to_string().contains("dividend"));
    }

    #[test]
    fn equity_forward_is_invariant_to_curve_clock_and_base() {
        let base = date!(2025 - 01 - 01);
        let surface_base = date!(2025 - 04 - 01);
        let expiry = date!(2026 - 04 - 01);
        let ex_date = date!(2025 - 07 - 01);
        let schedule = DividendSchedule::builder("SPX-DIVS")
            .underlying("SPX")
            .currency(Currency::USD)
            .cash(ex_date, Money::from((5_i64, Currency::USD)))
            .build()
            .expect("dividends");
        let context = MarketContext::new().insert_dividends(schedule);
        let expected = (100.0
            - 5.0 * (-0.05 * (ex_date - surface_base).whole_days() as f64 / 365.0).exp())
            * (0.05_f64).exp();
        for (day_count, denominator) in [(DayCount::Act365F, 365.0), (DayCount::Act360, 360.0)] {
            for curve_base in [base, surface_base] {
                let days = (expiry - curve_base).whole_days() as f64;
                let curve = DiscountCurve::builder("USD")
                    .base_date(curve_base)
                    .day_count(day_count)
                    .knots([
                        (0.0, 1.0),
                        (days / denominator, (-0.05 * days / 365.0).exp()),
                    ])
                    .build()
                    .expect("curve");
                let inputs = resolve_equity_forward_inputs(
                    "SPX",
                    surface_base,
                    100.0,
                    None,
                    &curve,
                    &context,
                )
                .expect("carry");
                assert!((inputs.forward(&curve, 1.0).expect("forward") - expected).abs() < 1e-10);
                let continuous = resolve_equity_forward_inputs(
                    "SPX",
                    surface_base,
                    100.0,
                    Some(0.02),
                    &curve,
                    &context,
                )
                .expect("yield");
                assert!(
                    (continuous.forward(&curve, 1.0).expect("forward") - 100.0 * (0.03_f64).exp())
                        .abs()
                        < 1e-10
                );
            }
        }
    }

    #[test]
    fn cash_dividend_schedule_drives_prepaid_forward() {
        let discount = discount_curve();
        let ex_date = date!(2025 - 07 - 01);
        let schedule = DividendSchedule::builder("SPX-DIVS")
            .underlying("SPX")
            .currency(Currency::USD)
            .cash(ex_date, Money::from((5_i64, Currency::USD)))
            .build()
            .expect("dividend schedule");
        let context = MarketContext::new().insert_dividends(schedule);
        let inputs = resolve_equity_forward_inputs(
            "SPX",
            date!(2025 - 01 - 01),
            100.0,
            None,
            &discount,
            &context,
        )
        .expect("schedule carry");

        let event_time = DayCount::Act365F
            .year_fraction(date!(2025 - 01 - 01), ex_date, DayCountContext::default())
            .expect("event time");
        let expected = (100.0 - 5.0 * discount.df(event_time)) / discount.df(1.0);
        let actual = inputs.forward(&discount, 1.0).expect("forward");
        assert!((actual - expected).abs() < 1e-12);
    }
}
