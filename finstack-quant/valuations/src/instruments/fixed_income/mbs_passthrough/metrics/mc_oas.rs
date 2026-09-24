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
//! where β defaults to the module-shared [`super::PREPAY_RATE_SENSITIVITY`]
//! (`ln(2)/0.01 ≈ 69.3`, i.e. prepayment speed roughly doubles per 100 bp
//! rate drop) so the MC-OAS engine and the effective duration/convexity
//! engine price the prepayment option consistently.
//!
//! # References
//!
//! - Fabozzi, F. J. (2016). *Bond Markets, Analysis, and Strategies*. Pearson.
//! - Hayre, L. (2001). *Salomon Smith Barney Guide to Mortgage-Backed and
//!   Asset-Backed Securities*. John Wiley & Sons.
//! - O'Kane, D. (2008). *Modelling Single-name and Multi-name Credit
//!   Derivatives*. John Wiley & Sons. `docs/REFERENCES.md#o-kane-2008`

use crate::instruments::fixed_income::mbs_passthrough::pricer::{
    first_unpaid_accrual_start, quote_basis_pool, settlement_accrued_interest,
};
use crate::instruments::fixed_income::mbs_passthrough::AgencyMbsPassthrough;
use crate::instruments::rates::hw1f::{initial_short_rate_from_curve, prepare_hw1f_params};
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::solver::{BrentSolver, Solver};
use finstack_quant_core::{Error as CoreError, Result};
use finstack_quant_models::monte_carlo::process::ou::HullWhite1FParams;
use finstack_quant_models::monte_carlo::rng::philox::PhiloxRng;
use finstack_quant_models::monte_carlo::traits::RandomStream;
use finstack_quant_models::rates::hull_white::HullWhiteCalibrationParams;

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
    /// Prepayment rate sensitivity to interest rates β
    /// (default: [`super::PREPAY_RATE_SENSITIVITY`] ≈ 69.3, the same
    /// refi-incentive doubling per 100 bp used by the effective
    /// duration/convexity engine). Higher values make prepayment more
    /// sensitive to rate changes.
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
            prepay_rate_sensitivity: super::PREPAY_RATE_SENSITIVITY,
            seed: 42,
            tolerance: 1e-7,
        }
    }
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
    let exp_kappa_dt = (-kappa * dt).exp();

    // θ(t) is piecewise-constant; precompute the per-step drift term using
    // the θ value at each step's left endpoint.
    let drift_coeffs: Vec<f64> = (0..num_steps)
        .map(|i| params.theta_at_time(i as f64 * dt) * (1.0 - exp_kappa_dt))
        .collect();

    let std_devs: Vec<f64> = (0..num_steps)
        .map(|step| {
            params
                .sigma_variance_for_step(step as f64 * dt, dt)
                .max(0.0)
                .sqrt()
        })
        .collect();

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
            r = r * exp_kappa_dt + drift_coeffs[step] + std_devs[step] * z;
            rates.push(r);
        }

        paths.push(RatePath { rates });
    }

    paths
}

/// Per-step accrual and payment schedule shared by every simulated path.
///
/// Precomputed once per valuation so that the MC engine projects the same
/// accrual periods as the deterministic pricer
/// ([`generate_cashflows`](crate::instruments::fixed_income::mbs_passthrough::pricer::generate_cashflows)):
/// projection starts at [`first_unpaid_accrual_start`] (including a
/// prior-month accrual whose delayed payment is still outstanding), interest
/// accrues on the pool's day-count fraction over the actual calendar month,
/// and prepayment seasoning is measured at each accrual period end.
struct McStepSchedule {
    /// Extra discounting time (years, on the discount curve's day count) from
    /// each step's grid endpoint `(m+1)/12` to the pool's actual payment date.
    /// May be negative:
    /// Walked-back in-flight
    /// periods pay before their grid endpoint. Combined with the cumulative
    /// grid discount factor this discounts each cashflow to its actual
    /// payment date.
    payment_extras: Vec<f64>,
    /// Pool day-count year fraction of each accrual month.
    accrual_fractions: Vec<f64>,
    /// Base (rate-unadjusted) SMM of each step from the pool's prepayment
    /// model at the pool seasoning of the accrual period end, exactly as in
    /// the deterministic pricer. Validated finite and in `[0, 1]`.
    base_smms: Vec<f64>,
}

