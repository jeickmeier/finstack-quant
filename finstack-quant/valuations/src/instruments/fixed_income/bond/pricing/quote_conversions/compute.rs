use super::spread_price::{
    par_swap_rate_from_discount, price_from_asw_market, price_from_dm, price_from_oas,
    price_from_z_spread,
};
use super::types::{BondQuoteInput, BondQuoteSet};
use super::yield_price::{price_from_japanese_simple_yield, price_from_ytm, price_from_ytw};
use crate::constants::numerical::ZERO_TOLERANCE;
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::fixed_income::bond::pricing::settlement::QuoteDateContext;
use crate::instruments::fixed_income::bond::Bond;
use crate::instruments::PricingOptions;
use crate::metrics::{standard_registry, MetricContext, MetricId};
use crate::pricer::{shared_standard_registry, ModelKey, PricingDispatch};
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::Result;
use std::sync::Arc;

/// Clear all price-driving market-quote overrides on a bond so downstream
/// pricing calls evaluate the model PV. Used by inversion helpers that need
/// the raw model response even when the bond carries a quoted price override.
pub(crate) fn clear_price_driving_overrides(bond: &mut Bond) {
    bond.instrument_pricing_overrides
        .market_quotes
        .clear_price_drivers();
}

/// Convert between price, yield, and spread metrics for a bond.
///
/// The engine:
/// - Normalizes the chosen `quote_input` into a **canonical dirty price in currency**.
/// - Derives the corresponding clean price (% of par) and stamps it into
///   `pricing_overrides.quoted_clean_price` on an internal bond clone.
/// - Uses the selected pricing and metric registries to compute the remaining
///   metrics.
///
/// # Arguments
///
/// * `bond` - Bond to normalize and value. The function clones it before
///   applying the derived clean-price override, so the caller's instance is
///   unchanged.
/// * `curves` - Market context supplying the bond schedule, discount curves,
///   forward curves, and other metric dependencies.
/// * `as_of` - Valuation or trade date from which settlement-aware accrued
///   interest and clean/dirty conversion are determined.
/// * `quote_input` - One observed clean/dirty price, yield, or spread quote
///   used to seed the internally consistent quote set.
/// * `options` - Pricing model, pricer registry, metric registry, configuration,
///   market history, and recalibration provider used by the quote conversion
///   and every downstream metric reprice. When no model is supplied, the
///   bond's default model is used.
///
/// # Returns
///
/// A `BondQuoteSet` containing all computed price, yield, and spread metrics.
///
/// # Errors
///
/// Returns `Err` when the input quote cannot be normalized, required market
/// data or cashflows are unavailable for that normalization, or the selected
/// model cannot produce the base value. Metrics that do not apply to the bond
/// are left unset in the returned quote set.
///
/// # Examples
///
/// ```
/// use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
/// use finstack_quant_valuations::instruments::fixed_income::bond::pricing::quote_conversions::{compute_quotes, BondQuoteInput};
/// use finstack_quant_core::market_data::context::MarketContext;
/// use finstack_quant_core::market_data::term_structures::DiscountCurve;
/// use finstack_quant_core::dates::Date;
///
/// # let as_of = Date::from_calendar_date(2024, time::Month::January, 15).unwrap();
/// # let bond = Bond::example().unwrap();
/// # let curves = MarketContext::new().insert(
/// #     DiscountCurve::builder("USD-TREASURY")
/// #         .base_date(as_of)
/// #         .knots([(0.0, 1.0), (10.0, 0.6)])
/// #         .build()?,
/// # );
/// let quotes = compute_quotes(
///     &bond,
///     &curves,
///     as_of,
///     BondQuoteInput::CleanPricePct(98.5),
///     finstack_quant_valuations::instruments::PricingOptions::default()
///         .with_model(finstack_quant_valuations::pricer::ModelKey::Discounting),
/// )?;
/// assert_eq!(quotes.clean_price_pct, 98.5);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn compute_quotes(
    bond: &Bond,
    curves: &MarketContext,
    as_of: Date,
    quote_input: BondQuoteInput,
    options: PricingOptions,
) -> Result<BondQuoteSet> {
    let model = options.model.unwrap_or_else(|| bond.default_model());
    let pricer_registry = options
        .registry
        .clone()
        .unwrap_or_else(shared_standard_registry);
    let pricing_dispatch = PricingDispatch::registered(model, Arc::clone(&pricer_registry));

    // Work on a local clone so we never mutate the caller's bond instance.
    let mut bond_for_metrics = bond.clone();

    // Quote normalization (clean/dirty conversion) must use accrued at quote/settlement date.
    let quote_ctx = QuoteDateContext::new(&bond_for_metrics, curves, as_of)?;
    let accrued_currency = quote_ctx.accrued_at_quote_date;

    let notional = bond_for_metrics.notional.amount();
    if notional.abs() < ZERO_TOLERANCE {
        return Ok(BondQuoteSet {
            clean_price_currency: 0.0,
            clean_price_pct: 0.0,
            dirty_price_currency: 0.0,
            ytm: None,
            ytw: None,
            z_spread: None,
            discount_margin: None,
            oas: None,
            asw_par: None,
            asw_market: None,
            i_spread: None,
            japanese_simple_yield: None,
            moosmuller_ytm: None,
        });
    }

    // 1) Stamp the quote input into the corresponding settlement-price driver,
    //    validate it through the canonical bond boundary, and normalize it
    //    without routing the settlement value through `Instrument::value`
    //    (whose contract is always an `as_of` NPV).
    clear_price_driving_overrides(&mut bond_for_metrics);
    {
        let quotes = &mut bond_for_metrics.instrument_pricing_overrides.market_quotes;
        match quote_input {
            BondQuoteInput::CleanPricePct(v) => quotes.quoted_clean_price = Some(v),
            BondQuoteInput::DirtyPriceCurrency(v) => quotes.quoted_dirty_price_currency = Some(v),
            BondQuoteInput::Ytm(v) => quotes.quoted_ytm = Some(v),
            BondQuoteInput::Ytw(v) => quotes.quoted_ytw = Some(v),
            BondQuoteInput::ZSpread(v) => quotes.quoted_z_spread = Some(v),
            BondQuoteInput::DiscountMargin(v) => quotes.quoted_discount_margin = Some(v),
            BondQuoteInput::Oas(v) => quotes.quoted_oas = Some(v),
            BondQuoteInput::AswMarket(v) => quotes.quoted_asw_market = Some(v),
            BondQuoteInput::ISpread(v) => quotes.quoted_i_spread = Some(v),
            BondQuoteInput::JapaneseSimpleYield(v) => quotes.quoted_japanese_simple_yield = Some(v),
        }
    }
    bond_for_metrics.validate()?;

    let dirty_price_currency = settlement_dirty_from_quote_overrides(
        &bond_for_metrics,
        curves,
        as_of,
        model,
        Some(&pricing_dispatch),
    )?
    .ok_or_else(|| {
        finstack_quant_core::Error::Validation(
            "bond quote input did not produce a settlement dirty price".to_string(),
        )
    })?;
    let clean_price_currency = dirty_price_currency - accrued_currency;
    let clean_price_pct = clean_price_currency / notional * 100.0;

    // Stamp the canonical clean price quote into pricing_overrides so that all
    // existing metric calculators interpret this as the market price.
    // (Replaces the specific quote field with the clean-price normalization
    // expected by the downstream metric calculators.)
    clear_price_driving_overrides(&mut bond_for_metrics);
    bond_for_metrics
        .instrument_pricing_overrides
        .market_quotes
        .quoted_clean_price = Some(clean_price_pct);

    // 2) Build metric context with the same model and registries for the rest.
    let base_value = finstack_quant_core::money::Money::new(
        pricing_dispatch.price_raw(&bond_for_metrics, curves, as_of)?,
        bond_for_metrics.notional.currency(),
    );
    let metric_registry = match options.metric_registry.as_deref() {
        Some(registry) => registry,
        None => standard_registry(),
    };

    let instrument_arc: Arc<dyn Instrument> = Arc::new(bond_for_metrics.clone());
    let curves_arc = Arc::new(curves.clone());
    let mut ctx = MetricContext::new(
        instrument_arc,
        curves_arc,
        as_of,
        base_value,
        options
            .config
            .clone()
            .unwrap_or_else(MetricContext::default_config),
    );
    if let Some(history) = options.market_history.clone() {
        ctx = ctx.with_market_history(history);
    }
    ctx.set_recalibration_provider(options.recalibration_provider.clone());
    ctx.set_pricer_dispatch(pricing_dispatch);
    ctx.set_instrument_overrides(bond_for_metrics.get_instrument_pricing_overrides().cloned());
    ctx.set_metric_overrides(bond_for_metrics.get_metric_pricing_overrides().cloned());
    bond_for_metrics.seed_metric_context(&mut ctx, curves, as_of);
    ctx.notional = Some(bond_for_metrics.notional);

    // Pre-populate accrued since we've already computed it.
    ctx.computed.insert(MetricId::Accrued, accrued_currency);

    // Request the core price/yield/spread metrics.
    let metric_ids = [
        MetricId::Ytm,
        MetricId::Ytw,
        MetricId::ZSpread,
        MetricId::DiscountMargin,
        MetricId::Oas,
        MetricId::ASWPar,
        MetricId::ASWMarket,
        MetricId::ISpread,
        MetricId::JapaneseSimpleYield,
        MetricId::MoosmullerYtm,
    ];

    // Some quote metrics are not applicable to all bond types (e.g. FRN vs fixed),
    // and we want `compute_quotes` to return whatever is available rather than
    // failing the entire quote set.
    for metric_id in &metric_ids {
        if let Err(err) = metric_registry.compute(std::slice::from_ref(metric_id), &mut ctx) {
            tracing::debug!(
                metric_id = metric_id.as_str(),
                error = %err,
                "Bond quote engine metric computation failed; leaving unset"
            );
        }
    }

    // Read back the metrics we care about.
    let ytm = ctx.computed.get(&MetricId::Ytm).copied();
    let ytw = ctx.computed.get(&MetricId::Ytw).copied();
    let z_spread = ctx.computed.get(&MetricId::ZSpread).copied();
    let discount_margin = ctx.computed.get(&MetricId::DiscountMargin).copied();
    let oas = ctx.computed.get(&MetricId::Oas).copied();
    let asw_par = ctx.computed.get(&MetricId::ASWPar).copied();
    let asw_market = ctx.computed.get(&MetricId::ASWMarket).copied();
    let i_spread = ctx.computed.get(&MetricId::ISpread).copied();
    let japanese_simple_yield = ctx.computed.get(&MetricId::JapaneseSimpleYield).copied();
    let moosmuller_ytm = ctx.computed.get(&MetricId::MoosmullerYtm).copied();

    Ok(BondQuoteSet {
        clean_price_currency,
        clean_price_pct,
        dirty_price_currency,
        ytm,
        ytw,
        z_spread,
        discount_margin,
        oas,
        asw_par,
        asw_market,
        i_spread,
        japanese_simple_yield,
        moosmuller_ytm,
    })
}

