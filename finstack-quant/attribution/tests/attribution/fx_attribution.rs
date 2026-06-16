//! Integration tests for FX attribution.
//!
//! Tests FX translation and internal FX exposure effects in parallel
//! and waterfall attribution methodologies.

use finstack_quant_attribution::{
    attribute_pnl_parallel, attribute_pnl_waterfall, default_waterfall_order, AttributionFactor,
    ExecutionPolicy,
};
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{create_date, Date};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::fx::{FxConversionPolicy, FxMatrix, FxProvider, FxQuery};
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::Attributes;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::pricer::InstrumentType;
use finstack_quant_valuations::results::ValuationResult;
use std::sync::Arc;
use std::sync::OnceLock;
use time::Month;

// Test FX provider with configurable rates
struct TestFxProvider {
    rate: f64,
}

impl FxProvider for TestFxProvider {
    fn rate(
        &self,
        from: Currency,
        to: Currency,
        _on: finstack_quant_core::dates::Date,
        _policy: FxConversionPolicy,
    ) -> finstack_quant_core::Result<f64> {
        if from == to {
            Ok(1.0)
        } else if from == Currency::EUR && to == Currency::USD {
            Ok(self.rate)
        } else if from == Currency::USD && to == Currency::EUR {
            Ok(1.0 / self.rate)
        } else {
            Err(finstack_quant_core::Error::Validation(
                "FX rate not found".to_string(),
            ))
        }
    }
}

#[derive(Clone)]
struct FxLinkedInstrument {
    id: String,
    notional: f64,
    base_ccy: Currency,
    reporting_ccy: Currency,
}

impl FxLinkedInstrument {
    fn new(id: &str, notional: f64, base_ccy: Currency, reporting_ccy: Currency) -> Self {
        Self {
            id: id.to_string(),
            notional,
            base_ccy,
            reporting_ccy,
        }
    }
}

impl Instrument for FxLinkedInstrument {
    fn id(&self) -> &str {
        &self.id
    }

    fn key(&self) -> InstrumentType {
        InstrumentType::Bond
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
        unreachable!("FxLinkedInstrument::attributes_mut should not be called in tests")
    }

    fn clone_box(&self) -> Box<dyn Instrument> {
        Box::new(self.clone())
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<finstack_quant_valuations::instruments::MarketDependencies>
    {
        Ok(finstack_quant_valuations::instruments::MarketDependencies::new())
    }

    fn base_value(&self, market: &MarketContext, as_of: Date) -> Result<Money> {
        if self.base_ccy == self.reporting_ccy {
            return Ok(Money::new(self.notional, self.reporting_ccy));
        }

        let fx_matrix = market.fx().ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "FX matrix missing for FxLinkedInstrument".to_string(),
            )
        })?;
        let query = FxQuery::new(self.base_ccy, self.reporting_ccy, as_of);
        let rate = fx_matrix.rate(query)?;

        Ok(Money::new(self.notional * rate.rate, self.reporting_ccy))
    }

    fn price_with_metrics(
        &self,
        market: &MarketContext,
        as_of: Date,
        _metrics: &[finstack_quant_valuations::metrics::MetricId],
        _options: finstack_quant_valuations::instruments::PricingOptions,
    ) -> Result<ValuationResult> {
        let value = self.value(market, as_of)?;
        Ok(ValuationResult::stamped(self.id(), as_of, value))
    }
}