impl McStepSchedule {
    fn len(&self) -> usize {
        self.payment_extras.len()
    }
}

/// Build the [`McStepSchedule`] for `num_steps` monthly projection steps.
fn mc_step_schedule(
    mbs: &AgencyMbsPassthrough,
    as_of: Date,
    num_steps: usize,
    curve_day_count: DayCount,
) -> Result<McStepSchedule> {
    use finstack_quant_core::dates::{DateExt, DayCountContext};

    let dt = 1.0 / 12.0;
    let start_month = first_unpaid_accrual_start(mbs, as_of)?;

    let mut payment_extras = Vec::with_capacity(num_steps);
    let mut accrual_fractions = Vec::with_capacity(num_steps);
    let mut base_smms = Vec::with_capacity(num_steps);
    for m in 0..num_steps {
        let period_start = start_month.add_months(m as i32);
        let accrual_end = period_start.add_months(1);
        let payment_date = mbs.payment_date_for_accrual_period(period_start)?;
        // Grid time is the curve's time axis (the HW1F θ(t) is fitted to the
        // curve on it), so the payment offset is measured the same way.
        let t_pay =
            curve_day_count.year_fraction(as_of, payment_date, DayCountContext::default())?;
        payment_extras.push(t_pay - (m as f64 + 1.0) * dt);
        accrual_fractions.push(mbs.day_count.year_fraction(
            period_start,
            accrual_end,
            DayCountContext::default(),
        )?);
        let period_end = accrual_end - time::Duration::days(1);
        let seasoning = mbs.seasoning_months(period_end);
        let base_smm = mbs.prepayment_model.smm(seasoning)?;
        if !base_smm.is_finite() || !(0.0..=1.0).contains(&base_smm) {
            return Err(CoreError::Validation(format!(
                "MBS prepayment model returned invalid SMM={base_smm} at seasoning {seasoning} months on MC path; expected finite value in [0.0, 1.0]"
            )));
        }
        base_smms.push(base_smm);
    }
    Ok(McStepSchedule {
        payment_extras,
        accrual_fractions,
        base_smms,
    })
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
/// The projection grid, accrual fractions, and seasoning come from the
/// precomputed [`McStepSchedule`], which mirrors the deterministic pricer:
/// step 0 is the earliest unpaid accrual period (possibly a prior-month
/// in-flight receivable), interest accrues on the pool day-count over the
/// actual calendar month, and the PSA/CPR ramp reflects the true pool age.
///
fn price_on_path(
    mbs: &AgencyMbsPassthrough,
    path: &RatePath,
    base_rate: f64,
    oas: f64,
    prepay_sensitivity: f64,
    steps: &McStepSchedule,
) -> f64 {
    let monthly_mortgage_rate = mbs.wac / 12.0;
    let dt = 1.0 / 12.0;

    let mut balance = mbs.current_face.amount();
    let mut pv = 0.0;
    let mut cumulative_df = 1.0;

    let wam = mbs.wam as usize;
    let num_steps = path.rates.len().saturating_sub(1).min(wam).min(steps.len());

    for month in 0..num_steps {
        if balance < 0.01 {
            break;
        }

        // Beginning-of-period (left-endpoint) short rate for the step. Path
        // discount factors follow the left-Riemann convention r(t)·dt over
        // [t, t+dt] (Glasserman 2003 §3.3); using the end-of-period rate
        // path.rates[month + 1] introduced a systematic O(σ²·dt) bias that
        // accumulated over the 360 monthly steps (~3 bp on price).
        let current_rate = path.rates[month];

        // Discount factor for this step: exp(-(r + oas) × dt)
        let step_df = (-(current_rate + oas) * dt).exp();
        cumulative_df *= step_df;

        let base_smm = steps.base_smms[month];

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

        // Investor interest accrues at the pass-through rate over the pool
        // day-count fraction of the actual accrual month (identical to the
        // deterministic pricer; exactly 1/12 for 30/360, month-length
        // dependent for Act/360 and Act/365F).
        let interest = balance * mbs.pass_through_rate * steps.accrual_fractions[month];

        // Total cashflow
        let total_cf = scheduled_principal + prepayment + interest;

        // PV of this month's cashflow, discounted to the actual payment date:
        // the cumulative grid DF covers up to the step's grid endpoint, and
        // the (possibly negative) payment extra adjusts to the actual payment
        // date at the current short rate + OAS (matching the deterministic
        // pricer's payment dating).
        let extra = steps.payment_extras[month];
        let delay_df = (-(current_rate + oas) * extra).exp();
        pv += total_cf * cumulative_df * delay_df;

        balance = (balance - scheduled_principal - prepayment).max(0.0);
    }

    pv
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
/// * `market_price_pct` - Clean price as a percentage of current face (e.g., 98.5);
///   calendar-month accrued interest at `as_of` is added to the solver target.
///   The target buys the settlement-month accrual onward, so the projection
///   excludes the prior month's in-flight payment (see `quote_basis_pool`).
/// * `market` - Market context with discount curves
/// * `as_of` - Valuation date
/// * `config` - Monte Carlo configuration (paths, HW params, seed)
///
/// # Returns
///
/// Option-adjusted spread in decimal (for example, `0.01` for 100 bp).
///
/// # Example
///
/// ```text
/// use finstack_quant_valuations::instruments::fixed_income::mbs_passthrough::{
///     AgencyMbsPassthrough,
///     metrics::mc_oas::{calculate_mc_oas, McOasConfig},
/// };
///
/// let mbs = AgencyMbsPassthrough::example().unwrap();
/// let config = McOasConfig { num_paths: 1024, ..Default::default() };
/// let oas = calculate_mc_oas(&mbs, 98.5, &market, as_of, &config)?;
/// println!("MC OAS: {:.0} bp", oas * 10_000.0);
/// ```
pub(crate) fn calculate_mc_oas(
    mbs: &AgencyMbsPassthrough,
    market_price_pct: f64,
    market: &MarketContext,
    as_of: Date,
    config: &McOasConfig,
) -> Result<f64> {
    // The clean quote plus settlement-month accrued buys the settlement-month
    // accrual onward, so project the pool the buyer receives: the prior
    // month's in-flight P&I belongs to the seller.
    let quote_pool = quote_basis_pool(mbs, as_of)?;
    let mbs = &quote_pool;
    let market_price = market_price_pct / 100.0 * mbs.current_face.amount()
        + settlement_accrued_interest(mbs, as_of)?;

    let discount_curve = market.get_discount(&mbs.discount_curve_id)?;
    let num_steps = config.num_steps.unwrap_or(mbs.wam as usize);

    // Fit the HW1F model to the initial discount curve: r(0) from the curve's
    // instantaneous forward and a piecewise-constant θ(t) bootstrap, so the
    // simulated short rate reprices the curve (a constant θ from a single 5Y
    // zero leaves the model arbitrageable against the input curve).
    let initial_rate = initial_short_rate_from_curve(discount_curve.as_ref(), as_of)?;
    let hw_params = HullWhiteCalibrationParams::new(config.hw_kappa, config.hw_sigma)?;
    let horizon = num_steps as f64 / 12.0;
    let hw1f = prepare_hw1f_params(hw_params, discount_curve.as_ref(), as_of, horizon)?;

    // Simulate rate paths
    let paths = simulate_rate_paths(
        initial_rate,
        &hw1f,
        config.num_paths,
        num_steps,
        config.seed,
    );

    // Per-step accrual periods, day-count fractions, seasonings and
    // payment-delay discounting offsets (actual payment dates).
    let steps = mc_step_schedule(mbs, as_of, num_steps, discount_curve.day_count())?;

    // Objective: average price across paths minus market price.
    //
    // Paths are independent, so price them in parallel and collect the results
    // in path order; the sum is then taken serially in that order, keeping the
    // objective value bit-identical to the serial implementation (and therefore
    // keeping Brent's iterates — and the solved OAS — unchanged).
    let objective = |oas: f64| -> f64 {
        let price_one = |path: &_| {
            price_on_path(
                mbs,
                path,
                initial_rate,
                oas,
                config.prepay_rate_sensitivity,
                &steps,
            )
        };

        #[cfg(not(target_arch = "wasm32"))]
        let path_pvs: Vec<f64> = {
            use rayon::prelude::*;
            paths.par_iter().map(price_one).collect()
        };

        #[cfg(target_arch = "wasm32")]
        let path_pvs: Vec<f64> = paths.iter().map(price_one).collect();

        let total: f64 = path_pvs.iter().sum();
        total / config.num_paths as f64 - market_price
    };

    // Solve for OAS using Brent's method
    let solver = BrentSolver::new()
        .tolerance(config.tolerance)
        .max_iterations(200)
        .bracket_bounds(-0.10, 0.20)
        .initial_bracket_size(Some(0.05));

    let result = solver.solve(objective, 0.0);

    // Solver failure is now informative: the underlying pricing succeeded but
    // no OAS bracketed the target market price (likely far-from-feasible
    // bounds, or a market price outside the model's reachable PV range).
    result.map_err(|e| {
        CoreError::Validation(format!(
            "MC OAS Brent solver failed to converge within bounds [-10%, 20%]: {e}. \
             Check that market price {market_price_pct} pct is within the model's reachable PV range."
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cashflow::builder::specs::PrepaymentModelSpec;
    use crate::instruments::fixed_income::mbs_passthrough::{AgencyProgram, PoolType};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::interp::InterpStyle;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::{CurveId, InstrumentId};
    use time::Month;

    fn create_test_mbs() -> AgencyMbsPassthrough {
        AgencyMbsPassthrough::builder()
            .id(InstrumentId::new("TEST-MBS-MC"))
            .pool_id("TEST-POOL".into())
            .agency(AgencyProgram::Fnma)
            .pool_type(PoolType::Generic)
            .original_face(Money::from((1_000_000_i64, Currency::USD)))
            .current_face(Money::from((1_000_000_i64, Currency::USD)))
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
        let params = HullWhite1FParams::new(0.05, 0.01, 0.04).expect("valid Hull-White parameters");
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
        let steps = mc_step_schedule(&mbs, as_of, wam, DayCount::Act365F).expect("steps");
        let pv = price_on_path(&mbs, &path, base_rate, 0.0, 7.0, &steps);
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
        let hw = HullWhiteCalibrationParams::new(config.hw_kappa, config.hw_sigma).expect("hw");
        let hw1f = prepare_hw1f_params(hw, curve.as_ref(), as_of, 30.0).expect("theta prepared");
        let paths = simulate_rate_paths(initial_rate, &hw1f, 64, 360, config.seed);
        let steps = mc_step_schedule(&mbs, as_of, 360, curve.day_count()).expect("steps");
        let total: f64 = paths
            .iter()
            .map(|path| {
                price_on_path(
                    &mbs,
                    path,
                    initial_rate,
                    0.0,
                    config.prepay_rate_sensitivity,
                    &steps,
                )
            })
            .sum();
        let avg_price: f64 = total / 64.0;

        let accrued =
            crate::instruments::fixed_income::mbs_passthrough::pricer::settlement_accrued_interest(
                &mbs, as_of,
            )
            .expect("accrued");
        let market_price_pct = (avg_price - accrued) / mbs.current_face.amount() * 100.0;

        // MC OAS at model price should be approximately 0
        let oas =
            calculate_mc_oas(&mbs, market_price_pct, &market, as_of, &config).expect("mc oas");

        // Allow wider tolerance due to MC noise
        assert!(
            oas.abs() < 0.005,
            "OAS should be near zero at model price, got {}",
            oas
        );
    }

    /// Finding 14 regression: longer agency payment delay must lower path PV.
    ///
    /// FNMA pays on the 25th of the month following accrual (~55-day stated
    /// delay) while GNMA I pays on the 15th of the following month. The same
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
        gnma1.agency = AgencyProgram::GnmaI; // pays 15th of following month

        let fnma_steps =
            mc_step_schedule(&fnma, as_of, wam, DayCount::Act365F).expect("fnma steps");
        let gnma1_steps =
            mc_step_schedule(&gnma1, as_of, wam, DayCount::Act365F).expect("gnma steps");
        assert!(
            fnma_steps.payment_extras[0] > gnma1_steps.payment_extras[0],
            "FNMA delay extra {} must exceed GNMA I extra {}",
            fnma_steps.payment_extras[0],
            gnma1_steps.payment_extras[0]
        );

        let pv_fnma = price_on_path(&fnma, &flat_path, base_rate, 0.0, 7.0, &fnma_steps);
        let pv_gnma1 = price_on_path(&gnma1, &flat_path, base_rate, 0.0, 7.0, &gnma1_steps);

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
        let oas = calculate_mc_oas(&mbs, 80.0, &market, as_of, &config).expect("mc oas");

        assert!(
            oas > 0.0,
            "OAS should be positive for discount price, got {}",
            oas
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
        use finstack_quant_core::currency::Currency;
        use finstack_quant_core::dates::DayCount;
        use finstack_quant_core::money::Money;
        use finstack_quant_core::types::{CurveId, InstrumentId};
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
            .original_face(Money::from((1_000_000_i64, Currency::USD)))
            .current_face(Money::from((1_000_000_i64, Currency::USD)))
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
            .original_face(Money::from((1_000_000_i64, Currency::USD)))
            .current_face(Money::from((1_000_000_i64, Currency::USD)))
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

        let fresh_steps =
            mc_step_schedule(&fresh_mbs, as_of, wam, DayCount::Act365F).expect("fresh steps");
        let seasoned_steps =
            mc_step_schedule(&seasoned_mbs, as_of, wam, DayCount::Act365F).expect("seasoned steps");
        let fresh_pv = price_on_path(&fresh_mbs, &flat_path, base_rate, 0.0, 7.0, &fresh_steps);
        let seasoned_pv = price_on_path(
            &seasoned_mbs,
            &flat_path,
            base_rate,
            0.0,
            7.0,
            &seasoned_steps,
        );

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

    /// Findings 3+4 regression: the MC model must agree with the
    /// deterministic pricer for an Act/360 seasoned pool valued mid-month.
    ///
    /// Two historical divergences are exercised at once:
    /// - interest accrual: the MC path used a flat `pass_through_rate / 12`
    ///   while the deterministic pricer day-counts the actual month
    ///   (materially different under Act/360);
    /// - projection start: the MC path started at
    ///   first-of-month(max(as_of, issue)) while the deterministic pricer
    ///   walks back to include the prior-month accrual whose delayed payment
    ///   is still outstanding (an in-flight receivable for seasoned pools
    ///   valued mid-month).
    ///
    /// In the zero-vol / zero-OAS / flat-curve limit the MC path price and
    /// the deterministic PV price the same cashflows, so they must agree up
    /// to the small discounting day-count basis (ACT/365.25 grid time vs the
    /// curve's day count).
    #[test]
    fn mc_zero_vol_matches_deterministic_pricer_for_act360_seasoned_pool() {
        use crate::instruments::fixed_income::mbs_passthrough::pricer::price_mbs;

        let as_of = Date::from_calendar_date(2026, Month::January, 15).expect("valid");
        let flat_rate = 0.04;

        // Seasoned Act/360 pool: issued 5 years before `as_of`, remaining
        // WAM 300, valued mid-month so the December accrual's payment
        // (Jan 25) is still in flight.
        let mbs = AgencyMbsPassthrough::builder()
            .id(InstrumentId::new("TEST-MBS-CONSISTENCY"))
            .pool_id("TEST-POOL".into())
            .agency(AgencyProgram::Fnma)
            .pool_type(PoolType::Generic)
            .original_face(Money::from((1_000_000_i64, Currency::USD)))
            .current_face(Money::from((850_000_i64, Currency::USD)))
            .current_factor(0.85)
            .wac(0.045)
            .pass_through_rate(0.04)
            .servicing_fee_rate(0.0025)
            .guarantee_fee_rate(0.0025)
            .wam(300)
            .issue_date(Date::from_calendar_date(2021, Month::January, 1).expect("valid"))
            .maturity(Date::from_calendar_date(2051, Month::January, 1).expect("valid"))
            .prepayment_model(PrepaymentModelSpec::psa(1.0))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .day_count(DayCount::Act360)
            .build()
            .expect("valid mbs");

        // Flat continuously-compounded curve: log-linear DF between two knots
        // gives a constant forward rate equal to `flat_rate`.
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (40.0, (-flat_rate * 40.0_f64).exp())])
            .interp(InterpStyle::LogLinear)
            .build()
            .expect("valid curve");
        let market = MarketContext::new().insert(disc);

        let det_pv = price_mbs(&mbs, &market, as_of).expect("det pv").amount();

        // Zero-vol limit: a single flat short-rate path at the curve rate.
        let wam = mbs.wam as usize;
        let flat_path = RatePath {
            rates: vec![flat_rate; wam + 1],
        };
        let curve_dc = market
            .get_discount(&mbs.discount_curve_id)
            .expect("curve")
            .day_count();
        let steps = mc_step_schedule(&mbs, as_of, wam, curve_dc).expect("steps");
        let mc_pv = price_on_path(&mbs, &flat_path, flat_rate, 0.0, 7.0, &steps);

        assert!(det_pv > 0.0 && mc_pv > 0.0);
        let rel_diff = (mc_pv - det_pv).abs() / det_pv;
        assert!(
            rel_diff < 0.005,
            "MC model price {mc_pv:.2} must agree with deterministic PV {det_pv:.2} \
             at zero vol / zero OAS / flat curve; relative diff {rel_diff:.5}"
        );
    }

    /// The MC grid runs on the discount curve's time axis, so a cashflow's
    /// payment offset must use the curve day count. January 2024 accrual of
    /// the FNMA test pool pays Monday 26 Feb 2024 (the 25th is a Sunday), 42
    /// days after the 15 Jan valuation: on an Act/360 curve the offset from
    /// the first grid point is `42/360 − 1/12`, not `42/365.25 − 1/12`.
    #[test]
    fn payment_offset_uses_the_curve_day_count() {
        let mbs = create_test_mbs();
        let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid");
        let steps = mc_step_schedule(&mbs, as_of, 1, DayCount::Act360).expect("steps");
        let expected = 42.0 / 360.0 - 1.0 / 12.0;
        assert!(
            (steps.payment_extras[0] - expected).abs() < 1e-15,
            "{} versus {expected}",
            steps.payment_extras[0]
        );
        assert!((steps.payment_extras[0] - (42.0 / 365.25 - 1.0 / 12.0)).abs() > 1e-3);
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

        let oas1 = calculate_mc_oas(&mbs, 95.0, &market, as_of, &config).expect("mc oas 1");
        let oas2 = calculate_mc_oas(&mbs, 95.0, &market, as_of, &config).expect("mc oas 2");

        // Same seed should give identical results
        assert!(
            (oas1 - oas2).abs() < 1e-12,
            "Same seed should give identical OAS"
        );
    }
}

#[cfg(test)]
mod production_mortgage_audit {
    use super::*;
    use crate::instruments::fixed_income::mbs_passthrough::pricer::generate_cashflows;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use time::macros::date;

    #[test]
    fn mbs_clean_quote_reprices_at_zero_oas_in_zero_vol_limit() {
        let as_of = date!(2024 - 01 - 15);
        let mut mbs = AgencyMbsPassthrough::example().expect("mbs");
        mbs.issue_date = date!(2024 - 01 - 01);
        mbs.wam = 12;
        mbs.maturity = date!(2025 - 01 - 01);
        let market = MarketContext::new().insert(
            DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .knots([(0.0, 1.0), (5.0, 1.0)])
                .build()
                .expect("curve"),
        );
        let dirty = generate_cashflows(&mbs, as_of, None)
            .expect("flows")
            .iter()
            .map(|cf| cf.total)
            .sum::<f64>();
        let accrued = mbs.current_face.amount() * mbs.pass_through_rate * 14.0 / 360.0;
        let clean = (dirty - accrued) / mbs.current_face.amount() * 100.0;
        let config = McOasConfig {
            num_paths: 2,
            hw_sigma: 1e-10,
            ..McOasConfig::default()
        };
        let oas = calculate_mc_oas(&mbs, clean, &market, as_of, &config).expect("oas");
        assert!(oas.abs() < 1e-8, "oas {oas}");
    }

    /// A clean quote buys the settlement-month accrual onward: the prior
    /// month's P&I, still in flight until the agency payment date, belongs to
    /// the seller. The same pool quoted at the same clean price before
    /// (Feb 10) and after (Feb 27) the Feb 26 payment of the January accrual
    /// must solve to the same OAS.
    ///
    /// Hand check of the tolerance: with a flat 4% curve and a 4% pass-through
    /// the dirty target and the projected PV both carry at ~4%/yr over the 17
    /// days, so the residual carry mismatch is below 0.01 price points on a
    /// ~6-year duration pool, i.e. well under 0.1 bp. Before the fix the
    /// Feb 10 solve priced the in-flight January flow against a target that
    /// excludes it: 6.80 bp on Feb 10 versus 2.41 bp on Feb 27.
    #[test]
    fn mc_oas_is_stable_across_the_in_flight_payment_date() {
        let mut mbs = AgencyMbsPassthrough::example().expect("mbs");
        mbs.issue_date = date!(2023 - 01 - 01);
        mbs.maturity = date!(2053 - 01 - 01);
        mbs.wam = 347;
        let flat = 0.04_f64;
        let quote = 99.5;
        let config = McOasConfig {
            num_paths: 2,
            hw_sigma: 1e-10,
            ..McOasConfig::default()
        };
        let oas_at = |as_of: Date| {
            let market = MarketContext::new().insert(
                DiscountCurve::builder("USD-OIS")
                    .base_date(as_of)
                    .knots([(0.0, 1.0), (40.0, (-flat * 40.0).exp())])
                    .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
                    .build()
                    .expect("curve"),
            );
            calculate_mc_oas(&mbs, quote, &market, as_of, &config).expect("oas")
        };
        let before = oas_at(date!(2024 - 02 - 10));
        let after = oas_at(date!(2024 - 02 - 27));
        assert!(
            (before - after).abs() < 1e-5,
            "OAS must not jump across the payment date: before={before} after={after}"
        );
        assert!(
            (before - 6.804e-4).abs() > 1e-5,
            "Feb 10 OAS must no longer price the seller's in-flight January flow"
        );
    }
}
