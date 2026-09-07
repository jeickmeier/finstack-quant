use finstack_quant_calibration::api::engine;
use finstack_quant_calibration::api::market_datum::MarketDatum;
use finstack_quant_calibration::api::prior_market::PriorMarketObject;
use finstack_quant_calibration::api::schema::{
    CalibrationEnvelope, CalibrationPlan, CalibrationSchema, CalibrationStep, HazardCurveParams,
    StepParams,
};
use finstack_quant_calibration::quotes::cds::CdsQuote;
use finstack_quant_calibration::quotes::ids::{Pillar, QuoteId};
use finstack_quant_calibration::quotes::market_quote::MarketQuote;
use finstack_quant_calibration::{CalibrationConfig, CalibrationMethod};
use finstack_quant_core::{
    currency::Currency,
    dates::Date,
    market_data::context::MarketContext,
    market_data::term_structures::{HazardCurve, ParInterp, Seniority},
    math::interp::InterpStyle,
    money::Money,
    types::{CurveId, InstrumentId},
    HashMap,
};
use finstack_quant_valuations::constants::isda::STANDARD_RECOVERY_SENIOR;
use finstack_quant_valuations::instruments::credit_derivatives::cds::{
    CdsConvention, CdsValuationConvention, CreditDefaultSwap, PayReceive, PremiumLegSpec,
    ProtectionLegSpec,
};
use finstack_quant_valuations::instruments::{
    Attributes, InstrumentPricingOverrides, PricingOptions,
};
use finstack_quant_valuations::market::conventions::ids::{CdsConventionKey, CdsDocClause};
use rust_decimal::Decimal;
use std::sync::Arc;

/// Build request-scoped pricing options with a fresh cached recalibration provider.
pub fn pricing_options() -> PricingOptions {
    PricingOptions::default().with_recalibration_provider(Arc::new(
        finstack_quant_calibration::recalibration::CachedRecalibrationProvider::new(),
    ))
}

/// Inputs for a test-only hazard curve calibrated from CDS par quotes.
pub struct CalibratedHazardSpec<'a> {
    /// Market that already contains the discount curve used as prior.
    pub source_market: &'a MarketContext,
    /// Curve base / valuation date.
    pub base_date: Date,
    /// Identifier stamped on the calibrated hazard curve.
    pub curve_id: &'a str,
    /// CDS reference entity used in the synthetic par quotes.
    pub entity: &'a str,
    /// Discount curve already present on `source_market`.
    pub discount_curve_id: &'a str,
    /// `(pillar_date, par_spread_bp)` pairs consumed by the bootstrap.
    pub pillars: &'a [(Date, f64)],
    /// Recovery rate as a decimal fraction in `[0, 1]`.
    pub recovery_rate: f64,
    /// Optional CDS valuation convention for the bootstrap CDS.
    pub cds_valuation_convention: Option<CdsValuationConvention>,
}

/// Calibrate a replayable hazard curve from explicit pillar dates and quotes.
///
/// # Arguments
///
/// * `spec` - Discount-curve market, curve identifiers, CDS par pillars in
///   `(date, spread_bp)` form, recovery as a decimal fraction in `[0, 1]`, and
///   the optional CDS valuation convention used by the bootstrap instruments.
pub fn calibrated_hazard_curve_from_spec(
    spec: CalibratedHazardSpec<'_>,
) -> finstack_quant_core::Result<HazardCurve> {
    let curve_id = CurveId::new(spec.curve_id);
    let discount_curve_id = CurveId::new(spec.discount_curve_id);
    let quotes: Vec<_> = spec
        .pillars
        .iter()
        .map(|(pillar_date, spread_bp)| {
            MarketQuote::Cds(CdsQuote::CdsParSpread {
                id: QuoteId::new(format!("{}-{pillar_date}", spec.entity)),
                entity: spec.entity.to_string(),
                pillar: Pillar::Date(*pillar_date),
                spread_bp: *spread_bp,
                recovery_rate: spec.recovery_rate,
                convention: CdsConventionKey {
                    currency: Currency::USD,
                    doc_clause: CdsDocClause::IsdaNa,
                },
            })
        })
        .collect();
    let params = StepParams::Hazard(HazardCurveParams {
        curve_id: curve_id.clone(),
        entity: spec.entity.to_string(),
        seniority: Seniority::Senior,
        currency: Currency::USD,
        base_date: spec.base_date,
        discount_curve_id: discount_curve_id.clone(),
        recovery_rate: spec.recovery_rate,
        notional: 1.0,
        method: CalibrationMethod::Bootstrap,
        interpolation: InterpStyle::LogLinear,
        par_interp: ParInterp::Linear,
        doc_clause: None,
        cds_valuation_convention: spec.cds_valuation_convention,
    });
    let quote_ids = quotes
        .iter()
        .map(|quote| match quote {
            MarketQuote::Cds(cds) => cds.id().clone(),
            _ => unreachable!("fixture only builds CDS quotes"),
        })
        .collect();
    let mut quote_sets = HashMap::default();
    quote_sets.insert("credit".to_string(), quote_ids);
    let discount = spec
        .source_market
        .get_discount(discount_curve_id.as_str())?;
    let envelope = CalibrationEnvelope {
        schema_url: None,
        schema: CalibrationSchema::CURRENT,
        plan: CalibrationPlan {
            id: format!("{}-hazard-fixture", spec.entity),
            description: None,
            quote_sets: quote_sets.into_iter().collect(),
            settings: CalibrationConfig::default(),
            steps: vec![CalibrationStep {
                id: "hazard".to_string(),
                quote_set: "credit".to_string(),
                params,
            }],
        },
        market_data: quotes.into_iter().map(MarketDatum::from).collect(),
        prior_market: vec![PriorMarketObject::DiscountCurve(discount.as_ref().clone())],
    };
    let result = engine::execute(&envelope)?;
    if !result.result.report.success {
        return Err(finstack_quant_core::Error::Calibration {
            message: format!("test hazard calibration failed for '{curve_id}'"),
            category: "test_fixture".to_string(),
        });
    }
    let calibrated_market = MarketContext::try_from(result.result.final_market)?;
    calibrated_market
        .get_hazard(curve_id.as_str())
        .map(|curve| curve.as_ref().clone())
}

