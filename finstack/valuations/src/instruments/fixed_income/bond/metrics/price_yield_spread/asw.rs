use crate::cashflow::{builder::CashFlowSchedule, primitives::CFKind};
use crate::instruments::fixed_income::bond::pricing::quote_conversions::{
    asset_swap_forward_components, fixed_leg_annuity, par_rate_and_annuity_from_discount,
};
use crate::instruments::fixed_income::bond::pricing::settlement::QuoteDateContext;
use crate::instruments::fixed_income::bond::CashflowSpec;
use crate::instruments::Bond;
use crate::metrics::{MetricCalculator, MetricContext, MetricId};
use finstack_core::dates::{
    BusinessDayConvention, Date, DayCount, DayCountContext, StubKind, Tenor,
};
use finstack_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_core::types::CurveId;
use rust_decimal::prelude::ToPrimitive;

fn resolved_asw_forward_curve_id(bond: &Bond) -> Option<CurveId> {
    bond.pricing_overrides
        .model_config
        .asw_forward_curve_id
        .clone()
        .or_else(|| bond.forward_curve_id.clone())
}

/// Configuration for fixed-leg conventions used in ASW par/market metrics.
///
/// Controls the day-count, frequency, business day convention, calendar, and stub
/// rules for the asset swap fixed leg. When a field is `None`, the corresponding
/// convention falls back to the bond's own coupon conventions.
///
/// # Examples
///
/// ```text
/// use finstack_valuations::instruments::fixed_income::bond::metrics::price_yield_spread::asw::AssetSwapConfig;
/// use finstack_core::dates::{DayCount, Tenor, BusinessDayConvention, StubKind};
///
/// let config = AssetSwapConfig {
///     fixed_leg_day_count: Some(DayCount::Act365F),
///     fixed_leg_frequency: Some(Tenor::semi_annual()),
///     fixed_leg_bdc: Some(BusinessDayConvention::ModifiedFollowing),
///     fixed_leg_calendar_id: Some("USGS".to_string()),
///     fixed_leg_stub: Some(StubKind::ShortFront),
/// };
/// ```
#[derive(Debug, Clone, Default)]
pub struct AssetSwapConfig {
    /// Day-count convention for the ASW fixed leg (annuity).
    pub fixed_leg_day_count: Option<DayCount>,
    /// Payment frequency for the ASW fixed leg.
    pub fixed_leg_frequency: Option<Tenor>,
    /// Business day convention for the ASW fixed-leg schedule.
    pub fixed_leg_bdc: Option<BusinessDayConvention>,
    /// Optional calendar identifier for business-day adjustment.
    pub fixed_leg_calendar_id: Option<String>,
    /// Stub convention for the ASW fixed-leg schedule.
    pub fixed_leg_stub: Option<StubKind>,
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
/// use finstack_valuations::instruments::fixed_income::bond::Bond;
/// use finstack_valuations::metrics::{MetricRegistry, MetricId, MetricContext};
/// use finstack_core::market_data::context::MarketContext;
/// use finstack_core::dates::Date;
///
/// # let bond = Bond::example().unwrap();
/// # let market = MarketContext::new();
/// # let as_of = Date::from_calendar_date(2024, time::Month::January, 15).unwrap();
/// // Par ASW is computed automatically when requesting bond metrics
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Default)]
pub struct AssetSwapParCalculator {
    config: AssetSwapConfig,
}

/// Asset swap market spread calculator using market price.
///
/// Market ASW is the spread that equates the asset-swap package to par. Uses the approximation:
/// ```text
/// asw_mkt ≈ coupon - par_rate + (1 - dirty/Notional)/annuity
/// ```
///
/// # Dependencies
///
/// Requires `Accrued` metric to be computed first (for dirty price calculation).
///
/// # Examples
///
/// ```text
/// use finstack_valuations::instruments::fixed_income::bond::Bond;
/// use finstack_valuations::metrics::{MetricRegistry, MetricId, MetricContext};
/// use finstack_core::market_data::context::MarketContext;
/// use finstack_core::dates::Date;
///
/// # let bond = Bond::example().unwrap();
/// # let market = MarketContext::new();
/// # let as_of = Date::from_calendar_date(2024, time::Month::January, 15).unwrap();
/// // Market ASW is computed automatically when requesting bond metrics
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Default)]
pub struct AssetSwapMarketCalculator {
    config: AssetSwapConfig,
}

