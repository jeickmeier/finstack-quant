use super::pricing::{cap_floor_bachelier_vega, cap_floor_periods, forward_rate_from_df};
use super::targets::{
    reject_at_bound_params, require_quote_vega, HullWhiteCapFloorTarget, PreparedCapFloor,
    HW_NUM_RESTARTS, HW_PERTURB_SCALE, HW_VALIDATION_TOLERANCE, KAPPA_MAX, KAPPA_MIN, SIGMA_MAX,
    SWAPTION_VEGA_FLOOR,
};
use super::*;

/// Calibrate Hull-White 1-factor parameters to cap/floor market quotes.
///
/// Normal cap/floor quotes are first converted to Bachelier cap/floor prices
/// using the supplied discount and projection curves. The HW1F objective then
/// reprices the same cap/floor decomposition using HW1F-implied normal caplet
/// volatilities. A single quote requires `config.fixed_kappa`; otherwise the
/// two model parameters are underdetermined.
///
/// # Arguments
///
/// * `discount_df` - Discount-factor function where `discount_df(t)` returns
///   `P(0,t)` for time `t` in years on the cap/floor quote time axis.
/// * `forward_df` - Projection-curve discount-factor function where
///   `forward_df(t)` returns `P(0,t)` for the same time axis; forward rates
///   are derived from ratios of these factors.
/// * `quotes` - Normal-vol cap or floor market quotes to fit. Each maturity,
///   strike, and volatility is interpreted using the configured payment
///   frequency and standard caplet fixing-date convention.
/// * `config` - Frequency plus fixed-mean-reversion or initial-parameter
///   settings. A one-quote calibration requires `config.fixed_kappa`.
pub fn calibrate_hull_white_to_cap_floors(
    discount_df: &dyn Fn(f64) -> f64,
    forward_df: &dyn Fn(f64) -> f64,
    quotes: &[CapFloorQuote],
    config: CapFloorCalibrationConfig,
) -> finstack_quant_core::Result<(HullWhiteCalibrationParams, CalibrationReport)> {
    if quotes.is_empty() {
        return Err(finstack_quant_core::Error::Validation(
            "Need at least one cap/floor quote for HW1F calibration".to_string(),
        ));
    }
    if quotes.len() == 1 && config.fixed_kappa.is_none() {
        return Err(finstack_quant_core::Error::Validation(
            "One cap/floor quote cannot calibrate both HW1F kappa and sigma; provide fixed_kappa"
                .to_string(),
        ));
    }
    for (idx, quote) in quotes.iter().enumerate() {
        validate_cap_floor_quote(
            quote.maturity,
            quote.strike,
            quote.volatility,
            quote.is_normal_vol,
        )
        .map_err(|err| {
            finstack_quant_core::Error::Validation(format!(
                "Invalid cap/floor quote at index {idx}: {err}"
            ))
        })?;
        // The spot-start caplet is excluded (it has no optionality), so a
        // quote must span at least two periods to contribute any caplet.
        let periods = (quote.maturity * config.frequency.periods_per_year() as f64)
            .round()
            .max(1.0) as usize;
        if periods < 2 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Cap/floor quote at index {idx} ({}Y at {:?} frequency) contains only the \
                 spot-start caplet, which is excluded from calibration; quote a longer maturity",
                quote.maturity, config.frequency
            )));
        }
    }

    let frequency = config.frequency;
    let market_prices: Vec<f64> = quotes
        .iter()
        .map(|quote| {
            bachelier_cap_floor_price(
                discount_df,
                forward_df,
                quote.maturity,
                quote.strike,
                quote.volatility,
                quote.is_cap,
                frequency,
            )
        })
        .collect();
    let vegas: Vec<f64> = quotes
        .iter()
        .map(|quote| {
            let raw = cap_floor_bachelier_vega(
                discount_df,
                forward_df,
                quote.maturity,
                quote.strike,
                quote.volatility,
                frequency,
            );
            let label = format!(
                "{}Y_{}_{:.6}",
                quote.maturity,
                if quote.is_cap { "cap" } else { "floor" },
                quote.strike
            );
            require_quote_vega(raw, SWAPTION_VEGA_FLOOR, &label)
        })
        .collect::<finstack_quant_core::Result<Vec<f64>>>()?;

    if let Some(fixed_kappa) = config.fixed_kappa {
        // Single-parameter (σ only) — keep the 1D path. The generic LM
        // machinery would add no value for a scalar minimisation.
        //
        // Guardrail parity with the two-parameter path: the fixed κ must
        // satisfy the same band the LM box constraints enforce, the σ search
        // spans up to SIGMA_MAX (not an arbitrary smaller cap), an at-bound
        // σ is rejected, and the report residuals are vega-scaled so the
        // validation tolerance is applied on the vol scale, matching the
        // two-parameter objective.
        let fixed = HullWhiteCalibrationParams::new(fixed_kappa, 1e-4)?.kappa;
        if !(KAPPA_MIN..=KAPPA_MAX).contains(&fixed) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Cap/floor HW1F fixed_kappa = {fixed:.6} outside the bounded range \
                 [{KAPPA_MIN}, {KAPPA_MAX}]"
            )));
        }
        let sigma = solve_cap_floor_sigma_for_fixed_kappa(
            fixed,
            discount_df,
            forward_df,
            quotes,
            &market_prices,
            frequency,
        )?;
        if sigma >= SIGMA_MAX * (1.0 - 1e-6) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Cap/floor HW1F sigma calibration hit the upper search bound \
                 ({sigma:.6} ≈ SIGMA_MAX = {SIGMA_MAX}); the quotes are inconsistent \
                 with the fixed kappa = {fixed:.6}"
            )));
        }
        let mut residuals = BTreeMap::new();
        for (idx, quote) in quotes.iter().enumerate() {
            let spec = CapFloorPriceSpec::from_quote(quote, frequency);
            let model_price = hw1f_cap_floor_price(fixed, sigma, discount_df, forward_df, spec);
            residuals.insert(
                format!(
                    "{idx}:{}Y_{}_{:.6}",
                    quote.maturity,
                    if quote.is_cap { "cap" } else { "floor" },
                    quote.strike
                ),
                // Vega-scaled (vol-units) residual, matching the
                // two-parameter LM objective so HW_VALIDATION_TOLERANCE
                // means the same thing on both paths.
                (model_price - market_prices[idx]) / vegas[idx],
            );
        }
        let moneyness = cap_floor_moneyness_summary(quotes, forward_df, frequency);
        let report = enrich_cap_floor_report(
            CalibrationReport::for_type_with_tolerance(
                "hull_white_1f_cap_floor",
                residuals,
                1,
                HW_VALIDATION_TOLERANCE,
            ),
            fixed,
            sigma,
            quotes.len(),
            true,
            frequency,
            moneyness,
        );
        return Ok((HullWhiteCalibrationParams::new(fixed, sigma)?, report));
    }

    // Two-parameter (κ, σ) path via GlobalFitOptimizer.
    let init = config.initial_guess.unwrap_or_default();
    let x0 = [init.kappa.ln(), init.sigma.ln()];

    let prepared: Vec<PreparedCapFloor> = market_prices
        .iter()
        .zip(vegas.iter())
        .map(|(&market_price, &vega)| PreparedCapFloor { market_price, vega })
        .collect();

    let target = HullWhiteCapFloorTarget {
        discount_df,
        forward_df,
        frequency,
        initial_x0: x0,
        prepared,
    };

    let mut config_lm = CalibrationConfig::default();
    config_lm.solver = config_lm
        .solver
        .with_tolerance(1e-12)
        .with_max_iterations(300);

    let multi_start = MultiStartConfig {
        num_restarts: HW_NUM_RESTARTS,
        perturbation_scale: HW_PERTURB_SCALE,
    };

    let (params, report) = GlobalFitOptimizer::optimize_with_multi_start(
        &target,
        quotes,
        &config_lm,
        HW_VALIDATION_TOLERANCE,
        Some(&multi_start),
    )?;

    reject_at_bound_params(
        params.kappa,
        params.sigma,
        "Hull-White cap/floor calibration",
    )?;

    let moneyness = cap_floor_moneyness_summary(quotes, forward_df, frequency);
    let report = enrich_cap_floor_report(
        report.with_metadata("type", "hull_white_1f_cap_floor".to_string()),
        params.kappa,
        params.sigma,
        quotes.len(),
        false,
        frequency,
        moneyness,
    );

    Ok((
        HullWhiteCalibrationParams::new(params.kappa, params.sigma)?,
        report,
    ))
}

