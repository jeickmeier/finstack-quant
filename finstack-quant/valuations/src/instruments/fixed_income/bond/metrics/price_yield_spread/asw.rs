//! Bond price, yield, spread, duration, and risk metric calculations.
//!
use crate::cashflow::{builder::CashFlowSchedule, primitives::CFKind};
use crate::instruments::fixed_income::bond::pricing::quote_conversions::{
    asset_swap_forward_components, asset_swap_projection_rate, fixed_leg_annuity,
    par_rate_and_annuity_from_discount,
};
use crate::instruments::fixed_income::bond::pricing::settlement::QuoteDateContext;
use crate::instruments::fixed_income::bond::CashflowSpec;
use crate::instruments::Bond;
use crate::metrics::{MetricCalculator, MetricContext, MetricId};
use finstack_quant_core::dates::{
    BusinessDayConvention, Date, DayCount, DayCountContext, StubKind, Tenor,
};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::types::CurveId;
use rust_decimal::prelude::ToPrimitive;

/// Forward curve for the asset-swap floating leg: the explicit
/// `model_config.asw_forward_curve_id` override, else the bond's own
/// `forward_curve_id`. `None` selects the discount-ratio fallback.
pub(crate) fn resolved_asw_forward_curve_id(bond: &Bond) -> Option<CurveId> {
    bond.instrument_pricing_overrides
        .model_config
        .asw_forward_curve_id
        .clone()
        .or_else(|| bond.forward_curve_id.clone())
}

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
/// use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
/// use finstack_quant_valuations::metrics::{MetricRegistry, MetricId, MetricContext};
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
/// use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
/// use finstack_quant_valuations::metrics::{MetricRegistry, MetricId, MetricContext};
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

fn build_asw_schedule(
    start: Date,
    end: Date,
    frequency: Tenor,
    stub: StubKind,
    business_day_convention: BusinessDayConvention,
    calendar_id: Option<&str>,
) -> finstack_quant_core::Result<Vec<Date>> {
    if start >= end {
        return Err(finstack_quant_core::Error::Validation(format!(
            "ASW schedule requires start before end: start={start}, end={end}"
        )));
    }
    let calendar = calendar_id.and_then(finstack_quant_core::dates::calendar::calendar_by_id);
    let adjust_date = |date: Date| -> finstack_quant_core::Result<Date> {
        if let Some(cal) = calendar {
            finstack_quant_core::dates::adjust(date, business_day_convention, cal)
        } else {
            Ok(date)
        }
    };

    let mut dates = Vec::new();
    dates.push(adjust_date(start)?);

    let mut raw = start;
    loop {
        let next = frequency.add_to_date(raw, None, BusinessDayConvention::Unadjusted)?;
        if next >= end {
            break;
        }
        dates.push(adjust_date(next)?);
        raw = next;
    }

    if !matches!(stub, StubKind::None) || dates.last().copied() != Some(end) {
        dates.push(adjust_date(end)?);
    }
    dates.sort();
    dates.dedup();
    Ok(dates)
}

fn asw_effective_date(
    issue_date: Date,
    quote_date: Date,
    end: Date,
    frequency: Tenor,
    _stub: StubKind,
    _business_day_convention: BusinessDayConvention,
    _calendar_id: Option<&str>,
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

fn asw_float_leg_conventions(
    fwd: &ForwardCurve,
) -> finstack_quant_core::Result<(DayCount, Tenor, BusinessDayConvention)> {
    let id = fwd.id().as_str().to_ascii_uppercase();
    if id.contains("SOFRRATE") || id.contains("SOFR") {
        Ok((
            DayCount::Act360,
            Tenor::annual(),
            BusinessDayConvention::ModifiedFollowing,
        ))
    } else if id.contains("EURIBOR-6M") || id.contains("EURIBOR6M") {
        Ok((
            DayCount::Act360,
            Tenor::semi_annual(),
            BusinessDayConvention::ModifiedFollowing,
        ))
    } else {
        Ok((
            fwd.day_count(),
            Tenor::from_years(fwd.tenor(), fwd.day_count())?,
            BusinessDayConvention::ModifiedFollowing,
        ))
    }
}

struct AssetSwapForwardInputs<'a> {
    disc: &'a DiscountCurve,
    fwd: &'a ForwardCurve,
    market: &'a MarketContext,
    as_of: Date,
    fixed_day_count: DayCount,
    fixed_frequency: Option<Tenor>,
    fixed_schedule: &'a [Date],
    float_day_count: DayCount,
    float_schedule: &'a [Date],
    float_spread_bp: f64,
}

