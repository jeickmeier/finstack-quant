use super::targets::{
    quote_fit_report, reject_at_bound_params, require_quote_vega, validate_fit_tolerance,
    HullWhiteSwaptionTarget, PreparedSwaption, HW_NUM_RESTARTS, HW_PERTURB_SCALE,
    SWAPTION_VEGA_FLOOR,
};
use super::*;
use finstack_quant_models::rates::hull_white::{hw_b, hw_bond_vol, hw_ln_a};

/// Calibrate Hull-White 1-factor parameters to European swaption market data.
///
/// Fits κ (mean reversion) and σ (short rate volatility) by minimising
/// squared differences between model and market swaption prices.
///
/// # Arguments
///
/// * `df` - Discount factor function: `df(t)` returns P(0, t). Must satisfy `df(0) ≈ 1`.
/// * `quotes` - Swaption market data.
/// * `frequency` - Coupon frequency of the underlying swap (e.g., semi-annual for USD,
///   annual for EUR). Used for the synthetic constant-period schedule when
///   `schedules` is `None`; ignored for annuity construction when contractual
///   schedules are supplied.
/// * `schedules` - Optional contractual fixed-leg schedules aligned with
///   `quotes`. `None` builds a synthetic constant-period schedule from
///   `frequency`. `Some` replaces that schedule with quote-aligned payment
///   times, accruals, and unlagged maturities (preserving calendars, stubs,
///   and payment lags).
/// * `initial_guess` - Optional seed for (κ, σ). Pass `None` to use built-in defaults.
/// * `fit_tolerance` - Required positive maximum absolute implied-quote error;
///   normal quotes use decimal rate volatility and Black quotes relative volatility.
///   Independent of solver tolerance; a failed fit returns `report.success = false`.
///
/// # Returns
///
/// Calibrated [`HullWhiteCalibrationParams`] and a [`CalibrationReport`] with residual diagnostics.
///
/// # Algorithm
///
/// 1. For each swaption quote, compute the market price from the quoted vol.
/// 2. Model prices are computed analytically via the Jamshidian (1989) decomposition.
/// 3. The Levenberg-Marquardt solver minimises the sum of squared price errors,
///    routed through `GlobalFitOptimizer` so HW1F shares the same numeric
///    plumbing (multi-start, diagnostics, error reporting) as curve calibration.
/// 4. Uses the unconstrained parameterisation: `(ln κ, ln σ)`.
///
/// # Residual scaling (ATM assumption)
///
/// The numerical optimizer uses `(price_model - price_market) / market_vega`
/// for the ATM quotes supported by this API. Final prices are inverted using
/// each quote's Normal or Black convention; the report and `fit_tolerance`
/// use those actual implied-volatility errors, not the optimizer's linear
/// approximation. Quotes with vanishing market vega are rejected.
///
/// # Post-calibration sanity
///
/// HW1F is arbitrage-free by construction, so a calibrated `(κ, σ)` cannot
/// introduce butterfly or calendar arbitrage into the model-implied swaption
/// surface; no arbitrage checks on the model output are required. What can
/// still fail numerically is the Jamshidian decomposition itself (degenerate
/// `r*` solve, pathological discount inputs), so every calibration quote is
/// repriced at the final parameters and any non-finite or negative model
/// price fails the calibration loudly. Fit quality is covered by the
/// per-quote residuals in the [`CalibrationReport`].
///
/// # Errors
///
/// Returns an error if:
/// - Fewer than 2 quotes are provided (2 free parameters)
/// - Calibration fails to converge
/// - Discount function returns invalid values
/// - A supplied schedule is malformed or its length does not match `quotes`
///
/// HW1F swaption calibration treats every leg as a vanilla fixed-vs-IBOR
/// swap. For OIS swaptions the daily compounding inside each accrual period
/// is approximated by a single forward rate.
pub fn calibrate_hull_white_to_swaptions(
    df: &(dyn Fn(f64) -> f64 + Sync),
    quotes: &[SwaptionQuote],
    frequency: SwapFrequency,
    schedules: Option<&[SwaptionSchedule]>,
    initial_guess: Option<HullWhiteCalibrationParams>,
    fit_tolerance: f64,
) -> finstack_quant_core::Result<(HullWhiteCalibrationParams, CalibrationReport)> {
    validate_fit_tolerance(fit_tolerance)?;
    let schedule_source = schedules.is_some().then_some("real_day_count");
    if quotes.len() < 2 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Need at least 2 swaption quotes for HW1F calibration (2 free parameters), got {}",
            quotes.len()
        )));
    }
    if let Some(schedules) = schedules {
        if schedules.len() != quotes.len() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "schedules.len() ({}) must match quotes.len() ({})",
                schedules.len(),
                quotes.len()
            )));
        }
    }
    for (i, q) in quotes.iter().enumerate() {
        if q.expiry <= 0.0 || q.tenor <= 0.0 || q.volatility <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Invalid swaption quote at index {i}: expiry={}, tenor={}, vol={}",
                q.expiry, q.tenor, q.volatility
            )));
        }
    }

    let n_quotes = quotes.len();
    let ppy = frequency.periods_per_year();

    // Pre-compute market data once; the LM hot loop only does numeric ops.
    let mut prepared = Vec::with_capacity(n_quotes);
    let mut fwd_swap_rates = Vec::with_capacity(n_quotes);
    for (idx, q) in quotes.iter().enumerate() {
        // Validate the per-quote schedule up front. Contractual schedules may
        // contain stubs, so their period count is not inferred from tenor.
        let schedule = schedules.and_then(|items| valid_swap_schedule(Some(&items[idx]), q.expiry));
        if schedules.is_some() && schedule.is_none() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "HW1F swaption calibration: contractual schedule for {}Yx{}Y is malformed; \
                 payment times must be strictly increasing after expiry, accruals must be \
                 positive and quote-aligned, and maturity must lie after expiry",
                q.expiry, q.tenor
            )));
        }
        let (annuity, fwd_rate) =
            compute_swap_annuity_and_rate_inner(df, q.expiry, q.tenor, ppy, schedule);
        let market_price = compute_swaption_market_price(
            annuity,
            fwd_rate,
            q.expiry,
            q.volatility,
            q.is_normal_vol,
        );
        let raw_vega =
            swaption_atm_vega(annuity, fwd_rate, q.expiry, q.volatility, q.is_normal_vol);
        let label = format!("{}Yx{}Y", q.expiry, q.tenor);
        let vega = require_quote_vega(raw_vega, SWAPTION_VEGA_FLOOR, &label)?;
        prepared.push(PreparedSwaption {
            market_price,
            fwd_swap_rate: fwd_rate,
            vega,
            schedule: schedule.cloned(),
        });
        fwd_swap_rates.push(fwd_rate);
    }

    let (default_kappa_init, default_sigma_init) = infer_hw_initial_guess(quotes, &fwd_swap_rates);
    let kappa_init: f64 = initial_guess.map(|p| p.kappa).unwrap_or(default_kappa_init);
    let sigma_init: f64 = initial_guess.map(|p| p.sigma).unwrap_or(default_sigma_init);
    let x0 = [kappa_init.ln(), sigma_init.ln()];

    let target = HullWhiteSwaptionTarget {
        df,
        ppy,
        initial_x0: x0,
        prepared,
    };

    // Use solver tolerance 1e-12 (matches the prior hand-rolled LM
    // settings) and validation tolerance 1e-6 (the historical
    // accept/reject threshold for HW1F price residuals).
    let mut config = CalibrationConfig::default();
    config.solver = config.solver.with_tolerance(1e-12).with_max_iterations(300);

    let multi_start = MultiStartConfig {
        num_restarts: HW_NUM_RESTARTS,
        perturbation_scale: HW_PERTURB_SCALE,
    };

    let (params, mut report) = GlobalFitOptimizer::optimize_with_multi_start(
        &target,
        quotes,
        &config,
        fit_tolerance,
        Some(&multi_start),
    )?;

    let mut quote_residuals = BTreeMap::new();
    for (idx, (quote, pre)) in quotes.iter().zip(&target.prepared).enumerate() {
        let (annuity, forward) = compute_swap_annuity_and_rate_inner(
            df,
            quote.expiry,
            quote.tenor,
            ppy,
            pre.schedule.as_ref(),
        );
        let price = hw1f_swaption_price_inner(Hw1fSwaptionPriceInput {
            kappa: params.kappa,
            sigma: params.sigma,
            df,
            t0: quote.expiry,
            tenor: quote.tenor,
            swap_rate: forward,
            periods_per_year: ppy,
            schedule: pre.schedule.as_ref(),
        });
        let implied = if quote.is_normal_vol {
            price / annuity * (2.0 * std::f64::consts::PI / quote.expiry).sqrt()
        } else {
            let probability = 0.5 * (1.0 + price / (annuity * forward));
            2.0 * finstack_quant_core::math::special_functions::standard_normal_inv_cdf(probability)
                / quote.expiry.sqrt()
        };
        if !implied.is_finite() || implied < 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Hull-White quote {idx} has no finite implied volatility"
            )));
        }
        quote_residuals.insert(
            format!("{idx}:{}Yx{}Y", quote.expiry, quote.tenor),
            implied - quote.volatility,
        );
    }
    report = quote_fit_report(report, quote_residuals, fit_tolerance);

    // Override the report type tag (stored in metadata["type"]) and add
    // HW-specific metadata. The framework reports a generic "global_fit"
    // type; HW consumers expect "hull_white_1f" for serialization stability.
    report = report
        .with_model_version(crate::versions::HULL_WHITE_1F)
        .with_metadata("type", "hull_white_1f".to_string())
        .with_metadata("kappa", format!("{:.6}", params.kappa))
        .with_metadata("sigma", format!("{:.6}", params.sigma))
        .with_metadata("initial_kappa", format!("{kappa_init:.6}"))
        .with_metadata("initial_sigma", format!("{sigma_init:.6}"))
        .with_metadata("multi_start_restarts", HW_NUM_RESTARTS.to_string())
        .with_metadata(
            "optimizer_residual_weighting",
            "1/vega (vega-weighted price residual)".to_string(),
        )
        .with_metadata(
            "swap_frequency",
            if schedules.is_some() {
                "quote_specific".to_string()
            } else {
                frequency.to_string()
            },
        );
    if let Some(schedule_source) = schedule_source {
        report = report.with_metadata("schedule_source", schedule_source.to_string());
    }
    reject_at_bound_params(
        params.kappa,
        params.sigma,
        "Hull-White swaption calibration",
    )?;

    validate_model_price_sanity(df, quotes, &target.prepared, ppy, &params)?;

    // Final validation of (κ, σ) > 0 through the calibration parameter gate.
    let params = HullWhiteCalibrationParams::new(params.kappa, params.sigma)?;
    Ok((params, report))
}

