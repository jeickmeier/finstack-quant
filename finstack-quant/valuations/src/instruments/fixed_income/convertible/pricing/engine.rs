//! Convertible pricing pipeline and public pricing helpers.

use finstack_quant_core::dates::{adjust, BusinessDayConvention, Date, DateExt, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::PriceId;
use finstack_quant_core::InputError;
use finstack_quant_core::{Error, Result};

use crate::cashflow::builder::CashFlowSchedule;
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::fixed_income::convertible::{
    market_inputs::{resolve_dividend_yield, volatility_candidate_ids},
    ConversionEvent, ConversionPolicy, ConvertibleBond,
};
use crate::metrics::bump_discount_curve_parallel;
use finstack_quant_models::{single_factor_equity_state, TreeGreeks};

use super::tsiveriotis_zhang::TsiveriotisZhangEngine;
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
/// [`DividendAdjustment`](crate::instruments::fixed_income::convertible::DividendAdjustment))
/// is 1.0 at `t = 0` and is applied per tree step inside the pricer, not here.
pub(crate) fn compute_conversion_value(bond: &ConvertibleBond, spot: f64) -> Result<f64> {
    match &bond.conversion.policy {
        ConversionPolicy::MandatoryVariable {
            upper_conversion_price,
            lower_conversion_price,
            ..
        } => {
            if *lower_conversion_price <= 0.0 || *upper_conversion_price <= 0.0 {
                return Err(Error::Validation(format!(
                    "Conversion prices must be positive: lower={}, upper={}",
                    lower_conversion_price, upper_conversion_price
                )));
            }
            // Reject inverted bounds. Without this guard the three-regime payoff
            // below collapses degenerately (no `lower < spot <= upper` regime
            // can fire) and produces NaN-adjacent values that propagate
            // silently into PV. Data-entry inversion at trade capture is the
            // most likely source.
            if *lower_conversion_price > *upper_conversion_price {
                return Err(Error::Validation(format!(
                    "MandatoryVariable conversion bounds inverted: lower={lower_conversion_price} \
                     must be <= upper={upper_conversion_price}"
                )));
            }
            let face = bond.notional.amount();
            if spot <= *lower_conversion_price {
                Ok((face / lower_conversion_price) * spot)
            } else if spot <= *upper_conversion_price {
                Ok(face)
            } else {
                Ok((face / upper_conversion_price) * spot)
            }
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

/// Tree model type selection for convertible bond pricing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConvertibleTreeType {
    /// Use binomial tree (CRR)
    Binomial(usize), // number of steps
    /// Use trinomial tree
    Trinomial(usize), // number of steps
}

impl Default for ConvertibleTreeType {
    fn default() -> Self {
        Self::Binomial(200)
    }
}

/// Resolved market data identifiers for Greek bumping.
struct ResolvedIds {
    spot_id: PriceId,
}

/// Extracted equity market state.
struct EquityState {
    spot: f64,
    spot_scalar: MarketScalar,
    volatility: f64,
    dividend_yield: f64,
    risk_free_rate: f64,
    time_to_maturity: f64,
    resolved_ids: ResolvedIds,
}

/// Extract equity market state from market context.
///
/// Uses **Act/365F** for all process/option time calculations (time_to_maturity,
/// vol surface lookup, drift estimation). This is deliberately decoupled from
/// the bond's coupon accrual day count, which can be 30/360 or other conventions.
fn extract_equity_state(
    bond: &ConvertibleBond,
    ctx: &MarketContext,
    as_of: Date,
) -> Result<EquityState> {
    let underlying_id = bond
        .underlying_equity_id
        .as_deref()
        .ok_or_else(|| Error::internal("convertible pricing requires underlying equity spot"))?;

    // Get spot price, preserving the original scalar variant for type-safe bumping
    let spot_scalar = ctx.get_price(underlying_id)?.clone();
    let spot = match &spot_scalar {
        MarketScalar::Price(money) => {
            if money.currency() != bond.notional.currency() {
                return Err(Error::CurrencyMismatch {
                    expected: bond.notional.currency(),
                    actual: money.currency(),
                });
            }
            money.amount()
        }
        MarketScalar::Unitless(value) => *value,
    };

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
        resolve_volatility(
            ctx,
            &volatility_candidate_ids(bond)?,
            time_to_maturity,
            strike,
        )?
    };

    let dividend_yield = resolve_dividend_yield(ctx, bond)?;

    let resolved_ids = ResolvedIds {
        spot_id: underlying_id.into(),
    };

    Ok(EquityState {
        spot,
        spot_scalar,
        volatility,
        dividend_yield,
        risk_free_rate,
        time_to_maturity,
        resolved_ids,
    })
}

/// Resolve the equity volatility at the contractual conversion strike.
fn resolve_volatility(
    ctx: &MarketContext,
    candidate_ids: &[String],
    time_to_maturity: f64,
    strike: f64,
) -> Result<f64> {
    let mut first_missing: Option<String> = None;

    for id in candidate_ids {
        match ctx.get_price(id) {
            Ok(MarketScalar::Unitless(vol)) => {
                return Ok(*vol);
            }
            Ok(_) => {}
            Err(err) => {
                if matches!(err, Error::Input(InputError::NotFound { .. })) {
                    if first_missing.is_none() {
                        first_missing = Some(id.clone());
                    }
                } else {
                    return Err(err);
                }
            }
        }

        match ctx.get_surface(id) {
            Ok(surface) => {
                let vol = finstack_quant_models::volatility::get_surface_vol_clamped(
                    &surface,
                    time_to_maturity,
                    strike,
                );
                return Ok(vol);
            }
            Err(err) => {
                if matches!(err, Error::Input(InputError::NotFound { .. })) {
                    if first_missing.is_none() {
                        first_missing = Some(id.clone());
                    }
                    continue;
                }
                return Err(err);
            }
        }
    }

    let missing_id = first_missing.unwrap_or_else(|| "volatility".to_string());
    Err(Error::from(InputError::NotFound { id: missing_id }))
}

/// Aggregated data required for tree pricing
pub(super) struct PricingInputs {
    pub(super) cashflow_schedule: CashFlowSchedule,
    spot: f64,
    pub(super) volatility: f64,
    dividend_yield: f64,
    risk_free_rate: f64,
    pub(super) time_to_maturity: f64,
    resolved_ids: ResolvedIds,
    /// Original spot scalar from market context, preserved for type-safe bumping.
    spot_scalar: MarketScalar,
}

/// Prepare all necessary inputs for pricing and greeks calculation.
pub(super) fn prepare_for_pricing(
    bond: &ConvertibleBond,
    market_context: &MarketContext,
    as_of: Date,
) -> Result<PricingInputs> {
    let cashflow_schedule = build_convertible_schedule(bond, market_context)?;
    let eq = extract_equity_state(bond, market_context, as_of)?;

    Ok(PricingInputs {
        cashflow_schedule,
        spot: eq.spot,
        volatility: eq.volatility,
        dividend_yield: eq.dividend_yield,
        risk_free_rate: eq.risk_free_rate,
        time_to_maturity: eq.time_to_maturity,
        resolved_ids: eq.resolved_ids,
        spot_scalar: eq.spot_scalar,
    })
}

/// Internal pricing function that reuses pre-computed `PricingInputs`.
///
/// Avoids redundant `prepare_for_pricing` when the caller already has the inputs
/// (e.g., `calculate_convertible_greeks` for the base price).
fn price_convertible_bond_with_inputs(
    bond: &ConvertibleBond,
    market_context: &MarketContext,
    inputs: &PricingInputs,
    tree_type: ConvertibleTreeType,
    as_of: Date,
) -> Result<Money> {
    if as_of > bond.maturity {
        return Ok(Money::from((0_i64, bond.notional.currency())));
    }

    if inputs.time_to_maturity <= 0.0 {
        let maturity_coupon: f64 = inputs
            .cashflow_schedule
            .coupons()
            .filter(|cf| cf.date == bond.maturity)
            .map(|cf| cf.amount.amount())
            .sum();

        let redemption_value = bond.notional.amount() + maturity_coupon;
        let conversion_value = compute_conversion_value(bond, inputs.spot)?;

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
            ConversionPolicy::UponEvent(ConversionEvent::PriceTrigger { threshold, .. }) => {
                inputs.spot >= *threshold
            }
            ConversionPolicy::UponEvent(
                ConversionEvent::QualifiedIpo | ConversionEvent::ChangeOfControl,
            ) => false,
        };
        let payoff = if is_mandatory && can_convert {
            conversion_value
        } else if can_convert {
            redemption_value.max(conversion_value)
        } else {
            redemption_value
        };

        return Money::new(payoff, bond.notional.currency());
    }

    let steps = match tree_type {
        ConvertibleTreeType::Binomial(n) | ConvertibleTreeType::Trinomial(n) => n,
    };

    let valuator = ConvertibleBondValuator::new(
        bond,
        &inputs.cashflow_schedule,
        inputs.time_to_maturity,
        steps,
        as_of,
        market_context,
        inputs.volatility,
    )?;

    let initial_vars = single_factor_equity_state(
        inputs.spot,
        inputs.risk_free_rate,
        inputs.dividend_yield,
        inputs.volatility,
    );

    let engine = TsiveriotisZhangEngine {
        valuator: &valuator,
        steps,
        time_to_maturity: inputs.time_to_maturity,
    };

    let (pv_amount, _) = engine.price(initial_vars, tree_type)?;

    Money::new(pv_amount, bond.notional.currency())
}

/// Main pricing function for convertible bonds
///
/// # Arguments
///
/// * `bond` - Convertible bond contract with cashflows, conversion terms, and
///   required market-data identifiers.
/// * `market_context` - Market context supplying discount curve, equity spot,
///   volatility, credit, and other pricing inputs.
/// * `tree_type` - Recombining tree specification controlling the convertible
///   equity/credit valuation discretization.
/// * `as_of` - Valuation date; dates after maturity return zero in the bond's
///   notional currency.
pub fn price_convertible_bond(
    bond: &ConvertibleBond,
    market_context: &MarketContext,
    tree_type: ConvertibleTreeType,
    as_of: Date,
) -> Result<Money> {
    bond.validate_for_pricing()?;
    validate_tree_type(tree_type)?;
    if as_of > bond.maturity {
        return Ok(Money::from((0_i64, bond.notional.currency())));
    }
    let inputs = prepare_for_pricing(bond, market_context, as_of)?;
    price_convertible_bond_with_inputs(bond, market_context, &inputs, tree_type, as_of)
}

/// Value the straight cash component on the canonical convertible tree grid.
/// Uses the same coupon mapping and per-step recovery blend as the main engine,
/// excluding conversion and issuer/holder exercise rights.
pub(crate) fn price_bond_floor(
    bond: &ConvertibleBond,
    market_context: &MarketContext,
    as_of: Date,
) -> Result<f64> {
    if as_of >= bond.maturity {
        return Ok(0.0);
    }
    let inputs = prepare_for_pricing(bond, market_context, as_of)?;
    let steps = match ConvertibleTreeType::default() {
        ConvertibleTreeType::Binomial(steps) | ConvertibleTreeType::Trinomial(steps) => steps,
    };
    let valuator = ConvertibleBondValuator::new(
        bond,
        &inputs.cashflow_schedule,
        inputs.time_to_maturity,
        steps,
        as_of,
        market_context,
        inputs.volatility,
    )?;
    let mut discount = 1.0;
    let mut value = valuator.coupon_map.get(&0).copied().unwrap_or(0.0);
    for (step, step_discount) in valuator.risky_step_dfs.iter().enumerate() {
        discount *= step_discount;
        value += valuator.coupon_map.get(&(step + 1)).copied().unwrap_or(0.0) * discount;
    }
    Ok(value + bond.notional.amount() * discount)
}

/// Calculate Greeks for a convertible bond using central finite differences.
///
/// All Greeks use full repricing with bumped market contexts to ensure consistency
/// with the full term structure discounting (M1). Each bump correctly propagates
/// through the entire pricing pipeline including per-step discount factor extraction.
///
/// # Greek Definitions
///
/// - **Delta**: `(P(S+h) - P(S-h)) / (2h)` where `h = bump_pct * S`
/// - **Gamma**: `(P(S+h) - 2*P(S) + P(S-h)) / h^2`
/// - **Vega**: `(P(σ+0.01) - P(σ-0.01)) / (vol_up - vol_down) * 0.01` — per 1% absolute vol move
///   Uses a forward difference when the lower quote is outside the selected
///   lattice's admissible volatility range.
/// - **Rho**: `(P(r+1bp) - P(r-1bp)) / 2` — per 1bp parallel shift of the
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
/// * `market_context` - Market context supplying baseline curves, equity spot,
///   volatility, and credit data for full repricing.
/// * `tree_type` - Recombining tree specification used consistently for every
///   bumped valuation.
/// * `bump_size` - Optional relative equity-spot bump as a decimal; `None`
///   uses `0.01` (one percent) for delta and gamma.
/// * `as_of` - Valuation date from which the one-day theta roll is measured.
pub fn calculate_convertible_greeks(
    bond: &ConvertibleBond,
    market_context: &MarketContext,
    tree_type: ConvertibleTreeType,
    bump_size: Option<f64>,
    as_of: Date,
) -> Result<TreeGreeks> {
    bond.validate_for_pricing()?;
    validate_tree_type(tree_type)?;
    let bump_pct = bump_size.unwrap_or(0.01);
    if !bump_pct.is_finite() || bump_pct <= 0.0 {
        return Err(Error::Validation(format!(
            "convertible Greek spot bump must be finite and positive, got {bump_pct}"
        )));
    }

    // Resolve market data and compute base price in one pass.
    // The base price is computed inline to avoid a second prepare_for_pricing call
    // (which would duplicate cashflow schedule build and market data resolution).
    let inputs = prepare_for_pricing(bond, market_context, as_of)?;
    let base_price =
        price_convertible_bond_with_inputs(bond, market_context, &inputs, tree_type, as_of)?;

    let mut greeks = TreeGreeks {
        price: base_price.amount(),
        delta: 0.0,
        gamma: 0.0,
        vega: 0.0,
        theta: 0.0,
        rho: 0.0,
        oas01: 0.0,
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

        let market_up = market_context.clone().insert_price(
            inputs.resolved_ids.spot_id.as_str(),
            bump_scalar(inputs.spot + h_spot)?,
        );
        let market_down = market_context.clone().insert_price(
            inputs.resolved_ids.spot_id.as_str(),
            bump_scalar(inputs.spot - h_spot)?,
        );

        let price_up = price_convertible_bond(bond, &market_up, tree_type, as_of)?.amount();
        let price_down = price_convertible_bond(bond, &market_down, tree_type, as_of)?.amount();

        greeks.delta = (price_up - price_down) / (2.0 * h_spot);
        greeks.gamma = (price_up - 2.0 * greeks.price + price_down) / (h_spot * h_spot);
    }

    // ---- Vega: bump volatility (B1: central differences) ----
    {
        let h_vol = 0.01; // 1% absolute
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
        let price_vol_up =
            price_convertible_bond(&bond_up, market_context, tree_type, as_of)?.amount();
        let price_vol_down =
            match price_convertible_bond(&bond_down, market_context, tree_type, as_of) {
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
        let h_rate = 1.0; // 1bp in bp-count units (BumpSpec::parallel_bp convention)
        let market_rate_up =
            bump_discount_curve_parallel(market_context, &bond.discount_curve_id, h_rate)?;
        let market_rate_down =
            bump_discount_curve_parallel(market_context, &bond.discount_curve_id, -h_rate)?;

        let price_rate_up =
            price_convertible_bond(bond, &market_rate_up, tree_type, as_of)?.amount();
        let price_rate_down =
            price_convertible_bond(bond, &market_rate_down, tree_type, as_of)?.amount();

        // Rho per 1bp: central difference
        greeks.rho = (price_rate_up - price_rate_down) / 2.0;
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
                let rolled_market = market_context.roll_forward(1)?;
                let fwd_price = price_convertible_bond(bond, &rolled_market, tree_type, next_day)?;
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
    if let Some(fixed_spec) = &bond.fixed_coupon {
        let _ = builder.fixed_cf(fixed_spec.clone());
    }
    if let Some(floating_spec) = &bond.floating_coupon {
        let _ = builder.floating_cf(floating_spec.clone());
    }
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

/// Calculate conversion premium
///
/// # Arguments
///
/// * `bond_price` - Observed or model convertible price in the same units as
///   the conversion value.
/// * `current_spot` - Current conversion-share price in the same quote units.
/// * `conversion_ratio` - Shares received per bond for conversion.
pub fn calculate_conversion_premium(
    bond_price: f64,
    current_spot: f64,
    conversion_ratio: f64,
) -> f64 {
    let conversion_value = current_spot * conversion_ratio;
    if conversion_value > 0.0 {
        (bond_price / conversion_value) - 1.0
    } else {
        0.0
    }
}

/// Compute the settlement date for a convertible bond.
///
/// If `settlement_days` is set, advances on the coupon schedule's holiday
/// calendar and applies its business-day convention. Zero-coupon bonds use the
/// canonical weekends-only calendar with Following adjustment. Otherwise
/// returns `as_of` unchanged.
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
    let (calendar_id, convention) = bond
        .fixed_coupon
        .as_ref()
        .map(|coupon| {
            (
                coupon.schedule.calendar_id.as_str(),
                coupon.schedule.business_day_convention,
            )
        })
        .or_else(|| {
            bond.floating_coupon.as_ref().map(|coupon| {
                (
                    coupon.schedule.calendar_id.as_str(),
                    coupon.schedule.business_day_convention,
                )
            })
        })
        .unwrap_or((
            crate::cashflow::builder::calendar::WEEKENDS_ONLY_ID,
            BusinessDayConvention::Following,
        ));
    let calendar = crate::cashflow::builder::calendar::resolve_calendar_strict(calendar_id)?;
    let advanced = as_of.add_business_days(days, calendar)?;
    adjust(advanced, convention, calendar)
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
/// * `market_context` - Forward curves and realized index fixings used to
///   determine floating coupon amounts; unused for fixed coupons.
/// * `as_of` - Trade or valuation date from which the bond settlement date is
///   calculated.
pub fn calculate_accrued_interest(
    bond: &ConvertibleBond,
    market_context: &MarketContext,
    as_of: Date,
) -> Result<f64> {
    bond.validate_for_pricing()?;
    if bond.fixed_coupon.is_none() && bond.floating_coupon.is_none() {
        return Ok(0.0); // Zero-coupon
    }

    let settle = settlement_date(bond, as_of)?;

    let schedule = build_convertible_schedule(bond, market_context)?;
    accrued_interest_at(bond, &schedule, settle)
}

/// Coupon accrual at the actual exercise date, without applying settlement lag.
pub(super) fn accrued_interest_at(
    bond: &ConvertibleBond,
    schedule: &CashFlowSchedule,
    date: Date,
) -> Result<f64> {
    let frequency = bond
        .fixed_coupon
        .as_ref()
        .map(|c| c.schedule.frequency)
        .or_else(|| bond.floating_coupon.as_ref().map(|c| c.schedule.frequency));
    crate::cashflow::accrual::accrued_interest_amount(
        schedule,
        date,
        &crate::cashflow::accrual::AccrualConfig {
            method: crate::cashflow::accrual::AccrualMethod::Linear,
            ex_coupon: None,
            include_pik: true,
            frequency,
        },
    )
}

fn validate_tree_type(tree_type: ConvertibleTreeType) -> Result<()> {
    let steps = match tree_type {
        ConvertibleTreeType::Binomial(steps) | ConvertibleTreeType::Trinomial(steps) => steps,
    };
    if steps == 0 {
        return Err(Error::Validation(
            "convertible tree must contain at least one time step".to_string(),
        ));
    }
    Ok(())
}
