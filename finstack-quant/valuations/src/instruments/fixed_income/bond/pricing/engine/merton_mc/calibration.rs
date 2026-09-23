//! Low-path MC calibration loop with common random numbers.

use super::{CalibrationParameter, MertonMcCalibrationSpec, MertonMcConfig, PikMode, PikSchedule};
use crate::cashflow::builder::specs::CouponType;
use crate::instruments::fixed_income::bond::pricing::quote_conversions::{
    price_from_japanese_simple_yield, price_from_ytm, price_from_ytw, price_from_z_spread,
    BondQuoteInput,
};
use crate::instruments::fixed_income::bond::pricing::settlement::QuoteDateContext;
use crate::instruments::fixed_income::bond::types::Bond;
use crate::instruments::fixed_income::bond::CashflowSpec;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::{InputError, Result};
use finstack_quant_models::credit::{AssetDynamics, BarrierType, MertonModel};

/// Output from MC calibration.
#[derive(Debug, Clone)]
pub struct MertonMcCalibrationOutput {
    /// Merton model with the calibrated parameter.
    pub calibrated_merton: MertonModel,
    /// PV at `as_of` produced by the calibration (low-path).
    pub calibrated_pv: f64,
    /// PV at `as_of` implied by the market quote (target).
    pub target_pv: f64,
    /// PV residual = calibrated_pv - target_pv.
    pub residual_pv: f64,
    /// Number of bisection iterations used.
    pub iterations: usize,
    /// Value of the calibrated parameter (barrier or asset vol).
    pub solved_parameter: f64,
}

/// Create a cash-equivalent bond for calibration.
///
/// # Arguments
///
/// * `bond` - Source bond to clone. Fixed and step-up PIK coupons are
///   converted to cash coupons on the clone; the caller's bond is never
///   mutated and floating coupon specifications are rejected.
pub fn cash_equivalent_bond(bond: &Bond) -> Result<Bond> {
    fn cashify_spec(spec: &CashflowSpec) -> Result<CashflowSpec> {
        Ok(match spec {
            CashflowSpec::Fixed(fixed) => {
                let mut f = fixed.clone();
                f.coupon_type = CouponType::Cash;
                CashflowSpec::Fixed(f)
            }
            CashflowSpec::Floating(_) => return Err(InputError::Invalid.into()),
            CashflowSpec::StepUp(s) => {
                let mut s = s.clone();
                s.coupon_type = CouponType::Cash;
                CashflowSpec::StepUp(s)
            }
            CashflowSpec::Amortizing { base, schedule } => CashflowSpec::Amortizing {
                base: Box::new(cashify_spec(base.as_ref())?),
                schedule: schedule.clone(),
            },
        })
    }

    let mut b = bond.clone();
    b.cashflow_spec = cashify_spec(&b.cashflow_spec)?;
    Ok(b)
}

fn with_parameter(
    base: &MertonModel,
    parameter: CalibrationParameter,
    x: f64,
) -> Result<MertonModel> {
    let barrier_type: BarrierType = *base.barrier_type();
    let dynamics: AssetDynamics = *base.dynamics();
    let (asset_value, mut asset_vol, mut debt_barrier) =
        (base.asset_value(), base.asset_vol(), base.debt_barrier());

    match parameter {
        CalibrationParameter::DebtBarrier => debt_barrier = x,
        CalibrationParameter::AssetVol => asset_vol = x,
    }

    MertonModel::new_with_dynamics(
        asset_value,
        asset_vol,
        debt_barrier,
        base.risk_free_rate(),
        base.payout_rate(),
        barrier_type,
        dynamics,
    )
}

fn target_pv_from_quote(
    bond: &Bond,
    market: &MarketContext,
    as_of: Date,
    target: &BondQuoteInput,
) -> Result<f64> {
    let quote_ctx = QuoteDateContext::new(bond, market, as_of)?;
    let quote_date = quote_ctx.quote_date;

    let dirty_quote_currency = match *target {
        BondQuoteInput::CleanPricePct(clean_pct) => {
            quote_ctx.dirty_from_clean_pct(clean_pct, bond.notional.amount())
        }
        BondQuoteInput::DirtyPriceCurrency(dirty_currency) => dirty_currency,
        BondQuoteInput::JapaneseSimpleYield(simple_yield) => {
            price_from_japanese_simple_yield(bond, quote_date, simple_yield)?
        }
        BondQuoteInput::Ytm(ytm) => {
            let flows = bond.pricing_dated_cashflows(market, as_of)?;
            price_from_ytm(bond, &flows, quote_date, ytm)?
        }
        BondQuoteInput::Ytw(ytw) => price_from_ytw(bond, market, as_of, ytw)?,
        // `price_from_z_spread` derives the settlement origin internally,
        // so it takes the valuation `as_of` (not the pre-computed quote_date).
        BondQuoteInput::ZSpread(z) => price_from_z_spread(bond, market, as_of, z)?,
        BondQuoteInput::DiscountMargin(_)
        | BondQuoteInput::Oas(_)
        | BondQuoteInput::AswMarket(_)
        | BondQuoteInput::ISpread(_) => return Err(InputError::Invalid.into()),
    };

    let disc = market.get_discount(&bond.discount_curve_id)?;
    let df_settle = if quote_date > as_of {
        disc.df_between_dates(as_of, quote_date)?
    } else {
        1.0
    };
    Ok(dirty_quote_currency * df_settle)
}