/// Post-calibration sanity gate: reprice every calibration quote at the
/// final `(κ, σ)` and reject non-finite or negative model prices.
///
/// HW1F is arbitrage-free by construction, so butterfly/calendar arbitrage
/// checks on the model-implied surface are unnecessary; the failure modes
/// this guards against are numerical — a degenerate Jamshidian `r*` solve or
/// pathological discount inputs producing a price a swaption cannot have.
/// Fit quality is judged from the final implied-quote report residuals.
fn validate_model_price_sanity(
    df: &(dyn Fn(f64) -> f64 + Sync),
    quotes: &[SwaptionQuote],
    prepared: &[PreparedSwaption],
    ppy: usize,
    params: &HullWhiteCalibrationParams,
) -> finstack_quant_core::Result<()> {
    for (q, pre) in quotes.iter().zip(prepared) {
        let model_price = hw1f_swaption_price_inner(Hw1fSwaptionPriceInput {
            kappa: params.kappa,
            sigma: params.sigma,
            df,
            t0: q.expiry,
            tenor: q.tenor,
            swap_rate: pre.fwd_swap_rate,
            periods_per_year: ppy,
            schedule: pre.schedule.as_ref(),
        });
        if !model_price.is_finite() || model_price < 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Hull-White swaption calibration: calibrated (κ={:.6e}, σ={:.6e}) reprices \
                 quote {}Yx{}Y to an invalid model price ({model_price:?}); a swaption \
                 price must be finite and non-negative. The optimizer terminated on a \
                 numerically degenerate solution — review the discount inputs and quote set.",
                params.kappa, params.sigma, q.expiry, q.tenor
            )));
        }
    }
    Ok(())
}

