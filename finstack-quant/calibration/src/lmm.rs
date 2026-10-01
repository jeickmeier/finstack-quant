//! LMM/BGM factor-loading calibration to the swaption volatility surface.
//!
//! The Bermudan LMM pricer uses a flat 2-factor loading structure
//!
//! ```text
//! λ_i = base_vol · ĝ_i,   ĝ_i = [1 − α·f_i, α·f_i, 0],   f_i = i / N
//! ```
//!
//! The shape vectors `ĝ_i` are fixed (a linear-decay proxy for the first two
//! principal components of the forward-rate correlation matrix), but the
//! overall scale `base_vol` is calibrated to the earliest remaining
//! co-terminal European swaption using the Rebonato approximation.
//! Plugging a raw swaption-surface vol straight in as `base_vol`
//! (the previous behaviour) is wrong: the surface quotes the *swap-rate*
//! Black vol, not the *forward-rate* instantaneous vol — the two differ by
//! the Rebonato shape factor `R` derived below.
//!
//! # Rebonato swaption-vol approximation
//!
//! For a European swaption with expiry `T_e` on the co-terminal swap covering
//! forwards `[first, N)`, the forward swap rate is the weighted basket
//! `S = Σ_i w_i F_i` with annuity weights `w_i = τ_i P(0,T_{i+1}) / A`. Its
//! Black variance to expiry is (Rebonato 2002, Ch. 8; Andersen–Piterbarg
//! 2010, §16.5)
//!
//! ```text
//! σ²_swaption · T_e ≈ (1/S²) Σ_i Σ_j w_i w_j F_i F_j ∫₀^{T_e} λ_i(t)·λ_j(t) dt
//! ```
//!
//! With **time-constant** loadings `λ_i = base_vol · ĝ_i` the integral is
//! `base_vol² · (ĝ_i·ĝ_j) · T_e`, so the swaption vol is *exactly linear* in
//! `base_vol`:
//!
//! ```text
//! σ_swaption = base_vol · R,
//! R = sqrt( (1/S²) Σ_i Σ_j w_i w_j F_i F_j (ĝ_i·ĝ_j) )
//! ```
//!
//! Without displacement, calibration is the closed-form
//! `base_vol = σ_market / R`. This matches the market volatility within the
//! frozen-weight Rebonato approximation; it is not an exact multi-forward
//! Monte Carlo repricing guarantee.
//!
//! For displaced (shifted-lognormal) dynamics the same identity holds with
//! `F_i → F_i + d_i` and `S → S + d`, which is the basket level the
//! shifted-lognormal swap rate diffuses. The market surface quotes the
//! *Black lognormal* vol on `S`, while `base_vol · R` is the lognormal vol
//! of the shifted level `S + d`. The market ATM Black premium is therefore
//! inverted at shifted forward and strike `S + d`, at the same expiry, before
//! the `1/R` division. This preserves the finite-expiry premium, including
//! for a single forward where the displaced-lognormal model is exact.
//!
//! # References
//!
//! - Rebonato, R. (2002). *Modern Pricing of Interest-Rate Derivatives*,
//!   Ch. 8, Princeton University Press. `docs/REFERENCES.md#rebonato-2004-volatility-correlation`
//! - Andersen, L. & Piterbarg, V. (2010). *Interest Rate Modeling*, Vol. 2,
//!   §16.5, Atlantic Financial Press. `docs/REFERENCES.md#andersen-piterbarg-interest-rate-modeling`

use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::Result;
use finstack_quant_models::closed_form::volatility::black_call;
use finstack_quant_models::volatility::implied_vol_black;
use finstack_quant_valuations::instruments::rates::swaption::lmm_pricer::BermudanSwaptionLmmPricer;
use finstack_quant_valuations::instruments::rates::swaption::BermudanSwaption;

/// Inputs describing the co-terminal swap underlying one European swaption
/// slice of a Bermudan exercise schedule, expressed in LMM forward-rate
/// coordinates.
#[derive(Debug, Clone)]
pub(crate) struct CoTerminalSlice<'a> {
    /// Tenor dates `T_0..T_N` (year fractions, length `N+1`).
    pub tenors: &'a [f64],
    /// Accrual factors `τ_i = T_{i+1} − T_i` (length `N`).
    pub accrual_factors: &'a [f64],
    /// Initial forward rates `F_i(0)` (length `N`).
    pub initial_forwards: &'a [f64],
    /// Displacements `d_i` (length `N`).
    pub displacements: &'a [f64],
    /// Unscaled factor-loading shapes `ĝ_i` per forward (length `N`).
    pub loading_shapes: &'a [[f64; 3]],
    /// Index of the first forward alive at the swaption expiry (`first`).
    pub first_alive: usize,
}