/// Resolve a bond price override into settlement-date dirty currency units.
///
/// The returned value is deliberately not an [`Instrument::value`] result:
/// it remains anchored at the bond's quote/settlement date. Callers that need
/// an `as_of` NPV must use
/// [`crate::instruments::fixed_income::bond::pricing::settlement::quote_dirty_at_as_of`].
///
/// `model` identifies the bond model for the native fallback. When
/// `pricing_dispatch` is present, OAS quotes are priced through that dispatch
/// so custom pricer registries remain authoritative; other quote forms retain
/// their market-convention inversion helpers.
///
/// Returns `Ok(None)` when no price-driving override is configured.
pub(crate) fn settlement_dirty_from_quote_overrides(
    bond: &Bond,
    curves: &MarketContext,
    as_of: Date,
    model: ModelKey,
    pricing_dispatch: Option<&PricingDispatch>,
) -> Result<Option<f64>> {
    let quotes = &bond.instrument_pricing_overrides.market_quotes;
    quotes.validate()?;

    if !quotes.has_price_driver() {
        return Ok(None);
    }
    let quote_ctx = QuoteDateContext::new(bond, curves, as_of)?;
    let notional = bond.notional.amount();
    let dirty = if let Some(dirty) = quotes.quoted_dirty_price_currency {
        dirty
    } else if let Some(clean_pct) = quotes.quoted_clean_price {
        quote_ctx.dirty_from_clean_pct(clean_pct, notional)
    } else if let Some(ytm) = quotes.quoted_ytm {
        let flows = quote_ctx.entitled_flows(bond, curves, as_of)?;
        price_from_ytm(bond, &flows, quote_ctx.quote_date, ytm)?
    } else if let Some(ytw) = quotes.quoted_ytw {
        price_from_ytw(bond, curves, as_of, ytw)?
    } else if let Some(z) = quotes.quoted_z_spread {
        price_from_z_spread(bond, curves, as_of, z)?
    } else if let Some(oas) = quotes.quoted_oas {
        match pricing_dispatch {
            Some(dispatch) => dispatch.price_raw(bond, curves, quote_ctx.quote_date)?,
            None => price_from_oas(bond, curves, quote_ctx.quote_date, model, oas)?,
        }
    } else if let Some(dm) = quotes.quoted_discount_margin {
        price_from_dm(bond, curves, quote_ctx.quote_date, dm)?
    } else if let Some(i_spread) = quotes.quoted_i_spread {
        let par_swap_rate = par_swap_rate_from_discount(bond, curves, quote_ctx.quote_date)?;
        let flows = quote_ctx.entitled_flows(bond, curves, as_of)?;
        price_from_ytm(bond, &flows, quote_ctx.quote_date, i_spread + par_swap_rate)?
    } else if let Some(asw) = quotes.quoted_asw_market {
        price_from_asw_market(bond, curves, quote_ctx.quote_date, asw)?
    } else if let Some(simple_yield) = quotes.quoted_japanese_simple_yield {
        price_from_japanese_simple_yield(bond, quote_ctx.quote_date, simple_yield)?
    } else {
        return Ok(None);
    };

    if !dirty.is_finite() || dirty <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "bond quote implies a non-positive or non-finite settlement dirty price: {dirty}"
        )));
    }

    Ok(Some(dirty))
}