/// ATM vega for a swaption expressed in the same volatility units as the
/// quote (Bachelier σ for normal vol, Black-76 σ for lognormal).
///
/// Used as the per-quote weight in the vega-weighted price residual; see
/// the module-level note in `calibrate_hull_white_to_swaptions`.
fn swaption_atm_vega(annuity: f64, fwd_rate: f64, expiry: f64, vol: f64, is_normal: bool) -> f64 {
    if is_normal {
        annuity
            * finstack_quant_models::closed_form::bachelier_vega(fwd_rate, fwd_rate, vol, expiry)
    } else {
        annuity * finstack_quant_models::closed_form::black_vega(fwd_rate, fwd_rate, vol, expiry)
    }
}

/// Compute annuity and forward swap rate for a swap starting at `t0`
/// with given `tenor` and `periods_per_year` coupon payments.
///
/// The schedule is synthetic (constant `dt = tenor/n_periods`). For real
/// market day-counts (Act/360 USD SOFR, 30/360 EUR EURIBOR, etc.), use
/// `compute_swap_annuity_and_rate_inner` with an explicit
/// [`SwaptionSchedule`].
///
/// Test-only: production calibration always calls
/// [`compute_swap_annuity_and_rate_inner`] directly with the resolved
/// `Option<SwaptionSchedule>`.
#[cfg(test)]
pub(crate) fn compute_swap_annuity_and_rate(
    df: &(dyn Fn(f64) -> f64 + Sync),
    t0: f64,
    tenor: f64,
    periods_per_year: usize,
) -> (f64, f64) {
    compute_swap_annuity_and_rate_inner(df, t0, tenor, periods_per_year, None)
}