/// Apply cap/floor metadata shared by the fixed-kappa and two-parameter paths.
fn enrich_cap_floor_report(
    report: CalibrationReport,
    kappa: f64,
    sigma: f64,
    quote_count: usize,
    fixed_kappa: bool,
    frequency: SwapFrequency,
    moneyness: MoneynessSummary,
) -> CalibrationReport {
    report
        .with_model_version(crate::versions::HULL_WHITE_1F)
        .with_metadata("kappa", format!("{kappa:.6}"))
        .with_metadata("sigma", format!("{sigma:.6}"))
        .with_metadata("quote_count", quote_count.to_string())
        .with_metadata("fixed_kappa", fixed_kappa.to_string())
        .with_metadata(
            "residual_weighting",
            "1/vega (vega-weighted price residual)".to_string(),
        )
        .with_metadata("calibration_family", "cap_floor_hw1f".to_string())
        .with_metadata("frequency", frequency.to_string())
        // Off-ATM diagnostic. Vega-weighted residuals linearise
        // around the *ATM* vega, so quotes whose strikes are far from the
        // per-caplet forward rate sit outside the regime where the
        // linearisation is accurate. Report both the max and mean
        // |strike − fwd| / fwd across all caplets so an analyst can spot
        // when the calibration was driven by deep-OTM/ITM quotes (the LM
        // objective is still descent-compatible but its scaling is
        // distorted; see the HW1F module-level docstring).
        .with_metadata("max_moneyness_distance", format!("{:.6}", moneyness.max))
        .with_metadata("mean_moneyness_distance", format!("{:.6}", moneyness.mean))
}