/// Rebonato decomposition of the co-terminal swap: shape factor plus the
/// basket levels needed to convert between Black and displaced vols.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RebonatoFactors {
    /// Shape factor `R` linking `base_vol` to the *displaced* swaption vol.
    pub shape_factor: f64,
    /// Unshifted forward swap rate `S = Σ w_i F_i`.
    pub swap_rate: f64,
    /// Shifted basket level `S + d = Σ w_i (F_i + d_i)`.
    pub shifted_level: f64,
}

/// Full Rebonato decomposition; see [`RebonatoFactors`].
pub(crate) fn rebonato_factors(slice: &CoTerminalSlice<'_>) -> Option<RebonatoFactors> {
    let n = slice.accrual_factors.len();
    let first = slice.first_alive;
    if first >= n || slice.tenors.len() != n + 1 {
        return None;
    }

    // Discount factors P(0, T_j) for j = first..=N from the live forwards.
    // P(0, T_first) is the numeraire base; carry it as 1.0 and divide out via
    // the annuity weights, which is scale-invariant for the basket.
    let live = n - first;
    let mut df = vec![1.0_f64; live + 1];
    for k in 1..=live {
        let idx = first + k - 1;
        let denom = 1.0 + slice.accrual_factors[idx] * slice.initial_forwards[idx];
        if denom.abs() < 1e-15 {
            return None;
        }
        df[k] = df[k - 1] / denom;
    }

    // Annuity A = Σ τ_j P(0, T_{j+1}).
    let mut annuity = 0.0_f64;
    for k in 0..live {
        annuity += slice.accrual_factors[first + k] * df[k + 1];
    }
    if annuity.abs() < 1e-15 {
        return None;
    }

    // Shifted basket level S + d = Σ w_j (F_j + d_j), weights w_j = τ_j DF_{j+1}/A.
    // The displaced-lognormal swap rate diffuses about this shifted level.
    // The unshifted swap rate S = Σ w_j F_j is carried alongside for the
    // Black → displaced vol conversion.
    let mut weights = vec![0.0_f64; live];
    let mut basket = 0.0_f64;
    let mut swap_rate = 0.0_f64;
    for k in 0..live {
        let idx = first + k;
        let w = slice.accrual_factors[idx] * df[k + 1] / annuity;
        weights[k] = w;
        swap_rate += w * slice.initial_forwards[idx];
        basket += w * (slice.initial_forwards[idx] + slice.displacements[idx]);
    }
    if !(basket.is_finite()) || basket <= 1e-12 {
        return None;
    }

    // R² = (1/S²) Σ_i Σ_j w_i w_j (F_i+d_i)(F_j+d_j) (ĝ_i·ĝ_j).
    let mut acc = 0.0_f64;
    for ki in 0..live {
        let i = first + ki;
        let fi = slice.initial_forwards[i] + slice.displacements[i];
        let gi = slice.loading_shapes[i];
        for kj in 0..live {
            let j = first + kj;
            let fj = slice.initial_forwards[j] + slice.displacements[j];
            let gj = slice.loading_shapes[j];
            let dot = gi[0] * gj[0] + gi[1] * gj[1] + gi[2] * gj[2];
            acc += weights[ki] * weights[kj] * fi * fj * dot;
        }
    }
    let r_sq = acc / (basket * basket);
    if !(r_sq.is_finite()) || r_sq <= 0.0 {
        return None;
    }
    Some(RebonatoFactors {
        shape_factor: r_sq.sqrt(),
        swap_rate,
        shifted_level: basket,
    })
}

/// Calibrate the LMM `base_vol` to the market ATM Black premium within the
/// frozen-weight Rebonato approximation.
///
/// The Black vol quotes lognormal dynamics on the unshifted swap rate `S`,
/// while `base_vol · R` is the lognormal vol of the shifted level `S + d`.
/// The Black premium is inverted at shifted forward and strike `S + d` and
/// then divided by `R`. With zero displacement the conversion is the identity.
///
/// # Arguments
///
/// * `slice` - Remaining co-terminal forward basket and unscaled factor loadings.
/// * `market_swaption_vol` - Positive finite annual Black-lognormal volatility,
///   expressed as a decimal, at the unshifted ATM swap rate.
/// * `expiry` - Positive finite ACT/365F time from valuation to exercise, in years.
///
/// # Returns
///
/// Returns `None` for invalid expiry or volatility, a degenerate swap, or an
/// ATM premium that cannot be converted to a positive finite loading scale.
pub(crate) fn calibrate_base_vol(
    slice: &CoTerminalSlice<'_>,
    market_swaption_vol: f64,
    expiry: f64,
) -> Option<f64> {
    if !market_swaption_vol.is_finite()
        || market_swaption_vol <= 0.0
        || !expiry.is_finite()
        || expiry <= 0.0
    {
        return None;
    }
    let factors = rebonato_factors(slice)?;
    let shape_factor = factors.shape_factor;
    if shape_factor <= 1e-12 || factors.swap_rate <= 1e-12 {
        return None;
    }
    let displaced_vol = if factors.swap_rate == factors.shifted_level {
        market_swaption_vol
    } else {
        let premium = black_call(
            factors.swap_rate,
            factors.swap_rate,
            market_swaption_vol,
            expiry,
        );
        implied_vol_black(
            premium,
            factors.shifted_level,
            factors.shifted_level,
            expiry,
            true,
        )
        .ok()?
    };
    let base_vol = displaced_vol / shape_factor;
    (base_vol.is_finite() && base_vol > 0.0).then_some(base_vol)
}