pub(super) fn compute_swap_annuity_and_rate_inner(
    df: &(dyn Fn(f64) -> f64 + Sync),
    t0: f64,
    tenor: f64,
    periods_per_year: usize,
    schedule: Option<&SwaptionSchedule>,
) -> (f64, f64) {
    if let Some(schedule) = valid_swap_schedule(schedule, t0) {
        let annuity = schedule
            .payment_times
            .iter()
            .zip(&schedule.accruals)
            .map(|(payment_time, accrual)| accrual * df(*payment_time))
            .sum();
        let fwd_rate = if annuity > 1e-15 {
            (df(schedule.swap_start_time) - df(schedule.maturity_time)) / annuity
        } else {
            0.0
        };
        return (annuity, fwd_rate);
    }

    let n_periods = (tenor * periods_per_year as f64).round().max(1.0) as usize;
    let dt = tenor / n_periods as f64;
    let annuity = (1..=n_periods)
        .map(|index| dt * df(t0 + index as f64 * dt))
        .sum();
    let maturity_time = t0 + tenor;

    let fwd_rate = if annuity > 1e-15 {
        (df(t0) - df(maturity_time)) / annuity
    } else {
        let p0 = df(t0).max(1e-12);
        let p_n = df(maturity_time).max(1e-12);
        ((p0 / p_n).ln() / tenor.max(1e-8)).max(0.0)
    };

    (annuity, fwd_rate)
}

#[inline]
pub(super) fn valid_swap_schedule(
    schedule: Option<&SwaptionSchedule>,
    expiry: f64,
) -> Option<&SwaptionSchedule> {
    schedule.filter(|schedule| {
        !schedule.payment_times.is_empty()
            && schedule.payment_times.len() == schedule.accruals.len()
            && schedule.swap_start_time.is_finite()
            && schedule.maturity_time.is_finite()
            && schedule.swap_start_time >= expiry
            && schedule.maturity_time > schedule.swap_start_time
            && schedule
                .accruals
                .iter()
                .all(|accrual| accrual.is_finite() && *accrual > 0.0)
            && schedule
                .payment_times
                .iter()
                .all(|time| time.is_finite() && *time > schedule.swap_start_time)
            && schedule
                .payment_times
                .last()
                .is_some_and(|time| *time >= schedule.maturity_time)
            && schedule
                .payment_times
                .windows(2)
                .all(|window| window[1] > window[0])
    })
}

pub(super) fn infer_hw_initial_guess(
    quotes: &[SwaptionQuote],
    fwd_swap_rates: &[f64],
) -> (f64, f64) {
    let horizon = if quotes.is_empty() {
        5.0
    } else {
        quotes.iter().map(|q| q.expiry + 0.5 * q.tenor).sum::<f64>() / quotes.len() as f64
    };
    // Average ABSOLUTE-rate vol, branched per quote on the vol regime so
    // the σ seed never conflates Bachelier and Black quotes (W-39):
    //  - normal (Bachelier) quote: the vol is already an absolute-rate
    //    vol, so it contributes directly;
    //  - lognormal (Black) quote: the vol is dimensionless, so `vol·fwd`
    //    recovers an absolute-rate scale.
    // The HW1F σ is an absolute short-rate vol, so this average is the
    // right order of magnitude for the seed.
    let avg_abs_vol = if quotes.is_empty() {
        0.01 * 0.02 // fallback: ~1% Black vol at a 2% forward.
    } else {
        let sum: f64 = quotes
            .iter()
            .enumerate()
            .map(|(i, q)| {
                let v = q.volatility.abs();
                if q.is_normal_vol {
                    v
                } else {
                    // fwd_swap_rates is built quote-aligned by the callers;
                    // fall back to a 2% forward if the slice is short.
                    let fwd = fwd_swap_rates.get(i).map_or(0.02, |r| r.abs()).max(0.005);
                    v * fwd
                }
            })
            .sum();
        sum / quotes.len() as f64
    };

    let kappa_init = (1.0 / horizon.max(0.5)).clamp(0.01, 0.30);
    let sigma_init = avg_abs_vol.clamp(0.001, 0.05);
    (kappa_init, sigma_init)
}