/// Aggregate off-ATM diagnostic: `|strike − caplet_forward| / caplet_forward`
/// summarised across every caplet of every cap/floor quote in the basket.
/// Returned zero for an empty basket or when forwards are non-positive.
#[derive(Clone, Copy, Debug, Default)]
struct MoneynessSummary {
    max: f64,
    mean: f64,
}

fn cap_floor_moneyness_summary(
    quotes: &[CapFloorQuote],
    forward_df: &dyn Fn(f64) -> f64,
    frequency: SwapFrequency,
) -> MoneynessSummary {
    let mut max_dist = 0.0_f64;
    let mut sum_dist = 0.0_f64;
    let mut count = 0_usize;
    for quote in quotes {
        for (t_start, t_end, _accrual) in cap_floor_periods(quote.maturity, frequency) {
            let fwd = forward_rate_from_df(forward_df, t_start, t_end);
            if !fwd.is_finite() || fwd.abs() < 1e-12 {
                continue;
            }
            let dist = ((quote.strike - fwd) / fwd).abs();
            if dist.is_finite() {
                max_dist = max_dist.max(dist);
                sum_dist += dist;
                count += 1;
            }
        }
    }
    if count == 0 {
        MoneynessSummary::default()
    } else {
        MoneynessSummary {
            max: max_dist,
            mean: sum_dist / count as f64,
        }
    }
}

pub(super) fn validate_cap_floor_quote(
    maturity: f64,
    strike: f64,
    volatility: f64,
    is_normal_vol: bool,
) -> finstack_quant_core::Result<()> {
    if !maturity.is_finite() || maturity <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "cap/floor maturity must be positive, got {maturity}"
        )));
    }
    if !strike.is_finite() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "cap/floor strike must be finite, got {strike}"
        )));
    }
    if !volatility.is_finite() || volatility <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "cap/floor volatility must be positive, got {volatility}"
        )));
    }
    if !is_normal_vol {
        return Err(finstack_quant_core::Error::Validation(
            "cap/floor HW1F calibration currently requires normal/Bachelier vol quotes".to_string(),
        ));
    }
    Ok(())
}

