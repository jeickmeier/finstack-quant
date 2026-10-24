//! Bond price, yield, spread, duration, and risk metric calculations.
//!
use crate::bond::pricing::quote_conversions::{
    asset_swap_forward_components, asw_leg_schedule, fixed_leg_annuity,
    floating_leg_pv_and_annuity, par_rate_and_annuity_from_discount, settlement_asw_components,
};
use crate::bond::pricing::settlement::QuoteDateContext;
use crate::bond::Bond;
use crate::bond::CashflowSpec;
use finstack_quant_cashflows::builder::CashFlowSchedule;
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::dates::{
    calendar_by_id_strict, BusinessDayConvention, Date, DayCount, StubKind, Tenor,
};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext, MetricId};
use rust_decimal::prelude::ToPrimitive;

/// Asset swap par spread calculator using discount-curve annuity approximation.
///
/// Par ASW is the spread such that the PV of fixed coupons at `(df * (1 + s*α))`
/// equals par. Uses the closed-form approximation: `asw_par ≈ coupon - par_swap_rate`.
///
/// # Dependencies
///
/// Requires `Ytm` metric to be computed first (for par rate calculation).
///
/// # Examples
///
/// ```text
/// use finstack_quant_valuations_fixed_income::bond::Bond;
/// use finstack_quant_valuations_core::metrics::{MetricRegistry, MetricId, MetricContext};
/// use finstack_quant_core::market_data::context::MarketContext;
/// use finstack_quant_core::dates::Date;
///
/// # let bond = Bond::example().unwrap();
/// # let market = MarketContext::new();
/// # let as_of = Date::from_calendar_date(2024, time::Month::January, 15).unwrap();
/// // Par ASW is computed automatically when requesting bond metrics
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Default)]
pub(crate) struct AssetSwapParCalculator;

/// Asset swap market spread calculator using market price.
///
/// Market ASW is the spread that equates the asset-swap package to par. Uses the
/// par-par formula:
/// ```text
/// asw_mkt = [(coupon - par_rate)·Ann_fixed + (1 - dirty/Notional)] / Ann_float
/// ```
/// where both the running coupon-vs-par term (weighted by the **fixed-leg**
/// annuity) and the upfront are amortized over the **floating-leg** annuity
/// (par-par derivation). In the discount-ratio fallback (no forward curve),
/// the floating leg is proxied on the fixed-leg schedule with the discount
/// curve's day count.
///
/// # Dependencies
///
/// Requires `Accrued` metric to be computed first (for dirty price calculation).
///
/// # Examples
///
/// ```text
/// use finstack_quant_valuations_fixed_income::bond::Bond;
/// use finstack_quant_valuations_core::metrics::{MetricRegistry, MetricId, MetricContext};
/// use finstack_quant_core::market_data::context::MarketContext;
/// use finstack_quant_core::dates::Date;
///
/// # let bond = Bond::example().unwrap();
/// # let market = MarketContext::new();
/// # let as_of = Date::from_calendar_date(2024, time::Month::January, 15).unwrap();
/// // Market ASW is computed automatically when requesting bond metrics
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Default)]
pub(crate) struct AssetSwapMarketCalculator;

fn build_future_dates_from_flows(
    flows: &[(
        finstack_quant_core::dates::Date,
        finstack_quant_core::money::Money,
    )],
    as_of: finstack_quant_core::dates::Date,
) -> Vec<finstack_quant_core::dates::Date> {
    use finstack_quant_core::dates::Date;
    use std::collections::BTreeSet;
    let mut set: BTreeSet<Date> = BTreeSet::new();
    for (d, _amt) in flows {
        if *d > as_of {
            set.insert(*d);
        }
    }
    let mut dates: Vec<Date> = Vec::with_capacity(set.len() + 1);
    dates.push(as_of);
    dates.extend(set);
    dates
}

/// Asset-swap effective date for a workout: the last contractual coupon date
/// (rolled forward from issue) strictly before `quote_date`, capped at
/// `quote_date`.
fn asw_effective_date(
    issue_date: Date,
    quote_date: Date,
    end: Date,
    frequency: Tenor,
) -> finstack_quant_core::Result<Date> {
    let mut previous = issue_date;
    let mut current = issue_date;
    while current < end {
        let next = frequency.add_to_date(current, None, BusinessDayConvention::Unadjusted)?;
        if next >= quote_date {
            break;
        }
        previous = next;
        current = next;
    }
    Ok(previous.min(quote_date))
}

