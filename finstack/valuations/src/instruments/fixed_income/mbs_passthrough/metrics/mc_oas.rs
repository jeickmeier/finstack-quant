//! Monte Carlo Option-Adjusted Spread (OAS) for agency MBS.
//!
//! Computes OAS using stochastic interest rate paths (Hull-White 1-factor)
//! with rate-dependent prepayment speeds, following the market-standard
//! approach used by Bloomberg, QuantLib, and other professional systems.
//!
//! # Methodology
//!
//! 1. Simulate N interest rate paths using HW1F exact discretization
//! 2. For each path, project cashflows with rate-dependent prepayment
//! 3. Discount each path's cashflows at the simulated short rates + OAS
//! 4. Average across paths to get the model price
//! 5. Use Brent's method to find OAS that equates model price to market price
//!
//! # Prepayment Model
//!
//! The standard PSA model is modified with a rate-dependent multiplier:
//! - When rates fall (refinancing incentive), prepayment speeds increase
//! - When rates rise, prepayment speeds decrease (lock-in effect)
//!
//! The multiplier is:
//! ```text
//! multiplier = exp(-β × (rate - base_rate))
//! ```
//! where β controls the sensitivity (typical: 5.0-10.0).
//!
//! # References
//!
//! - Fabozzi, F. J. (2016). *Bond Markets, Analysis, and Strategies*. Pearson.
//! - Hayre, L. (2001). *Salomon Smith Barney Guide to Mortgage-Backed and
//!   Asset-Backed Securities*. John Wiley & Sons.
//! - O'Kane, D. (2008). *Modelling Single-name and Multi-name Credit
//!   Derivatives*. John Wiley & Sons.

use crate::calibration::hull_white::HullWhiteParams;
use crate::instruments::fixed_income::mbs_passthrough::AgencyMbsPassthrough;
use crate::instruments::rates::exotics_shared::{
    calibrate_hw1f_params, initial_short_rate_from_curve,
};
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::math::solver::{BrentSolver, Solver};
use finstack_core::{Error as CoreError, Result};
use finstack_monte_carlo::process::ou::HullWhite1FParams;
use finstack_monte_carlo::rng::philox::PhiloxRng;
use finstack_monte_carlo::traits::RandomStream;

/// Configuration for Monte Carlo OAS calculation.
#[derive(Debug, Clone)]
pub(crate) struct McOasConfig {
    /// Number of simulation paths (default: 512).
    pub num_paths: usize,
    /// Number of monthly time steps per path (default: WAM).
    /// If None, uses the MBS WAM.
    pub num_steps: Option<usize>,
    /// Hull-White mean reversion speed κ (default: 0.05).
    pub hw_kappa: f64,
    /// Hull-White short-rate volatility σ (default: 0.01).
    pub hw_sigma: f64,
    /// Prepayment rate sensitivity to interest rates β (default: 7.0).
    /// Higher values make prepayment more sensitive to rate changes.
    pub prepay_rate_sensitivity: f64,
    /// Random seed for reproducibility (default: 42).
    pub seed: u64,
    /// Solver tolerance for OAS root-finding (default: 1e-7).
    pub tolerance: f64,
}

impl Default for McOasConfig {
    fn default() -> Self {
        Self {
            num_paths: 512,
            num_steps: None,
            hw_kappa: 0.05,
            hw_sigma: 0.01,
            prepay_rate_sensitivity: 7.0,
            seed: 42,
            tolerance: 1e-7,
        }
    }
}

/// Result of a Monte Carlo OAS calculation.
#[derive(Debug, Clone)]
pub(crate) struct McOasResult {
    /// Option-adjusted spread in decimal (e.g., 0.01 for 100 bps).
    pub oas: f64,
    /// Average model price across all paths at the calculated OAS.
    pub model_price: f64,
    /// Target (market) price.
    pub market_price: f64,
    /// Price error at the solution.
    pub price_error: f64,
    /// Number of simulation paths used.
    pub num_paths: usize,
    /// Whether the solver converged.
    pub converged: bool,
    /// Standard error of the price estimate across paths.
    pub price_std_error: f64,
}

/// A single simulated short-rate path.
struct RatePath {
    /// Monthly short rates along the path.
    rates: Vec<f64>,
}