/// Calibrate the HW1F volatility `sigma` against a basket of cap/floor quotes for a
/// fixed mean-reversion `kappa`.
///
/// # Item 7: minimise a residual norm, not a signed sum
///
/// A previous implementation root-found the **signed sum** `Σ (price_i − market_i)`
/// with a Brent solver. With more than one cap in the basket, opposite pricing errors
/// cancel in that sum: a `sigma` that overprices one cap and underprices another by the
/// same amount makes the signed sum zero, so Brent reports a "root" that is **not** a
/// least-squares fit — every individual cap can still be badly mispriced.
///
/// This implementation minimises the sum of squared residuals `Σ (price_i − market_i)²`
/// instead. Each cap/floor price is monotone in `sigma` (positive vega), so each squared
/// residual is unimodal in `sigma` and the SSE is unimodal — a golden-section search
/// over the plausible normal-vol range converges to the unique least-squares optimum.
pub(super) fn solve_cap_floor_sigma_for_fixed_kappa(
    kappa: f64,
    discount_df: &dyn Fn(f64) -> f64,
    forward_df: &dyn Fn(f64) -> f64,
    quotes: &[CapFloorQuote],
    market_prices: &[f64],
    frequency: SwapFrequency,
) -> finstack_quant_core::Result<f64> {
    // Sum of squared residuals across the whole basket. A non-finite price (pathological
    // sigma) is mapped to `+inf` so the minimiser steers away from it.
    let sse = |sigma: f64| -> f64 {
        let mut acc = 0.0_f64;
        for (quote, market_price) in quotes.iter().zip(market_prices.iter()) {
            let spec = CapFloorPriceSpec::from_quote(quote, frequency);
            let price = hw1f_cap_floor_price(kappa, sigma, discount_df, forward_df, spec);
            if !price.is_finite() {
                return f64::INFINITY;
            }
            let r = price - market_price;
            acc += r * r;
        }
        acc
    };

    // Plausible normal-vol search range for cap/floor sigma. The full
    // interval `[1e-8, SIGMA_MAX]` is split into three sub-brackets and each
    // is minimised independently. A single golden-section
    // sweep assumes the SSE is unimodal in σ, which holds for a single quote
    // (each cap's price is monotone in σ) but **not** for multi-quote
    // baskets at different strikes where individual squared residuals can
    // bottom out at different σ values, creating local minima between
    // them. Multi-start with one bracket per decade catches that case at
    // negligible cost (the pricer runs ~200×3 = 600 times vs ~200×1).
    // The upper limit matches the two-parameter LM box constraint
    // (SIGMA_MAX) so both paths search the same σ domain.
    let brackets: [(f64, f64); 3] = [(1e-8, 5e-3), (5e-3, 5e-2), (5e-2, SIGMA_MAX)];

    // Reject the case where the objective is non-finite across the whole range — the
    // pricer cannot produce a usable fit and a silent bogus sigma must not be returned.
    let any_finite = brackets.iter().any(|&(lo, hi)| {
        sse(lo).is_finite() || sse(hi).is_finite() || sse(0.5 * (lo + hi)).is_finite()
    });
    if !any_finite {
        return Err(finstack_quant_core::Error::Validation(
            "Cap/floor HW1F sigma calibration objective is non-finite across the search range"
                .to_string(),
        ));
    }

    let mut best_sigma: Option<f64> = None;
    let mut best_sse = f64::INFINITY;
    for &(lo, hi) in &brackets {
        if let Some((sigma, sse_val)) = golden_section_min(&sse, lo, hi, 1e-12, 200) {
            if sse_val < best_sse {
                best_sse = sse_val;
                best_sigma = Some(sigma);
            }
        }
    }

    let sigma = best_sigma.ok_or_else(|| {
        finstack_quant_core::Error::Validation(
            "Cap/floor HW1F sigma calibration could not locate a finite minimum across any \
             search bracket"
                .to_string(),
        )
    })?;
    if !sigma.is_finite() || sigma <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Cap/floor HW1F sigma calibration produced an invalid sigma: {sigma}"
        )));
    }
    Ok(sigma)
}

