//! Option-adjusted spread (OAS) for structured-credit tranches.
//!
//! Solves for the constant spread `s` such that the Monte-Carlo average tranche
//! PV — discounted at each scenario's discount factors times `exp(-s·t)` —
//! equals a quoted market price. Either stochastic dimension can be enabled:
//!
//! - **Stochastic rates**: a Hull-White 1-factor short rate, decomposed as the
//!   discount curve's deterministic forwards plus a mean-zero Ornstein-Uhlenbeck
//!   deviation, `r(t) = f(0,t) + x(t)`. The absolute path drives rate-dependent
//!   prepayment; discounting applies the *exact* curve discount factor times the
//!   OU factor `exp(-∫x)`, so the model is curve-consistent (no-arbitrage) and
//!   collapses to the curve when volatility is zero — no flat-rate proxy.
//! - **Stochastic credit**: a systematic factor per scenario applies correlated
//!   (mean-corrected lognormal) stress to default and prepayment.
//! - **Both**: each scenario carries an independent rate path and credit factor.
//!
//! With neither dimension enabled the OAS reduces to the deterministic z-spread
//! (a single curve-discounted scenario), which the tests anchor against.
//!
//! # References
//!
//! - Hull-White short-rate model: Hull & White (1990).
//! - Templated on the agency-MBS Monte-Carlo OAS
//!   ([`crate::instruments::fixed_income::mbs_passthrough`]).

use crate::instruments::fixed_income::structured_credit::pricing::simulation_engine::{
    run_simulation_with_source, OasPathFlowSource,
};
use crate::instruments::fixed_income::structured_credit::StructuredCredit;
use crate::instruments::Instrument;
use finstack_quant_core::dates::{Date, DateExt, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::solver::{BrentSolver, Solver};
use finstack_quant_core::Result;
use finstack_quant_monte_carlo::rng::philox::PhiloxRng;
use finstack_quant_monte_carlo::RandomStream;
use serde::{Deserialize, Serialize};

/// Configuration for the structured-credit OAS calculation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OasConfig {
    /// Number of Monte-Carlo scenarios (forced to 1 when neither dimension is
    /// stochastic, since every scenario is then identical).
    pub num_paths: usize,
    /// Couple a stochastic Hull-White short-rate path (rate paths + rate-dependent
    /// prepayment, stochastic discounting).
    pub stochastic_rates: bool,
    /// Couple a systematic stochastic-credit factor (correlated default/prepay stress).
    pub stochastic_credit: bool,
    /// Hull-White mean reversion `κ`.
    pub hw_kappa: f64,
    /// Hull-White short-rate volatility `σ`.
    pub hw_sigma: f64,
    /// Rate-dependent prepayment sensitivity `β`.
    pub prepay_beta: f64,
    /// Credit factor loading for the lognormal default/prepayment shocks.
    pub credit_loading: f64,
    /// RNG seed (deterministic, reproducible results).
    pub seed: u64,
    /// Brent solver tolerance.
    pub tolerance: f64,
}

impl Default for OasConfig {
    fn default() -> Self {
        Self {
            num_paths: 256,
            stochastic_rates: false,
            stochastic_credit: true,
            hw_kappa: 0.05,
            hw_sigma: 0.01,
            prepay_beta: 7.0,
            credit_loading: 0.3,
            seed: 42,
            tolerance: 1e-7,
        }
    }
}

/// Result of an OAS calculation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OasResult {
    /// Option-adjusted spread (decimal; `0.01` = 100 bps).
    pub oas: f64,
    /// Model price (% of original balance) at the solved OAS.
    pub model_price: f64,
    /// Target market price (% of original balance).
    pub market_price: f64,
    /// Number of scenarios used.
    pub num_paths: usize,
    /// Monte-Carlo standard error of the mean price (% of original balance).
    pub price_std_error: f64,
}