/// Floating-leg day count and roll frequency for the workout asset swap:
/// SOFR pays annual ACT/360, EURIBOR-6M semi-annual ACT/360, and any other
/// index follows its forward curve's day count and tenor.
fn asw_float_leg_conventions(fwd: &ForwardCurve) -> finstack_quant_core::Result<(DayCount, Tenor)> {
    let id = fwd.id().as_str().to_ascii_uppercase();
    if id.contains("SOFRRATE") || id.contains("SOFR") {
        Ok((DayCount::Act360, Tenor::annual()))
    } else if id.contains("EURIBOR-6M") || id.contains("EURIBOR6M") {
        Ok((DayCount::Act360, Tenor::semi_annual()))
    } else {
        Ok((
            fwd.day_count(),
            Tenor::from_years(fwd.tenor(), fwd.day_count())?,
        ))
    }
}

/// PV of coupon-only leg from a custom schedule (excludes amortization and principal).
fn pv_coupon_from_custom_schedule(
    disc: &DiscountCurve,
    schedule: &CashFlowSchedule,
    as_of: Date,
) -> finstack_quant_core::Result<f64> {
    use finstack_quant_core::math::summation::NeumaierAccumulator;

    let mut pv = NeumaierAccumulator::new();
    for cf in schedule.get_flows() {
        if cf.date <= as_of {
            continue;
        }
        match cf.kind {
            CFKind::Fixed | CFKind::Stub => {
                let df = disc.df_on_date(cf.date)?;
                pv.add(cf.amount.amount() * df);
            }
            _ => {}
        }
    }
    Ok(pv.total())
}

/// Forward-based par ASW and its floating-leg annuity, or `None` for a
/// zero-notional bond (whose ASW is zero by definition).
///
/// The fixed leg mirrors the bond's future holder cashflow dates; `calc`
/// names the calling calculation (`"par"` / `"market"`) in error messages.
fn forward_par_asw(
    bond: &Bond,
    market: &MarketContext,
    as_of: Date,
    forward_curve_id: &str,
    float_spread_bp: f64,
    fixed_leg_day_count: Option<DayCount>,
    calc: &str,
) -> finstack_quant_core::Result<Option<(f64, f64)>> {
    let disc = market.get_discount(&bond.discount_curve_id)?;
    let fwd = market.get_forward(forward_curve_id)?;

    // Trivial: zero-notional instruments have zero ASW by definition.
    // Return early to avoid validating schedule construction on a degenerate notionals.
    if bond.notional.amount().abs() < 1e-12 {
        return Ok(None);
    }

    // Mirror the bond schedule via holder flows
    let flows = bond.pricing_dated_cashflows(market, as_of)?;
    let sched = build_future_dates_from_flows(&flows, as_of);
    if sched.len() < 2 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "ASW forward {calc} calculation requires at least two fixed-leg schedule dates"
        )));
    }

    let df_as_of = disc.df_on_date(as_of)?;
    let fixed_day_count = fixed_leg_day_count.unwrap_or_else(|| bond.cashflow_spec.day_count());
    let fixed_frequency = Some(bond.cashflow_spec.frequency());
    let ann =
        fixed_leg_annuity(disc.as_ref(), fixed_day_count, fixed_frequency, &sched)? / df_as_of;
    // Use epsilon check to avoid unstable ratios when annuity is degenerate.
    if ann.abs() < 1e-12 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "ASW forward {calc} calculation is undefined for near-zero fixed-leg annuity"
        )));
    }
    let (float_pv, fixed_ann, float_ann) = asset_swap_forward_components(
        disc.as_ref(),
        fwd.as_ref(),
        fixed_day_count,
        fixed_frequency,
        &sched,
        float_spread_bp,
    )?;
    let (float_pv, fixed_ann, float_ann) = (
        float_pv / df_as_of,
        fixed_ann / df_as_of,
        float_ann / df_as_of,
    );
    if float_ann.abs() < 1e-12 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "ASW forward {calc} calculation is undefined for near-zero floating-leg annuity"
        )));
    }

    // Equivalent fixed rate from coupon-only PV
    let eq_coupon = if let Some(custom) = &bond.custom_cashflows {
        let pv_coupon = pv_coupon_from_custom_schedule(disc.as_ref(), custom, as_of)? / df_as_of;
        pv_coupon / (bond.notional.amount() * ann)
    } else {
        bond.cashflow_spec.plain_fixed_rate()?.ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "asset swap spread requires a plain fixed coupon rate".to_string(),
            )
        })?
    };
    Ok(Some((
        (eq_coupon * fixed_ann - float_pv) / float_ann,
        float_ann,
    )))
}

