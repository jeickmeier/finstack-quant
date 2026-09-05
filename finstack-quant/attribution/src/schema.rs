//! Schema registry for the attribution crate.
//!
//! The registry lives in the library rather than the generator binary so the
//! generator, the contract tests and the language bindings all render from one
//! definition. Render an entry with
//! [`SchemaArtifact::generate`](finstack_quant_core::schema::SchemaArtifact::generate);
//! `generated_schema` produces only the raw derived document and drifts from
//! the checked-in bytes.

use finstack_quant_core::schema::{SchemaArtifact, SchemaKind};

use crate::{AttributionEnvelope, AttributionResultEnvelope};

/// Stable base URI shared by every attribution artifact.
pub const ATTRIBUTION_SCHEMA_BASE: &str = "https://finstack_quant.dev/schemas/attribution/1/";

/// A canonical attribution result: a small carry-only P&L decomposition.
///
/// Effects that did not contribute are explicit zeros rather than omitted, which
/// is what the wire actually carries and what a reader has to interpret. The
/// results metadata drops its timestamp so the published example is stable
/// across regenerations.
fn attribution_result_examples() -> finstack_quant_core::Result<Vec<serde_json::Value>> {
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::money::Money;

    let date = |month, day| {
        finstack_quant_core::dates::Date::from_calendar_date(2024, month, day).map_err(|error| {
            finstack_quant_core::Error::Internal(format!("build example date: {error}"))
        })
    };
    let as_of_t0 = date(time::Month::January, 1)?;
    let as_of_t1 = date(time::Month::February, 1)?;
    let zero = Money::from((0_i64, Currency::USD));
    let attribution = crate::PnlAttribution {
        total_pnl: Money::from((12500_i64, Currency::USD)),
        mark_to_market_pnl: None,
        carry: Money::from((12500_i64, Currency::USD)),
        rates_curves_pnl: zero,
        credit_curves_pnl: zero,
        inflation_curves_pnl: zero,
        correlations_pnl: zero,
        fx_pnl: zero,
        fx_translation_pnl: zero,
        vol_pnl: zero,
        cross_factor_pnl: zero,
        model_params_pnl: zero,
        market_scalars_pnl: zero,
        residual: zero,
        carry_detail: None,
        rates_detail: None,
        credit_detail: None,
        inflation_detail: None,
        correlations_detail: None,
        fx_detail: None,
        vol_detail: None,
        cross_factor_detail: None,
        model_params_detail: None,
        scalars_detail: None,
        credit_factor_detail: None,
        credit_carry_decomposition: None,
        meta: crate::AttributionMeta {
            t0: as_of_t0,
            t1: as_of_t1,
            method: crate::AttributionMethod::default(),
            instrument_id: "DEP_3M".to_string(),
            num_repricings: 1,
            tolerance_abs: 1e-6,
            tolerance_pct: 1e-6,
            residual_pct: 0.0,
            rounding: Default::default(),
            fx_policy: None,
            execution_policy: None,
            notes: Vec::new(),
        },
        result_invalid: false,
    };
    let results_meta = finstack_quant_core::config::ResultsMeta {
        timestamp: None,
        ..finstack_quant_core::config::results_meta_now(
            &finstack_quant_core::config::FinstackConfig::default(),
        )
    };
    let result = crate::spec::AttributionResult {
        attribution,
        results_meta,
    };
    let value = serde_json::to_value(AttributionResultEnvelope::new(result)).map_err(|error| {
        finstack_quant_core::Error::Internal(format!(
            "serialize attribution result example: {error}"
        ))
    })?;
    Ok(vec![value])
}

/// A minimal but complete attribution run, built from public constructors.
///
/// A deposit priced against an empty market at two dates: enough to show the
/// envelope's shape - instrument, both market snapshots, both as-of dates and
/// the method - without the example carrying a whole calibrated market.
fn attribution_examples() -> finstack_quant_core::Result<Vec<serde_json::Value>> {
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::context::{MarketContext, MarketContextState};
    use finstack_quant_core::money::Money;
    use finstack_quant_valuations::instruments::json_loader::InstrumentJson;
    use finstack_quant_valuations::instruments::rates::deposit::Deposit;

    let date = |month, day| {
        finstack_quant_core::dates::Date::from_calendar_date(2024, month, day).map_err(|error| {
            finstack_quant_core::Error::Internal(format!("build example date: {error}"))
        })
    };
    let as_of_t0 = date(time::Month::January, 1)?;
    let as_of_t1 = date(time::Month::February, 1)?;
    let maturity = date(time::Month::April, 1)?;

    let deposit = Deposit::builder()
        .id("DEP_3M".into())
        .notional(Money::from((1000000_i64, Currency::USD)))
        .start_date(as_of_t0)
        .maturity(maturity)
        .day_count(finstack_quant_core::dates::DayCount::Act360)
        .discount_curve_id("USD-OIS".into())
        .build()
        .map_err(|error| {
            finstack_quant_core::Error::Internal(format!("build example deposit: {error}"))
        })?;

    let market = MarketContextState::from(&MarketContext::new());
    let spec = crate::spec::AttributionSpec {
        instrument: InstrumentJson::Deposit(deposit),
        market_t0: market.clone(),
        market_t1: market,
        as_of_t0,
        as_of_t1,
        method: crate::AttributionMethod::default(),
        model_params_t0: None,
        config: None,
        credit_factor_model: None,
        credit_factor_detail_options: crate::CreditFactorDetailOptions::default(),
        full_cross_attribution: false,
    };

    let value = serde_json::to_value(AttributionEnvelope::new(spec)).map_err(|error| {
        finstack_quant_core::Error::Internal(format!("serialize attribution example: {error}"))
    })?;
    Ok(vec![value])
}

/// The crate's complete schema registry, sorted by artifact path.
pub const ARTIFACTS: &[SchemaArtifact] = &[
    SchemaArtifact::new::<AttributionEnvelope>(
        "schemas/attribution/1/attribution.schema.json",
        "https://finstack_quant.dev/schemas/attribution/1/attribution.schema.json",
        "Finstack Quant Attribution Specification",
        "Complete specification for P&L attribution runs with instrument and market snapshots",
    )
    .with_kind(SchemaKind::Input)
    .with_summary("One multi-period P&L attribution run: positions, markets and method.")
    .with_examples(attribution_examples),
    SchemaArtifact::new::<AttributionResultEnvelope>(
        "schemas/attribution/1/attribution_result.schema.json",
        "https://finstack_quant.dev/schemas/attribution/1/attribution_result.schema.json",
        "Finstack Quant Attribution Result",
        "Complete result of a P&L attribution run including factor decomposition and metadata",
    )
    .with_kind(SchemaKind::Output)
    .with_summary("Attributed P&L by effect and grouping, with policy stamps.")
    .with_examples(attribution_result_examples),
];