/// Calibrate the explicit flat LMM loading scale for a Bermudan swaption.
///
/// The helper constructs the same valuations-owned tenor, forward,
/// displacement, and loading shape used by pricing, validates the swaption
/// surface as an ACT/365F expiry-by-strike Black-lognormal grid, targets the
/// earliest future exercise, and matches its ATM premium in the Rebonato
/// approximation. Past exercises and their preceding tenor periods are omitted.
///
/// # Arguments
///
/// * `swaption` - Bermudan contract whose schedule, exercise dates, curve
///   roles, and volatility-surface identifier define the target.
/// * `market` - Immutable market containing discount/projection curves and the
///   Black-lognormal swaption volatility surface.
/// * `as_of` - Calibration date used for curve and expiry year fractions;
///   only exercise dates strictly after this date remain eligible.
///
/// # Errors
///
/// Returns an error for invalid schedules, missing curves or surfaces,
/// unsupported LMM contract terms, mis-tagged volatility grids, invalid quotes,
/// a schedule without future exercises, or a degenerate Rebonato map.
/// The pricing engine's single-curve, matching-leg,
/// zero-spread/reset/payment-lag domain applies equally to calibration.
pub fn calibrate_bermudan_lmm_base_vol(
    swaption: &BermudanSwaption,
    market: &MarketContext,
    as_of: Date,
) -> Result<f64> {
    let discount = market.get_discount(swaption.get_discount_curve_id().as_str())?;
    let structure =
        BermudanSwaptionLmmPricer::build_lmm_params(swaption, discount.as_ref(), as_of, 1.0)
            .map_err(|error| {
                finstack_quant_core::Error::Validation(format!(
                    "LMM structure construction failed for '{}': {error}",
                    swaption.id
                ))
            })?;

    // The shared structure starts at the earliest remaining exercise, so its
    // clock and live forwards are identical to those used by the pricer.
    let expiry = structure.tenors[0];
    let loading_shapes = structure.vol_values.first().ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!(
            "LMM calibration for '{}' has no loading structure",
            swaption.id
        ))
    })?;
    let slice = CoTerminalSlice {
        tenors: &structure.tenors,
        accrual_factors: &structure.accrual_factors,
        initial_forwards: &structure.initial_forwards,
        displacements: &structure.displacements,
        loading_shapes,
        first_alive: 0,
    };
    let factors = rebonato_factors(&slice).ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!(
            "LMM calibration for '{}' has a degenerate co-terminal swap",
            swaption.id
        ))
    })?;

    let surface = market.get_surface(swaption.vol_surface_id.as_str())?;
    surface.require_secondary_axis(
        finstack_quant_core::market_data::surfaces::VolSurfaceAxis::Strike,
    )?;
    surface.require_quote_type(
        finstack_quant_core::market_data::surfaces::VolQuoteType::BlackLognormal,
    )?;
    let market_vol = finstack_quant_models::volatility::get_surface_vol_clamped(
        &surface,
        expiry,
        factors.swap_rate,
    );
    calibrate_base_vol(&slice, market_vol, expiry).ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!(
            "LMM calibration for '{}' is degenerate at expiry {expiry} and swap rate {}",
            swaption.id, factors.swap_rate
        ))
    })
}

