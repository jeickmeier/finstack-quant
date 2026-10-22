//! Convertible pricing pipeline and public pricing helpers.

use finstack_quant_core::dates::{adjust, Date, DateExt, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::surfaces::{VolQuoteType, VolSurfaceAxis};
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::PriceId;
use finstack_quant_core::InputError;
use finstack_quant_core::{Error, Result};

use crate::convertible::{
    market_inputs::resolve_dividend_yield, ConvertibleBond, ConvertibleGreeks,
};
use finstack_quant_cashflows::builder::CashFlowSchedule;
use finstack_quant_valuations_core::instruments::{ConversionEvent, ConversionPolicy};
use finstack_quant_valuations_core::instruments::{GreekBumps, Instrument};
use finstack_quant_valuations_core::metrics::bump_discount_curve_parallel;

use super::credit_tree::{terminal_payoff, CreditTreeEngine, CreditTreeMarketInputs};
use super::valuator::ConvertibleBondValuator;

/// Compute the conversion value for any conversion policy given the spot price.
///
/// This standalone function handles all `ConversionPolicy` variants, including
/// `MandatoryVariable` with its three-regime variable delivery ratio. Used by both
/// the tree terminal/interior nodes and the at-maturity early-exit path.
///
/// For standard policies: `conversion_ratio * spot`.
/// For `MandatoryVariable`:
///   - `spot <= lower_price`: `(face / lower_price) * spot` (max shares, loss)
///   - `lower < spot <= upper`: `face` (variable ratio delivers par)
///   - `spot > upper_price`: `(face / upper_price) * spot` (min shares, capped)
///
/// This is the **instantaneous** conversion value at the valuation date:
/// dividend-protection ratio accretion (`exp(q * t)`, see
/// [`DividendAdjustment`](finstack_quant_valuations_core::instruments::DividendAdjustment))
/// is 1.0 at `t = 0` and is applied per tree step inside the pricer, not here.
pub(crate) fn compute_conversion_value(bond: &ConvertibleBond, spot: f64) -> Result<f64> {
    match &bond.conversion.policy {
        ConversionPolicy::MandatoryVariable {
            upper_conversion_price,
            lower_conversion_price,
            ..
        } => {
            validate_mandatory_variable_bounds(*lower_conversion_price, *upper_conversion_price)?;
            Ok(mandatory_variable_value(
                bond.notional.amount(),
                *lower_conversion_price,
                *upper_conversion_price,
                spot,
            ))
        }
        _ => {
            let conversion_ratio =
                bond.effective_conversion_ratio()
                    .ok_or(Error::Input(InputError::NotFound {
                        id: "conversion_ratio_or_price".to_string(),
                    }))?;
            Ok(spot * conversion_ratio)
        }
    }
}

/// Reject non-positive or inverted mandatory-variable conversion prices.
///
/// Without the inversion guard the three-regime payoff of
/// [`mandatory_variable_value`] collapses degenerately (no
/// `lower < spot <= upper` regime can fire) and produces NaN-adjacent values
/// that propagate silently into PV. Data-entry inversion at trade capture is
/// the most likely source.
///
/// # Errors
///
/// Returns a validation error when either price is not positive or
/// `lower > upper`.
pub(super) fn validate_mandatory_variable_bounds(lower: f64, upper: f64) -> Result<()> {
    if lower <= 0.0 || upper <= 0.0 {
        return Err(Error::Validation(format!(
            "Conversion prices must be positive: lower={}, upper={}",
            lower, upper
        )));
    }
    if lower > upper {
        return Err(Error::Validation(format!(
            "MandatoryVariable conversion bounds inverted: lower={lower} \
             must be <= upper={upper}"
        )));
    }
    Ok(())
}

/// Mandatory-variable conversion value of `face` at `spot`: maximum shares
/// `face / lower` below `lower`, par between the bounds, minimum shares
/// `face / upper` above `upper`.
pub(super) fn mandatory_variable_value(face: f64, lower: f64, upper: f64, spot: f64) -> f64 {
    if spot <= lower {
        (face / lower) * spot
    } else if spot <= upper {
        face
    } else {
        (face / upper) * spot
    }
}

/// Survival from `start` to `end` conditional on survival to `start`,
/// `S(end) / S(start)`; `1.0` without a hazard curve.
///
/// # Errors
///
/// Returns a validation error when `S(start)` is not positive, or when the
/// curve cannot evaluate either date.
pub(super) fn conditional_survival(
    hazard: Option<&finstack_quant_core::market_data::term_structures::HazardCurve>,
    start: Date,
    end: Date,
) -> Result<f64> {
    let Some(curve) = hazard else {
        return Ok(1.0);
    };
    let sp = curve.survival_at_dates(&[start, end])?;
    if sp[0] <= 0.0 {
        return Err(Error::Validation(
            "convertible requires positive survival".into(),
        ));
    }
    Ok(sp[1] / sp[0])
}

/// Default lattice step count when
/// `instrument_pricing_overrides.model_config.tree_steps` is unset.
pub const DEFAULT_CONVERTIBLE_TREE_STEPS: usize = 200;

/// Lattice step count for `bond`: `model_config.tree_steps`, else
/// [`DEFAULT_CONVERTIBLE_TREE_STEPS`].
fn tree_steps(bond: &ConvertibleBond) -> Result<usize> {
    let steps = bond
        .instrument_pricing_overrides
        .model_config
        .tree_steps
        .unwrap_or(DEFAULT_CONVERTIBLE_TREE_STEPS);
    if steps == 0 {
        return Err(Error::Validation(
            "instrument_pricing_overrides.model_config.tree_steps must be at least 1 for a \
             convertible tree"
                .to_string(),
        ));
    }
    Ok(steps)
}

/// Prepare all necessary inputs for pricing and greeks calculation: the
/// cashflow schedule and the equity market state.
///
/// Uses **Act/365F** for all process/option time calculations (time_to_maturity,
/// vol surface lookup, drift estimation). This is deliberately decoupled from
/// the bond's coupon accrual day count, which can be 30/360 or other conventions.
pub(super) fn prepare_for_pricing(
    bond: &ConvertibleBond,
    ctx: &MarketContext,
    as_of: Date,
) -> Result<PricingInputs> {
    let cashflow_schedule = build_convertible_schedule(bond, ctx)?;

    // Get spot price, preserving the original scalar variant for type-safe bumping
    let spot_scalar = ctx.get_price(&bond.spot_id)?.clone();
    let spot = bond.spot_value(&spot_scalar)?;

    let discount_curve = ctx.get_discount(bond.discount_curve_id.as_str())?;
    // Use Act/365F for process time (tree steps, vol lookups, curve DF queries).
    // This is standard for equity option models and ensures consistency with
    // discount curve time axis (which defaults to Act/365F).
    let process_day_count = DayCount::Act365F;
    let time_to_maturity = process_day_count.year_fraction(
        as_of,
        bond.maturity,
        finstack_quant_core::dates::DayCountContext::default(),
    )?;

    // Short rate (instantaneous forward at t=0). The tree's per-step drift is
    // derived from the per-step forward discount factors inside the engine;
    // this rate only seeds the base evolution parameters (u/d factors and
    // their construction-time validity checks).
    //
    // Approximated as -ln(DF(epsilon))/epsilon with epsilon = 1/252 (~1 day).
    // Falls back to zero rate to maturity when TTM is very short.
    let risk_free_rate = if time_to_maturity > 0.0 {
        let next_day = as_of + time::Duration::days(1);
        let epsilon = process_day_count.year_fraction(
            as_of,
            next_day,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        let df_short = discount_curve.df_between_dates(as_of, next_day)?;
        if df_short > 0.0 {
            -df_short.ln() / epsilon
        } else {
            0.0
        }
    } else {
        0.0
    };

    let volatility = if let Some(volatility) = bond
        .instrument_pricing_overrides
        .market_quotes
        .implied_volatility
    {
        volatility
    } else {
        let ratio = bond.effective_conversion_ratio().ok_or_else(|| {
            Error::Validation("convertible volatility requires a valid conversion ratio".into())
        })?;
        let strike = bond.notional.amount() / ratio;
        resolve_volatility(ctx, bond.vol_surface_id.as_str(), time_to_maturity, strike)?
    };

    let dividend_yield = resolve_dividend_yield(ctx, bond)?;

    Ok(PricingInputs {
        cashflow_schedule,
        spot,
        volatility,
        dividend_yield,
        risk_free_rate,
        time_to_maturity,
        spot_id: bond.spot_id.clone(),
        spot_scalar,
    })
}

/// Resolve the equity volatility at the contractual conversion strike.
///
/// `vol_surface_id` names either a volatility surface (read at the conversion
/// strike and maturity) or a unitless scalar holding a flat volatility.
fn resolve_volatility(
    ctx: &MarketContext,
    vol_surface_id: &str,
    time_to_maturity: f64,
    strike: f64,
) -> Result<f64> {
    match ctx.get_price(vol_surface_id) {
        Ok(MarketScalar::Unitless(vol)) => return Ok(*vol),
        Ok(MarketScalar::Price(_)) => {
            return Err(Error::Validation(format!(
                "ConvertibleBond vol_surface_id '{vol_surface_id}' scalar must be unitless"
            )));
        }
        Err(Error::Input(InputError::NotFound { .. })) => {}
        Err(err) => return Err(err),
    }
    let surface = ctx.get_surface(vol_surface_id)?;
    surface.require_quote_type(VolQuoteType::BlackLognormal)?;
    surface.require_secondary_axis(VolSurfaceAxis::Strike)?;
    Ok(finstack_quant_models::volatility::get_surface_vol_clamped(
        &surface,
        time_to_maturity,
        strike,
    ))
}

/// Aggregated data required for tree pricing
pub(super) struct PricingInputs {
    pub(super) cashflow_schedule: CashFlowSchedule,
    spot: f64,
    pub(super) volatility: f64,
    dividend_yield: f64,
    risk_free_rate: f64,
    pub(super) time_to_maturity: f64,
    /// Market identifier of the equity spot, for Greek bumping.
    spot_id: PriceId,
    /// Original spot scalar from market context, preserved for type-safe bumping.
    spot_scalar: MarketScalar,
}

/// Resolve an already-exercised contract without recreating its debt or options.
fn resolved_value(
    bond: &ConvertibleBond,
    market: &MarketContext,
    as_of: Date,
) -> Result<Option<Money>> {
    if let Some(claim) = &bond.resolved_settlement {
        if as_of < claim.exercise_date {
            return Err(Error::Validation(
                "resolved convertible settlement cannot be valued before its exercise date".into(),
            ));
        }
        if as_of >= claim.delivery_date {
            return Ok(Some(Money::new(0.0, bond.notional.currency())?));
        }
        let discount = market.get_discount(bond.discount_curve_id.as_str())?;
        let df = discount.df_between_dates(as_of, claim.delivery_date)?;
        let hazard = bond
            .credit_curve_id
            .as_ref()
            .map(|id| market.get_hazard(id.as_str()))
            .transpose()?;
        let survival = conditional_survival(hazard.as_deref(), as_of, claim.delivery_date)?;
        // Pending fixed cash has its own contractual principal; the extinguished
        // bond notional must never be recovered again.
        let recovery = bond.recovery_rate.unwrap_or(0.0)
            * finstack_quant_valuations_core::instruments::pricing::rates_credit::continuous_frp_weight(
                df, survival,
            )?;
        let shares_value = if claim.shares > 0.0 {
            let spot = bond.spot_value(market.get_price(&bond.spot_id)?)?;
            let tau =
                DayCount::Act365F.year_fraction(as_of, claim.delivery_date, Default::default())?;
            claim.shares * spot * (-resolve_dividend_yield(market, bond)? * tau).exp()
        } else {
            0.0
        };
        return Ok(Some(Money::new(
            shares_value
                + claim.cash.amount() * df * survival
                + claim.recovery_basis.amount() * recovery,
            bond.notional.currency(),
        )?));
    }
    let mandatory_date = match bond.conversion.policy {
        ConversionPolicy::MandatoryOn(date) => Some(date),
        ConversionPolicy::MandatoryVariable {
            conversion_date, ..
        } => Some(conversion_date),
        _ => None,
    };
    let ended = mandatory_date
        .filter(|date| *date < as_of)
        .or_else(|| (as_of > bond.maturity).then_some(bond.maturity));
    let Some(ended) = ended else {
        return Ok(None);
    };
    // A same-day conversion/redemption has already settled. Retain only earned
    // coupons whose contractual accrual ended before exercise but pay later.
    let schedule = build_convertible_schedule(bond, market)?;
    let discount = market.get_discount(bond.discount_curve_id.as_str())?;
    let hazard = bond
        .credit_curve_id
        .as_ref()
        .map(|id| market.get_hazard(id.as_str()))
        .transpose()?;
    let mut value = 0.0;
    for cf in schedule
        .get_flows()
        .iter()
        .filter(|cf| cf.date > as_of && cf.kind.is_cash_settlement())
    {
        let earned_coupon = cf
            .accrual
            .as_ref()
            .is_some_and(|period| period.end <= ended)
            && !cf.kind.is_principal_like();
        let pending_redemption = mandatory_date.is_none() && cf.kind.is_principal_like();
        if earned_coupon || pending_redemption {
            let survival = conditional_survival(hazard.as_deref(), as_of, cf.date)?;
            let df = discount.df_between_dates(as_of, cf.date)?;
            let recovery = if pending_redemption {
                bond.recovery_rate.unwrap_or(0.0)
                    * finstack_quant_valuations_core::instruments::pricing::rates_credit::continuous_frp_weight(
                        df, survival,
                    )?
            } else {
                0.0
            };
            value += cf.amount.amount() * (df * survival + recovery);
        }
    }
    Ok(Some(Money::new(value, bond.notional.currency())?))
}

/// Internal pricing function that reuses pre-computed `PricingInputs`.
///
/// Avoids redundant `prepare_for_pricing` when the caller already has the inputs
/// (e.g., `calculate_convertible_greeks` for the base price).
fn price_convertible_bond_with_inputs(
    bond: &ConvertibleBond,
    market_context: &MarketContext,
    inputs: &PricingInputs,
    as_of: Date,
) -> Result<Money> {
    if let Some(value) = resolved_value(bond, market_context, as_of)? {
        return Ok(value);
    }

    if inputs.time_to_maturity <= 0.0 {
        let discount = market_context.get_discount(bond.discount_curve_id.as_str())?;
        let hazard = bond
            .credit_curve_id
            .as_ref()
            .map(|id| market_context.get_hazard(id.as_str()))
            .transpose()?;
        let mut maturity_coupon = 0.0;
        let mut redemption_pv = 0.0;
        let mut principal = 0.0;
        for cf in inputs
            .cashflow_schedule
            .get_flows()
            .iter()
            .filter(|cf| cf.date >= as_of && cf.kind.is_cash_settlement())
        {
            let survival = conditional_survival(hazard.as_deref(), as_of, cf.date)?;
            let df = discount.df_between_dates(as_of, cf.date)?;
            if cf.kind.is_principal_like() {
                principal += cf.amount.amount();
                redemption_pv += cf.amount.amount() * (df * survival + bond.recovery_rate.unwrap_or(0.0)
                    * finstack_quant_valuations_core::instruments::pricing::rates_credit::continuous_frp_weight(df, survival)?);
            } else {
                maturity_coupon += cf.amount.amount() * df * survival;
            }
        }
        let redemption_value = bond
            .call_put
            .iter()
            .flat_map(|schedule| &schedule.puts)
            .filter(|put| put.start <= as_of && as_of <= put.end)
            .map(|put| principal * put.price_pct_of_par / 100.0)
            .fold(redemption_pv, f64::max);
        let conversion_value =
            compute_conversion_value(bond, inputs.spot)? * principal / bond.notional.amount();

        let is_mandatory = matches!(
            bond.conversion.policy,
            ConversionPolicy::MandatoryOn(_) | ConversionPolicy::MandatoryVariable { .. }
        );
        let can_convert = match &bond.conversion.policy {
            ConversionPolicy::Voluntary => true,
            ConversionPolicy::MandatoryOn(date) => *date == bond.maturity,
            ConversionPolicy::MandatoryVariable {
                conversion_date, ..
            } => *conversion_date == bond.maturity,
            ConversionPolicy::Window { start, end } => {
                *start <= bond.maturity && bond.maturity <= *end
            }
            ConversionPolicy::UponEvent(ConversionEvent::PriceTrigger(trigger)) => {
                let conversion_price = bond
                    .effective_conversion_ratio()
                    .filter(|ratio| *ratio > 0.0)
                    .map_or(0.0, |ratio| bond.notional.amount() / ratio);
                inputs.spot >= trigger.level(conversion_price)
            }
            ConversionPolicy::UponEvent(
                ConversionEvent::QualifiedIpo | ConversionEvent::ChangeOfControl,
            ) => false,
        };
        let (payoff, _) = terminal_payoff(
            conversion_value,
            redemption_value,
            maturity_coupon,
            can_convert,
            is_mandatory,
            0.0,
        );

        return Money::new(payoff, bond.notional.currency());
    }

    let steps = tree_steps(bond)?;

    let valuator = ConvertibleBondValuator::new(
        bond,
        &inputs.cashflow_schedule,
        inputs.time_to_maturity,
        steps,
        as_of,
        market_context,
        inputs.volatility,
    )?;

    let engine = CreditTreeEngine {
        valuator: &valuator,
        steps,
        time_to_maturity: inputs.time_to_maturity,
    };

    let (pv_amount, _) = engine.price(CreditTreeMarketInputs {
        spot: inputs.spot,
        volatility: inputs.volatility,
        risk_free_rate: inputs.risk_free_rate,
        dividend_yield: inputs.dividend_yield,
    })?;

    Money::new(pv_amount, bond.notional.currency())
}

/// Main pricing function for convertible bonds
///
/// # Arguments
///
/// * `bond` - Convertible bond contract with cashflows, conversion terms, and
///   required market-data identifiers.
/// * `market` - Market context supplying discount curve, equity spot,
///   volatility, credit, and other pricing inputs.
/// * `as_of` - Valuation date. After exercise only recorded delivery claims and
///   earned unpaid coupons remain; settled claims return zero.
///
/// The Cox-Ross-Rubinstein binomial lattice step count is
/// `bond.instrument_pricing_overrides.model_config.tree_steps` (default
/// [`DEFAULT_CONVERTIBLE_TREE_STEPS`]).
pub fn price_convertible_bond(
    bond: &ConvertibleBond,
    market: &MarketContext,
    as_of: Date,
) -> Result<Money> {
    bond.validate_for_pricing()?;
    tree_steps(bond)?;
    if let Some(value) = resolved_value(bond, market, as_of)? {
        return Ok(value);
    }
    let inputs = prepare_for_pricing(bond, market, as_of)?;
    price_convertible_bond_with_inputs(bond, market, &inputs, as_of)
}

/// Value the straight cash component on the canonical convertible tree grid.
/// Uses the same debt-event mapping and explicit principal recovery as the main engine,
/// excluding conversion and issuer/holder exercise rights.
pub(crate) fn price_bond_floor(
    bond: &ConvertibleBond,
    market_context: &MarketContext,
    as_of: Date,
) -> Result<f64> {
    if let Some(value) = resolved_value(bond, market_context, as_of)? {
        return Ok(value.amount());
    }
    if as_of == bond.maturity {
        let schedule = build_convertible_schedule(bond, market_context)?;
        let curve = market_context.get_discount(bond.discount_curve_id.as_str())?;
        return schedule
            .get_flows()
            .iter()
            .filter(|cf| cf.date >= as_of && cf.kind.is_cash_settlement())
            .try_fold(0.0, |value, cf| {
                Ok(value + cf.amount.amount() * curve.df_between_dates(as_of, cf.date)?)
            });
    }
    let inputs = prepare_for_pricing(bond, market_context, as_of)?;
    let steps = tree_steps(bond)?;
    let valuator = ConvertibleBondValuator::new(
        bond,
        &inputs.cashflow_schedule,
        inputs.time_to_maturity,
        steps,
        as_of,
        market_context,
        inputs.volatility,
    )?;
    let payment = |step| {
        valuator.coupon_map.get(&step).copied().unwrap_or(0.0)
            + valuator.principal_map.get(&step).copied().unwrap_or(0.0)
    };
    let mut value = payment(steps);
    for step in (0..steps).rev() {
        value = payment(step)
            + value * valuator.rf_step_dfs[step] * valuator.survival_factors[step]
            + valuator.recovery_by_step[step];
    }
    Ok(value)
}

/// Calculate Greeks for a convertible bond using central finite differences.
///
/// All Greeks use full repricing with bumped market contexts to ensure consistency
/// with the full term structure discounting (M1). Each bump correctly propagates
/// through the entire pricing pipeline including per-step discount factor extraction.
///
/// # Greek Definitions
///
/// - **Delta**: `(P(S+h) - P(S-h)) / (2h)` where `h = spot_bump_decimal * S`
/// - **Gamma**: `(P(S+h) - 2*P(S) + P(S-h)) / h^2`
/// - **Vega**: `(P(σ+δσ) - P(σ-δσ)) / (vol_up - vol_down) * 0.01` with
///   `δσ = vol_bump_decimal` — per 1% absolute vol move
///   Uses a forward difference when the lower quote is outside the selected
///   lattice's admissible volatility range.
/// - **Rho**: `(P(r+δr) - P(r-δr)) / (2·rate_bump_bp)` — per 1bp parallel shift of the
///   **risk-free discount curve only**; a configured credit curve is held
///   fixed (spread implicitly narrows by the bump). Use the DV01 metric
///   (parallel, all curves) for the full parallel-rate sensitivity.
/// - **Theta**: `P(t+1d) - P(t)` — change per calendar day
///
/// # Volatility convention for delta/gamma
///
/// Surface volatility is sampled at the contractual conversion strike. Spot
/// bumps retain that strike, so delta and gamma use sticky-strike volatility.
///
/// # Arguments
///
/// * `bond` - Convertible bond contract with cashflows, conversion terms, and
///   required market-data identifiers.
/// * `market` - Market context supplying baseline curves, equity spot,
///   volatility, and credit data for full repricing. Every bumped valuation
///   uses the same binomial lattice with `model_config.tree_steps` steps
///   (default [`DEFAULT_CONVERTIBLE_TREE_STEPS`]).
/// * `bumps` - Finite-difference bump sizes: relative equity-spot bump for
///   delta and gamma, absolute volatility bump for vega and parallel
///   risk-free rate bump in bp for rho. Each must be finite and positive;
///   [`GreekBumps::default`] gives 1%, 1 vol point and 1bp.
/// * `as_of` - Valuation date from which the one-day theta roll is measured.
pub fn calculate_convertible_greeks(
    bond: &ConvertibleBond,
    market: &MarketContext,
    bumps: GreekBumps,
    as_of: Date,
) -> Result<ConvertibleGreeks> {
    bond.validate_for_pricing()?;
    tree_steps(bond)?;
    for (name, value) in [
        ("spot_bump_decimal", bumps.spot_bump_decimal),
        ("vol_bump_decimal", bumps.vol_bump_decimal),
        ("rate_bump_bp", bumps.rate_bump_bp),
    ] {
        if !value.is_finite() || value <= 0.0 {
            return Err(Error::Validation(format!(
                "convertible Greek {name} must be finite and positive, got {value}"
            )));
        }
    }
    let bump_pct = bumps.spot_bump_decimal;

    // Resolve market data and compute base price in one pass.
    // The base price is computed inline to avoid a second prepare_for_pricing call
    // (which would duplicate cashflow schedule build and market data resolution).
    let inputs = prepare_for_pricing(bond, market, as_of)?;
    let base_price = price_convertible_bond_with_inputs(bond, market, &inputs, as_of)?;

    let mut greeks = ConvertibleGreeks {
        price: base_price.amount(),
        delta: 0.0,
        gamma: 0.0,
        vega: 0.0,
        theta: 0.0,
        rho: 0.0,
    };

    // ---- Delta & Gamma: bump equity spot (central differences) ----
    let h_spot = bump_pct * inputs.spot;
    if h_spot > 0.0 {
        let bump_scalar = |amount: f64| -> finstack_quant_core::Result<MarketScalar> {
            Ok(match &inputs.spot_scalar {
                MarketScalar::Price(money) => MarketScalar::Price(
                    finstack_quant_core::money::Money::new(amount, money.currency())?,
                ),
                MarketScalar::Unitless(_) => MarketScalar::Unitless(amount),
            })
        };

        let market_up = market
            .clone()
            .insert_price(inputs.spot_id.as_str(), bump_scalar(inputs.spot + h_spot)?);
        let market_down = market
            .clone()
            .insert_price(inputs.spot_id.as_str(), bump_scalar(inputs.spot - h_spot)?);

        let price_up = price_convertible_bond(bond, &market_up, as_of)?.amount();
        let price_down = price_convertible_bond(bond, &market_down, as_of)?.amount();

        greeks.delta = (price_up - price_down) / (2.0 * h_spot);
        greeks.gamma = (price_up - 2.0 * greeks.price + price_down) / (h_spot * h_spot);
    }

    // ---- Vega: bump volatility (B1: central differences) ----
    {
        let h_vol = bumps.vol_bump_decimal; // absolute vol
        let mut vol_down = (inputs.volatility - h_vol).max(1e-6);
        let vol_up = inputs.volatility + h_vol;

        let mut bond_up = bond.clone();
        bond_up
            .instrument_pricing_overrides
            .market_quotes
            .implied_volatility = Some(vol_up);
        let mut bond_down = bond.clone();
        bond_down
            .instrument_pricing_overrides
            .market_quotes
            .implied_volatility = Some(vol_down);
        let price_vol_up = price_convertible_bond(&bond_up, market, as_of)?.amount();
        let price_vol_down = match price_convertible_bond(&bond_down, market, as_of) {
            Ok(price) => price.amount(),
            Err(Error::Validation(_)) => {
                // All other inputs already priced successfully. A lower quote
                // can violate the lattice's drift/probability constraint.
                vol_down = inputs.volatility;
                base_price.amount()
            }
            Err(error) => return Err(error),
        };
        let actual_width = vol_up - vol_down;

        // Vega per 1% vol move: central difference with actual bump width.
        // (P_up - P_down) / actual_width gives per-unit-vol sensitivity;
        // multiply by 0.01 to convert to "per 1% absolute vol move" convention.
        // When bumps are symmetric (actual_width == 0.02), this simplifies to
        // (P_up - P_down) / 2.0 as expected.
        greeks.vega = (price_vol_up - price_vol_down) / actual_width * 0.01;
    }

    // ---- Rho: bump discount curve (B2: central differences) ----
    {
        let h_rate = bumps.rate_bump_bp; // bp-count units (BumpSpec::parallel_bp convention)
        let market_rate_up = bump_discount_curve_parallel(market, &bond.discount_curve_id, h_rate)?;
        let market_rate_down =
            bump_discount_curve_parallel(market, &bond.discount_curve_id, -h_rate)?;

        let price_rate_up = price_convertible_bond(bond, &market_rate_up, as_of)?.amount();
        let price_rate_down = price_convertible_bond(bond, &market_rate_down, as_of)?.amount();

        // Rho per 1bp: central difference over the 2·h_rate bp width
        greeks.rho = (price_rate_up - price_rate_down) / (2.0 * h_rate);
    }

    // ---- Theta: 1-day roll (forward difference), reported per calendar day ----
    {
        if inputs.time_to_maturity > 1.0 / 365.25 {
            if let Some(next_day) = as_of.next_day() {
                // `roll_forward(1)` realizes one day of forwards on every
                // curve (discount curves renormalize by DF(1d), hazard
                // curves preserve hazard rates via conditional survival), so
                // theta = price(rolled, t+1d) - price(t) captures both carry
                // and roll-down. See module documentation
                // (realized-forward roll semantics).
                //
                // A roll fails when a curve is too sparse to retain ≥ 2 knots
                // after the shift; that error propagates rather than
                // repricing on a market still anchored at `t`.
                let rolled_market = market.roll_forward(1)?;
                let fwd_price = price_convertible_bond(bond, &rolled_market, next_day)?;
                // Theta = P(t+1d) - P(t), reported as change per calendar day.
                greeks.theta = fwd_price.amount() - greeks.price;
            }
        }
    }

    Ok(greeks)
}

/// Build the convertible bond cashflow schedule using common builder flow.
pub(crate) fn build_convertible_schedule(
    bond: &ConvertibleBond,
    market: &MarketContext,
) -> Result<CashFlowSchedule> {
    let mut builder = CashFlowSchedule::builder();
    let _ = builder.principal(bond.notional, bond.issue_date, bond.maturity);
    bond.cashflow_spec.add_to_builder(&mut builder)?;
    builder.build(Some(market))
}

/// Calculate convertible bond parity
///
/// Returns the instantaneous policy-specific conversion value divided by face
/// amount. Mandatory-variable contracts use their price-dependent delivery
/// ratio. Invalid conversion terms or negative/non-finite spot prices fail.
///
/// # Arguments
///
/// * `bond` - Convertible bond whose effective conversion ratio and notional
///   normalize the equity conversion value.
/// * `current_spot` - Current conversion-share price in the bond's quote
///   currency; finite and nonnegative.
pub fn calculate_parity(bond: &ConvertibleBond, current_spot: f64) -> Result<f64> {
    bond.validate_for_pricing()?;
    if !current_spot.is_finite() || current_spot < 0.0 {
        return Err(Error::Validation(
            "conversion-share price must be finite and nonnegative".into(),
        ));
    }
    Ok(compute_conversion_value(bond, current_spot)? / bond.notional.amount())
}

/// Compute the settlement date for a convertible bond.
///
/// If `settlement_days` is set, advances on the coupon schedule's holiday
/// calendar and applies its business-day convention. Otherwise returns
/// `as_of` unchanged.
///
/// # Arguments
///
/// * `bond` - Convertible bond whose optional business-day settlement lag is
///   applied.
/// * `as_of` - Trade or valuation date from which settlement is rolled.
///
/// # Errors
///
/// Returns an error if `settlement_days` exceeds the supported signed range,
/// the configured calendar is unknown, or business-day adjustment fails.
pub fn settlement_date(bond: &ConvertibleBond, as_of: Date) -> Result<Date> {
    let Some(days) = bond.settlement_days.filter(|days| *days > 0) else {
        return Ok(as_of);
    };
    let days = i32::try_from(days).map_err(|_| {
        Error::Validation(format!(
            "convertible settlement_days {days} exceeds the supported range"
        ))
    })?;
    let schedule = bond.cashflow_spec.schedule();
    let calendar =
        finstack_quant_core::dates::calendar_by_id_strict(schedule.calendar_id.as_str())?;
    let advanced = as_of.add_business_days(days, calendar)?;
    adjust(advanced, schedule.business_day_convention, calendar)
}

/// Forward a valuation-date PV to the buyer's settlement-date dirty price.
/// Excludes contractual cash paid on or before settlement, including principal.
/// Uses the same survival assumptions as pricing for intervening payments.
pub(crate) fn settlement_dirty_price(
    bond: &ConvertibleBond,
    market: &MarketContext,
    as_of: Date,
    present_value: f64,
) -> Result<f64> {
    let settle = settlement_date(bond, as_of)?;
    if settle == as_of {
        return Ok(present_value);
    }
    let discount = market.get_discount(bond.discount_curve_id.as_str())?;
    let hazard = bond
        .credit_curve_id
        .as_ref()
        .map(|id| market.get_hazard(id.as_str()))
        .transpose()?;
    let schedule = build_convertible_schedule(bond, market)?;
    let mut interim = 0.0;
    for cf in schedule
        .get_flows()
        .iter()
        .filter(|cf| cf.date > as_of && cf.date <= settle && cf.kind.is_cash_settlement())
    {
        let survival = conditional_survival(hazard.as_deref(), as_of, cf.date)?;
        interim += cf.amount.amount() * discount.df_between_dates(as_of, cf.date)? * survival;
    }
    Ok((present_value - interim) / discount.df_between_dates(as_of, settle)?)
}

/// Calculate accrued interest for a convertible bond.
///
/// Accrued interest is computed as of the **settlement date** (trade date +
/// `settlement_days` business days). If `settlement_days` is not set, `as_of`
/// is used directly.
///
/// Finds the accrual period containing the settlement date from the cashflow
/// schedule and computes the pro-rata portion of the coupon that has accrued.
///
/// Returns 0.0 for zero-coupon convertibles or if the date is outside all
/// accrual periods.
///
/// # Arguments
///
/// * `bond` - Convertible bond whose coupon schedule and settlement lag define
///   the accrued-interest period.
/// * `market` - Forward curves and realized index fixings used to
///   determine floating coupon amounts; unused for fixed coupons.
/// * `as_of` - Trade or valuation date from which the bond settlement date is
///   calculated.
pub fn calculate_accrued_interest(
    bond: &ConvertibleBond,
    market: &MarketContext,
    as_of: Date,
) -> Result<f64> {
    bond.validate_for_pricing()?;
    let settle = settlement_date(bond, as_of)?;

    let schedule = build_convertible_schedule(bond, market)?;
    accrual_index(bond, &schedule)?.accrued_at(settle)
}

/// Reusable coupon-accrual state for a convertible's schedule.
///
/// Accrual is linear, includes PIK, and has no ex-coupon window; queries do
/// not apply settlement lag.
pub(super) fn accrual_index(
    bond: &ConvertibleBond,
    schedule: &CashFlowSchedule,
) -> Result<finstack_quant_cashflows::accrual::AccrualIndex> {
    let frequency = Some(bond.cashflow_spec.frequency());
    finstack_quant_cashflows::accrual::AccrualIndex::build(
        schedule,
        &finstack_quant_cashflows::accrual::AccrualConfig {
            method: finstack_quant_cashflows::accrual::AccrualMethod::Linear,
            ex_coupon: None,
            include_pik: true,
            frequency,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::market_data::surfaces::VolSurface;

    #[test]
    fn volatility_resolution_rejects_incompatible_surface_contracts() {
        let base = VolSurface::builder("EQUITY-VOL")
            .expiries(&[1.0])
            .strikes(&[100.0])
            .row(&[0.2])
            .build()
            .expect("Black surface");
        let market = MarketContext::new().insert_surface(base.clone());
        assert_eq!(
            resolve_volatility(&market, "EQUITY-VOL", 1.0, 100.0).expect("volatility"),
            0.2
        );

        let incompatible = [
            (
                base.clone()
                    .with_quote_type(VolQuoteType::Normal)
                    .expect("normal surface"),
                "black_lognormal",
            ),
            (
                base.clone()
                    .with_displacements(&[10.0])
                    .expect("shifted surface"),
                "black_lognormal",
            ),
            (
                finstack_quant_core::market_data::surfaces::VolSurface::from_grid_opts(
                    base.id(),
                    base.expiries(),
                    base.strikes(),
                    base.vols(),
                    finstack_quant_core::market_data::surfaces::VolGridOpts {
                        secondary_axis: VolSurfaceAxis::Tenor,
                        ..finstack_quant_core::market_data::surfaces::VolGridOpts::default()
                    },
                    None,
                )
                .expect("tenor surface"),
                "strike",
            ),
        ];
        for (surface, expected) in incompatible {
            let invalid_market = market.clone().insert_surface(surface);
            let error = resolve_volatility(&invalid_market, "EQUITY-VOL", 1.0, 100.0)
                .expect_err("convertible tree requires unshifted Black strike surfaces");
            let message = error.to_string();
            assert!(message.contains("EQUITY-VOL"), "{message}");
            assert!(message.contains(expected), "{message}");
        }
    }
}