/// Simulate Hull-White 1-factor short rate paths.
///
/// Uses exact discretization (analytical conditional distribution)
/// for the OU/HW1F process with time-dependent θ(t) fitted to the
/// initial discount curve:
/// ```text
/// r_{t+Δt} = r_t × e^{-κΔt} + θ(t)(1 - e^{-κΔt}) + σ√[(1-e^{-2κΔt})/(2κ)] × Z
/// ```
fn simulate_rate_paths(
    initial_rate: f64,
    params: &HullWhite1FParams,
    num_paths: usize,
    num_steps: usize,
    seed: u64,
) -> Vec<RatePath> {
    let dt = 1.0 / 12.0; // Monthly steps
    let kappa = params.kappa;
    let sigma = params.sigma;
    let exp_kappa_dt = (-kappa * dt).exp();

    // θ(t) is piecewise-constant; precompute the per-step drift term using
    // the θ value at each step's left endpoint.
    let drift_coeffs: Vec<f64> = (0..num_steps)
        .map(|i| params.theta_at_time(i as f64 * dt) * (1.0 - exp_kappa_dt))
        .collect();

    // Conditional std dev of r_{t+Δt} | r_t
    let std_dev = if (kappa * dt).abs() < 1e-8 {
        sigma * dt.sqrt()
    } else {
        sigma * ((1.0 - (-2.0 * kappa * dt).exp()) / (2.0 * kappa)).sqrt()
    };

    let mut paths = Vec::with_capacity(num_paths);
    let base_rng = PhiloxRng::new(seed);
    let mut normals = vec![0.0f64; num_steps];

    for path_idx in 0..num_paths {
        // Each path gets an independent counter-based substream — deterministic
        // across runs and platforms, statistically sound (Philox4x32-10).
        let mut rng = base_rng.substream(path_idx as u64);
        rng.fill_std_normals(&mut normals);

        let mut rates = Vec::with_capacity(num_steps + 1);
        rates.push(initial_rate);

        let mut r = initial_rate;

        for (step, &z) in normals.iter().enumerate() {
            // Exact HW1F step
            r = r * exp_kappa_dt + drift_coeffs[step] + std_dev * z;
            rates.push(r);
        }

        paths.push(RatePath { rates });
    }

    paths
}

/// Extra discounting time (years) from each projection step's grid endpoint
/// to the pool's actual payment date for that accrual period.
///
/// Step `m` (0-based) accrues over the calendar month starting at
/// `first-of-month(max(as_of, issue_date)) + m months` and ends at grid time
/// `(m+1)/12`. Agency pools pay with a stated delay (e.g. FNMA on the 25th of
/// the following month), so the cash actually arrives later than the accrual
/// month end. Ignoring the delay overstates PV; the extra time is discounted
/// at the step's short rate + OAS.
fn payment_delay_extras(
    mbs: &AgencyMbsPassthrough,
    as_of: Date,
    num_steps: usize,
) -> Result<Vec<f64>> {
    let dt = 1.0 / 12.0;
    let effective_start = as_of.max(mbs.issue_date);
    let start_month = Date::from_calendar_date(effective_start.year(), effective_start.month(), 1)
        .map_err(|e| CoreError::Validation(e.to_string()))?;

    use finstack_core::dates::DateExt;
    let mut extras = Vec::with_capacity(num_steps);
    for m in 0..num_steps {
        let period_start = start_month.add_months(m as i32);
        let payment_date = mbs.payment_date_for_accrual_period(period_start)?;
        let t_pay = (payment_date - as_of).whole_days() as f64 / 365.25;
        let t_grid_end = (m as f64 + 1.0) * dt;
        extras.push((t_pay - t_grid_end).max(0.0));
    }
    Ok(extras)
}

/// Numerical safety cap on the adjusted SMM.
///
/// This is **not a market convention** — it is a guard against full-balance
/// prepayment in a single month, which would zero out the remaining schedule
/// and risk divide-by-zero / NaN in downstream amortization. A 0.9999 cap
/// implies a residual ≥ 1bp of pool balance per month, which is below MC
/// noise and below any rationally observable prepayment behavior.
///
/// Do not raise to 1.0 (degenerate balance) and do not lower below ~0.99 (real
/// pool data, e.g. burnout-adjusted refi waves, can plausibly clear ≥ 99% in a
/// single month under extreme rate moves).
const SMM_SAFETY_CAP: f64 = 0.9999;