/// Compute Par ASW using a forward-based methodology.
///
/// When `fixed_leg_day_count` is `Some`, that day-count builds the fixed-leg
/// annuity so callers can align with swap-market conventions (e.g. 30E/360).
/// `None` uses `bond.cashflow_spec.day_count()`.
///
/// # Arguments
///
/// * `bond` - Bond whose future fixed cashflows and contractual discount-curve
///   identifier define the asset-swap fixed leg.
/// * `market` - Market context providing the bond's discount curve and the
///   named forward curve.
/// * `as_of` - Valuation date used to select future bond cashflows and build
///   the mirrored fixed-leg schedule.
/// * `forward_curve_id` - Market-context identifier of the floating-leg forward
///   curve.
/// * `float_spread_bp` - Floating-leg contractual spread in basis points,
///   added to projected forward coupons.
/// * `fixed_leg_day_count` - Optional swap fixed-leg day-count convention for
///   the annuity. `None` uses the bond's coupon day count.
pub fn asw_par_with_forward(
    bond: &Bond,
    market: &MarketContext,
    as_of: Date,
    forward_curve_id: &str,
    float_spread_bp: f64,
    fixed_leg_day_count: Option<DayCount>,
) -> finstack_quant_core::Result<f64> {
    Ok(forward_par_asw(
        bond,
        market,
        as_of,
        forward_curve_id,
        float_spread_bp,
        fixed_leg_day_count,
        "par",
    )?
    .map_or(0.0, |(par_asw, _)| par_asw))
}

/// Compute Market ASW using a forward-based methodology.
///
/// Market ASW is the forward par ASW plus the upfront `(1 − dirty/notional)`
/// amortized over the floating-leg annuity. `dirty_price_currency` is
/// required: `None` returns `InputError::NotFound { id: "dirty_price_currency" }`
/// instead of assuming par. Pass `Some(bond.notional.amount())` for a par
/// market price. When `fixed_leg_day_count` is `Some`, that day-count builds
/// the fixed-leg annuity; `None` uses the bond's coupon day-count.
///
/// # Arguments
///
/// * `bond` - Bond whose future fixed cashflows and contractual discount-curve
///   identifier define the asset-swap fixed leg.
/// * `market` - Market context providing the bond's discount curve and the
///   named forward curve.
/// * `as_of` - Valuation date used to select future bond cashflows and build
///   the mirrored fixed-leg schedule.
/// * `forward_curve_id` - Market-context identifier of the floating-leg forward
///   curve.
/// * `float_spread_bp` - Floating-leg contractual spread in basis points,
///   added to projected forward coupons.
/// * `dirty_price_currency` - Dirty market price in the bond currency. `None`
///   returns `InputError::NotFound` instead of silently assuming par.
/// * `fixed_leg_day_count` - Optional swap fixed-leg day-count convention for
///   the annuity. `None` uses the bond's coupon day count.
pub fn asw_market_with_forward(
    bond: &Bond,
    market: &MarketContext,
    as_of: Date,
    forward_curve_id: &str,
    float_spread_bp: f64,
    dirty_price_currency: Option<f64>,
    fixed_leg_day_count: Option<DayCount>,
) -> finstack_quant_core::Result<f64> {
    let Some((par_asw, float_ann)) = forward_par_asw(
        bond,
        market,
        as_of,
        forward_curve_id,
        float_spread_bp,
        fixed_leg_day_count,
        "market",
    )?
    else {
        return Ok(0.0);
    };
    let Some(dirty) = dirty_price_currency else {
        return Err(finstack_quant_core::InputError::NotFound {
            id: "dirty_price_currency".to_string(),
        }
        .into());
    };
    let price_pct = dirty / bond.notional.amount();
    Ok(par_asw + (1.0 - price_pct) / float_ann)
}