fn asset_swap_forward_components_split(
    inputs: AssetSwapForwardInputs<'_>,
) -> finstack_quant_core::Result<(f64, f64, f64)> {
    let fixed_ann = fixed_leg_annuity(
        inputs.disc,
        inputs.fixed_day_count,
        inputs.fixed_frequency,
        inputs.fixed_schedule,
    )?;
    let float_schedule = inputs.float_schedule;
    if float_schedule.len() < 2 {
        return Ok((0.0, fixed_ann, 0.0));
    }

    let spread = inputs.float_spread_bp * 1e-4;
    let mut float_pv = finstack_quant_core::math::summation::NeumaierAccumulator::new();
    let mut float_ann = finstack_quant_core::math::summation::NeumaierAccumulator::new();

    let mut prev = float_schedule[0];
    for &date in &float_schedule[1..] {
        let yf = inputs
            .float_day_count
            .year_fraction(prev, date, DayCountContext::default())?;
        let df = inputs.disc.df_on_date_curve(date)?;
        let forward = if prev < inputs.as_of {
            let fixing_id = finstack_quant_core::market_data::fixings::fixing_series_id(
                inputs.fwd.id().as_str(),
            );
            let fixings = inputs.market.get_series(&fixing_id).map_err(|_| {
                finstack_quant_core::Error::Validation(format!(
                    "Seasoned asset swap requires historical fixing series '{}' for reset date {}; \
                     started term coupons must use observed fixings, not projection",
                    fixing_id, prev
                ))
            })?;
            fixings.value_on_exact(prev)?
        } else {
            asset_swap_projection_rate(inputs.fwd, prev, date)?
        };
        float_pv.add((forward + spread) * yf * df);
        float_ann.add(yf * df);
        prev = date;
    }

    Ok((float_pv.total(), fixed_ann, float_ann.total()))
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
                let df = disc.df_on_date_curve(cf.date)?;
                pv.add(cf.amount.amount() * df);
            }
            _ => {}
        }
    }
    Ok(pv.total())
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
/// * `curves` - Market context providing the bond's discount curve and the
///   named forward curve.
/// * `as_of` - Valuation date used to select future bond cashflows and build
///   the mirrored fixed-leg schedule.
/// * `fwd_curve_id` - Market-context identifier of the floating-leg forward
///   curve.
/// * `float_spread_bp` - Floating-leg contractual spread in basis points,
///   added to projected forward coupons.
/// * `fixed_leg_day_count` - Optional swap fixed-leg day-count convention for
///   the annuity. `None` uses the bond's coupon day count.
pub fn asw_par_with_forward(
    bond: &Bond,
    curves: &finstack_quant_core::market_data::context::MarketContext,
    as_of: finstack_quant_core::dates::Date,
    fwd_curve_id: &str,
    float_spread_bp: f64,
    fixed_leg_day_count: Option<DayCount>,
) -> finstack_quant_core::Result<f64> {
    let disc = curves.get_discount(&bond.discount_curve_id)?;
    let fwd = curves.get_forward(fwd_curve_id)?;

    // Trivial: zero-notional instruments have zero ASW by definition.
    // Return early to avoid validating schedule construction on a degenerate notionals.
    if bond.notional.amount().abs() < 1e-12 {
        return Ok(0.0);
    }

    // Mirror the bond schedule via holder flows
    let flows = bond.pricing_dated_cashflows(curves, as_of)?;
    let sched = build_future_dates_from_flows(&flows, as_of);
    if sched.len() < 2 {
        return Err(finstack_quant_core::Error::Validation(
            "ASW forward par calculation requires at least two fixed-leg schedule dates"
                .to_string(),
        ));
    }

    let fixed_day_count = fixed_leg_day_count.unwrap_or_else(|| bond.cashflow_spec.day_count());
    let fixed_frequency = Some(bond.cashflow_spec.frequency());
    let ann = fixed_leg_annuity(disc.as_ref(), fixed_day_count, fixed_frequency, &sched)?;
    // Use epsilon check to avoid unstable ratios when annuity is degenerate.
    if ann.abs() < 1e-12 {
        return Err(finstack_quant_core::Error::Validation(
            "ASW forward par calculation is undefined for near-zero fixed-leg annuity".to_string(),
        ));
    }
    let (float_pv, fixed_ann, float_ann) = asset_swap_forward_components(
        disc.as_ref(),
        fwd.as_ref(),
        fixed_day_count,
        fixed_frequency,
        &sched,
        float_spread_bp,
    )?;
    if float_ann.abs() < 1e-12 {
        return Err(finstack_quant_core::Error::Validation(
            "ASW forward par calculation is undefined for near-zero floating-leg annuity"
                .to_string(),
        ));
    }

    // Equivalent fixed rate from coupon-only PV
    let eq_coupon = if let Some(custom) = &bond.custom_cashflows {
        let pv_coupon = pv_coupon_from_custom_schedule(disc.as_ref(), custom, as_of)?;
        pv_coupon / (bond.notional.amount() * ann)
    } else {
        // Extract fixed coupon rate from cashflow_spec (converting Decimal to f64)
        match &bond.cashflow_spec {
            CashflowSpec::Fixed(spec) => spec.rate.to_f64().unwrap_or(0.0),
            _ => return Err(finstack_quant_core::InputError::Invalid.into()),
        }
    };
    Ok((eq_coupon * fixed_ann - float_pv) / float_ann)
}