/// Compute the market swaption price from the quoted volatility.
pub(super) fn compute_swaption_market_price(
    annuity: f64,
    fwd_rate: f64,
    expiry: f64,
    vol: f64,
    is_normal: bool,
) -> f64 {
    if is_normal {
        // Bachelier: ATM payer price ≈ annuity × σ_n × √T × √(2/π) ≈ annuity × bachelier_call
        annuity
            * finstack_quant_models::closed_form::bachelier_call(fwd_rate, fwd_rate, vol, expiry)
    } else {
        // Black-76: annuity × black_call(F, F, σ, T)
        annuity * finstack_quant_models::closed_form::black_call(fwd_rate, fwd_rate, vol, expiry)
    }
}

/// Price a European payer swaption under HW1F using Jamshidian decomposition.
///
/// The Jamshidian decomposition expresses a swaption as a portfolio of
/// zero-coupon bond options. The key steps are:
///
/// 1. Find the critical short rate r* where the swap value equals par.
/// 2. Each leg becomes a put on a zero-coupon bond with strike K_i = P_HW(r*, T₀, T_i).
/// 3. Sum the individual zero-coupon bond put prices.
///
/// Uses a synthetic constant-`dt` schedule. The production HW1F calibrator
/// (`calibrate_hull_white_to_swaptions` with contractual schedules) drives
/// [`hw1f_swaption_price_inner`] directly with real accrual fractions, so
/// this scalar-time wrapper exists only as a stable test harness.
#[cfg(test)]
pub(crate) fn hw1f_swaption_price(
    kappa: f64,
    sigma: f64,
    df: &(dyn Fn(f64) -> f64 + Sync),
    t0: f64,
    tenor: f64,
    swap_rate: f64,
    periods_per_year: usize,
) -> f64 {
    hw1f_swaption_price_inner(Hw1fSwaptionPriceInput {
        kappa,
        sigma,
        df,
        t0,
        tenor,
        swap_rate,
        periods_per_year,
        schedule: None,
    })
}

pub(super) struct Hw1fSwaptionPriceInput<'a> {
    pub(super) kappa: f64,
    pub(super) sigma: f64,
    pub(super) df: &'a (dyn Fn(f64) -> f64 + Sync),
    pub(super) t0: f64,
    pub(super) tenor: f64,
    pub(super) swap_rate: f64,
    pub(super) periods_per_year: usize,
    pub(super) schedule: Option<&'a SwaptionSchedule>,
}

fn build_swaption_cashflows(
    swap_rate: f64,
    t0: f64,
    tenor: f64,
    periods_per_year: usize,
    schedule: Option<&SwaptionSchedule>,
) -> Vec<(f64, f64)> {
    let n_periods = schedule.map_or_else(
        || (tenor * periods_per_year as f64).round().max(1.0) as usize,
        |schedule| schedule.accruals.len(),
    );
    let mut cashflows = Vec::with_capacity(n_periods + 1);
    if let Some(schedule) = schedule {
        cashflows.extend(
            schedule
                .payment_times
                .iter()
                .zip(&schedule.accruals)
                .map(|(&payment_time, &accrual)| (payment_time, swap_rate * accrual)),
        );
        cashflows.push((schedule.maturity_time, 1.0));
    } else {
        let dt = tenor / n_periods as f64;
        let maturity_time = t0 + tenor;
        for index in 1..=n_periods {
            let payment_time = if index == n_periods {
                maturity_time
            } else {
                t0 + index as f64 * dt
            };
            cashflows.push((payment_time, swap_rate * dt));
        }
        cashflows.push((maturity_time, 1.0));
    }

    cashflows.sort_by(|left, right| left.0.total_cmp(&right.0));
    let mut aggregated: Vec<(f64, f64)> = Vec::with_capacity(cashflows.len());
    for (time, amount) in cashflows {
        if let Some(last) = aggregated.last_mut() {
            if last.0.to_bits() == time.to_bits() {
                last.1 += amount;
                continue;
            }
        }
        aggregated.push((time, amount));
    }
    aggregated
}