/// Calculate the option-adjusted spread for a tranche.
///
/// # Arguments
///
/// * `deal` - Validated structured-credit deal owning the requested tranche
///   and its waterfall and credit assumptions.
/// * `tranche_id` - Identifier of the tranche whose option-adjusted spread is
///   solved.
/// * `market_price_pct` - Observed clean price as a percentage of original
///   tranche balance.
/// * `market` - Market context supplying the discount curve and stochastic
///   scenario dependencies.
/// * `as_of` - Valuation date used for projected tranche cashflows and
///   discounting.
/// * `config` - stochastic-rate/credit coupling and Monte-Carlo settings.
///
/// # Errors
///
/// Returns an error if the tranche is missing, the discount curve is
/// unavailable, a scenario fails to simulate, or the solver fails to converge.
pub fn calculate_tranche_oas(
    deal: &StructuredCredit,
    tranche_id: &str,
    market_price_pct: f64,
    market: &MarketContext,
    as_of: Date,
    config: &OasConfig,
) -> Result<OasResult> {
    deal.validate_for_pricing()?;
    let disc = market.get_discount(deal.discount_curve_id.as_str())?;
    let tranche = deal
        .tranches
        .tranches
        .iter()
        .find(|t| t.id.as_str() == tranche_id)
        .ok_or_else(|| {
            finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                id: format!("tranche:{tranche_id}"),
            })
        })?;
    let original_balance = tranche.original_balance.amount();
    let target_pv = market_price_pct / 100.0 * original_balance;

    let day_count = DayCount::Act365F;
    let maturity = deal
        .tranches
        .tranches
        .iter()
        .map(|t| t.maturity)
        .max()
        .unwrap_or(as_of);
    let num_months = as_of.months_until(maturity) as usize + 12;
    let base_rate = initial_short_rate(disc.as_ref(), as_of, maturity, &day_count)?;

    let stochastic = config.stochastic_rates || config.stochastic_credit;
    // SC-M17: bound the path count. Each path runs a full deterministic deal
    // simulation, so an unbounded `num_paths` from JSON is an unbounded-compute
    // vector.
    const MAX_OAS_PATHS: usize = 100_000;
    if config.num_paths > MAX_OAS_PATHS {
        return Err(finstack_quant_core::Error::Validation(format!(
            "OAS num_paths {} exceeds the {MAX_OAS_PATHS} cap; each path runs a \
             full deal simulation",
            config.num_paths
        )));
    }
    let num_paths = if stochastic {
        config.num_paths.max(1)
    } else {
        1
    };
    let rng = PhiloxRng::new(config.seed);

    // Hull-White is decomposed into the curve's deterministic forwards plus a
    // mean-zero Ornstein-Uhlenbeck deviation `x`. The forwards anchor the
    // rate-dependent prepayment path; discounting always uses the *exact* curve
    // discount factor times `exp(-∫x)`, so with zero volatility (x≡0) the model
    // reproduces the curve exactly (no-arbitrage), unlike a flat-rate proxy.
    let forwards = if config.stochastic_rates {
        Some(monthly_forwards(disc.as_ref(), num_months))
    } else {
        None
    };

    // Hull-White convexity adjustment for the stochastic discount factor. Each
    // path discounts by `curve_df · exp(-∫x)`, but `E[exp(-∫x)] = exp(+½·Var(∫x))
    // ≠ 1` (Jensen), so the *average* stochastic discount factor would exceed the
    // curve discount factor and bias PV (and the solved OAS) upward. The
    // deterministic, path-independent correction `exp(-½·Var(∫x to t))` makes
    // `E[stochastic DF] = curve DF` exactly (no-arbitrage), collapsing to 1 at
    // zero volatility. Computed once; applied per cashflow below.
    let rate_convexity_adj = if config.stochastic_rates {
        Some(ou_integral_convexity_adjustments(
            config.hw_kappa,
            config.hw_sigma,
            num_months,
        ))
    } else {
        None
    };

    // For each scenario, the per-cashflow `(year fraction t, CF·base_df)`. The
    // trial OAS only multiplies in `exp(-s·t)`, so the expensive simulation runs
    // once per scenario and the Brent solve over `s` is cheap.
    let mut scenarios: Vec<Vec<(f64, f64)>> = Vec::with_capacity(num_paths);

    for path in 0..num_paths {
        // Mean-zero OU deviation `x` for this scenario; the absolute rate path
        // fed to prepayment is `forward + x`.
        let deviation = if config.stochastic_rates {
            let mut sub = rng.substream(2 * path as u64);
            Some(simulate_ou_deviation(
                config.hw_kappa,
                config.hw_sigma,
                num_months,
                &mut sub,
            ))
        } else {
            None
        };
        let rate_path = match (&forwards, &deviation) {
            (Some(fwd), Some(dev)) => Some(absolute_rate_path(fwd, dev)),
            _ => None,
        };
        let credit_z = if config.stochastic_credit {
            let mut sub = rng.substream(2 * path as u64 + 1);
            Some(sub.next_std_normal())
        } else {
            None
        };

        let mut source = OasPathFlowSource::new(
            as_of,
            rate_path,
            credit_z,
            config.prepay_beta,
            base_rate,
            config.credit_loading,
        );
        let results = run_simulation_with_source(deal, market, as_of, &mut source)?;
        let cashflows = &results
            .get(tranche_id)
            .ok_or_else(|| {
                finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                    id: format!("tranche:{tranche_id}"),
                })
            })?
            .cashflows;

        let mut entries = Vec::with_capacity(cashflows.len());
        for (date, amount) in cashflows {
            if *date <= as_of {
                continue;
            }
            let t = day_count.year_fraction(as_of, *date, DayCountContext::default())?;
            // Exact curve discount factor (no-arbitrage), times the OU stochastic
            // factor when rates are stochastic, de-biased by the Hull-White
            // convexity adjustment so the average stochastic DF matches the curve.
            let mut base_df = disc.df_between_dates(as_of, *date)?;
            if let Some(dev) = &deviation {
                let month = as_of.months_until(*date) as usize;
                base_df *= ou_discount_factor(dev, month);
                if let Some(adj) = &rate_convexity_adj {
                    base_df *= adj
                        .get(month)
                        .copied()
                        .or_else(|| adj.last().copied())
                        .unwrap_or(1.0);
                }
            }
            entries.push((t, amount.amount() * base_df));
        }
        scenarios.push(entries);
    }

    let path_count = num_paths as f64;
    let objective = |oas: f64| -> f64 {
        let mut total = 0.0;
        for entries in &scenarios {
            for (t, cf_base_df) in entries {
                total += cf_base_df * (-oas * t).exp();
            }
        }
        total / path_count - target_pv
    };

    let solver = BrentSolver::new()
        .tolerance(config.tolerance)
        .initial_bracket_size(Some(0.05));
    let oas = solver.solve(objective, 0.0)?;

    // Per-scenario PV at the solved OAS for the model price and MC std error.
    let path_pvs: Vec<f64> = scenarios
        .iter()
        .map(|entries| {
            entries
                .iter()
                .map(|(t, cf_base_df)| cf_base_df * (-oas * t).exp())
                .sum()
        })
        .collect();
    let mean_pv = path_pvs.iter().sum::<f64>() / path_count;
    let model_price = if original_balance > 0.0 {
        mean_pv / original_balance * 100.0
    } else {
        0.0
    };
    let price_std_error = if num_paths > 1 && original_balance > 0.0 {
        // Unbiased (Bessel-corrected) sample variance, then the standard error
        // of the mean = sqrt(var / n).
        let var = path_pvs
            .iter()
            .map(|pv| (pv - mean_pv).powi(2))
            .sum::<f64>()
            / (path_count - 1.0);
        (var / path_count).sqrt() / original_balance * 100.0
    } else {
        0.0
    };

    Ok(OasResult {
        oas,
        model_price,
        market_price: market_price_pct,
        num_paths,
        price_std_error,
    })
}

