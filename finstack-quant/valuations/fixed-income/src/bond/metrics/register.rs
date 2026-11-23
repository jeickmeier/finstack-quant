//! Metric registration and calculators.

use super::{
    AccruedInterestCalculator, AssetSwapMarketCalculator, AssetSwapParCalculator,
    BondBucketedCs01Calculator, BondBucketedDv01Calculator, BondCs01Calculator, BondDv01Calculator,
    BondSpreadDurationCalculator, BondVegaCalculator, BondWalCalculator, CleanPriceCalculator,
    ConvexityCalculator, DirtyPriceCalculator, DiscountMarginCalculator,
    EmbeddedOptionValueCalculator, GSpreadCalculator, ISpreadCalculator,
    JapaneseSimpleYieldCalculator, MacaulayDurationCalculator, ModifiedDurationCalculator,
    OasCalculator, YieldDv01Calculator, YtmCalculator, YtwCalculator, ZSpreadCalculator,
};
use crate::bond::pricing::quote_conversions::WorkoutPath;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;

/// Pricing model the metric request selected, else the bond's default model.
pub(crate) fn active_model(
    context: &finstack_quant_valuations_core::metrics::MetricContext,
    bond: &crate::bond::Bond,
) -> finstack_quant_valuations_core::pricer::ModelKey {
    context
        .pricing_model()
        .unwrap_or_else(|| bond.default_pricing_model())
}

/// `bond` with its price-driving quotes replaced by a quoted OAS (decimal).
pub(crate) fn oas_pinned(mut bond: crate::bond::Bond, oas: f64) -> crate::bond::Bond {
    crate::bond::pricing::quote_conversions::clear_price_driving_overrides(&mut bond);
    bond.instrument_pricing_overrides.market_quotes.quoted_oas = Some(oas);
    bond
}

/// `bond` with its price-driving quotes replaced by a quoted Z-spread (decimal).
pub(crate) fn z_pinned(mut bond: crate::bond::Bond, z_spread: f64) -> crate::bond::Bond {
    crate::bond::pricing::quote_conversions::clear_price_driving_overrides(&mut bond);
    bond.instrument_pricing_overrides
        .market_quotes
        .quoted_z_spread = Some(z_spread);
    bond
}

/// Reprice `bond` at `as_of` through the context's dispatch when the request
/// selected a model, else through `model` directly on the bond.
pub(crate) fn reprice_selected(
    context: &finstack_quant_valuations_core::metrics::MetricContext,
    bond: &crate::bond::Bond,
    model: finstack_quant_valuations_core::pricer::ModelKey,
    as_of: Date,
) -> finstack_quant_core::Result<f64> {
    if context.pricing_model().is_some() {
        context.reprice_instrument_raw(bond, context.curves.as_ref(), as_of)
    } else {
        bond.price_for_model_raw(model, context.curves.as_ref(), as_of)
    }
}

pub(crate) fn bond_risk_basis(
    context: &finstack_quant_valuations_core::metrics::MetricContext,
) -> finstack_quant_valuations_core::instruments::pricing_overrides::BondRiskBasis {
    context.get_metric_pricing_overrides().map_or_else(
        finstack_quant_valuations_core::instruments::pricing_overrides::BondRiskBasis::default,
        |overrides| overrides.bond_risk_basis_or_default(),
    )
}

/// [`quoted_workout_path`](crate::bond::pricing::quote_conversions::quoted_workout_path) for the context's bond on its entitled flows at
/// the quote date, computed once per metric request and cached on the
/// context.
pub(crate) fn context_workout_path(
    context: &mut finstack_quant_valuations_core::metrics::MetricContext,
) -> finstack_quant_core::Result<Option<WorkoutPath>> {
    if let Some(cached) = context.ext::<WorkoutPathCache>() {
        return Ok(cached.0.clone());
    }
    let path = {
        let bond: &crate::bond::Bond = context.instrument_as()?;
        let may_have_workout = bond.has_exercise_rights()
            && bond
                .instrument_pricing_overrides
                .market_quotes
                .quoted_clean_price_pct
                .is_some();
        if !may_have_workout {
            context.set_ext(WorkoutPathCache(None));
            return Ok(None);
        }
        let quote_ctx = crate::bond::pricing::settlement::QuoteDateContext::new(
            bond,
            &context.curves,
            context.as_of,
        )?;
        let flows = quote_ctx.entitled_flows(bond, &context.curves, context.as_of)?;
        crate::bond::pricing::quote_conversions::quoted_workout_path(
            bond,
            context.curves.as_ref(),
            context.as_of,
            &flows,
        )?
    };
    context.set_ext(WorkoutPathCache(path.clone()));
    Ok(path)
}

/// Quoted-yield risk inputs shared by Macaulay duration, convexity and yield
/// DV01: the workout yield and path when one is quoted, else `ytm` on the
/// context's cached flows, timed from the quote date.
pub(crate) struct YieldRiskBasis {
    /// Yield the risk is measured at (workout yield or YTM).
    pub(crate) yield_rate: f64,
    /// Risk cashflows with their year fractions from the quote date.
    pub(crate) timed: Vec<(Date, f64, Money)>,
    /// Effective discount law for the dated yield experiment.
    pub(crate) compounding: crate::bond::pricing::quote_conversions::YieldCompounding,
    /// Quote-law price of `timed` at `yield_rate`.
    pub(crate) price: f64,
}