pub(super) fn hw1f_swaption_price_inner(
    Hw1fSwaptionPriceInput {
        kappa,
        sigma,
        df,
        t0,
        tenor,
        swap_rate,
        periods_per_year,
        schedule,
    }: Hw1fSwaptionPriceInput<'_>,
) -> f64 {
    let schedule = valid_swap_schedule(schedule, t0);
    let swap_start_time = schedule.map_or(t0, |schedule| schedule.swap_start_time);
    let cashflow_entries =
        build_swaption_cashflows(swap_rate, t0, tenor, periods_per_year, schedule);
    let n_cashflows = cashflow_entries.len();
    let (payment_times, cashflows): (Vec<_>, Vec<_>) = cashflow_entries.into_iter().unzip();

    // Pre-compute B and ln A for each payment date
    let b_vals: Vec<f64> = payment_times
        .iter()
        .map(|&t_i| hw_b(kappa, t0, t_i))
        .collect();
    let ln_a_vals: Vec<f64> = payment_times
        .iter()
        .map(|&t_i| hw_ln_a(kappa, sigma, t0, t_i, df))
        .collect();
    let b_start = hw_b(kappa, t0, swap_start_time);
    let ln_a_start = hw_ln_a(kappa, sigma, t0, swap_start_time, df);

    // Find r* such that the fixed-bond value equals the forward-start bond:
    // Σ c_i P(T₀,T_i;r*) / P(T₀,T_start;r*) = 1.
    let g = |r: f64| -> f64 {
        let mut sum = 0.0;
        for i in 0..n_cashflows {
            let log_ratio = ln_a_vals[i] - ln_a_start - (b_vals[i] - b_start) * r;
            sum += cashflows[i] * log_ratio.exp();
        }
        sum - 1.0
    };

    let g_prime = |r: f64| -> f64 {
        let mut sum = 0.0;
        for i in 0..n_cashflows {
            let b_ratio = b_vals[i] - b_start;
            let log_ratio = ln_a_vals[i] - ln_a_start - b_ratio * r;
            sum -= cashflows[i] * b_ratio * log_ratio.exp();
        }
        sum
    };

    // Natural magnitude scale of `g'(r)` at a given `r`: the sum of the *absolute
    // values* of the per-cashflow terms that make up `g'`. `g'` itself is a signed sum
    // and can suffer catastrophic cancellation; comparing `|g'|` against this scale
    // (rather than a fixed absolute floor) detects a numerically near-flat objective.
    let g_prime_scale = |r: f64| -> f64 {
        let mut sum = 0.0;
        for i in 0..n_cashflows {
            let b_ratio = b_vals[i] - b_start;
            let log_ratio = ln_a_vals[i] - ln_a_start - b_ratio * r;
            sum += (cashflows[i] * b_ratio * log_ratio.exp()).abs();
        }
        sum
    };

    // Initial guess: the instantaneous forward rate at t0
    let h = (t0 * 1e-3).clamp(1e-6, 1e-3);
    let f0t0 = if t0 > h {
        -(df(t0 + h).ln() - df(t0 - h).ln()) / (2.0 * h)
    } else {
        -(df(h).ln()) / h
    };

    // Newton iterations to find r*.
    //
    // Derivative guard: a fixed `|g'| < 1e-15` *absolute* floor is the wrong
    // criterion. A `g'` of ~1e-10 — a near-flat objective — sails straight past it, and
    // `step = g / g'` then explodes to a ~1e8-scale jump that throws the iterate far
    // outside any plausible short-rate range. Two scale-aware guards replace it:
    //
    //  1. A *relative* derivative-magnitude guard: `|g'|` must be a non-trivial fraction
    //     of its own term-wise magnitude scale `Σ|c_i B_i e^…|`. This catches the
    //     catastrophic-cancellation regime where the signed sum `g'` collapses toward
    //     zero while its constituent terms are not.
    //  2. A safeguarded step bound: even a "large enough" `g'` can yield an absurd step
    //     when the objective is flat. A Newton step that would move `r` by more than
    //     `NEWTON_MAX_STEP` is untrustworthy; we hand off to the bracketed Brent
    //     fallback instead of accepting the jump.
    let mut r_star = f0t0;
    let mut newton_converged = false;
    const NEWTON_DERIV_REL_EPS: f64 = 1e-10;
    // Cap on a single Newton step in absolute short-rate units. A short rate moving by
    // more than 5.0 (500%) in one step is non-physical; the Brent fallback bracket is
    // sized to cover the plausible range under HW1F dynamics.
    const NEWTON_MAX_STEP: f64 = 5.0;
    for _ in 0..50 {
        let gv = g(r_star);
        let gp = g_prime(r_star);
        let gp_scale = g_prime_scale(r_star);
        // Near-flat / fully-cancelled derivative: hand off to Brent rather than take an
        // unbounded Newton step.
        if !gp.is_finite() || gp.abs() <= NEWTON_DERIV_REL_EPS * gp_scale.max(f64::MIN_POSITIVE) {
            break;
        }
        let step = gv / gp;
        // A non-finite or absurdly large step means the local linearisation is
        // unreliable (near-flat objective); stop and let Brent bracket the root.
        if !step.is_finite() || step.abs() > NEWTON_MAX_STEP {
            break;
        }
        r_star -= step;
        if step.abs() < 1e-12 {
            newton_converged = true;
            break;
        }
    }
    // Newton may have walked the iterate to a non-finite value before the step-size
    // convergence test fired; treat that as non-convergence so the Brent fallback runs.
    if !r_star.is_finite() {
        newton_converged = false;
    }

    // Brent fallback if Newton didn't converge.
    //
    // Bracket width must scale with both rate level and HW1F vol-to-expiry to
    // stay valid under negative-rate (EUR) and distressed-sovereign regimes.
    // The previous fixed `±0.20` bracket was too narrow for f0 ≈ 15% sovereign
    // yields and too narrow at long expiries where σ√t0 dominates.
    //
    // Heuristic: half-width = max(0.5, 5·σ√t0) — covers ±5σ of the short-rate
    // distribution under HW1F (more than enough to bracket r*) plus a 50%
    // (5,000bp) floor for short-expiry, low-vol cases.
    if !newton_converged {
        tracing::warn!(
            "HW1F r* Newton solver did not converge (kappa={kappa:.4}, sigma={sigma:.4}), \
             falling back to Brent"
        );
        let half_width = (5.0 * sigma * t0.sqrt()).max(0.5);
        let bracket_lo = f0t0 - half_width;
        let bracket_hi = f0t0 + half_width;
        let brent = BrentSolver::new()
            .tolerance(1e-12)
            .bracket_bounds(bracket_lo, bracket_hi);
        match brent.solve(g, f0t0) {
            Ok(r) => r_star = r,
            Err(_) => {
                tracing::warn!("HW1F r* Brent fallback also failed; returning NaN");
                r_star = f64::NAN;
            }
        }
    }

    // r* solver failure (NaN) and pathological discount factors must propagate
    // as NaN to the caller — `.max(0.0)` would silently turn NaN into 0.0
    // because IEEE 754 `max(NaN, 0.0) == 0.0`, fooling the LM closure into
    // treating the input as a legitimate zero-price swaption.
    if !r_star.is_finite() {
        return f64::NAN;
    }

    // Compute strike ratios K_i = P(T₀,T_i;r*) / P(T₀,T_start;r*).
    let k_strikes: Vec<f64> = (0..n_cashflows)
        .map(|i| (ln_a_vals[i] - ln_a_start - (b_vals[i] - b_start) * r_star).exp())
        .collect();

    // Sum zero-coupon bond put prices (payer swaption = portfolio of bond puts)
    // ZBO_put(0, T₀, T_i, K_i) = K_i P(0,T₀) N(−d₂) − P(0,T_i) N(−d₁)
    let p0_start = df(swap_start_time);
    if !(p0_start > 0.0 && p0_start.is_finite()) {
        return f64::NAN;
    }
    let mut swaption_price = 0.0;
    let start_bond_vol = hw_bond_vol(kappa, sigma, 0.0, t0, swap_start_time);

    for i in 0..n_cashflows {
        let t_i = payment_times[i];
        let p0_ti = df(t_i);
        if !(p0_ti > 0.0 && p0_ti.is_finite()) {
            return f64::NAN;
        }
        let sigma_p = (hw_bond_vol(kappa, sigma, 0.0, t0, t_i) - start_bond_vol).abs();

        if sigma_p < 1e-15 {
            // Degenerate: intrinsic value. `< 0.0` is false for NaN so NaN
            // would propagate, but inputs are positive-finite by the checks
            // above, so the subtraction is safe.
            let put_intrinsic_raw = k_strikes[i] * p0_start - p0_ti;
            let put_intrinsic = if put_intrinsic_raw < 0.0 {
                0.0
            } else {
                put_intrinsic_raw
            };
            swaption_price += cashflows[i] * put_intrinsic;
            continue;
        }

        let d1 = ((p0_ti / (k_strikes[i] * p0_start)).ln() + 0.5 * sigma_p * sigma_p) / sigma_p;
        let d2 = d1 - sigma_p;

        let put_price = k_strikes[i] * p0_start * norm_cdf(-d2) - p0_ti * norm_cdf(-d1);
        // Preserve NaN: `put_price < 0.0` is false for NaN, so NaN flows
        // through; only genuinely-negative numerical noise gets clamped.
        let put_price_clamped = if put_price < 0.0 { 0.0 } else { put_price };
        swaption_price += cashflows[i] * put_price_clamped;
    }

    if swaption_price < 0.0 {
        0.0
    } else {
        swaption_price
    }
}