/// Parse a canonical instrument envelope and calibrate the Bermudan LMM loading scale.
///
/// The envelope must contain a `bermudan_swaption` payload. Host bindings
/// extract only the market and valuation date; this function owns the
/// instrument parse and kind check.
///
/// # Arguments
///
/// * `instrument_json` - Canonical instrument envelope JSON. The payload
///   `type` must be `bermudan_swaption`; any other supported instrument is
///   rejected after parse.
/// * `market` - Immutable market containing discount/projection curves and the
///   Black-lognormal swaption volatility surface.
/// * `as_of` - Calibration date used for curve and expiry year fractions;
///   exercise dates on or before this date are excluded from calibration.
///
/// # Errors
///
/// Returns an error if `instrument_json` is malformed, exceeds the instrument
/// size cap, is not a Bermudan swaption, or the typed calibration fails.
pub fn calibrate_bermudan_lmm_base_vol_from_json(
    instrument_json: &str,
    market: &MarketContext,
    as_of: Date,
) -> Result<f64> {
    let instrument =
        finstack_quant_valuations::pricer::parse_instrument_from_json(instrument_json)?;
    let finstack_quant_valuations::instruments::InstrumentJson::BermudanSwaption(swaption) =
        instrument
    else {
        return Err(finstack_quant_core::Error::Validation(format!(
            "instrument_json must contain a bermudan_swaption envelope, got '{}'",
            instrument.type_tag()
        )));
    };
    calibrate_bermudan_lmm_base_vol(&swaption, market, as_of)
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::Tenor;
    use finstack_quant_core::market_data::surfaces::VolSurface;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::Money;
    use finstack_quant_valuations::instruments::rates::hw1f::RateExoticMcConfig;
    use finstack_quant_valuations::instruments::rates::irs::FloatingLegCompounding;
    use finstack_quant_valuations::instruments::rates::swaption::lmm_pricer::BermudanSwaptionLmmPricer;
    use finstack_quant_valuations::instruments::rates::swaption::{
        BermudanSchedule, BermudanSwaption,
    };
    use finstack_quant_valuations::instruments::OptionType;
    use finstack_quant_valuations::pricer::Pricer;
    use time::Month;

    const SURFACE_VOL: f64 = 0.22;

    fn test_swaption() -> BermudanSwaption {
        let swap_start = Date::from_calendar_date(2026, Month::January, 17).expect("swap start");
        let swap_end = Date::from_calendar_date(2032, Month::January, 17).expect("swap end");
        let schedule = BermudanSchedule::new(
            (2028..2032)
                .map(|year| {
                    Date::from_calendar_date(year, Month::January, 17).expect("exercise date")
                })
                .collect(),
        );
        let mut swaption = BermudanSwaption::new(
            "BERM-LMM-CAL",
            OptionType::Call,
            Money::from((10_000_000_i64, Currency::USD)),
            0.03,
            swap_start,
            swap_end,
            schedule,
            "D",
            "D",
            "USD-SWPNVOL",
        )
        .expect("Bermudan swaption");
        swaption.underlying_fixed_leg.frequency = Tenor::annual();
        swaption.underlying_float_leg.frequency = Tenor::annual();
        swaption.underlying_float_leg.day_count = swaption.underlying_fixed_leg.day_count;
        // A neutral single-curve ID represents simple tenor forwards without
        // selecting registered overnight-index compounding conventions.
        swaption.underlying_float_leg.compounding = FloatingLegCompounding::Simple;
        swaption.underlying_fixed_leg.business_day_convention =
            finstack_quant_core::dates::BusinessDayConvention::Unadjusted;
        swaption.underlying_float_leg.business_day_convention =
            finstack_quant_core::dates::BusinessDayConvention::Unadjusted;
        swaption
    }

    fn test_discount_curve(as_of: Date, curve_id: &str) -> DiscountCurve {
        DiscountCurve::builder(curve_id)
            .base_date(as_of)
            .knots([
                (0.0, 1.0),
                (1.0, (-0.03_f64).exp()),
                (3.0, (-0.03_f64 * 3.0).exp()),
                (6.0, (-0.03_f64 * 6.0).exp()),
                (12.0, (-0.03_f64 * 12.0).exp()),
            ])
            .build()
            .expect("discount curve")
    }

    fn test_surface() -> VolSurface {
        VolSurface::builder("USD-SWPNVOL")
            .expiries(&[0.5, 12.0])
            .strikes(&[0.001, 0.20])
            .row(&[SURFACE_VOL, SURFACE_VOL])
            .row(&[SURFACE_VOL, SURFACE_VOL])
            .build()
            .expect("swaption surface")
    }

    /// Build the linear-decay loading shapes used by the LMM Bermudan pricer.
    fn loading_shapes(n: usize, alpha: f64) -> Vec<[f64; 3]> {
        (0..n)
            .map(|i| {
                let frac = i as f64 / n.max(1) as f64;
                [1.0 - alpha * frac, alpha * frac, 0.0]
            })
            .collect()
    }

    fn implied_swaption_vol(slice: &CoTerminalSlice<'_>, base_vol: f64, expiry: f64) -> f64 {
        let factors = rebonato_factors(slice).expect("factors");
        let premium = black_call(
            factors.shifted_level,
            factors.shifted_level,
            base_vol * factors.shape_factor,
            expiry,
        );
        implied_vol_black(premium, factors.swap_rate, factors.swap_rate, expiry, true)
            .expect("Black equivalent of displaced premium")
    }

    #[test]
    fn shape_factor_is_positive_and_finite() {
        let tenors = vec![0.0, 1.0, 2.0, 3.0, 4.0];
        let accruals = vec![1.0; 4];
        let forwards = vec![0.03, 0.032, 0.034, 0.036];
        let disp = vec![0.0; 4];
        let shapes = loading_shapes(4, 0.4);
        let slice = CoTerminalSlice {
            tenors: &tenors,
            accrual_factors: &accruals,
            initial_forwards: &forwards,
            displacements: &disp,
            loading_shapes: &shapes,
            first_alive: 0,
        };
        let r = rebonato_factors(&slice).expect("factors").shape_factor;
        assert!(
            r.is_finite() && r > 0.0,
            "R must be positive finite, got {r}"
        );
    }

    #[test]
    fn public_helper_produces_positive_finite_override() {
        let as_of = Date::from_calendar_date(2025, Month::January, 17).expect("as of");
        let market = MarketContext::new()
            .insert(test_discount_curve(as_of, "D"))
            .insert_surface(test_surface());
        let base_vol = calibrate_bermudan_lmm_base_vol(&test_swaption(), &market, as_of)
            .expect("LMM base-vol calibration");

        assert!(base_vol.is_finite() && base_vol > 0.0);
        assert!(
            (base_vol - SURFACE_VOL).abs() > 1.0e-6,
            "the forward-loading scale must not be the raw swaption quote"
        );
    }

    #[test]
    fn public_helper_matches_exact_one_forward_displaced_black_premium() {
        use finstack_quant_core::dates::DayCount;
        use finstack_quant_core::math::interp::InterpStyle;
        use finstack_quant_models::closed_form::volatility::black_shifted_call;

        // Ten-year expiry and one one-year forward reproduce the supported
        // low-rate case where the old instantaneous conversion overpriced by 7.7%.
        let as_of = time::macros::date!(2025 - 01 - 01);
        let start = time::macros::date!(2034 - 12 - 30);
        let end = time::macros::date!(2035 - 12 - 30);
        let mut swaption = test_swaption();
        swaption.underlying_fixed_leg.start = start;
        swaption.underlying_fixed_leg.end = end;
        swaption.underlying_fixed_leg.day_count = DayCount::Act365F;
        swaption.underlying_float_leg.start = start;
        swaption.underlying_float_leg.end = end;
        swaption.underlying_float_leg.day_count = DayCount::Act365F;
        swaption.exercise_schedule = BermudanSchedule::new(vec![start]);
        let rate = 1.005_f64.ln();
        let curve = DiscountCurve::builder("D")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (20.0, (-rate * 20.0).exp())])
            .interp(InterpStyle::LogLinear)
            .build()
            .expect("low-rate curve");
        let surface = VolSurface::builder("USD-SWPNVOL")
            .expiries(&[0.5, 12.0])
            .strikes(&[0.001, 0.20])
            .row(&[0.50, 0.50])
            .row(&[0.50, 0.50])
            .build()
            .expect("Black surface");
        let market = MarketContext::new()
            .insert(curve.clone())
            .insert_surface(surface);
        let base_vol = calibrate_bermudan_lmm_base_vol_from_json(
            &bermudan_envelope_json(swaption.clone()),
            &market,
            as_of,
        )
        .expect("one-forward calibration");
        let params =
            BermudanSwaptionLmmPricer::build_lmm_params(&swaption, &curve, as_of, base_vol)
                .expect("one-forward structure");

        assert_eq!(params.num_forwards, 1);
        assert_eq!(params.tenors[0], 10.0);
        assert!((params.initial_forwards[0] - 0.005).abs() < 1e-14);
        assert!((params.displacements[0] - 0.005).abs() < 1e-14);
        let model_premium = black_shifted_call(
            params.initial_forwards[0],
            params.initial_forwards[0],
            base_vol,
            params.tenors[0],
            params.displacements[0],
        );
        let market_premium = black_call(
            params.initial_forwards[0],
            params.initial_forwards[0],
            0.50,
            params.tenors[0],
        );
        assert!(
            (model_premium - market_premium).abs() < 1e-14,
            "conversion must preserve the canonical Black premium: model={model_premium}, market={market_premium}"
        );
        // Independent analytical ATM Black premium, per unit annuity.
        // The existing statrs normal-CDF approximation differs from the
        // reference by about 2.1e-13 here. A 1e-12 absolute reference budget
        // is less than $0.00001 on 10MM notional with a unit annuity; the
        // premium-preserving conversion above retains its tighter budget.
        let expected_premium = 0.002_854_023_497_798_254_7;
        assert!(
            (model_premium - expected_premium).abs() < 1e-12,
            "finite-expiry premium mismatch: model={model_premium}, market={expected_premium}"
        );
        assert!((base_vol - 0.25).abs() > 0.01);
    }

    #[test]
    fn calibration_uses_next_exercise_on_and_between_exercise_dates() {
        let start = time::macros::date!(2025 - 01 - 01);
        let next_exercise = time::macros::date!(2026 - 01 - 01);
        let end = time::macros::date!(2027 - 01 - 01);
        let mut swaption = test_swaption();
        swaption.underlying_fixed_leg.start = start;
        swaption.underlying_fixed_leg.end = end;
        swaption.underlying_float_leg.start = start;
        swaption.underlying_float_leg.end = end;
        swaption.exercise_schedule = BermudanSchedule::new(vec![start, next_exercise]);

        for as_of in [start, time::macros::date!(2025 - 07 - 01)] {
            let curve = test_discount_curve(as_of, "D");
            let market = MarketContext::new()
                .insert(curve.clone())
                .insert_surface(test_surface());
            let base_vol = calibrate_bermudan_lmm_base_vol_from_json(
                &bermudan_envelope_json(swaption.clone()),
                &market,
                as_of,
            )
            .expect("remaining-exercise calibration");
            let params =
                BermudanSwaptionLmmPricer::build_lmm_params(&swaption, &curve, as_of, base_vol)
                    .expect("remaining-exercise model");
            let expiry = finstack_quant_models::rates::clock::model_time(as_of, next_exercise);
            assert_eq!(params.num_forwards, 1);
            assert_eq!(params.tenors[0], expiry);
            assert!(params.tenors.iter().all(|tenor| *tenor > 0.0));
            let factors = rebonato_factors(&CoTerminalSlice {
                tenors: &params.tenors,
                accrual_factors: &params.accrual_factors,
                initial_forwards: &params.initial_forwards,
                displacements: &params.displacements,
                loading_shapes: &params.vol_values[0],
                first_alive: 0,
            })
            .expect("actual calibrated loadings");
            let model_premium = black_call(
                factors.shifted_level,
                factors.shifted_level,
                factors.shape_factor,
                expiry,
            );
            let market_premium =
                black_call(factors.swap_rate, factors.swap_rate, SURFACE_VOL, expiry);
            assert!((model_premium - market_premium).abs() < 1e-14);
        }
    }

    #[test]
    fn calibration_rejects_contract_without_remaining_exercises() {
        let swaption = test_swaption();
        let as_of = swaption.last_exercise().expect("last exercise");
        let market = MarketContext::new().insert(test_discount_curve(as_of, "D"));
        let error = calibrate_bermudan_lmm_base_vol(&swaption, &market, as_of)
            .expect_err("no remaining option to calibrate")
            .to_string();
        assert!(
            error.contains("future exercise"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn calibration_rejects_overnight_compounding_before_surface_lookup() {
        let as_of = Date::from_calendar_date(2025, Month::January, 17).expect("as of");
        let market = MarketContext::new().insert(test_discount_curve(as_of, "USD-OIS"));
        let mut swaption = test_swaption();
        swaption.underlying_fixed_leg.discount_curve_id = "USD-OIS".into();
        swaption.underlying_float_leg.discount_curve_id = "USD-OIS".into();
        swaption.underlying_float_leg.forward_curve_id = "USD-OIS".into();
        swaption.underlying_float_leg.compounding =
            FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 };
        swaption
            .build_swap_schedule()
            .expect("overnight contract has a valid underlying IRS schedule");

        let error = calibrate_bermudan_lmm_base_vol(&swaption, &market, as_of)
            .expect_err("overnight compounding lies outside the LMM domain")
            .to_string();
        assert!(
            error.contains("LMM unsupported contract") && error.contains("simple compounding"),
            "unexpected error: {error}"
        );
        assert!(
            !error.contains("surface"),
            "calibration attempted a volatility-surface lookup: {error}"
        );
    }

    fn bermudan_envelope_json(swaption: BermudanSwaption) -> String {
        let envelope = finstack_quant_valuations::instruments::InstrumentEnvelope::new(
            finstack_quant_valuations::instruments::InstrumentJson::BermudanSwaption(swaption),
        );
        serde_json::to_string(&envelope).expect("serialize Bermudan envelope")
    }

    #[test]
    fn from_json_matches_typed_helper() {
        let as_of = Date::from_calendar_date(2025, Month::January, 17).expect("as of");
        let market = MarketContext::new()
            .insert(test_discount_curve(as_of, "D"))
            .insert_surface(test_surface());
        let swaption = test_swaption();
        let typed = calibrate_bermudan_lmm_base_vol(&swaption, &market, as_of)
            .expect("typed LMM base-vol calibration");
        let from_json = calibrate_bermudan_lmm_base_vol_from_json(
            &bermudan_envelope_json(swaption),
            &market,
            as_of,
        )
        .expect("JSON LMM base-vol calibration");
        assert_eq!(typed, from_json);
    }

    #[test]
    fn from_json_rejects_non_bermudan_instrument() {
        let as_of = Date::from_calendar_date(2025, Month::January, 17).expect("as of");
        let bond = finstack_quant_valuations::instruments::Bond::fixed(
            "TEST-BOND",
            Money::from((1_000_000_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
            Date::from_calendar_date(2024, Month::January, 1).expect("start"),
            Date::from_calendar_date(2034, Month::January, 1).expect("end"),
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("bond");
        let envelope = finstack_quant_valuations::instruments::InstrumentEnvelope::new(
            finstack_quant_valuations::instruments::InstrumentJson::Bond(bond),
        );
        let json = serde_json::to_string(&envelope).expect("serialize bond envelope");
        let err = calibrate_bermudan_lmm_base_vol_from_json(&json, &MarketContext::new(), as_of)
            .expect_err("non-Bermudan instrument must be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("bermudan_swaption") && msg.contains("bond"),
            "unexpected kind-check error: {msg}"
        );
    }

    #[test]
    fn pricing_consumes_override_without_reading_surface() {
        let as_of = Date::from_calendar_date(2025, Month::January, 17).expect("as of");
        let curve = test_discount_curve(as_of, "D");
        let calibration_market = MarketContext::new()
            .insert(curve.clone())
            .insert_surface(test_surface());
        let mut swaption = test_swaption();
        let base_vol = calibrate_bermudan_lmm_base_vol(&swaption, &calibration_market, as_of)
            .expect("LMM base-vol calibration");
        swaption
            .instrument_pricing_overrides
            .model_config
            .lmm_base_vol = Some(base_vol);

        let pricing_market = MarketContext::new().insert(curve);
        let pricer = BermudanSwaptionLmmPricer::with_config(RateExoticMcConfig {
            num_paths: 32,
            min_steps_between_events: 1,
            ..RateExoticMcConfig::lmm_bermudan()
        });
        let result = pricer
            .price_dyn(&swaption, &pricing_market, as_of)
            .expect("pricing with explicit LMM base vol and no surface");

        assert!(result.value.amount().is_finite());
    }

    #[test]
    fn pricing_rejects_missing_or_invalid_override_before_surface_lookup() {
        let as_of = Date::from_calendar_date(2025, Month::January, 17).expect("as of");
        let market = MarketContext::new().insert(test_discount_curve(as_of, "D"));
        let pricer = BermudanSwaptionLmmPricer::with_config(RateExoticMcConfig {
            num_paths: 8,
            min_steps_between_events: 1,
            ..RateExoticMcConfig::lmm_bermudan()
        });

        for invalid in [None, Some(0.0), Some(-0.1), Some(f64::NAN)] {
            let mut swaption = test_swaption();
            swaption
                .instrument_pricing_overrides
                .model_config
                .lmm_base_vol = invalid;
            let error = pricer
                .price_dyn(&swaption, &market, as_of)
                .expect_err("invalid LMM base vol must fail")
                .to_string();
            assert!(error.contains("lmm_base_vol"), "unexpected error: {error}");
            assert!(
                !error.contains("surface"),
                "pricing attempted a volatility-surface lookup: {error}"
            );
        }
    }

    #[test]
    fn calibrated_base_vol_reprices_swaption() {
        // The whole point: base_vol · R == market vol by construction.
        let tenors = vec![0.0, 1.0, 2.0, 3.0, 4.0];
        let accruals = vec![1.0; 4];
        let forwards = vec![0.03, 0.032, 0.034, 0.036];
        let disp = vec![0.0; 4];
        let shapes = loading_shapes(4, 0.4);
        let slice = CoTerminalSlice {
            tenors: &tenors,
            accrual_factors: &accruals,
            initial_forwards: &forwards,
            displacements: &disp,
            loading_shapes: &shapes,
            first_alive: 0,
        };
        let market_vol = 0.22;
        let base_vol = super::calibrate_base_vol(&slice, market_vol, 2.0).expect("calibration");
        let implied_swaption_vol = implied_swaption_vol(&slice, base_vol, 2.0);
        assert!(
            (implied_swaption_vol - market_vol).abs() < 1e-12,
            "calibrated LMM should reprice swaption vol {market_vol}, got {}",
            implied_swaption_vol
        );
        // base_vol differs from the raw surface vol — this is the defect fix:
        // feeding `market_vol` directly as base_vol would mis-price by 1/R.
        assert!(
            (base_vol - market_vol).abs() > 1e-6,
            "shape factor R must be != 1, otherwise calibration is a no-op"
        );
    }

    #[test]
    fn first_alive_offset_handled() {
        // Co-terminal swaption with expiry past the first tenor: only
        // forwards [first_alive, N) participate.
        let tenors = vec![0.0, 1.0, 2.0, 3.0, 4.0];
        let accruals = vec![1.0; 4];
        let forwards = vec![0.03, 0.032, 0.034, 0.036];
        let disp = vec![0.0; 4];
        let shapes = loading_shapes(4, 0.4);
        let slice = CoTerminalSlice {
            tenors: &tenors,
            accrual_factors: &accruals,
            initial_forwards: &forwards,
            displacements: &disp,
            loading_shapes: &shapes,
            first_alive: 2,
        };
        let base_vol = super::calibrate_base_vol(&slice, 0.20, 2.0).expect("calibration");
        assert!(base_vol.is_finite() && base_vol > 0.0);
        let implied_swaption_vol = implied_swaption_vol(&slice, base_vol, 2.0);
        assert!((implied_swaption_vol - 0.20).abs() < 1e-12);
    }

    #[test]
    fn rejects_degenerate_inputs() {
        let tenors = vec![0.0, 1.0];
        let accruals = vec![1.0];
        let forwards = vec![0.03];
        let disp = vec![0.0];
        let shapes = loading_shapes(1, 0.4);
        let slice = CoTerminalSlice {
            tenors: &tenors,
            accrual_factors: &accruals,
            initial_forwards: &forwards,
            displacements: &disp,
            loading_shapes: &shapes,
            first_alive: 1, // no live forwards
        };
        assert!(rebonato_factors(&slice).is_none());
        assert!(super::calibrate_base_vol(&slice, 0.2, 1.0).is_none());
        // Non-positive market vol rejected.
        let live_slice = CoTerminalSlice {
            first_alive: 0,
            ..slice
        };
        assert!(super::calibrate_base_vol(&live_slice, 0.0, 1.0).is_none());
        assert!(super::calibrate_base_vol(&live_slice, -0.1, 1.0).is_none());
        for expiry in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(super::calibrate_base_vol(&live_slice, 0.2, expiry).is_none());
        }
    }

    #[test]
    fn displaced_calibration_preserves_finite_expiry_atm_premium() {
        let tenors = vec![0.0, 1.0, 2.0, 3.0, 4.0];
        let accruals = vec![1.0; 4];
        let forwards = vec![0.01, 0.012, 0.014, 0.016];
        let shapes = loading_shapes(4, 0.4);
        let shifted = vec![0.02; 4];
        let slice = CoTerminalSlice {
            tenors: &tenors,
            accrual_factors: &accruals,
            initial_forwards: &forwards,
            displacements: &shifted,
            loading_shapes: &shapes,
            first_alive: 0,
        };
        let market_vol = 0.30;
        let expiry = 10.0;
        let base_vol = super::calibrate_base_vol(&slice, market_vol, expiry).expect("calibration");
        let factors = rebonato_factors(&slice).expect("factors");

        // Instantaneous absolute-volatility matching is insufficient at
        // finite expiry. Compare premiums rather than inverting that ratio.
        let instantaneous_base =
            market_vol * factors.swap_rate / factors.shifted_level / factors.shape_factor;
        assert!(
            (base_vol - instantaneous_base).abs() > 1e-4,
            "finite-expiry premium matching must differ from the instantaneous approximation"
        );
        let market_premium = black_call(factors.swap_rate, factors.swap_rate, market_vol, expiry);
        let model_premium = black_call(
            factors.shifted_level,
            factors.shifted_level,
            base_vol * factors.shape_factor,
            expiry,
        );
        assert!(
            (model_premium - market_premium).abs() < 1e-14,
            "model premium {model_premium} must match market premium {market_premium}"
        );
        // The Black-convention implied vol still round-trips.
        let implied_swaption_vol = implied_swaption_vol(&slice, base_vol, expiry);
        assert!(
            (implied_swaption_vol - market_vol).abs() < 1e-12,
            "implied Black vol {} should round-trip market {market_vol}",
            implied_swaption_vol
        );
    }

    #[test]
    fn displacement_shifts_basket() {
        // With a positive displacement the basket level rises, so for the
        // same market vol the calibrated base_vol changes — confirms the
        // shift feeds through the shape factor.
        let tenors = vec![0.0, 1.0, 2.0, 3.0, 4.0];
        let accruals = vec![1.0; 4];
        let forwards = vec![0.01, 0.012, 0.014, 0.016];
        let shapes = loading_shapes(4, 0.4);
        let no_shift = vec![0.0; 4];
        let shifted = vec![0.02; 4];
        let base = CoTerminalSlice {
            tenors: &tenors,
            accrual_factors: &accruals,
            initial_forwards: &forwards,
            displacements: &no_shift,
            loading_shapes: &shapes,
            first_alive: 0,
        };
        let shifted_slice = CoTerminalSlice {
            displacements: &shifted,
            ..base.clone()
        };
        let r0 = rebonato_factors(&base).expect("base factors").shape_factor;
        let r1 = rebonato_factors(&shifted_slice)
            .expect("shifted factors")
            .shape_factor;
        assert!(
            (r0 - r1).abs() > 1e-9,
            "displacement must change the shape factor"
        );
    }
}