/// Compute Market ASW using a forward-based methodology.
///
/// `dirty_price_currency` is required: `None` returns
/// `InputError::NotFound { id: "dirty_price_currency" }` instead of assuming
/// par. Pass `Some(bond.notional.amount())` for a par market price.
/// When `fixed_leg_day_count` is `Some`, that day-count builds the fixed-leg
/// annuity; `None` uses the bond's coupon day-count.
///
/// # Arguments
///
/// * `bond` - Bond whose future fixed cashflows and contractual discount-curve
///   identifier define the asset-swap fixed leg.
/// * `curves` - Market context providing the bond's discount curve and the
///   named forward curve.
/// * `as_of` - Valuation date used to select future bond cashflows and build
///   the mirrored fixed-leg schedule.
/// * `fwd_curve_id` - Market-context identifier of the floating-leg forward
///   curve.
/// * `float_spread_bp` - Floating-leg contractual spread in basis points,
///   added to projected forward coupons.
/// * `dirty_price_currency` - Dirty market price in the bond currency. `None`
///   returns `InputError::NotFound` instead of silently assuming par.
/// * `fixed_leg_day_count` - Optional swap fixed-leg day-count convention for
///   the annuity. `None` uses the bond's coupon day count.
pub fn asw_market_with_forward(
    bond: &Bond,
    curves: &finstack_quant_core::market_data::context::MarketContext,
    as_of: finstack_quant_core::dates::Date,
    fwd_curve_id: &str,
    float_spread_bp: f64,
    dirty_price_currency: Option<f64>,
    fixed_leg_day_count: Option<DayCount>,
) -> finstack_quant_core::Result<f64> {
    let disc = curves.get_discount(&bond.discount_curve_id)?;
    let flows = bond.pricing_dated_cashflows(curves, as_of)?;
    let sched = build_future_dates_from_flows(&flows, as_of);
    if sched.len() < 2 {
        return Err(finstack_quant_core::Error::Validation(
            "ASW forward market calculation requires at least two fixed-leg schedule dates"
                .to_string(),
        ));
    }
    let fixed_day_count = fixed_leg_day_count.unwrap_or_else(|| bond.cashflow_spec.day_count());
    let fixed_frequency = Some(bond.cashflow_spec.frequency());
    let ann = fixed_leg_annuity(disc.as_ref(), fixed_day_count, fixed_frequency, &sched)?;
    if ann.abs() < 1e-12 {
        return Err(finstack_quant_core::Error::Validation(
            "ASW forward market calculation is undefined for near-zero fixed-leg annuity"
                .to_string(),
        ));
    }
    if bond.notional.amount().abs() < 1e-12 {
        return Ok(0.0);
    }

    let fwd = curves.get_forward(fwd_curve_id)?;
    let (float_pv, fixed_ann, float_ann) = asset_swap_forward_components(
        disc.as_ref(),
        fwd.as_ref(),
        fixed_day_count,
        fixed_frequency,
        &sched,
        float_spread_bp,
    )?;
    if float_ann.abs() < 1e-12 {
        return Err(finstack_quant_core::Error::Validation(
            "ASW forward market calculation is undefined for near-zero floating-leg annuity"
                .to_string(),
        ));
    }
    let notional = bond.notional.amount();
    let Some(dirty) = dirty_price_currency else {
        return Err(finstack_quant_core::InputError::NotFound {
            id: "dirty_price_currency".to_string(),
        }
        .into());
    };
    let price_pct = dirty / notional;
    let eq_coupon = if let Some(custom) = &bond.custom_cashflows {
        let pv_coupon = pv_coupon_from_custom_schedule(disc.as_ref(), custom, as_of)?;
        pv_coupon / (notional * ann)
    } else {
        match &bond.cashflow_spec {
            CashflowSpec::Fixed(spec) => spec.rate.to_f64().unwrap_or(0.0),
            CashflowSpec::Floating(_)
            | CashflowSpec::StepUp(_)
            | CashflowSpec::Amortizing { .. } => {
                return Err(finstack_quant_core::InputError::Invalid.into())
            }
        }
    };
    let par_asw = (eq_coupon * fixed_ann - float_pv) / float_ann;
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
                        spec.rate_spec.index_id.as_str(),
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
        let asw_forward_curve_id = resolved_asw_forward_curve_id(bond);
        let disc = context.curves.get_discount(&discount_curve_id)?;

        // The ASW fixed leg follows the bond's coupon conventions.
        let (frequency, business_day_convention, calendar_id, stub) = match &bond.cashflow_spec {
            CashflowSpec::Fixed(spec) => (
                spec.schedule.frequency,
                spec.schedule.business_day_convention,
                Some(spec.schedule.calendar_id.as_str()),
                spec.schedule.stub,
            ),
            CashflowSpec::Floating(spec) => (
                spec.schedule.frequency,
                spec.schedule.business_day_convention,
                Some(spec.schedule.calendar_id.as_str()),
                spec.schedule.stub,
            ),
            CashflowSpec::StepUp(spec) => (
                spec.schedule.frequency,
                spec.schedule.business_day_convention,
                Some(spec.schedule.calendar_id.as_str()),
                spec.schedule.stub,
            ),
            CashflowSpec::Amortizing { base, .. } => match &**base {
                CashflowSpec::Fixed(spec) => (
                    spec.schedule.frequency,
                    spec.schedule.business_day_convention,
                    Some(spec.schedule.calendar_id.as_str()),
                    spec.schedule.stub,
                ),
                CashflowSpec::Floating(spec) => (
                    spec.schedule.frequency,
                    spec.schedule.business_day_convention,
                    Some(spec.schedule.calendar_id.as_str()),
                    spec.schedule.stub,
                ),
                CashflowSpec::StepUp(spec) => (
                    spec.schedule.frequency,
                    spec.schedule.business_day_convention,
                    Some(spec.schedule.calendar_id.as_str()),
                    spec.schedule.stub,
                ),
                CashflowSpec::Amortizing { .. } => {
                    return Err(finstack_quant_core::InputError::Invalid.into())
                }
            },
        };

        // Market standard: Par swap rate via discount ratio on the ASW fixed-leg
        // schedule, matching the bond schedule.
        if context.as_of >= maturity {
            return Err(finstack_quant_core::Error::Validation(
                "ASW par calculation requires at least two fixed-leg schedule dates".to_string(),
            ));
        }
        let quote_ctx = QuoteDateContext::new(bond, &context.curves, context.as_of)?;
        let mut builder =
            finstack_quant_core::dates::ScheduleBuilder::new(quote_ctx.quote_date, maturity)?
                .frequency(frequency)
                .stub_rule(stub);

        if let Some(id) = calendar_id {
            if let Some(cal) = finstack_quant_core::dates::calendar::calendar_by_id(id) {
                builder = builder.adjust_with(business_day_convention, cal);
            }
        }

        let sched: Vec<Date> = builder.build()?.into_iter().collect();
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
        let coupon = match &bond.cashflow_spec {
            CashflowSpec::Fixed(spec) => spec.rate.to_f64().unwrap_or(0.0),
            _ => return Err(finstack_quant_core::InputError::Invalid.into()),
        };
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
        let (
            discount_curve_id,
            maturity,
            day_count,
            notional_amt,
            quoted_clean,
            is_custom,
            coupon,
            asw_forward_curve_id,
        ) = {
            let b: &Bond = context.instrument_as()?;
            let coupon_rate = match &b.cashflow_spec {
                CashflowSpec::Fixed(spec) => spec.rate.to_f64().unwrap_or(0.0),
                _ => 0.0, // Will be handled later if needed
            };
            (
                b.discount_curve_id.to_owned(),
                b.maturity,
                b.cashflow_spec.day_count(),
                b.notional.amount(),
                b.instrument_pricing_overrides
                    .market_quotes
                    .quoted_clean_price,
                b.custom_cashflows.is_some(),
                coupon_rate,
                resolved_asw_forward_curve_id(b),
            )
        };
        let disc = context.curves.get_discount(&discount_curve_id)?;

        // Dirty market value in currency
        let dirty_currency = if let Some(clean_px) = quoted_clean {
            let accrued = context
                .computed
                .get(&MetricId::Accrued)
                .copied()
                .ok_or_else(|| {
                    finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                        id: "metric:Accrued".to_string(),
                    })
                })?;
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
                        spec.rate_spec.index_id.as_str(),
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

        // Fixed coupon-leg PV (exclude principal) at zero spread
        if context.cashflows.is_none() {
            let (disc_id_capture, dc_capture, built) = {
                let b: &Bond = context.instrument_as()?;
                // Coupon entitlement at the quote/settlement date, consistent
                // with the other quote-derived metrics sharing this cache.
                let quote_date =
                    crate::instruments::fixed_income::bond::pricing::settlement::settlement_date(
                        b,
                        context.as_of,
                    )?;
                (
                    b.discount_curve_id.to_owned(),
                    b.cashflow_spec.day_count(),
                    b.pricing_dated_cashflows_at(&context.curves, context.as_of, quote_date)?,
                )
            };
            context.cashflows = Some(built);
            context.discount_curve_id = Some(disc_id_capture);
            context.day_count = Some(dc_capture);
        }
        let _flows = context.cashflows.as_ref().ok_or_else(|| {
            finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                id: "cashflows".to_string(),
            })
        })?;
        // Note: we no longer pre-compute coupon-only PV here; custom-bond coupon PV is
        // computed below when needed to derive the equivalent coupon.

        // Forward-based path when configured for non-custom bonds is available
        // via explicit helper methods (ASW*Fwd calculators). Here we keep fallback-only.

        // Market standard: discount-ratio using the bond's fixed-leg schedule.
        let bond: &Bond = context.instrument_as()?;
        let (frequency, stub, business_day_convention, calendar_id) = match &bond.cashflow_spec {
            CashflowSpec::Fixed(spec) => (
                spec.schedule.frequency,
                spec.schedule.stub,
                spec.schedule.business_day_convention,
                Some(spec.schedule.calendar_id.as_str()),
            ),
            CashflowSpec::Floating(spec) => (
                spec.schedule.frequency,
                spec.schedule.stub,
                spec.schedule.business_day_convention,
                Some(spec.schedule.calendar_id.as_str()),
            ),
            CashflowSpec::StepUp(spec) => (
                spec.schedule.frequency,
                spec.schedule.stub,
                spec.schedule.business_day_convention,
                Some(spec.schedule.calendar_id.as_str()),
            ),
            CashflowSpec::Amortizing { base, .. } => match &**base {
                CashflowSpec::Fixed(spec) => (
                    spec.schedule.frequency,
                    spec.schedule.stub,
                    spec.schedule.business_day_convention,
                    Some(spec.schedule.calendar_id.as_str()),
                ),
                CashflowSpec::Floating(spec) => (
                    spec.schedule.frequency,
                    spec.schedule.stub,
                    spec.schedule.business_day_convention,
                    Some(spec.schedule.calendar_id.as_str()),
                ),
                CashflowSpec::StepUp(spec) => (
                    spec.schedule.frequency,
                    spec.schedule.stub,
                    spec.schedule.business_day_convention,
                    Some(spec.schedule.calendar_id.as_str()),
                ),
                CashflowSpec::Amortizing { .. } => {
                    return Err(finstack_quant_core::InputError::Invalid.into())
                }
            },
        };
        let quote_ctx = QuoteDateContext::new(bond, &context.curves, context.as_of)?;
        if let Some(clean_px) = quoted_clean {
            let flows = quote_ctx.entitled_flows(bond, &context.curves, context.as_of)?;
            if let Some((_, workout_flows, workout_quote_date)) =
                crate::instruments::fixed_income::bond::metrics::quoted_workout_path(
                    bond,
                    context.curves.as_ref(),
                    context.as_of,
                    &flows,
                )?
            {
                let workout_maturity = workout_flows
                    .last()
                    .map(|(date, _)| *date)
                    .unwrap_or(maturity);
                if workout_maturity < maturity {
                    let effective_date = asw_effective_date(
                        bond.issue_date,
                        workout_quote_date,
                        workout_maturity,
                        frequency,
                        stub,
                        business_day_convention,
                        calendar_id,
                    )?;
                    let fixed_schedule = build_asw_schedule(
                        effective_date,
                        workout_maturity,
                        frequency,
                        StubKind::None,
                        business_day_convention,
                        calendar_id,
                    )?;
                    if fixed_schedule.len() < 2 {
                        return Err(finstack_quant_core::Error::Validation(
                            "ASW market calculation requires at least two fixed-leg schedule dates"
                                .to_string(),
                        ));
                    }
                    let dc_fixed = day_count;
                    if let Some(fwd_id) = asw_forward_curve_id.as_ref() {
                        let fwd = context.curves.get_forward(fwd_id)?;
                        let (float_day_count, float_frequency, float_business_day_convention) =
                            asw_float_leg_conventions(&fwd)?;
                        let float_schedule = build_asw_schedule(
                            effective_date,
                            workout_maturity,
                            float_frequency,
                            StubKind::None,
                            float_business_day_convention,
                            None,
                        )?;
                        let (float_pv, fixed_ann, float_ann) =
                            asset_swap_forward_components_split(AssetSwapForwardInputs {
                                disc: disc.as_ref(),
                                fwd: fwd.as_ref(),
                                market: context.curves.as_ref(),
                                as_of: context.as_of,
                                fixed_day_count: dc_fixed,
                                fixed_frequency: Some(frequency),
                                fixed_schedule: &fixed_schedule,
                                float_day_count,
                                float_schedule: &float_schedule,
                                float_spread_bp: 0.0,
                            })?;
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
                        )?;
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
        if quote_ctx.quote_date >= maturity {
            return Err(finstack_quant_core::Error::Validation(
                "ASW market calculation requires at least two fixed-leg schedule dates".to_string(),
            ));
        }
        let mut builder =
            finstack_quant_core::dates::ScheduleBuilder::new(quote_ctx.quote_date, maturity)?
                .frequency(frequency)
                .stub_rule(stub);

        if let Some(id) = calendar_id {
            if let Some(cal) = finstack_quant_core::dates::calendar::calendar_by_id(id) {
                builder = builder.adjust_with(business_day_convention, cal);
            }
        }

        let sched: Vec<Date> = builder.build()?.into_iter().collect();
        if sched.len() < 2 {
            return Err(finstack_quant_core::Error::Validation(
                "ASW market calculation requires at least two fixed-leg schedule dates".to_string(),
            ));
        }
        let dc_fixed = day_count;
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
        let (par_rate, ann) = if let Some((float_pv, fixed_ann, float_ann)) = forward_components {
            if fixed_ann.abs() < 1e-12 {
                (0.0, 0.0)
            } else {
                (float_pv / fixed_ann, float_ann)
            }
        } else {
            par_rate_and_annuity_from_discount(disc.as_ref(), dc_fixed, Some(frequency), &sched)?
        };
        if ann.abs() < 1e-12 {
            return Err(finstack_quant_core::Error::Validation(
                "ASW market calculation is undefined for near-zero fixed-leg annuity".to_string(),
            ));
        }
        if notional_amt.abs() < 1e-12 {
            return Ok(0.0);
        }
        // Equivalent coupon from coupon PV only for custom bonds; otherwise stated coupon
        let eq_coupon = if let Some(custom) = &context.instrument_as::<Bond>()?.custom_cashflows {
            let pv_coupon = pv_coupon_from_custom_schedule(disc.as_ref(), custom, context.as_of)?;
            pv_coupon / (notional_amt * ann)
        } else {
            coupon
        };
        let price_pct = dirty_currency / notional_amt;
        let asw_mkt = if let Some((float_pv, fixed_ann, float_ann)) = forward_components {
            (eq_coupon * fixed_ann + 1.0 - price_pct - float_pv) / float_ann
        } else {
            // Exact par-par form:
            // spread = [(C - par_rate)·Ann_fixed + (1 - p)] / Ann_float.
            // Without a forward curve the floating leg is proxied on the same
            // schedule using the discount curve's day count.
            let float_ann = fixed_leg_annuity(disc.as_ref(), disc.day_count(), None, &sched)?;
            if float_ann.abs() < 1e-12 {
                return Err(finstack_quant_core::Error::Validation(
                    "ASW market calculation is undefined for near-zero floating-leg annuity"
                        .to_string(),
                ));
            }
            ((eq_coupon - par_rate) * ann + (1.0 - price_pct)) / float_ann
        };
        Ok(asw_mkt)
    }
}