impl MetricCalculator for AssetSwapParCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let bond: &Bond = context.instrument_as()?;

        // If the bond has custom cashflows, compute ASW using a forward-based
        // custom-swap constructed on the same schedule. Requires a float spec.
        if bond.custom_cashflows.is_some() {
            match &bond.cashflow_spec {
                CashflowSpec::Floating(spec) => {
                    return asw_par_with_forward(
                        bond,
                        &context.curves,
                        context.as_of,
                        spec.rate_spec.forward_curve_id.as_str(),
                        spec.rate_spec.spread_bp.to_f64().unwrap_or_default(),
                        None,
                    );
                }
                _ => {
                    return Err(finstack_quant_core::InputError::NotFound {
                        id: "bond.cashflow_spec.floating".to_string(),
                    }
                    .into());
                }
            }
        }

        let discount_curve_id = bond.discount_curve_id.to_owned();
        let maturity = bond.maturity;
        let bond_day_count = bond.cashflow_spec.day_count();
        let asw_forward_curve_id = bond
            .instrument_pricing_overrides
            .model_config
            .asw_forward_curve_id
            .clone();
        let disc = context.curves.get_discount(&discount_curve_id)?;

        // The ASW fixed leg follows the bond's coupon conventions.
        let params = bond.cashflow_spec.schedule();
        let frequency = params.frequency;

        // Market standard: Par swap rate via discount ratio on the ASW fixed-leg
        // schedule, matching the bond schedule.
        if context.as_of >= maturity {
            return Err(finstack_quant_core::Error::Validation(
                "ASW par calculation requires at least two fixed-leg schedule dates".to_string(),
            ));
        }
        let quote_ctx = QuoteDateContext::new(bond, &context.curves, context.as_of)?;
        let sched = asw_leg_schedule(quote_ctx.quote_date, maturity, frequency, params.stub, None)?;
        if sched.len() < 2 {
            return Err(finstack_quant_core::Error::Validation(
                "ASW par calculation requires at least two fixed-leg schedule dates".to_string(),
            ));
        }
        let dc_fixed = bond_day_count;
        let forward_components = if let Some(fwd_id) = asw_forward_curve_id {
            let fwd = context.curves.get_forward(&fwd_id)?;
            Some(asset_swap_forward_components(
                disc.as_ref(),
                fwd.as_ref(),
                dc_fixed,
                Some(frequency),
                &sched,
                0.0,
            )?)
        } else {
            None
        };
        let ann = if let Some((_, _, float_ann)) = forward_components {
            float_ann
        } else {
            par_rate_and_annuity_from_discount(disc.as_ref(), dc_fixed, Some(frequency), &sched)?.1
        };
        if ann.abs() < 1e-12 {
            return Err(finstack_quant_core::Error::Validation(
                "ASW par calculation is undefined for near-zero spread annuity".to_string(),
            ));
        }
        // Use stated coupon for non-custom bonds; for custom bonds, this branch is not reached
        let coupon = bond.cashflow_spec.plain_fixed_rate()?.ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "asset swap spread requires a plain fixed coupon rate".to_string(),
            )
        })?;
        if let Some((float_pv, fixed_ann, float_ann)) = forward_components {
            Ok((coupon * fixed_ann - float_pv) / float_ann)
        } else {
            let (par_rate, _) = par_rate_and_annuity_from_discount(
                disc.as_ref(),
                dc_fixed,
                Some(frequency),
                &sched,
            )?;
            Ok(coupon - par_rate)
        }
    }
}

impl MetricCalculator for AssetSwapMarketCalculator {
    fn dependencies(&self) -> &[MetricId] {
        &[MetricId::Accrued]
    }

    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let workout = crate::bond::metrics::context_workout_path(context)?;
        let (
            discount_curve_id,
            maturity,
            day_count,
            notional_amt,
            quoted_clean_price_pct,
            is_custom,
            coupon,
            asw_forward_curve_id,
        ) = {
            let b: &Bond = context.instrument_as()?;
            let coupon_rate = b.cashflow_spec.plain_fixed_rate()?.unwrap_or(0.0);
            (
                b.discount_curve_id.to_owned(),
                b.maturity,
                b.cashflow_spec.day_count(),
                b.notional.amount(),
                b.instrument_pricing_overrides
                    .market_quotes
                    .quoted_clean_price_pct,
                b.custom_cashflows.is_some(),
                coupon_rate,
                b.instrument_pricing_overrides
                    .model_config
                    .asw_forward_curve_id
                    .clone(),
            )
        };
        let disc = context.curves.get_discount(&discount_curve_id)?;

        // Dirty market value in currency
        let dirty_currency = if let Some(clean_px) = quoted_clean_price_pct {
            let accrued = context.computed_metric(&MetricId::Accrued)?;
            clean_px * notional_amt / 100.0 + accrued
        } else {
            context.base_value.amount()
        };

        // If the bond has custom cashflows, compute forward-based ASW using the
        // bond's float spec on the same (custom) schedule. Requires a float spec.
        if is_custom {
            let bond: &Bond = context.instrument_as()?;
            match &bond.cashflow_spec {
                CashflowSpec::Floating(spec) => {
                    return asw_market_with_forward(
                        bond,
                        &context.curves,
                        context.as_of,
                        spec.rate_spec.forward_curve_id.as_str(),
                        spec.rate_spec.spread_bp.to_f64().unwrap_or_default(),
                        Some(dirty_currency),
                        None,
                    );
                }
                _ => {
                    return Err(finstack_quant_core::InputError::NotFound {
                        id: "bond.cashflow_spec.floating".to_string(),
                    }
                    .into());
                }
            }
        }

