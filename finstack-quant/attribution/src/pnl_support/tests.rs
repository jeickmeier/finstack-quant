use super::carry::*;
use super::endpoints::*;
use crate::model_params::ModelParamsSnapshot;
use crate::{AttributionFactor, AttributionMethod, ExecutionPolicy, PnlAttribution};
use finstack_quant_core::Result;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::{Instrument, MarketDependencies, PricingOptions};
use finstack_quant_valuations::metrics::MetricId;

use finstack_quant_core::Error;
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::money::fx::{FxConversionPolicy, FxMatrix, FxProvider};
use std::sync::Arc;
use time::macros::date;

#[derive(Clone)]
struct CountingInstrument {
    calls: Arc<std::sync::atomic::AtomicUsize>,
    attributes: finstack_quant_core::types::Attributes,
    fail_metrics: bool,
    fail_ytm: bool,
    fail_values: bool,
    has_yield: bool,
    has_funding: bool,
}

finstack_quant_valuations::impl_empty_cashflow_provider!(
    CountingInstrument,
    finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
);

impl Instrument for CountingInstrument {
    fn includes_valuation_date_cashflows(&self) -> bool {
        true
    }
    fn id(&self) -> &str {
        "COUNTED-CARRY"
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
    fn attributes(&self) -> &finstack_quant_core::types::Attributes {
        &self.attributes
    }
    fn attributes_mut(&mut self) -> &mut finstack_quant_core::types::Attributes {
        &mut self.attributes
    }
    fn clone_box(&self) -> Box<dyn Instrument> {
        Box::new(self.clone())
    }
    fn market_dependencies(&self) -> Result<MarketDependencies> {
        let mut deps = MarketDependencies::new();
        deps.add_discount_curve("USD-OIS");
        Ok(deps)
    }
    fn repo_curve_id(&self) -> Option<finstack_quant_core::types::CurveId> {
        self.has_funding.then(|| "USD-REPO".into())
    }
    fn base_value(&self, _market: &MarketContext, _as_of: Date) -> Result<Money> {
        self.calls
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if self.fail_values {
            return Err(Error::Validation("intentional value failure".into()));
        }
        Ok(Money::from((100_i64, Currency::USD)))
    }
    fn price_with_metrics(
        &self,
        _market: &MarketContext,
        as_of: Date,
        metrics: &[MetricId],
        _options: PricingOptions,
    ) -> std::result::Result<
        finstack_quant_valuations::results::ValuationResult,
        finstack_quant_valuations::pricer::PricingError,
    > {
        self.calls
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if self.fail_metrics || (self.fail_ytm && metrics.contains(&MetricId::Ytm)) {
            return Err(Error::Validation("intentional metrics failure".into()).into());
        }
        let mut result = finstack_quant_valuations::results::ValuationResult::stamped(
            self.id(),
            as_of,
            Money::from((100_i64, Currency::USD)),
        );
        if metrics.contains(&MetricId::Accrued) {
            result
                .measures
                .insert(MetricId::Accrued, f64::from(as_of.day()));
        }
        if self.has_yield && metrics.contains(&MetricId::Ytm) {
            result.measures.insert(MetricId::Ytm, 0.05);
        }
        Ok(result)
    }
}

fn counting_market() -> MarketContext {
    let curve = |id| {
        DiscountCurve::builder(id)
            .base_date(date!(2025 - 01 - 15))
            .knots([(0.0, 1.0), (100.0, (-0.05_f64 * 100.0).exp())])
            .interp(InterpStyle::LogLinear)
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .expect("curve")
    };
    MarketContext::new()
        .try_insert(curve("USD-OIS"))
        .expect("insert market curve")
        .try_insert(curve("USD-REPO"))
        .expect("insert market curve")
}

#[test]
fn carry_helper_counts_all_attempted_metric_flat_and_funding_valuations() {
    for (fail_metrics, fail_values, has_yield, has_funding, expected) in [
        (false, false, false, false, 2),
        (false, false, true, true, 5),
        (true, false, false, true, 4),
        (false, true, true, false, 3),
    ] {
        let instrument = CountingInstrument {
            calls: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            attributes: Default::default(),
            fail_metrics,
            fail_ytm: false,
            fail_values,
            has_yield,
            has_funding,
        };
        let inputs = total_return_carry_inputs(
            &instrument,
            &counting_market(),
            date!(2025 - 01 - 15),
            date!(2025 - 01 - 16),
            Currency::USD,
        )
        .expect("optional carry inputs");
        assert_eq!(inputs.num_repricings, expected);
        assert_eq!(
            instrument.calls.load(std::sync::atomic::Ordering::Relaxed),
            expected
        );
    }
}

#[test]
fn repricing_methods_include_actual_carry_helper_cost_in_metadata() {
    for method in [
        AttributionMethod::Parallel,
        AttributionMethod::Waterfall(crate::default_waterfall_order()),
        AttributionMethod::Taylor(Default::default()),
        AttributionMethod::Taylor(crate::TaylorAttributionConfig {
            include_gamma: true,
            ..Default::default()
        }),
    ] {
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let instrument: Arc<dyn Instrument> = Arc::new(CountingInstrument {
            calls: Arc::clone(&calls),
            attributes: Default::default(),
            fail_metrics: false,
            fail_ytm: false,
            fail_values: false,
            has_yield: true,
            has_funding: true,
        });
        let market = counting_market();
        let attribution = crate::attribute_pnl(
            &method,
            &crate::AttributionRequest::new(
                &instrument,
                &market,
                &market,
                date!(2025 - 01 - 15),
                date!(2025 - 01 - 16),
                &FinstackConfig::default(),
            ),
        )
        .expect("attribution");
        assert_eq!(
            attribution.meta.num_repricings,
            calls.load(std::sync::atomic::Ordering::Relaxed)
        );
    }
}

#[test]
fn unavailable_yield_preserves_the_independently_available_accrual_change() {
    let instrument = CountingInstrument {
        calls: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        attributes: Default::default(),
        fail_metrics: false,
        fail_ytm: true,
        fail_values: false,
        has_yield: true,
        has_funding: false,
    };
    let inputs = total_return_carry_inputs(
        &instrument,
        &counting_market(),
        date!(2025 - 01 - 15),
        date!(2025 - 01 - 16),
        Currency::USD,
    )
    .expect("carry inputs without yield");
    assert_eq!(
        inputs.delta_accrued.expect("recovered accrued").amount(),
        1.0
    );
    assert_eq!(inputs.flat_window_diff, None);
    assert_eq!(inputs.num_repricings, 3);
}

#[test]
fn carry_partition_separates_principal_and_coupon_accrual() {
    let mut attribution = PnlAttribution::new(
        Money::from((-50_i64, Currency::USD)),
        "CARRY",
        date!(2025 - 01 - 15),
        date!(2025 - 01 - 16),
        AttributionMethod::Parallel,
    );
    let inputs = TotalReturnCarryInputs {
        cash_paid: Money::from((55_i64, Currency::USD)),
        income_cash_paid: Money::from((5_i64, Currency::USD)),
        delta_accrued: Some(Money::from((-4_i64, Currency::USD))),
        flat_window_diff: Some(Money::from((-49_i64, Currency::USD))),
        funding_cost: None,
        warnings: Vec::new(),
        num_repricings: 0,
    };
    apply_total_return_carry(
        &mut attribution,
        Money::from((-50_i64, Currency::USD)),
        inputs,
    )
    .expect("carry partition");
    let detail = attribution.carry_detail.expect("detail");
    assert_eq!(detail.total.amount(), 5.0);
    assert_eq!(detail.coupon_income.expect("income").total.amount(), 1.0);
    assert_eq!(detail.pull_to_par.expect("pull to par").amount(), 5.0);
    assert_eq!(detail.roll_down.expect("roll down").total.amount(), -1.0);
}

// Simple test FX provider
struct TestFx;
impl FxProvider for TestFx {
    fn rate(
        &self,
        from: Currency,
        to: Currency,
        _on: Date,
        _policy: FxConversionPolicy,
    ) -> Result<f64> {
        if from == Currency::EUR && to == Currency::USD {
            Ok(1.1)
        } else if from == Currency::USD && to == Currency::EUR {
            Ok(1.0 / 1.1)
        } else if from == to {
            Ok(1.0)
        } else {
            Err(Error::Validation("FX rate not found".to_string()))
        }
    }
}

#[test]
fn validate_attribution_period_accepts_forward_and_same_day() {
    assert!(
        validate_attribution_period(date!(2025 - 01 - 15), date!(2025 - 01 - 16)).is_ok(),
        "a forward period must be accepted"
    );
    // Same-day attribution is degenerate but permitted (theta over zero days).
    assert!(
        validate_attribution_period(date!(2025 - 01 - 15), date!(2025 - 01 - 15)).is_ok(),
        "a zero-length period must be accepted"
    );
}

#[test]
fn validate_attribution_period_rejects_reversed_period() {
    assert!(
        validate_attribution_period(date!(2025 - 01 - 16), date!(2025 - 01 - 15)).is_err(),
        "a reversed period (t1 < t0) must be rejected"
    );
}

#[test]
fn discount_only_bond_does_not_use_unused_book_families() {
    use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
    let bond = Bond::fixed(
        "USE-BOND",
        Money::from((1_000_000_i64, Currency::USD)),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        date!(2025 - 01 - 01),
        date!(2030 - 01 - 01),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond");
    let factor_use = InstrumentFactorUse::of(&bond);
    assert!(factor_use.uses_attribution_factor(&AttributionFactor::RatesCurves));
    for unused in [
        AttributionFactor::CreditCurves,
        AttributionFactor::InflationCurves,
        AttributionFactor::Fx,
        AttributionFactor::Volatility,
        AttributionFactor::MarketScalars,
    ] {
        assert!(!factor_use.uses_attribution_factor(&unused), "{unused:?}");
    }
    assert!(factor_use.uses_attribution_factor(&AttributionFactor::Correlations));
    assert!(factor_use.uses_attribution_factor(&AttributionFactor::Carry));
    assert!(factor_use.uses_attribution_factor(&AttributionFactor::ModelParameters));

    // Empty declarations are undeclared, not "uses nothing".
    let undeclared = InstrumentFactorUse::from_deps(&MarketDependencies::new(), &bond);
    for factor in [
        AttributionFactor::Fx,
        AttributionFactor::CreditCurves,
        AttributionFactor::MarketScalars,
    ] {
        assert!(undeclared.uses_attribution_factor(&factor), "{factor:?}");
    }
}

/// Flat-YTM window curve must invert Street compounding at the bond's
/// coupon frequency. US corporate (`Bond::fixed`) is semi-annual:
/// ytm = 5% at t = 1y → DF = (1 + 0.05/2)^(−2), NOT exp(−0.05).
/// Knot grid must reach 100y for ultra-long bonds.
#[test]
fn flat_ytm_market_uses_street_semiannual_compounding() {
    use finstack_quant_valuations::instruments::fixed_income::bond::Bond;

    let as_of = date!(2025 - 01 - 01);
    let bond = Bond::fixed(
        "FLAT-YTM-BOND",
        Money::from((1_000_000_i64, Currency::USD)),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        date!(2024 - 01 - 01),
        date!(2034 - 01 - 01),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond construction");
    let base_curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (10.0, (-0.03_f64 * 10.0).exp())])
        .interp(InterpStyle::LogLinear)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("base curve");
    let market = MarketContext::new()
        .try_insert(base_curve)
        .expect("insert market curve");

    let flat = build_flat_ytm_market(&bond, &market, 0.05).expect("flat market");
    let curve = flat.get_discount("USD-OIS").expect("flat curve");

    let expected_1y = (1.0_f64 + 0.05 / 2.0).powi(-2);
    assert!(
        (curve.df(1.0).expect("df") - expected_1y).abs() < 1e-12,
        "DF(1y) at 5% street (semi-annual) YTM must be (1.025)^-2 = \
         {expected_1y}, got {} (continuous exp(-0.05) = {})",
        curve.df(1.0).expect("df"),
        (-0.05_f64).exp()
    );

    // Grid must stay on the semi-annual flat curve out to 100y (log-linear
    // extrapolation of a flat yield is exact) without building a 201-knot
    // 0..=200 half-year grid for a 10y bond.
    let expected_100y = (1.0_f64 + 0.05 / 2.0).powi(-200);
    assert!(
        ((curve.df(100.0).expect("df") - expected_100y) / expected_100y).abs() < 1e-9,
        "DF(100y) must be on the semi-annual flat curve, expected \
         {expected_100y}, got {}",
        curve.df(100.0).expect("df")
    );
    assert!(
        curve.knots().len() < 50,
        "10y bond flat-YTM grid should be maturity-sized, got {} knots",
        curve.knots().len()
    );
}

/// Annual-pay bonds (Bund / EUR corporate) solve Street YTM at frequency 1.
/// DF(1y) at 5% must be 1/1.05, not the hardcoded Street-2 (1.025)^-2.
#[test]
fn flat_ytm_market_uses_bond_coupon_frequency() {
    use finstack_quant_valuations::instruments::fixed_income::BondConvention;
    use finstack_quant_valuations::instruments::fixed_income::bond::Bond;

    let as_of = date!(2025 - 01 - 01);
    let bond = Bond::with_convention(
        "FLAT-YTM-BUND",
        Money::from((1_000_000_i64, Currency::EUR)),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        date!(2024 - 01 - 01),
        date!(2034 - 01 - 01),
        BondConvention::GermanBund,
        "EUR-OIS",
    )
    .expect("bund construction");
    let base_curve = DiscountCurve::builder("EUR-OIS")
        .base_date(as_of)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .knots([(0.0, 1.0), (10.0, (-0.03_f64 * 10.0).exp())])
        .interp(InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("base curve");
    let market = MarketContext::new()
        .try_insert(base_curve)
        .expect("insert market curve");

    let flat = build_flat_ytm_market(&bond, &market, 0.05).expect("flat market");
    let curve = flat.get_discount("EUR-OIS").expect("flat curve");

    let expected_1y = 1.0_f64 / 1.05;
    let street2_1y = (1.0_f64 + 0.05 / 2.0).powi(-2);
    assert!(
        (curve.df(1.0).expect("df") - expected_1y).abs() < 1e-12,
        "DF(1y) at 5% annual Street YTM must be 1/1.05 = {expected_1y}, \
         got {} (hardcoded Street-2 would be {street2_1y})",
        curve.df(1.0).expect("df")
    );
}

#[test]
fn reprice_funding_cost_uses_repo_curve_when_configured() {
    use finstack_quant_core::types::CurveId;
    use finstack_quant_valuations::instruments::fixed_income::bond::Bond;

    let as_of_t0 = date!(2025 - 01 - 15);
    let as_of_t1 = date!(2025 - 01 - 16);
    let mut bond = Bond::fixed(
        "FUND-BOND",
        Money::from((1_000_000_i64, Currency::USD)),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        date!(2025 - 01 - 15),
        date!(2030 - 01 - 15),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond");
    bond.repo_curve_id = Some(CurveId::new("USD-REPO"));

    let ois = DiscountCurve::builder("USD-OIS")
        .base_date(as_of_t0)
        .knots([(0.0, 1.0), (10.0, (-0.05_f64 * 10.0).exp())])
        .interp(InterpStyle::LogLinear)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("ois");
    let repo = DiscountCurve::builder("USD-REPO")
        .base_date(as_of_t0)
        .knots([(0.0, 1.0), (10.0, (-0.03_f64 * 10.0).exp())])
        .interp(InterpStyle::LogLinear)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("repo");
    let market = MarketContext::new()
        .try_insert(ois)
        .expect("insert market curve")
        .try_insert(repo)
        .expect("insert market curve");

    let mut warnings = Vec::new();
    let mut num_repricings = 0;
    let funding = reprice_funding_cost(
        &bond,
        &market,
        as_of_t0,
        as_of_t1,
        Currency::USD,
        &mut warnings,
        &mut num_repricings,
    )
    .expect("funding cost");
    assert!(
        funding.amount() > 0.0,
        "repo funding cost must be positive, got {}",
        funding.amount()
    );
    assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
}

#[test]
fn test_compute_pnl() {
    let val_t0 = Money::from((1000_i64, Currency::EUR));
    let val_t1 = Money::from((1100_i64, Currency::EUR));
    let fx = FxMatrix::new(Arc::new(TestFx), None);
    let market = MarketContext::new().insert_fx(fx);
    let as_of = date!(2025 - 01 - 15);

    let pnl = compute_pnl(val_t0, val_t1, Currency::USD, &market, as_of)
        .expect("PNL computation should succeed in test");
    // (1100 - 1000) EUR * 1.1 = 110 USD
    assert_eq!(pnl.amount(), 110.0);
    assert_eq!(pnl.currency(), Currency::USD);
}

#[test]
fn test_compute_pnl_with_fx() {
    // Test FX translation isolation
    let pv = Money::from((1000_i64, Currency::EUR));

    // T0 market: EUR/USD = 1.1
    let fx_t0 = FxMatrix::new(Arc::new(TestFx), None);
    let market_t0 = MarketContext::new().insert_fx(fx_t0);

    // T1 market: EUR/USD = 1.2 (10% appreciation)
    struct TestFxT1;
    impl FxProvider for TestFxT1 {
        fn rate(
            &self,
            from: Currency,
            to: Currency,
            _on: Date,
            _policy: FxConversionPolicy,
        ) -> Result<f64> {
            if from == Currency::EUR && to == Currency::USD {
                Ok(1.2)
            } else if from == Currency::USD && to == Currency::EUR {
                Ok(1.0 / 1.2)
            } else if from == to {
                Ok(1.0)
            } else {
                Err(Error::Validation("FX rate not found".to_string()))
            }
        }
    }
    let fx_t1 = FxMatrix::new(Arc::new(TestFxT1), None);
    let market_t1 = MarketContext::new().insert_fx(fx_t1);

    let as_of_t0 = date!(2025 - 01 - 15);
    let as_of_t1 = date!(2025 - 01 - 16);

    // PV unchanged in EUR, but FX moved
    let pnl = compute_pnl_with_fx(
        pv,
        pv,
        Currency::USD,
        &market_t0,
        &market_t1,
        as_of_t0,
        as_of_t1,
    )
    .expect("PNL computation with FX should succeed in test");

    // FX translation: 1000 EUR @ 1.2 - 1000 EUR @ 1.1 = 1200 - 1100 = 100 USD
    assert_eq!(pnl.amount(), 100.0);
    assert_eq!(pnl.currency(), Currency::USD);
}

#[test]
fn instrument_at_t0_without_params_reuses_the_instrument() {
    let instrument: Arc<dyn Instrument> = Arc::new(
        Bond::fixed(
            "PLAIN-BOND-002",
            Money::from((1_000_000_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
            finstack_quant_core::dates::create_date(2024, time::Month::January, 1)
                .expect("valid issue date"),
            finstack_quant_core::dates::create_date(2029, time::Month::January, 1)
                .expect("valid maturity date"),
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("valid fixed bond"),
    );
    for params in [None, Some(&ModelParamsSnapshot::None)] {
        let unchanged = instrument_at_t0(&instrument, params).expect("no-op parameters");
        assert!(Arc::ptr_eq(&instrument, &unchanged));
    }
}

#[test]
fn instrument_at_t0_rejects_parameters_of_another_instrument_type() {
    use finstack_quant_valuations::instruments::fixed_income::structured_credit::StructuredCredit;
    use finstack_quant_valuations::instruments::fixed_income::{
        AntiDilutionPolicy, ConversionPolicy, ConversionSpec, DividendAdjustment,
    };

    let instrument: Arc<dyn Instrument> = Arc::new(StructuredCredit::example().expect("example"));
    let params = ModelParamsSnapshot::Convertible {
        conversion_spec: ConversionSpec {
            ratio: Some(10.0),
            price: None,
            policy: ConversionPolicy::Voluntary,
            anti_dilution: AntiDilutionPolicy::None,
            dividend_adjustment: DividendAdjustment::None,
            dilution_events: Vec::new(),
        },
    };

    let Err(error) = instrument_at_t0(&instrument, Some(&params)) else {
        panic!("mismatched model parameter snapshot should fail")
    };
    assert!(error.to_string().contains("StructuredCredit"));
}

#[test]
fn parallel_policy_reports_the_serial_first_error() {
    let items: Vec<usize> = (0..256).collect();
    let fails = |i: &usize| if i % 9 == 4 { Err(*i) } else { Ok(*i) };
    let serial = try_map_policy(ExecutionPolicy::Serial, &items, fails);
    assert_eq!(serial, Err(4));
    for _ in 0..30 {
        assert_eq!(
            try_map_policy(ExecutionPolicy::Parallel, &items, fails),
            serial
        );
    }
}