/// Build the [`YieldRiskBasis`] for the context's bond.
///
/// # Arguments
///
/// * `context` - Metric context whose cached cashflows (built by the `Ytm`
///   dependency) and quoted workout path are used.
/// * `ytm` - Yield to maturity used when the bond has no quoted workout path.
pub(crate) fn yield_risk_basis(
    context: &mut finstack_quant_valuations_core::metrics::MetricContext,
    ytm: f64,
) -> finstack_quant_core::Result<YieldRiskBasis> {
    let workout = context_workout_path(context)?;
    let bond: &crate::bond::Bond = context.instrument_as()?;
    let flows = context.require_cashflows()?;
    let quote_date = crate::bond::pricing::settlement::settlement_date(bond, context.as_of)?;
    let (yield_rate, risk_flows) = match workout {
        Some(path) => (path.yield_rate, std::borrow::Cow::Owned(path.flows)),
        None => (ytm, std::borrow::Cow::Borrowed(flows)),
    };
    let (timed, compounding) = crate::bond::pricing::quote_conversions::street_flow_times(
        bond,
        risk_flows.as_ref(),
        quote_date,
    )?;
    let price = crate::bond::pricing::quote_conversions::price_from_ytm_timed(
        &timed,
        bond.cashflow_spec.frequency(),
        yield_rate,
        compounding,
    )?;
    Ok(YieldRiskBasis {
        yield_rate,
        timed,
        compounding,
        price,
    })
}

/// Quoted workout path cached on the metric context for the bond under
/// evaluation; `None` records that the bond has no quoted workout path.
struct WorkoutPathCache(Option<WorkoutPath>);

/// Registers all bond metrics to a registry.
///
/// This function registers all bond-specific metric calculators to the provided
/// metric registry, enabling computation of price, yield, duration, convexity,
/// and spread metrics for bonds.
///
/// # Arguments
///
/// * `registry` - The metric registry to register bond metrics into
pub(crate) fn register_bond_metrics(
    registry: &mut finstack_quant_valuations_core::metrics::MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use crate::bond::pricing::quote_conversions::YieldCompounding;
    use finstack_quant_valuations_core::metrics::sensitivities::cross_factor::{
        CrossFactorCalculator, make_credit_bumper, make_rates_bumper,
    };
    use finstack_quant_valuations_core::pricer::InstrumentType;

    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::Bond,
        metrics: [
            (Accrued, AccruedInterestCalculator, NativeMoney),
            (DirtyPrice, DirtyPriceCalculator),
            (CleanPrice, CleanPriceCalculator),
            (Ytm, YtmCalculator { compounding: YieldCompounding::Street }),
            (Ytw, YtwCalculator),
            (JapaneseSimpleYield, JapaneseSimpleYieldCalculator),
            (MoosmullerYtm, YtmCalculator { compounding: YieldCompounding::Moosmuller }),

            (DurationMac, MacaulayDurationCalculator),
            (DurationMod, ModifiedDurationCalculator),
            (YieldDv01, YieldDv01Calculator),
            (Convexity, ConvexityCalculator),

            (Oas, OasCalculator),
            (EmbeddedOptionValue, EmbeddedOptionValueCalculator),
            (Vega, BondVegaCalculator),
            (ZSpread, ZSpreadCalculator),
            (ISpread, ISpreadCalculator),
            (GSpread, GSpreadCalculator),
            (DiscountMargin, DiscountMarginCalculator),
            (ASWPar, AssetSwapParCalculator),
            (ASWMarket, AssetSwapMarketCalculator),

            (WAL, BondWalCalculator),
            (CarryTotal, finstack_quant_valuations_core::metrics::sensitivities::carry_decomposition::CarryDecompositionCalculator::with_clock(super::carry::BondYieldClock)),

            (Dv01, BondDv01Calculator),
            (BucketedDv01, BondBucketedDv01Calculator),

            (Cs01, BondCs01Calculator),
            (BucketedCs01, BondBucketedCs01Calculator),
            // Always quote-reproducing Z-spread risk, even when CS01 uses a
            // hazard/par-CDS curve.
            (SpreadDuration, BondSpreadDurationCalculator),

            (Recovery01, super::default_risk::BondRecovery01Calculator),
            (JumpToDefault, super::default_risk::BondJumpToDefaultCalculator),

            (Moic, super::return_metrics::moic::MoicCalculator),
            (MoicToWorst, super::return_metrics::moic::MoicToWorstCalculator),
            (Xirr, super::return_metrics::xirr::XirrCalculator),
            (XirrToWorst, super::return_metrics::xirr::XirrToWorstCalculator),
            (CrossGammaRatesCredit, CrossFactorCalculator::new(make_rates_bumper, make_credit_bumper)),

        ]
    };
    Ok(())
}