        // Market standard: discount-ratio using the bond's fixed-leg schedule.
        let bond: &Bond = context.instrument_as()?;
        let params = bond.cashflow_spec.schedule();
        let frequency = params.frequency;
        let quote_ctx = QuoteDateContext::new(bond, &context.curves, context.as_of)?;
        if let Some(clean_px) = quoted_clean_price_pct {
            if let Some(workout) = workout {
                let workout_maturity = workout
                    .flows
                    .last()
                    .map(|(date, _)| *date)
                    .unwrap_or(maturity);
                if workout_maturity < maturity {
                    let effective_date = asw_effective_date(
                        bond.issue_date,
                        quote_ctx.quote_date,
                        workout_maturity,
                        frequency,
                    )?;
                    let fixed_schedule = asw_leg_schedule(
                        effective_date,
                        workout_maturity,
                        frequency,
                        StubKind::ShortBack,
                        Some((
                            params.business_day_convention,
                            calendar_by_id_strict(&params.calendar_id)?,
                        )),
                    )?;
                    if fixed_schedule.len() < 2 {
                        return Err(finstack_quant_core::Error::Validation(
                            "ASW market calculation requires at least two fixed-leg schedule dates"
                                .to_string(),
                        ));
                    }
                    let dc_fixed = day_count;
                    let df_settlement = disc.df_on_date(quote_ctx.quote_date)?;
                    if let Some(fwd_id) = asw_forward_curve_id.as_ref() {
                        let fwd = context.curves.get_forward(fwd_id)?;
                        let (float_day_count, float_frequency) = asw_float_leg_conventions(&fwd)?;
                        let float_schedule = asw_leg_schedule(
                            effective_date,
                            workout_maturity,
                            float_frequency,
                            StubKind::ShortBack,
                            None,
                        )?;
                        // Seasoned workout swap: the floating leg has its own
                        // conventions and schedule, and fixes started periods.
                        let fixed_ann = fixed_leg_annuity(
                            disc.as_ref(),
                            dc_fixed,
                            Some(frequency),
                            &fixed_schedule,
                        )? / df_settlement;
                        let (float_pv, float_ann) = floating_leg_pv_and_annuity(
                            disc.as_ref(),
                            fwd.as_ref(),
                            float_day_count,
                            &float_schedule,
                            0.0,
                            Some((context.curves.as_ref(), context.as_of)),
                        )?;
                        let (float_pv, float_ann) =
                            (float_pv / df_settlement, float_ann / df_settlement);
                        if float_ann.abs() < 1e-12 {
                            return Err(finstack_quant_core::Error::Validation(
                                "ASW market calculation is undefined for near-zero floating-leg annuity"
                                    .to_string(),
                            ));
                        }
                        return Ok(
                            (coupon * fixed_ann + 1.0 - clean_px / 100.0 - float_pv) / float_ann
                        );
                    } else {
                        let (par_rate, ann) = par_rate_and_annuity_from_discount(
                            disc.as_ref(),
                            dc_fixed,
                            Some(frequency),
                            &fixed_schedule,
                        )?;
                        let ann = ann / df_settlement;
                        if ann.abs() < 1e-12 {
                            return Err(finstack_quant_core::Error::Validation(
                                "ASW market calculation is undefined for near-zero fixed-leg annuity"
                                    .to_string(),
                            ));
                        }
                        // Upfront is amortized over the floating-leg annuity
                        // (par-par derivation), proxied on the same schedule
                        // with the discount curve's day count.
                        let float_ann = fixed_leg_annuity(
                            disc.as_ref(),
                            disc.day_count(),
                            None,
                            &fixed_schedule,
                        )? / df_settlement;
                        if float_ann.abs() < 1e-12 {
                            return Err(finstack_quant_core::Error::Validation(
                                "ASW market calculation is undefined for near-zero floating-leg annuity"
                                    .to_string(),
                            ));
                        }
                        return Ok(
                            ((coupon - par_rate) * ann + (1.0 - clean_px / 100.0)) / float_ann
                        );
                    }
                }
            }
        }
        let (coupon_pv, float_pv, float_annuity) =
            settlement_asw_components(bond, &context.curves, quote_ctx.quote_date)?;
        let dirty_at_settlement = super::quote_date_dirty_target(bond, context, &quote_ctx)?;
        Ok((coupon_pv + 1.0 - dirty_at_settlement / notional_amt - float_pv) / float_annuity)
    }
}