/// Fixed-κ settings for sequential piecewise HW1F volatility calibration.
#[derive(Debug, Clone, Copy)]
pub struct PiecewiseSigmaCalibrationConfig {
    /// Mean reversion held fixed while bootstrapping the volatility schedule.
    pub fixed_kappa: f64,
    /// Inclusive lower short-rate volatility search bound.
    pub sigma_min: f64,
    /// Inclusive upper short-rate volatility search bound.
    pub sigma_max: f64,
    /// Coupon frequency used to decompose each market cap/floor quote.
    pub frequency: SwapFrequency,
}

impl PiecewiseSigmaCalibrationConfig {
    /// Validate bootstrap settings.
    fn validate(self) -> finstack_quant_core::Result<()> {
        if !(KAPPA_MIN..=KAPPA_MAX).contains(&self.fixed_kappa) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "piecewise HW1F fixed_kappa={} outside [{KAPPA_MIN}, {KAPPA_MAX}]",
                self.fixed_kappa
            )));
        }
        if !self.sigma_min.is_finite()
            || !self.sigma_max.is_finite()
            || self.sigma_min <= 0.0
            || self.sigma_max <= self.sigma_min
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "invalid piecewise HW1F sigma bounds [{}, {}]",
                self.sigma_min, self.sigma_max
            )));
        }
        Ok(())
    }
}