impl AssetSwapParCalculator {
    /// Create a par ASW calculator with default behaviour (bond conventions).
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a par ASW calculator with explicit fixed-leg conventions.
    pub fn with_config(config: AssetSwapConfig) -> Self {
        Self { config }
    }
}

impl AssetSwapMarketCalculator {
    /// Create a market ASW calculator with default behaviour (bond conventions).
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a market ASW calculator with explicit fixed-leg conventions.
    pub fn with_config(config: AssetSwapConfig) -> Self {
        Self { config }
    }
}

fn build_future_dates_from_flows(
    flows: &[(finstack_core::dates::Date, finstack_core::money::Money)],
    as_of: finstack_core::dates::Date,
) -> Vec<finstack_core::dates::Date> {
    use finstack_core::dates::Date;
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
    bdc: BusinessDayConvention,
    calendar_id: Option<&str>,
) -> finstack_core::Result<Vec<Date>> {
    if start >= end {
        return Err(finstack_core::Error::Validation(format!(
            "ASW schedule requires start before end: start={start}, end={end}"
        )));
    }
    let calendar = calendar_id.and_then(finstack_core::dates::calendar::calendar_by_id);
    let adjust_date = |date: Date| -> finstack_core::Result<Date> {
        if let Some(cal) = calendar {
            finstack_core::dates::adjust(date, bdc, cal)
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
    _bdc: BusinessDayConvention,
    _calendar_id: Option<&str>,
) -> finstack_core::Result<Date> {
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

fn asw_float_leg_conventions(fwd: &ForwardCurve) -> (DayCount, Tenor, BusinessDayConvention) {
    let id = fwd.id().as_str().to_ascii_uppercase();
    if id.contains("SOFRRATE") || id.contains("SOFR") {
        (
            DayCount::Act360,
            Tenor::annual(),
            BusinessDayConvention::ModifiedFollowing,
        )
    } else if id.contains("EURIBOR-6M") || id.contains("EURIBOR6M") {
        (
            DayCount::Act360,
            Tenor::semi_annual(),
            BusinessDayConvention::ModifiedFollowing,
        )
    } else {
        (
            fwd.day_count(),
            Tenor::from_years(fwd.tenor(), fwd.day_count()),
            BusinessDayConvention::ModifiedFollowing,
        )
    }
}

struct AssetSwapForwardInputs<'a> {
    disc: &'a DiscountCurve,
    fwd: &'a ForwardCurve,
    fixed_dc: DayCount,
    fixed_schedule: &'a [Date],
    float_dc: DayCount,
    float_schedule: &'a [Date],
    float_spread_bp: f64,
}

fn asset_swap_forward_components_split(
    inputs: AssetSwapForwardInputs<'_>,
) -> finstack_core::Result<(f64, f64, f64)> {
    let fixed_ann = fixed_leg_annuity(inputs.disc, inputs.fixed_dc, inputs.fixed_schedule)?;
    let float_schedule = inputs.float_schedule;
    if float_schedule.len() < 2 {
        return Ok((0.0, fixed_ann, 0.0));
    }

    let f_base = inputs.fwd.base_date();
    let spread = inputs.float_spread_bp * 1e-4;
    let mut float_pv = finstack_core::math::summation::NeumaierAccumulator::new();
    let mut float_ann = finstack_core::math::summation::NeumaierAccumulator::new();

    let mut prev = float_schedule[0];
    for &date in &float_schedule[1..] {
        let t1 = if prev <= f_base {
            0.0
        } else {
            inputs
                .float_dc
                .year_fraction(f_base, prev, DayCountContext::default())?
        };
        let t2 = if date <= f_base {
            0.0
        } else {
            inputs
                .float_dc
                .year_fraction(f_base, date, DayCountContext::default())?
        };
        let yf = inputs
            .float_dc
            .year_fraction(prev, date, DayCountContext::default())?;
        let df = inputs.disc.df_on_date_curve(date)?;
        float_pv.add((inputs.fwd.rate_period(t1, t2) + spread) * yf * df);
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
) -> finstack_core::Result<f64> {
    use finstack_core::math::summation::NeumaierAccumulator;

    let mut pv = NeumaierAccumulator::new();
    for cf in &schedule.flows {
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

/// Compute Par ASW using a forward-based methodology with explicit parameters.
///
/// Note:
/// - This helper uses the bond's coupon day-count for the fixed-leg annuity.
/// - In many markets, par asset swaps are quoted on swap fixed-leg conventions
///   that may differ from the bond's coupon convention.
///
/// For explicit control over the fixed-leg convention (e.g., to align with
/// a swap market standard per currency), prefer
/// [`asw_par_with_forward_config`], which accepts an optional fixed-leg
/// day-count override.
pub fn asw_par_with_forward(
    bond: &Bond,
    curves: &finstack_core::market_data::context::MarketContext,
    as_of: finstack_core::dates::Date,
    fwd_curve_id: &str,
    float_spread_bp: f64,
) -> finstack_core::Result<f64> {
    asw_par_with_forward_config(bond, curves, as_of, fwd_curve_id, float_spread_bp, None)
}

/// Compute Par ASW using a forward-based methodology with explicit fixed-leg
/// conventions.
///
/// - `fixed_leg_day_count`: when `Some`, this day-count is used to build the
///   fixed-leg annuity, allowing callers to align with swap fixed-leg market
///   conventions (e.g., 30E/360) instead of the bond's coupon convention.
/// - When `None`, this falls back to `bond.cashflow_spec.day_count()`.
pub fn asw_par_with_forward_config(
    bond: &Bond,
    curves: &finstack_core::market_data::context::MarketContext,
    as_of: finstack_core::dates::Date,
    fwd_curve_id: &str,
    float_spread_bp: f64,
    fixed_leg_day_count: Option<DayCount>,
) -> finstack_core::Result<f64> {
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
        return Err(finstack_core::Error::Validation(
            "ASW forward par calculation requires at least two fixed-leg schedule dates"
                .to_string(),
        ));
    }

    let fixed_dc = fixed_leg_day_count.unwrap_or_else(|| bond.cashflow_spec.day_count());
    let ann = fixed_leg_annuity(disc.as_ref(), fixed_dc, &sched)?;
    // Use epsilon check to avoid unstable ratios when annuity is degenerate.
    if ann.abs() < 1e-12 {
        return Err(finstack_core::Error::Validation(
            "ASW forward par calculation is undefined for near-zero fixed-leg annuity".to_string(),
        ));
    }
    let (float_pv, fixed_ann, float_ann) = asset_swap_forward_components(
        disc.as_ref(),
        fwd.as_ref(),
        fixed_dc,
        &sched,
        float_spread_bp,
    )?;
    if float_ann.abs() < 1e-12 {
        return Err(finstack_core::Error::Validation(
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
            _ => return Err(finstack_core::InputError::Invalid.into()),
        }
    };
    Ok((eq_coupon * fixed_ann - float_pv) / float_ann)
}

/// Compute Market ASW using forward-based methodology with explicit parameters.
///
/// Note:
/// - This helper requires an explicit dirty market price in currency.
///   Callers **must** pass `Some(dirty_price_ccy)` even when interpreting
///   ASW relative to par (in which case, pass `bond.notional.amount()`).
/// - In many markets, the fixed leg follows swap fixed-leg conventions that
///   may differ from the bond's coupon convention.
///
/// For explicit control over fixed-leg conventions (e.g., swap day-count),
/// prefer [`asw_market_with_forward_config`], which accepts an optional
/// fixed-leg day-count override.
pub fn asw_market_with_forward(
    bond: &Bond,
    curves: &finstack_core::market_data::context::MarketContext,
    as_of: finstack_core::dates::Date,
    fwd_curve_id: &str,
    float_spread_bp: f64,
    dirty_price_ccy: Option<f64>,
) -> finstack_core::Result<f64> {
    asw_market_with_forward_config(
        bond,
        curves,
        as_of,
        fwd_curve_id,
        float_spread_bp,
        dirty_price_ccy,
        None,
    )
}

/// Compute Market ASW using forward-based methodology with explicit
/// fixed-leg conventions.
///
/// - `dirty_price_ccy`: dirty market price expressed in currency. When
///   `None`, this returns `InputError::NotFound { id: "dirty_price_ccy" }`
///   instead of silently assuming par.
/// - `fixed_leg_day_count`: when `Some`, this day-count is used to build
///   the fixed-leg annuity; otherwise the bond's coupon day-count is used.
pub fn asw_market_with_forward_config(
    bond: &Bond,
    curves: &finstack_core::market_data::context::MarketContext,
    as_of: finstack_core::dates::Date,
    fwd_curve_id: &str,
    float_spread_bp: f64,
    dirty_price_ccy: Option<f64>,
    fixed_leg_day_count: Option<DayCount>,
) -> finstack_core::Result<f64> {
    let disc = curves.get_discount(&bond.discount_curve_id)?;
    let flows = bond.pricing_dated_cashflows(curves, as_of)?;
    let sched = build_future_dates_from_flows(&flows, as_of);
    if sched.len() < 2 {
        return Err(finstack_core::Error::Validation(
            "ASW forward market calculation requires at least two fixed-leg schedule dates"
                .to_string(),
        ));
    }
    let fixed_dc = fixed_leg_day_count.unwrap_or_else(|| bond.cashflow_spec.day_count());
    let ann = fixed_leg_annuity(disc.as_ref(), fixed_dc, &sched)?;
    if ann.abs() < 1e-12 {
        return Err(finstack_core::Error::Validation(
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
        fixed_dc,
        &sched,
        float_spread_bp,
    )?;
    if float_ann.abs() < 1e-12 {
        return Err(finstack_core::Error::Validation(
            "ASW forward market calculation is undefined for near-zero floating-leg annuity"
                .to_string(),
        ));
    }
    let notional = bond.notional.amount();
    let dirty = match dirty_price_ccy {
        Some(v) => v,
        None => {
            return Err(finstack_core::InputError::NotFound {
                id: "dirty_price_ccy".to_string(),
            }
            .into());
        }
    };
    let price_pct = dirty / notional;
    let eq_coupon = if let Some(custom) = &bond.custom_cashflows {
        let pv_coupon = pv_coupon_from_custom_schedule(disc.as_ref(), custom, as_of)?;
        pv_coupon / (notional * ann)
    } else {
        match &bond.cashflow_spec {
            CashflowSpec::Fixed(spec) => spec.rate.to_f64().unwrap_or(0.0),
            _ => return Err(finstack_core::InputError::Invalid.into()),
        }
    };
    let par_asw = (eq_coupon * fixed_ann - float_pv) / float_ann;
    Ok(par_asw + (1.0 - price_pct) / float_ann)
}

impl MetricCalculator for AssetSwapParCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_core::Result<f64> {
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
                    );
                }
                _ => {
                    return Err(finstack_core::InputError::NotFound {
                        id: "bond.cashflow_spec.floating".to_string(),
                    }
                    .into());
                }
            }
        }

        let discount_curve_id = bond.discount_curve_id.to_owned();
        let maturity = bond.maturity;
        let bond_dc = bond.cashflow_spec.day_count();
        let asw_forward_curve_id = resolved_asw_forward_curve_id(bond);
        let disc = context.curves.get_discount(&discount_curve_id)?;

        // Extract schedule params from cashflow_spec, allowing ASW config to
        // override fixed-leg conventions when provided.
        let (bond_freq, bond_bdc, bond_calendar_id, bond_stub) = match &bond.cashflow_spec {
            CashflowSpec::Fixed(spec) => (
                spec.freq,
                spec.bdc,
                Some(spec.calendar_id.as_str()),
                spec.stub,
            ),
            CashflowSpec::Floating(spec) => (
                spec.freq,
                spec.rate_spec.bdc,
                Some(spec.rate_spec.calendar_id.as_str()),
                spec.stub,
            ),
            CashflowSpec::StepUp(spec) => (
                spec.freq,
                spec.bdc,
                Some(spec.calendar_id.as_str()),
                spec.stub,
            ),
            CashflowSpec::Amortizing { base, .. } => match &**base {
                CashflowSpec::Fixed(spec) => (
                    spec.freq,
                    spec.bdc,
                    Some(spec.calendar_id.as_str()),
                    spec.stub,
                ),
                CashflowSpec::Floating(spec) => (
                    spec.freq,
                    spec.rate_spec.bdc,
                    Some(spec.rate_spec.calendar_id.as_str()),
                    spec.stub,
                ),
                CashflowSpec::StepUp(spec) => (
                    spec.freq,
                    spec.bdc,
                    Some(spec.calendar_id.as_str()),
                    spec.stub,
                ),
                _ => return Err(finstack_core::InputError::Invalid.into()),
            },
        };

        let freq = self.config.fixed_leg_frequency.unwrap_or(bond_freq);
        let bdc = self.config.fixed_leg_bdc.unwrap_or(bond_bdc);
        let calendar_id = self
            .config
            .fixed_leg_calendar_id
            .as_deref()
            .or(bond_calendar_id);
        let stub = self.config.fixed_leg_stub.unwrap_or(bond_stub);

        // Market standard: Par swap rate via discount ratio on the ASW fixed-leg
        // schedule. By default this matches the bond schedule; callers may
        // override fixed-leg conventions via AssetSwapConfig.
        if context.as_of >= maturity {
            return Err(finstack_core::Error::Validation(
                "ASW par calculation requires at least two fixed-leg schedule dates".to_string(),
            ));
        }
        let quote_ctx = QuoteDateContext::new(bond, &context.curves, context.as_of)?;
        let mut builder =
            finstack_core::dates::ScheduleBuilder::new(quote_ctx.quote_date, maturity)?
                .frequency(freq)
                .stub_rule(stub);

        if let Some(id) = calendar_id {
            if let Some(cal) = finstack_core::dates::calendar::calendar_by_id(id) {
                builder = builder.adjust_with(bdc, cal);
            }
        }

        let sched: Vec<Date> = builder.build()?.into_iter().collect();
        if sched.len() < 2 {
            return Err(finstack_core::Error::Validation(
                "ASW par calculation requires at least two fixed-leg schedule dates".to_string(),
            ));
        }
        let dc_fixed = self.config.fixed_leg_day_count.unwrap_or(bond_dc);
        let forward_components = if let Some(fwd_id) = asw_forward_curve_id {
            let fwd = context.curves.get_forward(&fwd_id)?;
            Some(asset_swap_forward_components(
                disc.as_ref(),
                fwd.as_ref(),
                dc_fixed,
                &sched,
                0.0,
            )?)
        } else {
            None
        };
        let ann = if let Some((_, _, float_ann)) = forward_components {
            float_ann
        } else {
            par_rate_and_annuity_from_discount(disc.as_ref(), dc_fixed, &sched)?.1
        };
        if ann.abs() < 1e-12 {
            return Err(finstack_core::Error::Validation(
                "ASW par calculation is undefined for near-zero spread annuity".to_string(),
            ));
        }
        // Use stated coupon for non-custom bonds; for custom bonds, this branch is not reached
        let coupon = match &bond.cashflow_spec {
            CashflowSpec::Fixed(spec) => spec.rate.to_f64().unwrap_or(0.0),
            _ => return Err(finstack_core::InputError::Invalid.into()),
        };
        if let Some((float_pv, fixed_ann, float_ann)) = forward_components {
            Ok((coupon * fixed_ann - float_pv) / float_ann)
        } else {
            let (par_rate, _) =
                par_rate_and_annuity_from_discount(disc.as_ref(), dc_fixed, &sched)?;
            Ok(coupon - par_rate)
        }
    }
}

impl MetricCalculator for AssetSwapMarketCalculator {
    fn dependencies(&self) -> &[MetricId] {
        &[MetricId::Accrued]
    }

    fn calculate(&self, context: &mut MetricContext) -> finstack_core::Result<f64> {
        let (
            discount_curve_id,
            maturity,
            dc,
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
                b.pricing_overrides.market_quotes.quoted_clean_price,
                b.custom_cashflows.is_some(),
                coupon_rate,
                resolved_asw_forward_curve_id(b),
            )
        };
        let disc = context.curves.get_discount(&discount_curve_id)?;

        // Dirty market value in currency
        let dirty_ccy = if let Some(clean_px) = quoted_clean {
            let accrued = context
                .computed
                .get(&MetricId::Accrued)
                .copied()
                .ok_or_else(|| {
                    finstack_core::Error::from(finstack_core::InputError::NotFound {
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
                        Some(dirty_ccy),
                    );
                }
                _ => {
                    return Err(finstack_core::InputError::NotFound {
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
                (
                    b.discount_curve_id.to_owned(),
                    b.cashflow_spec.day_count(),
                    b.pricing_dated_cashflows(&context.curves, context.as_of)?,
                )
            };
            context.cashflows = Some(built);
            context.discount_curve_id = Some(disc_id_capture);
            context.day_count = Some(dc_capture);
        }
        let _flows = context.cashflows.as_ref().ok_or_else(|| {
            finstack_core::Error::from(finstack_core::InputError::NotFound {
                id: "cashflows".to_string(),
            })
        })?;
        // Note: we no longer pre-compute coupon-only PV here; custom-bond coupon PV is
        // computed below when needed to derive the equivalent coupon.

        // Forward-based path when configured for non-custom bonds is available
        // via explicit helper methods (ASW*Fwd calculators). Here we keep fallback-only.

        // Market standard: discount-ratio using ASW fixed-leg schedule (defaulting
        // to bond conventions but allowing overrides via AssetSwapConfig).
        let bond: &Bond = context.instrument_as()?;
        let (bond_freq, bond_stub, bond_bdc, bond_calendar_id) = match &bond.cashflow_spec {
            CashflowSpec::Fixed(spec) => (
                spec.freq,
                spec.stub,
                spec.bdc,
                Some(spec.calendar_id.as_str()),
            ),
            CashflowSpec::Floating(spec) => (
                spec.freq,
                spec.stub,
                spec.rate_spec.bdc,
                Some(spec.rate_spec.calendar_id.as_str()),
            ),
            CashflowSpec::StepUp(spec) => (
                spec.freq,
                spec.stub,
                spec.bdc,
                Some(spec.calendar_id.as_str()),
            ),
            CashflowSpec::Amortizing { base, .. } => match &**base {
                CashflowSpec::Fixed(spec) => (
                    spec.freq,
                    spec.stub,
                    spec.bdc,
                    Some(spec.calendar_id.as_str()),
                ),
                CashflowSpec::Floating(spec) => (
                    spec.freq,
                    spec.stub,
                    spec.rate_spec.bdc,
                    Some(spec.rate_spec.calendar_id.as_str()),
                ),
                CashflowSpec::StepUp(spec) => (
                    spec.freq,
                    spec.stub,
                    spec.bdc,
                    Some(spec.calendar_id.as_str()),
                ),
                _ => return Err(finstack_core::InputError::Invalid.into()),
            },
        };
        let freq = self.config.fixed_leg_frequency.unwrap_or(bond_freq);
        let stub = self.config.fixed_leg_stub.unwrap_or(bond_stub);
        let bdc = self.config.fixed_leg_bdc.unwrap_or(bond_bdc);
        let calendar_id = self
            .config
            .fixed_leg_calendar_id
            .as_deref()
            .or(bond_calendar_id);

        let quote_ctx = QuoteDateContext::new(bond, &context.curves, context.as_of)?;
        if let Some(clean_px) = quoted_clean {
            let flows = bond.pricing_dated_cashflows(&context.curves, context.as_of)?;
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
                        freq,
                        stub,
                        bdc,
                        calendar_id,
                    )?;
                    let fixed_schedule = build_asw_schedule(
                        effective_date,
                        workout_maturity,
                        freq,
                        StubKind::None,
                        bdc,
                        calendar_id,
                    )?;
                    if fixed_schedule.len() < 2 {
                        return Err(finstack_core::Error::Validation(
                            "ASW market calculation requires at least two fixed-leg schedule dates"
                                .to_string(),
                        ));
                    }
                    let dc_fixed = self.config.fixed_leg_day_count.unwrap_or(dc);
                    if let Some(fwd_id) = asw_forward_curve_id.as_ref() {
                        let fwd = context.curves.get_forward(fwd_id)?;
                        let (float_dc, float_freq, float_bdc) = asw_float_leg_conventions(&fwd);
                        let float_schedule = build_asw_schedule(
                            effective_date,
                            workout_maturity,
                            float_freq,
                            StubKind::None,
                            float_bdc,
                            None,
                        )?;
                        let (float_pv, fixed_ann, float_ann) =
                            asset_swap_forward_components_split(AssetSwapForwardInputs {
                                disc: disc.as_ref(),
                                fwd: fwd.as_ref(),
                                fixed_dc: dc_fixed,
                                fixed_schedule: &fixed_schedule,
                                float_dc,
                                float_schedule: &float_schedule,
                                float_spread_bp: 0.0,
                            })?;
                        if float_ann.abs() < 1e-12 {
                            return Err(finstack_core::Error::Validation(
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
                            &fixed_schedule,
                        )?;
                        if ann.abs() < 1e-12 {
                            return Err(finstack_core::Error::Validation(
                                "ASW market calculation is undefined for near-zero fixed-leg annuity"
                                    .to_string(),
                            ));
                        }
                        return Ok((coupon - par_rate) + (1.0 - clean_px / 100.0) / ann);
                    }
                }
            }
        }
        if quote_ctx.quote_date >= maturity {
            return Err(finstack_core::Error::Validation(
                "ASW market calculation requires at least two fixed-leg schedule dates".to_string(),
            ));
        }
        let mut builder =
            finstack_core::dates::ScheduleBuilder::new(quote_ctx.quote_date, maturity)?
                .frequency(freq)
                .stub_rule(stub);

        if let Some(id) = calendar_id {
            if let Some(cal) = finstack_core::dates::calendar::calendar_by_id(id) {
                builder = builder.adjust_with(bdc, cal);
            }
        }

        let sched: Vec<Date> = builder.build()?.into_iter().collect();
        if sched.len() < 2 {
            return Err(finstack_core::Error::Validation(
                "ASW market calculation requires at least two fixed-leg schedule dates".to_string(),
            ));
        }
        let dc_fixed = self.config.fixed_leg_day_count.unwrap_or(dc);
        let forward_components = if let Some(fwd_id) = asw_forward_curve_id {
            let fwd = context.curves.get_forward(&fwd_id)?;
            Some(asset_swap_forward_components(
                disc.as_ref(),
                fwd.as_ref(),
                dc_fixed,
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
            par_rate_and_annuity_from_discount(disc.as_ref(), dc_fixed, &sched)?
        };
        if ann.abs() < 1e-12 {
            return Err(finstack_core::Error::Validation(
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
        let price_pct = dirty_ccy / notional_amt;
        let asw_mkt = if let Some((float_pv, fixed_ann, float_ann)) = forward_components {
            (eq_coupon * fixed_ann + 1.0 - price_pct - float_pv) / float_ann
        } else {
            // Market ASW = Par ASW + upfront discount amortized over the fixed-leg annuity.
            (eq_coupon - par_rate) + (1.0 - price_pct) / ann
        };
        Ok(asw_mkt)
    }
}