/// Flat proxy for the initial short rate: the continuously-compounded average
/// zero rate from `as_of` to `maturity`.
fn initial_short_rate(
    curve: &DiscountCurve,
    as_of: Date,
    maturity: Date,
    day_count: &DayCount,
) -> Result<f64> {
    let t = day_count.year_fraction(as_of, maturity, DayCountContext::default())?;
    if t <= 0.0 {
        return Ok(0.0);
    }
    let df = curve.df_between_dates(as_of, maturity)?;
    Ok(-df.ln() / t)
}

/// Monthly continuously-compounded forward rates from the discount curve over
/// `[m/12, (m+1)/12]` for `m` in `0..num_months`. These are the deterministic
/// term-structure anchor for the Hull-White short rate `r(t) = forward(t) + x(t)`.
fn monthly_forwards(curve: &DiscountCurve, num_months: usize) -> Vec<f64> {
    let dt = 1.0 / 12.0;
    (0..num_months)
        .map(|m| {
            let t1 = m as f64 * dt;
            let t2 = (m + 1) as f64 * dt;
            // `forward` only errors on a non-positive interval, impossible here.
            curve.forward(t1, t2).unwrap_or_else(|_| curve.zero(t2))
        })
        .collect()
}

/// Simulate a monthly mean-zero Ornstein-Uhlenbeck deviation `x` (the stochastic
/// part of the Hull-White short rate), `x₀ = 0`. Exact OU discretization.
fn simulate_ou_deviation(
    kappa: f64,
    sigma: f64,
    num_months: usize,
    rng: &mut PhiloxRng,
) -> Vec<f64> {
    let dt = 1.0 / 12.0;
    let exp_k = (-kappa * dt).exp();
    let vol = if kappa > 0.0 {
        sigma * ((1.0 - (-2.0 * kappa * dt).exp()) / (2.0 * kappa)).sqrt()
    } else {
        sigma * dt.sqrt()
    };
    let mut deviation = Vec::with_capacity(num_months + 1);
    let mut x = 0.0;
    deviation.push(x);
    for _ in 0..num_months {
        let z = rng.next_std_normal();
        x = x * exp_k + vol * z;
        deviation.push(x);
    }
    deviation
}