#[cfg(test)]
mod timing_tests {
    use super::*;

    fn lagged_schedule() -> SwaptionSchedule {
        SwaptionSchedule {
            swap_start_time: 1.01,
            payment_times: vec![1.51, 2.01, 2.11],
            accruals: vec![0.5, 0.5, 0.1],
            maturity_time: 2.01,
        }
    }

    #[test]
    fn swaption_schedule_forward_uses_contractual_start() {
        let df = |time: f64| (-0.03 * time).exp();
        let schedule = lagged_schedule();

        let (annuity, forward) =
            compute_swap_annuity_and_rate_inner(&df, 1.0, 1.01, 2, Some(&schedule));
        let expected_annuity = schedule
            .payment_times
            .iter()
            .zip(&schedule.accruals)
            .map(|(&payment_time, &accrual)| accrual * df(payment_time))
            .sum::<f64>();
        let expected_forward =
            (df(schedule.swap_start_time) - df(schedule.maturity_time)) / expected_annuity;

        assert!((annuity - expected_annuity).abs() < 1.0e-15);
        assert!((forward - expected_forward).abs() < 1.0e-15);
    }

    #[test]
    fn jamshidian_cashflows_keep_redemption_at_contractual_maturity() {
        let schedule = lagged_schedule();
        let cashflows = build_swaption_cashflows(0.04, 1.0, 1.01, 2, Some(&schedule));

        assert_eq!(cashflows.len(), 3);
        assert_eq!(cashflows[0], (1.51, 0.02));
        assert_eq!(cashflows[1], (2.01, 1.02));
        assert_eq!(cashflows[2], (2.11, 0.004));
    }