fn mc_cash_pv(
    bond_cash: &Bond,
    as_of: Date,
    discount_rate: f64,
    base_config: &MertonMcConfig,
    low_paths: usize,
    seed_override: Option<u64>,
    merton: MertonModel,
) -> Result<f64> {
    let cash_schedule = PikSchedule::Stepped(vec![(0.0, PikMode::Cash)]);

    let mut cfg = base_config.clone();
    cfg.merton = merton;
    cfg.num_paths = low_paths;
    cfg.pik_schedule = cash_schedule;
    cfg.calibration = None;
    if let Some(seed) = seed_override {
        cfg.seed = seed;
    }

    let result = bond_cash.price_merton_mc(&cfg, discount_rate, as_of)?;
    Ok(result.dirty_price_pct / 100.0 * bond_cash.notional.amount())
}

/// Calibrate a structural parameter to a market quote using the same MC engine.
///
/// Uses bisection with common random numbers (deterministic per-path RNG streams)
/// by reusing the same seed and simulation settings across iterations.
///
/// # Arguments
///
/// * `bond` - Bond whose cash-equivalent contractual flows are repriced
///   under each candidate Merton parameter.
/// * `market` - Valuation market context used to convert `spec.target` into
///   a currency present-value target.
/// * `as_of` - Valuation date at which both the market target and simulated
///   cash-bond present values are compared.
/// * `discount_rate` - Annual discount rate passed to the Merton Monte
///   Carlo cash-bond pricer for every bisection evaluation.
/// * `base_config` - Simulation settings and baseline Merton model; the
///   selected calibration parameter is varied on cloned configurations.
/// * `spec` - Target quote, parameter choice, bracket, tolerance, seed,
///   and low-path bisection controls for the calibration.
pub fn calibrate_parameter_to_market(
    bond: &Bond,
    market: &MarketContext,
    as_of: Date,
    discount_rate: f64,
    base_config: &MertonMcConfig,
    spec: &MertonMcCalibrationSpec,
) -> Result<MertonMcCalibrationOutput> {
    let bond_cash = cash_equivalent_bond(bond)?;
    let target_pv = target_pv_from_quote(&bond_cash, market, as_of, &spec.target)?;

    let base_merton = &base_config.merton;
    let asset_value = base_merton.asset_value();
    if asset_value <= 0.0 {
        return Err(InputError::NonPositiveValue.into());
    }

    let (mut lo, mut hi) = spec.bracket.unwrap_or(match spec.parameter {
        CalibrationParameter::DebtBarrier => (0.001 * asset_value, 0.999 * asset_value),
        CalibrationParameter::AssetVol => (0.01, 2.0),
    });
    if !(lo.is_finite() && hi.is_finite() && lo > 0.0 && hi > lo) {
        return Err(InputError::Invalid.into());
    }

    let eval = |x: f64| -> Result<(f64, f64)> {
        let m = with_parameter(base_merton, spec.parameter, x)?;
        let pv = mc_cash_pv(
            &bond_cash,
            as_of,
            discount_rate,
            base_config,
            spec.low_paths.max(2),
            spec.seed,
            m,
        )?;
        Ok((pv, pv - target_pv))
    };

    let (pv_lo, mut f_lo) = eval(lo)?;
    let (pv_hi, f_hi) = eval(hi)?;
    if f_lo == 0.0 {
        return Ok(MertonMcCalibrationOutput {
            calibrated_merton: with_parameter(base_merton, spec.parameter, lo)?,
            calibrated_pv: pv_lo,
            target_pv,
            residual_pv: 0.0,
            iterations: 0,
            solved_parameter: lo,
        });
    }
    if f_hi == 0.0 {
        return Ok(MertonMcCalibrationOutput {
            calibrated_merton: with_parameter(base_merton, spec.parameter, hi)?,
            calibrated_pv: pv_hi,
            target_pv,
            residual_pv: 0.0,
            iterations: 0,
            solved_parameter: hi,
        });
    }

    if f_lo.signum() == f_hi.signum() {
        return Err(InputError::SolverConvergenceFailed {
            iterations: 0,
            residual: f_hi.abs().min(f_lo.abs()),
            last_x: hi,
            reason: format!(
                "Calibration bracket does not straddle root: f(lo)={f_lo:.6e}, f(hi)={f_hi:.6e}"
            ),
        }
        .into());
    }

    let mut iterations = 0usize;
    let mut mid = 0.5 * (lo + hi);
    let mut pv_mid = 0.0;
    let mut f_mid = 0.0;
    let mut converged = false;

    for i in 0..spec.max_iter.max(1) {
        iterations = i + 1;
        mid = 0.5 * (lo + hi);
        let (pv, f) = eval(mid)?;
        pv_mid = pv;
        f_mid = f;

        if f_mid.abs() <= spec.tolerance_pv.max(0.0) {
            converged = true;
            break;
        }

        if f_lo.signum() == f_mid.signum() {
            lo = mid;
            f_lo = f_mid;
        } else {
            hi = mid;
        }
    }

    if !converged {
        return Err(InputError::SolverConvergenceFailed {
            iterations,
            residual: f_mid.abs(),
            last_x: mid,
            reason: format!(
                "Merton MC calibration did not meet tolerance_pv = {} after {} \
                 iterations: residual_pv = {f_mid:.6e}",
                spec.tolerance_pv, iterations
            ),
        }
        .into());
    }

    let calibrated_merton = with_parameter(base_merton, spec.parameter, mid)?;
    Ok(MertonMcCalibrationOutput {
        calibrated_merton,
        calibrated_pv: pv_mid,
        target_pv,
        residual_pv: f_mid,
        iterations,
        solved_parameter: mid,
    })
}