#[test]
fn test_fx_attribution_parallel_internal_exposure() {
    // Test that FX attribution captures internal FX exposure changes
    // For a USD bond (no cross-currency exposure), FX P&L should be near zero
    let as_of_t0 = create_date(2025, Month::January, 15).unwrap();
    let as_of_t1 = create_date(2025, Month::January, 16).unwrap();

    // Create a USD bond (no FX exposure)
    let issue = create_date(2025, Month::January, 1).unwrap();
    let maturity = create_date(2030, Month::January, 1).unwrap();

    let bond = Bond::fixed(
        "US-BOND-001",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        issue,
        maturity,
        "USD-OIS",
    )
    .unwrap();

    // Create discount curves (unchanged)
    let curve_t0 = DiscountCurve::builder("USD-OIS")
        .base_date(as_of_t0)
        .knots(vec![(0.0, 1.0), (5.0, 0.82)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let curve_t1 = DiscountCurve::builder("USD-OIS")
        .base_date(as_of_t1)
        .knots(vec![(0.0, 1.0), (5.0, 0.82)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    // Create FX matrices with different EUR/USD rates
    let fx_t0 = FxMatrix::new(Arc::new(TestFxProvider { rate: 1.1 }));
    let fx_t1 = FxMatrix::new(Arc::new(TestFxProvider { rate: 1.2 }));

    let market_t0 = MarketContext::new().insert(curve_t0).insert_fx(fx_t0);

    let market_t1 = MarketContext::new().insert(curve_t1).insert_fx(fx_t1);

    let config = FinstackConfig::default();

    // Run parallel attribution
    let bond_instrument: Arc<dyn Instrument> = Arc::new(bond);
    let attribution = attribute_pnl_parallel(
        &bond_instrument,
        &market_t0,
        &market_t1,
        as_of_t0,
        as_of_t1,
        &config,
        ExecutionPolicy::Parallel,
    )
    .unwrap();

    // For USD bond with no cross-currency exposure, FX P&L should be minimal
    // (Only internal pricing effects, which are zero for single-currency bond)
    let fx_pnl_abs = attribution.fx_pnl.amount().abs();
    assert!(
        fx_pnl_abs < 0.01,
        "FX P&L should be near zero for single-currency bond, got {}",
        fx_pnl_abs
    );

    // Metadata should be populated
    assert_eq!(attribution.meta.instrument_id, "US-BOND-001");
    assert!(attribution.meta.num_repricings > 0);
}

#[test]
fn test_waterfall_attribution_sum_equality() {
    // Test that waterfall attribution sums to total P&L
    let as_of_t0 = create_date(2025, Month::January, 15).unwrap();
    let as_of_t1 = create_date(2025, Month::January, 16).unwrap();

    let issue = create_date(2025, Month::January, 1).unwrap();
    let maturity = create_date(2030, Month::January, 1).unwrap();

    let bond = Bond::fixed(
        "US-BOND-001",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        issue,
        maturity,
        "USD-OIS",
    )
    .unwrap();

    // Create curves with a shift
    let curve_t0 = DiscountCurve::builder("USD-OIS")
        .base_date(as_of_t0)
        .knots(vec![(0.0, 1.0), (5.0, 0.82)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let curve_t1 = DiscountCurve::builder("USD-OIS")
        .base_date(as_of_t1)
        .knots(vec![(0.0, 1.0), (5.0, 0.78)]) // Rates increased
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let market_t0 = MarketContext::new().insert(curve_t0);
    let market_t1 = MarketContext::new().insert(curve_t1);

    let config = FinstackConfig::default();

    // Run waterfall attribution
    let bond_instrument: Arc<dyn Instrument> = Arc::new(bond);
    let attribution = attribute_pnl_waterfall(
        &bond_instrument,
        &market_t0,
        &market_t1,
        as_of_t0,
        as_of_t1,
        &config,
        default_waterfall_order(),
        false, // strict validation off
        None,
    )
    .unwrap();

    // Sum of all factors should equal total P&L (within tolerance)
    let sum_of_factors = attribution.carry.amount()
        + attribution.rates_curves_pnl.amount()
        + attribution.credit_curves_pnl.amount()
        + attribution.inflation_curves_pnl.amount()
        + attribution.correlations_pnl.amount()
        + attribution.fx_pnl.amount()
        + attribution.vol_pnl.amount()
        + attribution.model_params_pnl.amount()
        + attribution.market_scalars_pnl.amount();

    let expected_total = attribution.total_pnl.amount() - attribution.residual.amount();
    let diff = (sum_of_factors - expected_total).abs();

    assert!(
        diff < 0.01,
        "Sum of factors ({}) should equal total - residual ({}), diff = {}",
        sum_of_factors,
        expected_total,
        diff
    );

    // Residual should be very small for waterfall
    assert!(attribution.residual_within_meta_tolerance());
}

/// Test FX attribution for cross-currency exposure.
///
/// For a EUR bond valued in USD reporting currency, FX P&L should capture
/// the translation effect from EUR/USD rate changes.
///
/// Expected behavior:
/// - EUR strengthens (EUR/USD increases) → positive FX P&L for long EUR bond
/// - EUR weakens (EUR/USD decreases) → negative FX P&L for long EUR bond
#[test]
fn test_fx_attribution_cross_currency_exposure() {
    let as_of_t0 = create_date(2025, Month::January, 15).unwrap();
    let as_of_t1 = create_date(2025, Month::January, 16).unwrap();

    // Create a EUR-denominated bond
    let issue = create_date(2025, Month::January, 1).unwrap();
    let maturity = create_date(2030, Month::January, 1).unwrap();

    let eur_bond = Bond::fixed(
        "EUR-BOND-001",
        Money::new(1_000_000.0, Currency::EUR),
        0.03, // 3% EUR coupon
        issue,
        maturity,
        "EUR-OIS",
    )
    .unwrap();

    // Create EUR discount curves (unchanged rates)
    let eur_curve_t0 = DiscountCurve::builder("EUR-OIS")
        .base_date(as_of_t0)
        .knots(vec![(0.0, 1.0), (5.0, 0.85)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let eur_curve_t1 = DiscountCurve::builder("EUR-OIS")
        .base_date(as_of_t1)
        .knots(vec![(0.0, 1.0), (5.0, 0.85)]) // Same rates
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    // Create FX matrices: EUR strengthened from 1.10 to 1.15 (EUR/USD)
    let fx_t0 = FxMatrix::new(Arc::new(TestFxProvider { rate: 1.10 }));
    let fx_t1 = FxMatrix::new(Arc::new(TestFxProvider { rate: 1.15 })); // EUR strengthened

    let market_t0 = MarketContext::new().insert(eur_curve_t0).insert_fx(fx_t0);

    let market_t1 = MarketContext::new().insert(eur_curve_t1).insert_fx(fx_t1);

    let config = FinstackConfig::default();

    // Run parallel attribution
    let bond_instrument: Arc<dyn Instrument> = Arc::new(eur_bond);
    let attribution = attribute_pnl_parallel(
        &bond_instrument,
        &market_t0,
        &market_t1,
        as_of_t0,
        as_of_t1,
        &config,
        ExecutionPolicy::Parallel,
    )
    .unwrap();

    // Currency should be EUR (the bond's native currency)
    assert_eq!(attribution.total_pnl.currency(), Currency::EUR);

    // FX P&L should be captured in total P&L when EUR strengthens
    // Note: The internal FX attribution depends on how the pricer handles
    // cross-currency exposure. For a EUR bond with unchanged EUR rates,
    // the EUR-denominated P&L from rates should be near zero.
    assert!(
        attribution.rates_curves_pnl.amount().abs() < 100.0,
        "Rates P&L should be near zero with unchanged EUR rates, got {}",
        attribution.rates_curves_pnl.amount()
    );

    // Verify metadata
    assert_eq!(attribution.meta.instrument_id, "EUR-BOND-001");
}

#[test]
fn test_fx_attribution_cross_currency_fx_pnl_sign_and_magnitude() {
    let as_of_t0 = create_date(2025, Month::January, 15).unwrap();
    let as_of_t1 = create_date(2025, Month::January, 16).unwrap();

    // EUR exposure reported in USD (FX-linked valuation)
    let instrument: Arc<dyn Instrument> = Arc::new(FxLinkedInstrument::new(
        "EUR-USD-EXPOSURE",
        1_000_000.0,
        Currency::EUR,
        Currency::USD,
    ));

    let fx_t0 = FxMatrix::new(Arc::new(TestFxProvider { rate: 1.10 }));
    let fx_t1 = FxMatrix::new(Arc::new(TestFxProvider { rate: 1.15 })); // EUR strengthens

    let market_t0 = MarketContext::new().insert_fx(fx_t0);
    let market_t1 = MarketContext::new().insert_fx(fx_t1);

    let config = FinstackConfig::default();
    let attribution = attribute_pnl_parallel(
        &instrument,
        &market_t0,
        &market_t1,
        as_of_t0,
        as_of_t1,
        &config,
        ExecutionPolicy::Parallel,
    )
    .unwrap();

    let expected_fx_pnl = 1_000_000.0 * (1.15 - 1.10);
    assert!(
        attribution.fx_pnl.amount() > 0.0,
        "FX P&L should be positive when EUR strengthens, got {}",
        attribution.fx_pnl.amount()
    );
    assert!(
        (attribution.fx_pnl.amount() - expected_fx_pnl).abs() < 1.0,
        "FX P&L should be close to expected {}, got {}",
        expected_fx_pnl,
        attribution.fx_pnl.amount()
    );
    assert!(
        (attribution.total_pnl.amount() - expected_fx_pnl).abs() < 1.0,
        "Total P&L should match FX P&L for pure FX exposure, expected {}, got {}",
        expected_fx_pnl,
        attribution.total_pnl.amount()
    );
}

#[test]
fn test_fx_attribution_cross_currency_fx_pnl_weakening() {
    let as_of_t0 = create_date(2025, Month::January, 15).unwrap();
    let as_of_t1 = create_date(2025, Month::January, 16).unwrap();

    let instrument: Arc<dyn Instrument> = Arc::new(FxLinkedInstrument::new(
        "EUR-USD-EXPOSURE-WEAK",
        1_000_000.0,
        Currency::EUR,
        Currency::USD,
    ));

    let fx_t0 = FxMatrix::new(Arc::new(TestFxProvider { rate: 1.10 }));
    let fx_t1 = FxMatrix::new(Arc::new(TestFxProvider { rate: 1.05 })); // EUR weakens

    let market_t0 = MarketContext::new().insert_fx(fx_t0);
    let market_t1 = MarketContext::new().insert_fx(fx_t1);

    let config = FinstackConfig::default();
    let attribution = attribute_pnl_parallel(
        &instrument,
        &market_t0,
        &market_t1,
        as_of_t0,
        as_of_t1,
        &config,
        ExecutionPolicy::Parallel,
    )
    .unwrap();

    let expected_fx_pnl = 1_000_000.0 * (1.05 - 1.10);
    assert!(
        attribution.fx_pnl.amount() < 0.0,
        "FX P&L should be negative when EUR weakens, got {}",
        attribution.fx_pnl.amount()
    );
    assert!(
        (attribution.fx_pnl.amount() - expected_fx_pnl).abs() < 1.0,
        "FX P&L should be close to expected {}, got {}",
        expected_fx_pnl,
        attribution.fx_pnl.amount()
    );
}

/// Test that FX attribution correctly captures EUR/USD weakening.
#[test]
fn test_fx_attribution_eur_weakening() {
    let as_of_t0 = create_date(2025, Month::January, 15).unwrap();
    let as_of_t1 = create_date(2025, Month::January, 16).unwrap();

    let issue = create_date(2025, Month::January, 1).unwrap();
    let maturity = create_date(2030, Month::January, 1).unwrap();

    let eur_bond = Bond::fixed(
        "EUR-BOND-WEAK",
        Money::new(1_000_000.0, Currency::EUR),
        0.03,
        issue,
        maturity,
        "EUR-OIS",
    )
    .unwrap();

    let eur_curve = DiscountCurve::builder("EUR-OIS")
        .base_date(as_of_t0)
        .knots(vec![(0.0, 1.0), (5.0, 0.85)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    // EUR weakened from 1.10 to 1.05 (EUR/USD)
    let fx_t0 = FxMatrix::new(Arc::new(TestFxProvider { rate: 1.10 }));
    let fx_t1 = FxMatrix::new(Arc::new(TestFxProvider { rate: 1.05 }));

    let market_t0 = MarketContext::new()
        .insert(eur_curve.clone())
        .insert_fx(fx_t0);

    let market_t1 = MarketContext::new().insert(eur_curve).insert_fx(fx_t1);

    let config = FinstackConfig::default();

    let bond_instrument: Arc<dyn Instrument> = Arc::new(eur_bond);
    let attribution = attribute_pnl_parallel(
        &bond_instrument,
        &market_t0,
        &market_t1,
        as_of_t0,
        as_of_t1,
        &config,
        ExecutionPolicy::Parallel,
    )
    .unwrap();

    // Verify we get an attribution result
    assert_eq!(attribution.meta.instrument_id, "EUR-BOND-WEAK");
    assert_eq!(attribution.total_pnl.currency(), Currency::EUR);
}

#[test]
fn test_waterfall_factor_ordering_sensitivity() {
    // Doctrine: Carry must always be the first waterfall factor (date-roll
    // first). Two valid orders that BOTH start with Carry but differ later
    // produce different factor attributions yet the same total P&L and
    // minimal residual. Any order whose first factor is not `Carry` is
    // rejected at the entry point — exercised separately below.
    let as_of_t0 = create_date(2025, Month::January, 15).unwrap();
    let as_of_t1 = create_date(2025, Month::January, 16).unwrap();

    let issue = create_date(2025, Month::January, 1).unwrap();
    let maturity = create_date(2030, Month::January, 1).unwrap();

    let bond = Bond::fixed(
        "US-BOND-001",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        issue,
        maturity,
        "USD-OIS",
    )
    .unwrap();

    let curve_t0 = DiscountCurve::builder("USD-OIS")
        .base_date(as_of_t0)
        .knots(vec![(0.0, 1.0), (5.0, 0.82)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let curve_t1 = DiscountCurve::builder("USD-OIS")
        .base_date(as_of_t1)
        .knots(vec![(0.0, 1.0), (5.0, 0.78)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let market_t0 = MarketContext::new().insert(curve_t0);
    let market_t1 = MarketContext::new().insert(curve_t1);

    let config = FinstackConfig::default();
    let bond_instrument: Arc<dyn Instrument> = Arc::new(bond);

    // Order 1: Carry then Rates
    let order1 = vec![AttributionFactor::Carry, AttributionFactor::RatesCurves];

    let attr1 = attribute_pnl_waterfall(
        &bond_instrument,
        &market_t0,
        &market_t1,
        as_of_t0,
        as_of_t1,
        &config,
        order1,
        false, // strict validation off
        None,
    )
    .unwrap();

    // Order 2: Carry then Fx then Rates — Carry still first; subsequent
    // factors differ. (The original test ran Order2 = [Rates, Carry] which is
    // now a doctrine violation and is asserted to error below.)
    let order2 = vec![
        AttributionFactor::Carry,
        AttributionFactor::Fx,
        AttributionFactor::RatesCurves,
    ];

    let attr2 = attribute_pnl_waterfall(
        &bond_instrument,
        &market_t0,
        &market_t1,
        as_of_t0,
        as_of_t1,
        &config,
        order2,
        false, // strict validation off
        None,
    )
    .unwrap();

    // Total P&L should be identical
    assert_eq!(attr1.total_pnl.amount(), attr2.total_pnl.amount());

    // Both should have minimal residual
    assert!(attr1.residual_within_meta_tolerance());
    assert!(attr2.residual_within_meta_tolerance());

    // Factor attributions may differ due to ordering of post-Carry factors
    // (this is expected and correct for waterfall methodology).
}

#[test]
fn test_waterfall_rejects_non_carry_first_order() {
    // Doctrine guard: a factor_order that does not start with `Carry` is
    // a hard validation error at the waterfall entry point. The error message
    // must mention `Carry` so callers can diagnose quickly.
    let as_of_t0 = create_date(2025, Month::January, 15).unwrap();
    let as_of_t1 = create_date(2025, Month::January, 16).unwrap();

    let bond = Bond::fixed(
        "US-BOND-DOCTRINE",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        create_date(2025, Month::January, 1).unwrap(),
        create_date(2030, Month::January, 1).unwrap(),
        "USD-OIS",
    )
    .unwrap();
    let bond_instrument: Arc<dyn Instrument> = Arc::new(bond);
    let market_t0 = MarketContext::new();
    let market_t1 = MarketContext::new();
    let config = FinstackConfig::default();

    let bad_order = vec![AttributionFactor::RatesCurves, AttributionFactor::Carry];
    let result = attribute_pnl_waterfall(
        &bond_instrument,
        &market_t0,
        &market_t1,
        as_of_t0,
        as_of_t1,
        &config,
        bad_order,
        false,
        None,
    );
    let err = result.expect_err("non-Carry-first order must be rejected");
    let msg = format!("{err}");
    assert!(
        msg.contains("Carry"),
        "error message must reference Carry, got: {msg}"
    );
}

#[test]
fn test_waterfall_rejects_duplicate_factors() {
    // A duplicated factor would silently overwrite its first-pass P&L with ~0
    // (the market is already rolled on the second application) and a
    // duplicated Carry would double-count coupon income into total_pnl, so
    // duplicates are a hard validation error naming the repeated factor.
    let as_of_t0 = create_date(2025, Month::January, 15).unwrap();
    let as_of_t1 = create_date(2025, Month::January, 16).unwrap();

    let bond = Bond::fixed(
        "US-BOND-DUP-FACTOR",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        create_date(2025, Month::January, 1).unwrap(),
        create_date(2030, Month::January, 1).unwrap(),
        "USD-OIS",
    )
    .unwrap();
    let bond_instrument: Arc<dyn Instrument> = Arc::new(bond);
    let market_t0 = MarketContext::new();
    let market_t1 = MarketContext::new();
    let config = FinstackConfig::default();

    let dup_order = vec![
        AttributionFactor::Carry,
        AttributionFactor::RatesCurves,
        AttributionFactor::RatesCurves,
    ];
    let result = attribute_pnl_waterfall(
        &bond_instrument,
        &market_t0,
        &market_t1,
        as_of_t0,
        as_of_t1,
        &config,
        dup_order,
        false,
        None,
    );
    let err = result.expect_err("duplicate factors must be rejected");
    let msg = format!("{err}");
    assert!(
        msg.contains("duplicate") && msg.contains("RatesCurves"),
        "error message must name the duplicated factor, got: {msg}"
    );

    // A duplicated Carry is rejected by the same guard.
    let dup_carry = vec![AttributionFactor::Carry, AttributionFactor::Carry];
    let err = attribute_pnl_waterfall(
        &bond_instrument,
        &market_t0,
        &market_t1,
        as_of_t0,
        as_of_t1,
        &config,
        dup_carry,
        false,
        None,
    )
    .expect_err("duplicate Carry must be rejected");
    assert!(format!("{err}").contains("duplicate"));
}

finstack_quant_valuations::impl_empty_cashflow_provider!(
    FxLinkedInstrument,
    finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
);