/// Calibrate a replayable USD senior hazard curve from standard CDS par quotes.
pub fn calibrated_hazard_curve(
    source_market: &MarketContext,
    base_date: Date,
    curve_id: impl Into<CurveId>,
    entity: &str,
    discount_curve_id: impl Into<CurveId>,
) -> finstack_quant_core::Result<HazardCurve> {
    calibrated_hazard_curve_with_pillars(
        source_market,
        base_date,
        curve_id,
        entity,
        discount_curve_id,
        &[
            (365, 100.0),
            (3 * 365, 100.0),
            (5 * 365, 100.0),
            (10 * 365, 100.0),
        ],
    )
}

/// Calibrate a replayable hazard curve from explicit day-offset par pillars.
pub fn calibrated_hazard_curve_with_pillars(
    source_market: &MarketContext,
    base_date: Date,
    curve_id: impl Into<CurveId>,
    entity: &str,
    discount_curve_id: impl Into<CurveId>,
    pillars: &[(i64, f64)],
) -> finstack_quant_core::Result<HazardCurve> {
    let curve_id = curve_id.into();
    let discount_curve_id = discount_curve_id.into();
    let dated: Vec<(Date, f64)> = pillars
        .iter()
        .map(|(day_offset, spread_bp)| (base_date + time::Duration::days(*day_offset), *spread_bp))
        .collect();
    calibrated_hazard_curve_from_spec(CalibratedHazardSpec {
        source_market,
        base_date,
        curve_id: curve_id.as_str(),
        entity,
        discount_curve_id: discount_curve_id.as_str(),
        pillars: &dated,
        recovery_rate: STANDARD_RECOVERY_SENIOR,
        cds_valuation_convention: None,
    })
}

/// Create a CDS buy protection position using the builder pattern.
#[allow(clippy::too_many_arguments)]
pub fn cds_buy_protection(
    id: impl Into<InstrumentId>,
    notional: Money,
    spread_bp: f64,
    start: Date,
    maturity: Date,
    discount_curve_id: impl Into<CurveId>,
    credit_id: impl Into<CurveId>,
) -> finstack_quant_core::Result<CreditDefaultSwap> {
    let convention = CdsConvention::IsdaNa;
    let day_count = convention.day_count();
    let frequency = convention.frequency();
    let business_day_convention = convention.business_day_convention();
    let stub = convention.stub_convention();

    let spread_bp_decimal = Decimal::try_from(spread_bp).map_err(|e| {
        finstack_quant_core::Error::Validation(format!(
            "spread_bp {} cannot be represented as Decimal: {}",
            spread_bp, e
        ))
    })?;

    let cds = CreditDefaultSwap::builder()
        .id(id.into())
        .notional(notional)
        .side(PayReceive::Pay)
        .convention(convention)
        .premium(PremiumLegSpec {
            start,
            end: maturity,
            frequency,
            stub,
            business_day_convention,
            calendar_id: Some(convention.default_calendar().to_string()),
            day_count,
            spread_bp: spread_bp_decimal,
            discount_curve_id: discount_curve_id.into(),
        })
        .protection(ProtectionLegSpec {
            credit_curve_id: credit_id.into(),
            recovery_rate: STANDARD_RECOVERY_SENIOR,
            settlement_delay: convention.settlement_delay(),
        })
        .instrument_pricing_overrides(InstrumentPricingOverrides::default())
        .attributes(Attributes::new())
        .build()?;

    cds.validate()?;
    Ok(cds)
}

/// Create a CDS sell protection position using the builder pattern.
#[allow(clippy::too_many_arguments)]
pub fn cds_sell_protection(
    id: impl Into<InstrumentId>,
    notional: Money,
    spread_bp: f64,
    start: Date,
    maturity: Date,
    discount_curve_id: impl Into<CurveId>,
    credit_id: impl Into<CurveId>,
) -> finstack_quant_core::Result<CreditDefaultSwap> {
    let convention = CdsConvention::IsdaNa;
    let day_count = convention.day_count();
    let frequency = convention.frequency();
    let business_day_convention = convention.business_day_convention();
    let stub = convention.stub_convention();

    let spread_bp_decimal = Decimal::try_from(spread_bp).map_err(|e| {
        finstack_quant_core::Error::Validation(format!(
            "spread_bp {} cannot be represented as Decimal: {}",
            spread_bp, e
        ))
    })?;

    let cds = CreditDefaultSwap::builder()
        .id(id.into())
        .notional(notional)
        .side(PayReceive::Receive)
        .convention(convention)
        .premium(PremiumLegSpec {
            start,
            end: maturity,
            frequency,
            stub,
            business_day_convention,
            calendar_id: Some(convention.default_calendar().to_string()),
            day_count,
            spread_bp: spread_bp_decimal,
            discount_curve_id: discount_curve_id.into(),
        })
        .protection(ProtectionLegSpec {
            credit_curve_id: credit_id.into(),
            recovery_rate: STANDARD_RECOVERY_SENIOR,
            settlement_delay: convention.settlement_delay(),
        })
        .instrument_pricing_overrides(InstrumentPricingOverrides::default())
        .attributes(Attributes::new())
        .build()?;

    cds.validate()?;
    Ok(cds)
}