    #[test]
    fn explicit_zero_lag_schedule_reduces_to_synthetic_price() {
        let df = |time: f64| (-0.03 * time).exp();
        let schedule = SwaptionSchedule {
            swap_start_time: 1.0,
            payment_times: vec![1.5, 2.0],
            accruals: vec![0.5, 0.5],
            maturity_time: 2.0,
        };
        let (_, forward) = compute_swap_annuity_and_rate(&df, 1.0, 1.0, 2);
        let synthetic = hw1f_swaption_price(0.05, 0.01, &df, 1.0, 1.0, forward, 2);
        let explicit = hw1f_swaption_price_inner(Hw1fSwaptionPriceInput {
            kappa: 0.05,
            sigma: 0.01,
            df: &df,
            t0: 1.0,
            tenor: 1.0,
            swap_rate: forward,
            periods_per_year: 2,
            schedule: Some(&schedule),
        });

        assert!((explicit - synthetic).abs() < 1.0e-14);
    }

    #[test]
    fn swaption_schedule_rejects_malformed_time_roles() {
        let mut schedule = lagged_schedule();
        assert!(valid_swap_schedule(Some(&schedule), 1.0).is_some());

        schedule.swap_start_time = 0.99;
        assert!(valid_swap_schedule(Some(&schedule), 1.0).is_none());

        schedule = lagged_schedule();
        schedule.swap_start_time = schedule.maturity_time;
        assert!(valid_swap_schedule(Some(&schedule), 1.0).is_none());

        schedule = lagged_schedule();
        schedule.payment_times.swap(0, 1);
        assert!(valid_swap_schedule(Some(&schedule), 1.0).is_none());

        schedule = lagged_schedule();
        schedule.payment_times = vec![1.2, 1.5, 1.9];
        assert!(valid_swap_schedule(Some(&schedule), 1.0).is_none());
    }
}