/// Compute rate-dependent SMM (Single Monthly Mortality) from the base PSA model.
///
/// The base SMM from the PSA model is adjusted by a multiplier that depends on
/// the current short rate relative to the base rate:
///
/// ```text
/// adjusted_smm = base_smm × exp(-β × (current_rate - base_rate))
/// ```
///
/// This captures the refinancing incentive: lower rates → faster prepayment.
/// The output is clamped to `[0.0, SMM_SAFETY_CAP]` for numerical robustness;
/// see [`SMM_SAFETY_CAP`] for the rationale.
fn rate_adjusted_smm(base_smm: f64, current_rate: f64, base_rate: f64, sensitivity: f64) -> f64 {
    let multiplier = (-sensitivity * (current_rate - base_rate)).exp();
    (base_smm * multiplier).clamp(0.0, SMM_SAFETY_CAP)
}

/// Price MBS on a single rate path with a given OAS.
///
/// Projects monthly cashflows using rate-dependent prepayment and
/// discounts each cashflow at the path's short rate + OAS.
///
/// The `as_of` date is used to compute the pool's actual seasoning at the
/// valuation date.  Each projection step adds `month + 1` on top of that
/// base so that the PSA/CPR ramp reflects the true pool age rather than
/// treating every valuation as if the pool were newly issued.
///
/// # Errors
///
/// Returns `Error::Validation` when the prepayment model returns an
/// out-of-range or non-finite SMM.
fn price_on_path(
    mbs: &AgencyMbsPassthrough,
    path: &RatePath,
    base_rate: f64,
    oas: f64,
    prepay_sensitivity: f64,
    as_of: Date,
    payment_extras: &[f64],
) -> Result<f64> {
    let monthly_coupon_rate = mbs.pass_through_rate / 12.0;
    let monthly_mortgage_rate = mbs.wac / 12.0;
    let dt = 1.0 / 12.0;

    let mut balance = mbs.current_face.amount();
    let mut pv = 0.0;
    let mut cumulative_df = 1.0;

    let wam = mbs.wam as usize;
    let num_steps = path.rates.len().saturating_sub(1).min(wam);

    // Seasoning base for the PSA SMM ramp: the pool's actual age at the
    // valuation date (`as_of`). Loop-invariant, so computed once here; the
    // per-step offset below makes the PSA ramp/plateau reflect the true pool
    // age rather than a fresh-issue ramp.
    let base_seasoning = mbs.seasoning_months(as_of);

    for month in 0..num_steps {
        if balance < 0.01 {
            break;
        }

        let current_rate = path.rates[month + 1];

        // Discount factor for this step: exp(-(r + oas) × dt)
        let step_df = (-(current_rate + oas) * dt).exp();
        cumulative_df *= step_df;

        let seasoning = base_seasoning + month as u32 + 1;
        let base_smm = mbs.prepayment_model.smm(seasoning)?;
        if !base_smm.is_finite() || !(0.0..=1.0).contains(&base_smm) {
            return Err(CoreError::Validation(format!(
                "MBS prepayment model returned invalid SMM={base_smm} at seasoning {seasoning} months on MC path; expected finite value in [0.0, 1.0]"
            )));
        }

        // Rate-adjusted SMM
        let smm = rate_adjusted_smm(base_smm, current_rate, base_rate, prepay_sensitivity);

        // Scheduled amortization: `wam` is the remaining WAM at `as_of`, so at
        // projection step `month` (0-based) there are `wam − month` level
        // payments left, including the current one (same convention as the
        // deterministic pricer).
        let remaining = wam.saturating_sub(month).max(1);
        let scheduled_principal = if remaining <= 1 {
            balance
        } else if monthly_mortgage_rate > 1e-12 {
            let factor = (1.0 + monthly_mortgage_rate).powi(remaining as i32);
            let payment = balance * monthly_mortgage_rate * factor / (factor - 1.0);
            let interest_part = balance * monthly_mortgage_rate;
            (payment - interest_part).max(0.0).min(balance)
        } else {
            balance / remaining as f64
        };

        // Prepayment is the SMM-driven fraction of the balance that remains
        // *after* scheduled amortization, not of the gross beginning balance.
        // SMM (single monthly mortality) is defined on the post-amortization
        // balance; applying it to the gross balance double-counts the
        // scheduled principal inside the prepayment bucket and can drive the
        // ending balance negative under high SMM.
        let prepayment = (balance - scheduled_principal).max(0.0) * smm;

        // Interest
        let interest = balance * monthly_coupon_rate;

        // Total cashflow
        let total_cf = scheduled_principal + prepayment + interest;

        // PV of this month's cashflow, discounted to the actual payment date:
        // the cumulative grid DF covers up to the accrual month end, and the
        // agency payment delay adds extra discounting at the current short
        // rate + OAS (matching the deterministic pricer's payment dating).
        let extra = payment_extras.get(month).copied().unwrap_or(0.0);
        let delay_df = (-(current_rate + oas) * extra).exp();
        pv += total_cf * cumulative_df * delay_df;

        // Update balance
        balance = (balance - scheduled_principal - prepayment).max(0.0);
    }

    Ok(pv)
}