/// Absolute monthly short-rate path `r_m = forward_m + x_m` for the prepayment
/// model. Length matches `forwards`.
fn absolute_rate_path(forwards: &[f64], deviation: &[f64]) -> Vec<f64> {
    forwards
        .iter()
        .enumerate()
        .map(|(m, f)| f + deviation.get(m).copied().unwrap_or(0.0))
        .collect()
}

/// Stochastic (OU) contribution to the discount factor at `month`:
/// `exp(-Δt · Σ_{m<month} x_m)`. The curve discount factor is applied
/// separately, so this is `1.0` when the deviation is identically zero.
fn ou_discount_factor(deviation: &[f64], month: usize) -> f64 {
    let dt = 1.0 / 12.0;
    let last = month.min(deviation.len());
    let acc: f64 = deviation[..last].iter().map(|x| -x * dt).sum();
    acc.exp()
}

/// Per-month Hull-White convexity adjustments `exp(-½·Var(∫₀ᵗ x ds))` for the
/// discretised integrated OU deviation used by [`ou_discount_factor`].
///
/// `adj[m]` corresponds to a cashflow `m` months out (whose stochastic discount
/// factor uses `W_m = Σ_{j<m} x_j`, the left-Riemann sum `Δt·W_m ≈ ∫x`). Because
/// `Δt·W_m` is exactly Gaussian, multiplying `exp(-Δt·W_m)` by
/// `exp(-½·Var(Δt·W_m))` makes its expectation exactly `1`, removing the
/// Jensen/convexity bias so the model is martingale-consistent (the average
/// stochastic discount factor reproduces the curve discount factor).
///
/// The variance recursion tracks `Var(x_m)`, `Cov(W_m, x_m)` and `Var(W_m)`
/// along the exact OU step `x_{m+1} = a·x_m + b·z_m` (`a = e^{-κΔt}`, `b` the
/// exact OU step volatility), so it is exact for the discretised process and
/// needs no closed-form integral. Returns `num_months + 1` factors (`m = 0..=num_months`);
/// every factor is `1.0` at zero volatility.
///
/// # References
/// - Brigo & Mercurio (2006), *Interest Rate Models — Theory and Practice*,
///   §3.3 (Hull-White bond reconstitution / convexity).
fn ou_integral_convexity_adjustments(kappa: f64, sigma: f64, num_months: usize) -> Vec<f64> {
    let dt = 1.0 / 12.0;
    let a = (-kappa * dt).exp();
    let b = if kappa > 0.0 {
        sigma * ((1.0 - (-2.0 * kappa * dt).exp()) / (2.0 * kappa)).sqrt()
    } else {
        sigma * dt.sqrt()
    };

    let mut adj = Vec::with_capacity(num_months + 1);
    let (mut var_x, mut cov_wx, mut var_w) = (0.0_f64, 0.0_f64, 0.0_f64);
    for _ in 0..=num_months {
        adj.push((-0.5 * dt * dt * var_w).exp());
        // Advance to the next month: W_{m+1} = W_m + x_m, x_{m+1} = a·x_m + b·z.
        let new_var_w = var_w + 2.0 * cov_wx + var_x;
        let new_cov_wx = a * (cov_wx + var_x);
        let new_var_x = a * a * var_x + b * b;
        var_w = new_var_w;
        cov_wx = new_cov_wx;
        var_x = new_var_x;
    }
    adj
}

