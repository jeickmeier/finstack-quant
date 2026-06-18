//! Option-adjusted spread (OAS) for structured-credit tranches.
//!
//! Solves for the constant spread `s` such that the Monte-Carlo average tranche
//! PV — discounted at each scenario's discount factors times `exp(-s·t)` —
//! equals a quoted market price. Either stochastic dimension can be enabled:
//!
//! - **Stochastic rates**: a Hull-White 1-factor short-rate path per scenario
//!   drives both rate-dependent prepayment and the discount factors.
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
/// * `deal`, `tranche_id` - the tranche to price.
/// * `market_price_pct` - quoted price as a percentage of original balance.
/// * `market`, `as_of` - market context and valuation date.
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
    let num_paths = if stochastic {
        config.num_paths.max(1)
    } else {
        1
    };
    let rng = PhiloxRng::new(config.seed);

    // For each scenario, the per-cashflow `(year fraction t, CF·base_df)`. The
    // trial OAS only multiplies in `exp(-s·t)`, so the expensive simulation runs
    // once per scenario and the Brent solve over `s` is cheap.
    let mut scenarios: Vec<Vec<(f64, f64)>> = Vec::with_capacity(num_paths);

    for path in 0..num_paths {
        let rate_path = if config.stochastic_rates {
            let mut sub = rng.substream(2 * path as u64);
            Some(simulate_hw1f_path(
                base_rate,
                config.hw_kappa,
                config.hw_sigma,
                num_months,
                &mut sub,
            ))
        } else {
            None
        };
        let credit_z = if config.stochastic_credit {
            let mut sub = rng.substream(2 * path as u64 + 1);
            Some(sub.next_std_normal())
        } else {
            None
        };

        let mut source = OasPathFlowSource::new(
            as_of,
            rate_path.clone(),
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
            let base_df = match &rate_path {
                Some(rp) => path_discount_factor(rp, as_of.months_until(*date) as usize),
                None => disc.df_between_dates(as_of, *date)?,
            };
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
        let var = path_pvs
            .iter()
            .map(|pv| (pv - mean_pv).powi(2))
            .sum::<f64>()
            / path_count;
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

/// Simulate a monthly Hull-White 1-factor short-rate path mean-reverting to
/// `base_rate` (a flat `θ`). Exact Ornstein-Uhlenbeck discretization.
fn simulate_hw1f_path(
    base_rate: f64,
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
    let mut rates = Vec::with_capacity(num_months + 1);
    let mut r = base_rate;
    rates.push(r);
    for _ in 0..num_months {
        let z = rng.next_std_normal();
        r = r * exp_k + base_rate * (1.0 - exp_k) + vol * z;
        rates.push(r);
    }
    rates
}

/// Cumulative discount factor to `month` along a monthly short-rate path:
/// `exp(-Δt · Σ_{m<month} r_m)`.
fn path_discount_factor(rate_path: &[f64], month: usize) -> f64 {
    let dt = 1.0 / 12.0;
    let last = month.min(rate_path.len());
    let acc: f64 = rate_path[..last].iter().map(|r| -r * dt).sum();
    acc.exp()
}