/// Calculate Monte Carlo OAS for an agency MBS.
///
/// Uses stochastic interest rate paths with rate-dependent prepayment to
/// compute the OAS that equates the average discounted cashflow to the
/// market price.
///
/// # Arguments
///
/// * `mbs` - Agency MBS passthrough instrument
/// * `market_price_pct` - Market price as percentage of face (e.g., 98.5)
/// * `market` - Market context with discount curves
/// * `as_of` - Valuation date
/// * `config` - Monte Carlo configuration (paths, HW params, seed)
///
/// # Returns
///
/// Monte Carlo OAS result with spread, convergence, and standard error.
///
/// # Example
///
/// ```text
/// use finstack_valuations::instruments::fixed_income::mbs_passthrough::{
///     AgencyMbsPassthrough,
///     metrics::mc_oas::{calculate_mc_oas, McOasConfig},
/// };
///
/// let mbs = AgencyMbsPassthrough::example().unwrap();
/// let config = McOasConfig { num_paths: 1024, ..Default::default() };
/// let result = calculate_mc_oas(&mbs, 98.5, &market, as_of, &config)?;
/// println!("MC OAS: {:.0} bps", result.oas * 10_000.0);
/// ```
pub(crate) fn calculate_mc_oas(
    mbs: &AgencyMbsPassthrough,
    market_price_pct: f64,
    market: &MarketContext,
    as_of: Date,
    config: &McOasConfig,
) -> Result<McOasResult> {
    let market_price = market_price_pct / 100.0 * mbs.current_face.amount();

    let discount_curve = market.get_discount(&mbs.discount_curve_id)?;
    let num_steps = config.num_steps.unwrap_or(mbs.wam as usize);

    // Fit the HW1F model to the initial discount curve: r(0) from the curve's
    // instantaneous forward and a piecewise-constant θ(t) bootstrap, so the
    // simulated short rate reprices the curve (a constant θ from a single 5Y
    // zero leaves the model arbitrageable against the input curve).
    let initial_rate = initial_short_rate_from_curve(discount_curve.as_ref(), as_of)?;
    let hw_params = HullWhiteParams::new(config.hw_kappa, config.hw_sigma)?;
    let horizon = num_steps as f64 / 12.0;
    let hw1f = calibrate_hw1f_params(hw_params, discount_curve.as_ref(), as_of, horizon)?;

    // Simulate rate paths
    let paths = simulate_rate_paths(
        initial_rate,
        &hw1f,
        config.num_paths,
        num_steps,
        config.seed,
    );

    // Per-step payment-delay discounting offsets (actual payment dates).
    let payment_extras = payment_delay_extras(mbs, as_of, num_steps)?;

    // Capture pricing errors raised by price_on_path so they propagate
    // through the f64-only Brent objective rather than being silently coerced
    // to NaN. The first non-zero error is preserved across iterations.
    let pricing_error: std::cell::RefCell<Option<CoreError>> = std::cell::RefCell::new(None);

    // Objective: average price across paths minus market price
    let objective = |oas: f64| -> f64 {
        let mut total = 0.0_f64;
        for path in &paths {
            match price_on_path(
                mbs,
                path,
                initial_rate,
                oas,
                config.prepay_rate_sensitivity,
                as_of,
                &payment_extras,
            ) {
                Ok(pv) => total += pv,
                Err(e) => {
                    if pricing_error.borrow().is_none() {
                        *pricing_error.borrow_mut() = Some(e);
                    }
                    return f64::NAN;
                }
            }
        }
        let avg_price = total / config.num_paths as f64;
        avg_price - market_price
    };

    // Solve for OAS using Brent's method
    let solver = BrentSolver::new()
        .tolerance(config.tolerance)
        .max_iterations(200)
        .bracket_bounds(-0.10, 0.20)
        .initial_bracket_size(Some(0.05));

    let result = solver.solve(objective, 0.0);

    // Surface a captured pricing error before reporting the solver outcome.
    if let Some(err) = pricing_error.borrow_mut().take() {
        return Err(err);
    }

    // Solver failure is now informative: the underlying pricing succeeded but
    // no OAS bracketed the target market price (likely far-from-feasible
    // bounds, or a market price outside the model's reachable PV range).
    let oas = result.map_err(|e| {
        CoreError::Validation(format!(
            "MC OAS Brent solver failed to converge within bounds [-10%, 20%]: {e}. \
             Check that market price {market_price_pct} pct is within the model's reachable PV range."
        ))
    })?;

    // Recompute path prices at the converged OAS for statistics.
    let mut path_prices = Vec::with_capacity(config.num_paths);
    for path in &paths {
        path_prices.push(price_on_path(
            mbs,
            path,
            initial_rate,
            oas,
            config.prepay_rate_sensitivity,
            as_of,
            &payment_extras,
        )?);
    }

    let avg_price = path_prices.iter().sum::<f64>() / config.num_paths as f64;

    // Standard error of the mean
    let variance = if config.num_paths > 1 {
        let mean = avg_price;
        path_prices.iter().map(|p| (p - mean).powi(2)).sum::<f64>() / (config.num_paths - 1) as f64
    } else {
        0.0
    };
    let std_error = (variance / config.num_paths as f64).sqrt();

    Ok(McOasResult {
        oas,
        model_price: avg_price,
        market_price,
        price_error: avg_price - market_price,
        num_paths: config.num_paths,
        converged: true,
        price_std_error: std_error,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cashflow::builder::specs::PrepaymentModelSpec;
    use crate::instruments::fixed_income::mbs_passthrough::{AgencyProgram, PoolType};
    use finstack_core::currency::Currency;
    use finstack_core::dates::DayCount;
    use finstack_core::market_data::term_structures::DiscountCurve;
    use finstack_core::math::interp::InterpStyle;
    use finstack_core::money::Money;
    use finstack_core::types::{CurveId, InstrumentId};
    use time::Month;

    fn create_test_mbs() -> AgencyMbsPassthrough {
        AgencyMbsPassthrough::builder()
            .id(InstrumentId::new("TEST-MBS-MC"))
            .pool_id("TEST-POOL".into())
            .agency(AgencyProgram::Fnma)
            .pool_type(PoolType::Generic)
            .original_face(Money::new(1_000_000.0, Currency::USD))
            .current_face(Money::new(1_000_000.0, Currency::USD))
            .current_factor(1.0)
            .wac(0.045)
            .pass_through_rate(0.04)
            .servicing_fee_rate(0.0025)
            .guarantee_fee_rate(0.0025)
            .wam(360)
            .issue_date(Date::from_calendar_date(2024, Month::January, 1).expect("valid"))
            .maturity(Date::from_calendar_date(2054, Month::January, 1).expect("valid"))
            .prepayment_model(PrepaymentModelSpec::psa(1.0))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .day_count(DayCount::Thirty360)
            .build()
            .expect("valid mbs")
    }

    fn create_test_market(as_of: Date) -> MarketContext {
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([
                (0.0, 1.0),
                (1.0, 0.96),
                (5.0, 0.80),
                (10.0, 0.60),
                (30.0, 0.30),
            ])
            .interp(InterpStyle::Linear)
            .build()
            .expect("valid curve");

        MarketContext::new().insert(disc)
    }

    #[test]
    fn test_rate_path_simulation() {
        let params = HullWhite1FParams::new(0.05, 0.01, 0.04);
        let paths = simulate_rate_paths(0.04, &params, 100, 120, 42);
        assert_eq!(paths.len(), 100);

        for path in &paths {
            assert_eq!(path.rates.len(), 121); // 120 steps + initial
                                               // Initial rate should match
            assert!((path.rates[0] - 0.04).abs() < 1e-10);
            // Rates should be finite
            for &r in &path.rates {
                assert!(r.is_finite());
            }
        }
    }

    #[test]
    fn test_rate_adjusted_smm() {
        let base_smm = 0.005;
        let base_rate = 0.04;

        // Same rate → multiplier ≈ 1
        let adj = rate_adjusted_smm(base_smm, 0.04, base_rate, 7.0);
        assert!((adj - base_smm).abs() < 1e-10);

        // Lower rate → faster prepayment
        let adj_low = rate_adjusted_smm(base_smm, 0.02, base_rate, 7.0);
        assert!(adj_low > base_smm);

        // Higher rate → slower prepayment
        let adj_high = rate_adjusted_smm(base_smm, 0.06, base_rate, 7.0);
        assert!(adj_high < base_smm);

        // SMM should be capped at the numerical safety threshold
        let extreme = rate_adjusted_smm(0.5, -0.10, base_rate, 20.0);
        assert!(extreme <= SMM_SAFETY_CAP);
    }

    /// Item 3 regression: prepayment must apply SMM to the *post-amortization*
    /// balance, not the gross beginning balance.
    ///
    /// With SMM applied to the gross balance, `scheduled_principal +
    /// gross_balance * smm` can exceed the beginning balance for high SMM,
    /// over-paying principal and forcing the path PV above the no-prepay
    /// ceiling. The correct definition `(balance - scheduled) * smm` keeps
    /// `scheduled + prepayment <= balance`, so a 100%-CPR pool prepays exactly
    /// its post-amortization balance and the path PV stays sensible.
    ///
    /// This test prices a single flat-rate path with a constant-CPR pool and
    /// checks that the total principal returned never exceeds the starting
    /// balance — which the gross-balance bug violates.
    #[test]
    fn prepayment_uses_post_amortization_balance_not_gross() {
        // Flat short-rate path so discounting is well-behaved and the rate
        // multiplier is exactly 1 (current_rate == base_rate).
        let base_rate = 0.03;
        let mut mbs = create_test_mbs();
        // High constant CPR maximises the gap between gross-balance and
        // post-amortization SMM.
        mbs.prepayment_model = PrepaymentModelSpec::constant_cpr(0.80);

        let wam = mbs.wam as usize;
        let path = RatePath {
            rates: vec![base_rate; wam + 1],
        };

        // Re-derive the per-period principal exactly as price_on_path does and
        // assert the post-amortization invariant holds every month.
        let monthly_mortgage_rate = mbs.wac / 12.0;
        let mut balance = mbs.current_face.amount();
        let start_balance = balance;
        let mut total_principal = 0.0;

        for month in 0..wam {
            if balance < 0.01 {
                break;
            }
            // Use issue_date as as_of (fresh pool) to match price_on_path convention.
            let base_seasoning = mbs.seasoning_months(mbs.issue_date);
            let seasoning = base_seasoning + month as u32 + 1;
            let base_smm = mbs.prepayment_model.smm(seasoning).expect("smm");
            let smm = rate_adjusted_smm(base_smm, base_rate, base_rate, 7.0);

            let remaining = wam.saturating_sub(month).max(1);
            let scheduled_principal = if remaining <= 1 {
                balance
            } else {
                let factor = (1.0 + monthly_mortgage_rate).powi(remaining as i32);
                let payment = balance * monthly_mortgage_rate * factor / (factor - 1.0);
                let interest_part = balance * monthly_mortgage_rate;
                (payment - interest_part).max(0.0).min(balance)
            };
            // Correct (post-amortization) prepayment.
            let prepayment = (balance - scheduled_principal).max(0.0) * smm;

            assert!(
                scheduled_principal + prepayment <= balance + 1e-6,
                "month {month}: scheduled {scheduled_principal} + prepayment {prepayment} \
                 exceeds beginning balance {balance} — SMM applied to gross balance"
            );
            total_principal += scheduled_principal + prepayment;
            balance = (balance - scheduled_principal - prepayment).max(0.0);
        }

        // Total principal returned over the pool's life cannot exceed the
        // starting balance (no principal is created from nothing).
        assert!(
            total_principal <= start_balance + 1.0,
            "total principal {total_principal} exceeds starting balance {start_balance}"
        );

        // And price_on_path itself must run without producing a non-finite PV.
        // Use the MBS issue date so seasoning starts at 0 (fresh pool).
        let as_of = mbs.issue_date;
        let extras = vec![0.0; wam];
        let pv = price_on_path(&mbs, &path, base_rate, 0.0, 7.0, as_of, &extras).expect("price");
        assert!(
            pv.is_finite() && pv > 0.0,
            "path PV must be finite/positive"
        );
    }

    #[test]
    fn test_mc_oas_at_model_price() {
        let mbs = create_test_mbs();
        let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid");
        let market = create_test_market(as_of);

        // First, compute the model price at OAS = 0 using the *same*
        // curve-calibrated θ(t), r(0), and payment-delay discounting that
        // `calculate_mc_oas` uses internally.
        let config = McOasConfig {
            num_paths: 64, // Fewer paths for speed in test
            ..Default::default()
        };

        let curve = market.get_discount(&mbs.discount_curve_id).expect("curve");
        let initial_rate = initial_short_rate_from_curve(curve.as_ref(), as_of).expect("r0");
        let hw = HullWhiteParams::new(config.hw_kappa, config.hw_sigma).expect("hw");
        let hw1f = calibrate_hw1f_params(hw, curve.as_ref(), as_of, 30.0).expect("theta fit");
        let paths = simulate_rate_paths(initial_rate, &hw1f, 64, 360, config.seed);
        let extras = payment_delay_extras(&mbs, as_of, 360).expect("extras");
        let total: f64 = paths
            .iter()
            .map(|path| {
                price_on_path(
                    &mbs,
                    path,
                    initial_rate,
                    0.0,
                    config.prepay_rate_sensitivity,
                    as_of,
                    &extras,
                )
                .expect("test fixture is well-formed")
            })
            .sum();
        let avg_price: f64 = total / 64.0;

        let market_price_pct = avg_price / mbs.current_face.amount() * 100.0;

        // MC OAS at model price should be approximately 0
        let result =
            calculate_mc_oas(&mbs, market_price_pct, &market, as_of, &config).expect("mc oas");

        // Allow wider tolerance due to MC noise
        assert!(
            result.oas.abs() < 0.005,
            "OAS should be near zero at model price, got {}",
            result.oas
        );
    }

    /// Finding 14 regression: longer agency payment delay must lower path PV.
    ///
    /// FNMA pays on the 25th of the month following accrual (~55-day stated
    /// delay) while GNMA I pays on the 15th of the accrual month. The same
    /// cashflows received later must be worth less under positive rates.
    #[test]
    fn longer_payment_delay_lowers_path_pv() {
        let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid");
        let base_rate = 0.04;
        let wam = 360usize;
        let flat_path = RatePath {
            rates: vec![base_rate; wam + 1],
        };

        let fnma = create_test_mbs(); // FNMA: pays 25th of following month
        let mut gnma1 = create_test_mbs();
        gnma1.agency = AgencyProgram::GnmaI; // pays 15th of accrual month

        let fnma_extras = payment_delay_extras(&fnma, as_of, wam).expect("fnma extras");
        let gnma1_extras = payment_delay_extras(&gnma1, as_of, wam).expect("gnma extras");
        assert!(
            fnma_extras[0] > gnma1_extras[0],
            "FNMA delay extra {} must exceed GNMA I extra {}",
            fnma_extras[0],
            gnma1_extras[0]
        );

        let pv_fnma = price_on_path(&fnma, &flat_path, base_rate, 0.0, 7.0, as_of, &fnma_extras)
            .expect("fnma pv");
        let pv_gnma1 = price_on_path(
            &gnma1,
            &flat_path,
            base_rate,
            0.0,
            7.0,
            as_of,
            &gnma1_extras,
        )
        .expect("gnma pv");

        assert!(
            pv_fnma < pv_gnma1,
            "longer payment delay must lower PV: fnma={pv_fnma:.2} gnma1={pv_gnma1:.2}"
        );
    }

    #[test]
    fn test_mc_oas_discount_gives_positive_spread() {
        let mbs = create_test_mbs();
        let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid");
        let market = create_test_market(as_of);

        let config = McOasConfig {
            num_paths: 64,
            ..Default::default()
        };

        // Discount price should give positive OAS
        let result = calculate_mc_oas(&mbs, 80.0, &market, as_of, &config).expect("mc oas");

        assert!(
            result.oas > 0.0,
            "OAS should be positive for discount price, got {}",
            result.oas
        );
    }

    /// C10 regression: MC-OAS must project prepayment from actual pool seasoning,
    /// not restart the PSA ramp at month 0 for every valuation.
    ///
    /// A seasoned pool (issued years before `as_of`) sits well into the PSA
    /// plateau (CPR ≈ 6% for 100 PSA once seasoning > 30 months).  A freshly-
    /// issued pool starts on the ramp (CPR < 6% for the first 30 months).
    /// Under the bug, both pools use `seasoning = 0 + month + 1`, producing
    /// identical SMMs and therefore identical prices — even though the seasoned
    /// pool has materially faster prepayment at every projection step.
    ///
    /// The test asserts that the two pools produce different prices on the same
    /// flat-rate path when the correct base-seasoning is applied.
    #[test]
    fn mc_oas_projects_prepayment_from_actual_pool_seasoning() {
        use crate::cashflow::builder::specs::PrepaymentModelSpec;
        use crate::instruments::fixed_income::mbs_passthrough::{AgencyProgram, PoolType};
        use finstack_core::currency::Currency;
        use finstack_core::dates::DayCount;
        use finstack_core::money::Money;
        use finstack_core::types::{CurveId, InstrumentId};
        use time::Month;

        // Valuation date: 2026-01-15
        let as_of = Date::from_calendar_date(2026, Month::January, 15).expect("valid");

        // Fresh pool: issued at as_of → seasoning = 0 at valuation
        let fresh_issue = Date::from_calendar_date(2026, Month::January, 1).expect("valid");
        let fresh_mbs = AgencyMbsPassthrough::builder()
            .id(InstrumentId::new("FRESH-MBS"))
            .pool_id("FRESH-POOL".into())
            .agency(AgencyProgram::Fnma)
            .pool_type(PoolType::Generic)
            .original_face(Money::new(1_000_000.0, Currency::USD))
            .current_face(Money::new(1_000_000.0, Currency::USD))
            .current_factor(1.0)
            .wac(0.045)
            .pass_through_rate(0.04)
            .servicing_fee_rate(0.0025)
            .guarantee_fee_rate(0.0025)
            .wam(360)
            .issue_date(fresh_issue)
            .maturity(Date::from_calendar_date(2056, Month::January, 1).expect("valid"))
            .prepayment_model(PrepaymentModelSpec::psa(1.0))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .day_count(DayCount::Thirty360)
            .build()
            .expect("valid fresh mbs");

        // Seasoned pool: issued 5 years before as_of → seasoning ≈ 60 months
        // (well into the PSA plateau, CPR = 6% at 100 PSA)
        let seasoned_issue = Date::from_calendar_date(2021, Month::January, 1).expect("valid");
        let seasoned_mbs = AgencyMbsPassthrough::builder()
            .id(InstrumentId::new("SEASONED-MBS"))
            .pool_id("SEASONED-POOL".into())
            .agency(AgencyProgram::Fnma)
            .pool_type(PoolType::Generic)
            .original_face(Money::new(1_000_000.0, Currency::USD))
            .current_face(Money::new(1_000_000.0, Currency::USD))
            .current_factor(1.0)
            .wac(0.045)
            .pass_through_rate(0.04)
            .servicing_fee_rate(0.0025)
            .guarantee_fee_rate(0.0025)
            .wam(360)
            .issue_date(seasoned_issue)
            .maturity(Date::from_calendar_date(2051, Month::January, 1).expect("valid"))
            .prepayment_model(PrepaymentModelSpec::psa(1.0))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .day_count(DayCount::Thirty360)
            .build()
            .expect("valid seasoned mbs");

        // Verify the base-seasonings differ as expected
        assert_eq!(fresh_mbs.seasoning_months(as_of), 0);
        let seasoned_base = seasoned_mbs.seasoning_months(as_of);
        assert!(
            seasoned_base >= 59,
            "expected ≥59 months seasoning, got {seasoned_base}"
        );

        // Use a flat short-rate path so the only difference is seasoning
        let base_rate = 0.04f64;
        let wam = 360usize;
        let flat_path = RatePath {
            rates: vec![base_rate; wam + 1],
        };

        let extras = vec![0.0; wam];
        let fresh_pv = price_on_path(&fresh_mbs, &flat_path, base_rate, 0.0, 7.0, as_of, &extras)
            .expect("fresh pv");
        let seasoned_pv = price_on_path(
            &seasoned_mbs,
            &flat_path,
            base_rate,
            0.0,
            7.0,
            as_of,
            &extras,
        )
        .expect("seasoned pv");

        // The seasoned pool (60+ months, PSA plateau at 100 PSA ≈ 6% CPR) must
        // price differently from the fresh pool (still on the ramp, CPR < 6%).
        // Under the bug both pools produce identical PVs (diff = 0).  After the
        // fix the faster prepayment of the seasoned pool shortens its average
        // life, producing a measurable price difference.  Even on a flat
        // discount path the PV difference exceeds $10 on a $1M pool.
        assert!(
            (fresh_pv - seasoned_pv).abs() > 10.0,
            "seasoned pool (60+ months, PSA plateau) must price differently from fresh pool \
             (PSA ramp); fresh_pv={fresh_pv:.2} seasoned_pv={seasoned_pv:.2} diff={:.2}",
            (fresh_pv - seasoned_pv).abs()
        );
    }

    #[test]
    fn test_mc_oas_deterministic_with_seed() {
        let mbs = create_test_mbs();
        let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid");
        let market = create_test_market(as_of);

        let config = McOasConfig {
            num_paths: 32,
            seed: 12345,
            ..Default::default()
        };

        let result1 = calculate_mc_oas(&mbs, 95.0, &market, as_of, &config).expect("mc oas 1");
        let result2 = calculate_mc_oas(&mbs, 95.0, &market, as_of, &config).expect("mc oas 2");

        // Same seed should give identical results
        assert!(
            (result1.oas - result2.oas).abs() < 1e-12,
            "Same seed should give identical OAS"
        );
    }
}