#[cfg(test)]
mod tests {
    use super::*;

    /// With the convexity correction, the Monte-Carlo mean of the stochastic
    /// discount factor `exp(-∫x)·adj` must equal 1 (so `E[stochastic DF] = curve
    /// DF`, no-arbitrage) across horizons. Deterministic seed → stable.
    #[test]
    fn convexity_adjustment_makes_discount_factor_a_martingale() {
        let kappa = 0.05;
        let sigma = 0.01;
        let num_months = 120;
        let adj = ou_integral_convexity_adjustments(kappa, sigma, num_months);
        let rng = PhiloxRng::new(12_345);
        let n_paths = 20_000;
        for &month in &[12usize, 60, 120] {
            let mut sum = 0.0;
            for p in 0..n_paths {
                let mut sub = rng.substream(p as u64);
                let dev = simulate_ou_deviation(kappa, sigma, num_months, &mut sub);
                sum += ou_discount_factor(&dev, month) * adj[month];
            }
            let mean = sum / f64::from(n_paths);
            assert!(
                (mean - 1.0).abs() < 1e-2,
                "month {month}: mean stochastic DF {mean} is not a martingale (expected ~1)"
            );
        }
    }

    /// At zero volatility every adjustment is exactly the identity.
    #[test]
    fn zero_vol_convexity_adjustment_is_identity() {
        let adj = ou_integral_convexity_adjustments(0.05, 0.0, 24);
        assert!(adj.iter().all(|&a| (a - 1.0).abs() < 1e-15));
    }

    /// The adjustment is non-increasing in the horizon (variance accumulates),
    /// and strictly below 1 once volatility is positive and time has elapsed.
    #[test]
    fn convexity_adjustment_decreases_with_horizon() {
        let adj = ou_integral_convexity_adjustments(0.05, 0.01, 60);
        for w in adj.windows(2) {
            assert!(w[1] <= w[0] + 1e-15, "adjustment must be non-increasing");
        }
        assert!(*adj.last().unwrap() < 1.0);
    }
}