/// Bootstrap one left-continuous HW1F volatility segment per cap/floor expiry.
///
/// Quotes must be normal-vol cap/floor prices at strictly increasing maturities.
/// Earlier segments remain frozen when solving a later pillar. Multiple quotes
/// at a common expiry are deliberately rejected here: a single segment cannot
/// exactly identify a smile without an explicit least-squares policy.
///
/// # Arguments
///
/// * `discount_df` - Discount-factor function where `discount_df(t)` returns
///   `P(0,t)` for time `t` in years on the cap/floor quote time axis.
/// * `forward_df` - Projection-curve discount-factor function where
///   `forward_df(t)` returns `P(0,t)` on the same time axis; its factor ratios
///   determine forward rates for the caplet decomposition.
/// * `quotes` - Normal-vol cap or floor quotes with distinct increasing
///   maturities. One left-continuous volatility segment is solved at each
///   quoted maturity.
/// * `config` - Fixed mean-reversion, sigma search bounds, and coupon
///   frequency used while sequentially solving the volatility schedule.
pub fn bootstrap_hull_white_sigma_schedule_to_cap_floors(
    discount_df: &dyn Fn(f64) -> f64,
    forward_df: &dyn Fn(f64) -> f64,
    quotes: &[CapFloorQuote],
    config: PiecewiseSigmaCalibrationConfig,
) -> finstack_quant_core::Result<(HullWhiteParams, CalibrationReport)> {
    config.validate()?;
    if quotes.is_empty() {
        return Err(finstack_quant_core::Error::Validation(
            "piecewise HW1F bootstrap requires at least one cap/floor quote".into(),
        ));
    }

    let mut ordered = quotes.to_vec();
    ordered.sort_by(|left, right| left.maturity.total_cmp(&right.maturity));
    for pair in ordered.windows(2) {
        if pair[1].maturity <= pair[0].maturity {
            return Err(finstack_quant_core::Error::Validation(format!(
                "piecewise HW1F bootstrap requires distinct increasing maturities; got {} then {}",
                pair[0].maturity, pair[1].maturity
            )));
        }
    }
    for (index, quote) in ordered.iter().enumerate() {
        validate_cap_floor_quote(
            quote.maturity,
            quote.strike,
            quote.volatility,
            quote.is_normal_vol,
        )
        .map_err(|error| {
            finstack_quant_core::Error::Validation(format!(
                "invalid piecewise cap/floor quote at index {index}: {error}"
            ))
        })?;
    }

    let mut times = vec![0.0];
    let mut sigmas = Vec::with_capacity(ordered.len());
    let mut residuals = BTreeMap::new();
    for (index, quote) in ordered.iter().enumerate() {
        let market_price = bachelier_cap_floor_price(
            discount_df,
            forward_df,
            quote.maturity,
            quote.strike,
            quote.volatility,
            quote.is_cap,
            config.frequency,
        );
        let spec =
            CapFloorPriceSpec::new(quote.maturity, quote.strike, quote.is_cap, config.frequency);
        let model_price = |candidate: f64| -> finstack_quant_core::Result<f64> {
            let mut candidate_sigmas = sigmas.clone();
            candidate_sigmas.push(candidate);
            let model = HullWhiteParams::new(
                config.fixed_kappa,
                PiecewiseConstantCurve::new(times.clone(), candidate_sigmas)?,
            )?;
            hw1f_cap_floor_price_with_model(&model, discount_df, forward_df, spec)
        };
        let residual = |candidate: f64| match model_price(candidate) {
            Ok(price) => price - market_price,
            Err(_) => f64::NAN,
        };
        let lower = residual(config.sigma_min);
        let upper = residual(config.sigma_max);
        if !lower.is_finite()
            || !upper.is_finite()
            || (lower.signum() == upper.signum() && lower != 0.0 && upper != 0.0)
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "piecewise HW1F bootstrap pillar {}Y is not bracketed on [{:.6e}, {:.6e}]: residuals {:.6e}, {:.6e}",
                quote.maturity, config.sigma_min, config.sigma_max, lower, upper
            )));
        }
        let solved = BrentSolver::new()
            .tolerance(1.0e-12)
            .max_iterations(200)
            .solve_in_bracket(residual, config.sigma_min, config.sigma_max)?;
        if solved >= config.sigma_max * (1.0 - 1.0e-6) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "piecewise HW1F bootstrap pillar {}Y hit sigma upper bound {}",
                quote.maturity, config.sigma_max
            )));
        }
        let residual_value = model_price(solved)? - market_price;
        residuals.insert(format!("{}Y", quote.maturity), residual_value);
        sigmas.push(solved);
        if index + 1 < ordered.len() {
            times.push(quote.maturity);
        }
    }
    let volatility = PiecewiseConstantCurve::new(times, sigmas)?;
    let model = HullWhiteParams::new(config.fixed_kappa, volatility)?;
    let report = CalibrationReport::for_type_with_tolerance(
        "hull_white_1f_cap_floor_piecewise",
        residuals,
        ordered.len(),
        HW_VALIDATION_TOLERANCE,
    )
    .with_metadata("fixed_kappa", config.fixed_kappa.to_string())
    .with_metadata("volatility_mode", "piecewise".to_string());
    Ok((model, report))
}

/// Golden-section minimisation of `f` on `[lo, hi]`. Returns the minimiser
/// `x` and `f(x)` after contracting the bracket below `x_tol`, capped at
/// `max_iters`. Returns `None` when the objective is non-finite at every
/// probe point in the bracket (the caller can then skip the bracket).
fn golden_section_min(
    f: &impl Fn(f64) -> f64,
    lo: f64,
    hi: f64,
    x_tol: f64,
    max_iters: usize,
) -> Option<(f64, f64)> {
    const INV_PHI: f64 = 0.618_033_988_749_894_8; // 1/φ
    let mut a = lo;
    let mut b = hi;
    let mut c = b - INV_PHI * (b - a);
    let mut d = a + INV_PHI * (b - a);
    let mut fc = f(c);
    let mut fd = f(d);
    for _ in 0..max_iters {
        if (b - a).abs() <= x_tol {
            break;
        }
        if fc <= fd {
            b = d;
            d = c;
            fd = fc;
            c = b - INV_PHI * (b - a);
            fc = f(c);
        } else {
            a = c;
            c = d;
            fc = fd;
            d = a + INV_PHI * (b - a);
            fd = f(d);
        }
    }
    let x = 0.5 * (a + b);
    let fx = f(x);
    if !x.is_finite() || x <= 0.0 || !fx.is_finite() {
        return None;
    }
    Some((x, fx))
}
