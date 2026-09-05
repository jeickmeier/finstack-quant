// Shared test helper fragment for attribution modules and integration tests.
use finstack_quant_calibration::api::engine;
use finstack_quant_calibration::api::market_datum::MarketDatum;
use finstack_quant_calibration::api::prior_market::PriorMarketObject;
use finstack_quant_calibration::api::schema::{
    CalibrationEnvelope, CalibrationPlan, CalibrationSchema, CalibrationStep, HazardCurveParams,
    StepParams,
};
use finstack_quant_calibration::quotes::cds::CdsQuote;
use finstack_quant_calibration::quotes::ids::{Pillar, QuoteId};
use finstack_quant_calibration::{CalibrationConfig, CalibrationMethod};
use finstack_quant_core::dates::{Date, Tenor, TenorUnit};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{
    DiscountCurve, HazardCurve, ParInterp, Seniority,
};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::HashMap;
use finstack_quant_core::Result;
use finstack_quant_valuations::instruments::{Attributes, Instrument, MarketDependencies};
use finstack_quant_valuations::market::conventions::ids::CdsConventionKey;
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_valuations::results::ValuationResult;
use smallvec::SmallVec;
use std::sync::OnceLock;

/// Calibrate a replayable hazard curve from explicit CDS par-spread pillars.
pub fn calibrated_hazard_curve(
    discount_curve: &DiscountCurve,
    base_date: Date,
    curve_id: impl Into<CurveId>,
    entity: &str,
    recovery_rate: f64,
    convention: CdsConventionKey,
    pillars: &[(u32, f64)],
) -> Result<HazardCurve> {
    let curve_id = curve_id.into();
    let quotes = pillars
        .iter()
        .map(|&(years, spread_bp)| CdsQuote::CdsParSpread {
            id: QuoteId::new(format!("{entity}-CDS-{years}Y")),
            entity: entity.to_string(),
            pillar: Pillar::Tenor(
                Tenor::new(years, TenorUnit::Years).expect("valid tenor fixture"),
            ),
            spread_bp,
            recovery_rate,
            convention: convention.clone(),
        })
        .collect::<Vec<_>>();
    let quote_ids = quotes.iter().map(|quote| quote.id().clone()).collect();
    let mut quote_sets = HashMap::default();
    quote_sets.insert("credit".to_string(), quote_ids);
    let envelope = CalibrationEnvelope {
        schema_url: None,
        schema: CalibrationSchema::CURRENT,
        plan: CalibrationPlan {
            id: format!("{entity}-hazard-fixture"),
            description: None,
            quote_sets: quote_sets.into_iter().collect(),
            settings: CalibrationConfig::default(),
            steps: vec![CalibrationStep {
                id: "hazard".to_string(),
                quote_set: "credit".to_string(),
                params: StepParams::Hazard(HazardCurveParams {
                    curve_id: curve_id.clone(),
                    entity: entity.to_string(),
                    seniority: Seniority::Senior,
                    currency: convention.currency,
                    base_date,
                    discount_curve_id: discount_curve.id().clone(),
                    recovery_rate,
                    notional: 1.0,
                    method: CalibrationMethod::Bootstrap,
                    interpolation: InterpStyle::LogLinear,
                    par_interp: ParInterp::Linear,
                    doc_clause: Some(convention.doc_clause.as_str().to_string()),
                    cds_valuation_convention: None,
                }),
            }],
        },
        market_data: quotes.into_iter().map(MarketDatum::CdsQuote).collect(),
        prior_market: vec![PriorMarketObject::DiscountCurve(discount_curve.clone())],
    };
    let result = engine::execute(&envelope)?;
    let calibrated_market = MarketContext::try_from(result.result.final_market)?;
    calibrated_market
        .get_hazard(curve_id.as_str())
        .map(|curve| curve.as_ref().clone())
}

#[derive(Clone)]
pub struct TestInstrument {
    id: String,
    value: Money,
    discount_curves: SmallVec<[CurveId; 2]>,
    forward_curves: SmallVec<[CurveId; 2]>,
    inflation_curves: SmallVec<[CurveId; 2]>,
}

impl TestInstrument {
    pub fn new(id: &str, value: Money) -> Self {
        Self {
            id: id.to_string(),
            value,
            discount_curves: SmallVec::<[CurveId; 2]>::new(),
            forward_curves: SmallVec::<[CurveId; 2]>::new(),
            inflation_curves: SmallVec::<[CurveId; 2]>::new(),
        }
    }

    pub fn with_discount_curves(mut self, curves: &[&str]) -> Self {
        self.discount_curves = curves.iter().map(|id| CurveId::new(*id)).collect();
        self
    }

    pub fn with_forward_curves(mut self, curves: &[&str]) -> Self {
        self.forward_curves = curves.iter().map(|id| CurveId::new(*id)).collect();
        self
    }

    pub fn with_inflation_curves(mut self, curves: &[&str]) -> Self {
        self.inflation_curves = curves.iter().map(|id| CurveId::new(*id)).collect();
        self
    }
}

impl Instrument for TestInstrument {
    fn id(&self) -> &str {
        &self.id
    }

    fn key(&self) -> finstack_quant_valuations::pricer::InstrumentType {
        finstack_quant_valuations::pricer::InstrumentType::Bond
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn attributes(&self) -> &Attributes {
        static ATTRS: OnceLock<Attributes> = OnceLock::new();
        ATTRS.get_or_init(Attributes::default)
    }

    fn attributes_mut(&mut self) -> &mut Attributes {
        unreachable!("TestInstrument::attributes_mut should not be called in tests")
    }

    fn clone_box(&self) -> Box<dyn Instrument> {
        Box::new(self.clone())
    }

    fn market_dependencies(&self) -> finstack_quant_core::Result<MarketDependencies> {
        let mut deps = MarketDependencies::new();
        for curve in &self.discount_curves {
            deps.add_discount_curve(curve.clone());
        }
        for curve in &self.forward_curves {
            deps.add_forward_curve(curve.clone());
        }
        for curve in &self.inflation_curves {
            deps.add_inflation_curve(curve.clone());
        }
        Ok(deps)
    }

    fn base_value(&self, market: &MarketContext, _as_of: Date) -> Result<Money> {
        let mut amt = self.value.amount();
        for id in &self.forward_curves {
            let fwd = market.get_forward(id.as_str())?;
            // Deterministic exposure to parallel forward moves (test-only stub).
            amt += fwd.rate(1.0) * 1_000_000.0;
        }
        Ok(Money::new(amt, self.value.currency()).expect("valid money fixture"))
    }

    fn price_with_metrics(
        &self,
        market: &MarketContext,
        as_of: Date,
        _metrics: &[MetricId],
        _options: finstack_quant_valuations::instruments::PricingOptions,
    ) -> finstack_quant_valuations::Result<ValuationResult> {
        let value = self.value(market, as_of)?;
        Ok(ValuationResult::stamped(self.id(), as_of, value))
    }
}

finstack_quant_valuations::impl_empty_cashflow_provider!(
    TestInstrument,
    finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
);
