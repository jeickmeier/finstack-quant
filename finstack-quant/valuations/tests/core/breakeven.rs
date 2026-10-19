//! Breakeven spread widening over built-in instruments.

use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::Bond;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::{BreakevenConfig, BreakevenMode, BreakevenTarget};
use finstack_quant_valuations::metrics::sensitivities::breakeven::BreakevenCalculator;
use finstack_quant_valuations::metrics::{
    calculate_theta_date, MetricCalculator, MetricContext, MetricId,
};
use std::sync::Arc;
use time::macros::date;

fn flat_discount_curve(
    id: &str,
    rate: f64,
    base_date: finstack_quant_core::dates::Date,
) -> DiscountCurve {
    let knots: Vec<(f64, f64)> = (0..=20)
        .map(|i| {
            let t = i as f64 * 0.5;
            (t, (-rate * t).exp())
        })
        .collect();
    DiscountCurve::builder(id)
        .base_date(base_date)
        .day_count(DayCount::Act365F)
        .knots(knots)
        .interp(InterpStyle::LogLinear)
        .build()
        .expect("flat discount curve")
}

fn context_with_carry_and_sensitivity(
    carry_total: f64,
    sensitivity: f64,
    target: BreakevenTarget,
    mode: BreakevenMode,
) -> MetricContext {
    let as_of = date!(2025 - 01 - 15);
    let bond = Bond::fixed(
        "TEST",
        Money::from((100_i64, Currency::USD)),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        as_of,
        date!(2030 - 01 - 15),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond");
    let market = MarketContext::new().insert(flat_discount_curve("USD-OIS", 0.04, as_of));
    let instrument: Arc<dyn Instrument> = Arc::new(bond);
    let base_value = instrument.value(&market, as_of).expect("pv");

    let mut ctx = MetricContext::new(
        instrument,
        Arc::new(market),
        as_of,
        base_value,
        Arc::new(FinstackConfig::default()),
    );
    ctx.computed.insert(MetricId::CarryTotal, carry_total);
    ctx.computed
        .insert(target.sensitivity_metric(), sensitivity);

    let overrides = finstack_quant_valuations::instruments::MetricPricingOverrides::default()
        .with_breakeven_config(BreakevenConfig { target, mode });
    ctx.set_metric_pricing_overrides(Some(overrides));
    ctx
}

#[test]
fn test_linear_breakeven_positive_carry() {
    let mut ctx = context_with_carry_and_sensitivity(
        0.50,
        -0.04,
        BreakevenTarget::ZSpread,
        BreakevenMode::Linear,
    );
    let result = BreakevenCalculator.calculate(&mut ctx).expect("breakeven");
    assert!((result - 12.5).abs() < 1e-10, "got {result}");
}

#[test]
fn test_linear_breakeven_negative_carry() {
    let mut ctx = context_with_carry_and_sensitivity(
        -0.30,
        -0.04,
        BreakevenTarget::ZSpread,
        BreakevenMode::Linear,
    );
    let result = BreakevenCalculator.calculate(&mut ctx).expect("breakeven");
    assert!((result - (-7.5)).abs() < 1e-10, "got {result}");
}

#[test]
fn test_linear_breakeven_zero_sensitivity_returns_error() {
    let mut ctx = context_with_carry_and_sensitivity(
        0.50,
        0.0,
        BreakevenTarget::ZSpread,
        BreakevenMode::Linear,
    );
    let result = BreakevenCalculator.calculate(&mut ctx);
    assert!(result.is_err(), "zero sensitivity should error");
}

#[test]
fn test_missing_sensitivity_returns_error() {
    let as_of = date!(2025 - 01 - 15);
    let bond = Bond::fixed(
        "TEST",
        Money::from((100_i64, Currency::USD)),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        as_of,
        date!(2030 - 01 - 15),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond");
    let market = MarketContext::new().insert(flat_discount_curve("USD-OIS", 0.04, as_of));
    let instrument: Arc<dyn Instrument> = Arc::new(bond);
    let base_value = instrument.value(&market, as_of).expect("pv");

    let mut ctx = MetricContext::new(
        instrument,
        Arc::new(market),
        as_of,
        base_value,
        Arc::new(FinstackConfig::default()),
    );
    ctx.computed.insert(MetricId::CarryTotal, 0.50);
    let overrides = finstack_quant_valuations::instruments::MetricPricingOverrides::default()
        .with_breakeven_config(BreakevenConfig {
            target: BreakevenTarget::ZSpread,
            mode: BreakevenMode::Linear,
        });
    ctx.set_metric_pricing_overrides(Some(overrides));

    let result = BreakevenCalculator.calculate(&mut ctx);
    assert!(result.is_err(), "missing sensitivity should error");
}

#[test]
fn test_missing_config_returns_error() {
    let as_of = date!(2025 - 01 - 15);
    let bond = Bond::fixed(
        "TEST",
        Money::from((100_i64, Currency::USD)),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        as_of,
        date!(2030 - 01 - 15),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond");
    let market = MarketContext::new().insert(flat_discount_curve("USD-OIS", 0.04, as_of));
    let instrument: Arc<dyn Instrument> = Arc::new(bond);
    let base_value = instrument.value(&market, as_of).expect("pv");

    let mut ctx = MetricContext::new(
        instrument,
        Arc::new(market),
        as_of,
        base_value,
        Arc::new(FinstackConfig::default()),
    );
    ctx.computed.insert(MetricId::CarryTotal, 0.50);
    ctx.computed.insert(MetricId::Cs01, -0.04);

    let result = BreakevenCalculator.calculate(&mut ctx);
    assert!(result.is_err(), "missing breakeven config should error");
}

#[test]
fn test_linear_breakeven_ytm_target() {
    let mut ctx = context_with_carry_and_sensitivity(
        0.25,
        -0.05,
        BreakevenTarget::Ytm,
        BreakevenMode::Linear,
    );
    let result = BreakevenCalculator.calculate(&mut ctx).expect("breakeven");
    assert!((result - 5.0).abs() < 1e-10, "got {result}");
}

#[test]
fn test_linear_breakeven_base_correlation_target() {
    let mut ctx = context_with_carry_and_sensitivity(
        0.25,
        -0.02,
        BreakevenTarget::BaseCorrelation,
        BreakevenMode::Linear,
    );

    let result = BreakevenCalculator.calculate(&mut ctx).expect("breakeven");

    assert!((result - 12.5).abs() < 1e-10, "got {result}");
}

#[test]
fn test_iterative_breakeven_base_correlation_is_explicitly_unsupported() {
    let mut ctx = context_with_carry_and_sensitivity(
        0.25,
        -0.02,
        BreakevenTarget::BaseCorrelation,
        BreakevenMode::Iterative,
    );

    let err = BreakevenCalculator
        .calculate(&mut ctx)
        .expect_err("iterative base-correlation breakeven should be unsupported");

    assert!(
        err.to_string().contains("BaseCorrelation")
            && err.to_string().contains("Iterative")
            && err.to_string().contains("unsupported"),
        "unexpected error: {err}"
    );
}

#[test]
fn test_breakeven_via_standard_registry() {
    use finstack_quant_valuations::instruments::Instrument;
    use finstack_quant_valuations::instruments::MetricPricingOverrides;
    use finstack_quant_valuations::instruments::PricingOptions;

    let as_of = date!(2025 - 01 - 15);
    let mut bond = Bond::fixed(
        "CARRY-TEST",
        Money::from((100_i64, Currency::USD)),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        as_of,
        date!(2030 - 01 - 15),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond");

    bond.metric_pricing_overrides = MetricPricingOverrides::default()
        .with_theta_period(Tenor::semi_annual())
        .with_breakeven_config(BreakevenConfig {
            target: BreakevenTarget::ZSpread,
            mode: BreakevenMode::Linear,
        });

    let market = MarketContext::new().insert(flat_discount_curve("USD-OIS", 0.04, as_of));

    let result = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::CarryTotal, MetricId::Cs01, MetricId::Breakeven],
            PricingOptions::default(),
        )
        .expect("price_with_metrics should succeed");

    let carry = result
        .measures
        .get(MetricId::CarryTotal.as_str())
        .copied()
        .expect("carry_total");
    let cs01 = result
        .measures
        .get(MetricId::Cs01.as_str())
        .copied()
        .expect("cs01");
    let breakeven = result
        .measures
        .get(MetricId::Breakeven.as_str())
        .copied()
        .expect("breakeven");

    // Verify: breakeven = -carry / cs01
    let expected = -carry / cs01;
    assert!(
        (breakeven - expected).abs() < 1e-8,
        "breakeven={breakeven}, expected={expected}, carry={carry}, cs01={cs01}"
    );
}

/// Build a bond configured for breakeven, priced off a flat 4% curve.
fn breakeven_bond(id: &str, target: BreakevenTarget, mode: BreakevenMode) -> Bond {
    use finstack_quant_valuations::instruments::MetricPricingOverrides;
    let as_of = date!(2025 - 01 - 15);
    let mut bond = Bond::fixed(
        id,
        Money::from((100_i64, Currency::USD)),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        as_of,
        date!(2030 - 01 - 15),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond");
    bond.metric_pricing_overrides = MetricPricingOverrides::default()
        .with_theta_period(Tenor::semi_annual())
        .with_breakeven_config(BreakevenConfig { target, mode });
    bond
}

/// The sensitivity metric is declared via `dynamic_dependencies`, so the
/// result must not depend on the order the caller lists metrics in.
///
/// Regression: `dependencies()` declared only `CarryTotal`, so
/// `[Cs01, Breakeven]` returned a value while `[Breakeven, Cs01]` and a
/// bare `[Breakeven]` both failed with "compute cs01 alongside Breakeven".
#[test]
fn test_breakeven_is_independent_of_request_order() {
    use finstack_quant_valuations::instruments::Instrument;
    use finstack_quant_valuations::instruments::PricingOptions;

    let as_of = date!(2025 - 01 - 15);
    let market = MarketContext::new().insert(flat_discount_curve("USD-OIS", 0.04, as_of));
    let bond = breakeven_bond("ORDER", BreakevenTarget::ZSpread, BreakevenMode::Linear);

    let breakeven_for = |ids: &[MetricId]| -> f64 {
        bond.price_with_metrics(&market, as_of, ids, PricingOptions::default())
            .unwrap_or_else(|e| panic!("price_with_metrics failed for {ids:?}: {e}"))
            .measures
            .get(MetricId::Breakeven.as_str())
            .copied()
            .unwrap_or_else(|| panic!("breakeven missing for {ids:?}"))
    };

    let sensitivity_first = breakeven_for(&[MetricId::Cs01, MetricId::Breakeven]);
    let breakeven_first = breakeven_for(&[MetricId::Breakeven, MetricId::Cs01]);
    let breakeven_alone = breakeven_for(&[MetricId::Breakeven]);

    assert!(
        sensitivity_first.is_finite() && sensitivity_first.abs() > 1e-9,
        "expected a meaningful breakeven, got {sensitivity_first}"
    );
    assert_eq!(sensitivity_first, breakeven_first);
    assert_eq!(sensitivity_first, breakeven_alone);
}

/// A `Ytm` breakeven depends on `Dv01`, not `Cs01`; the dynamic dependency
/// must follow the configured target.
#[test]
fn test_breakeven_ytm_target_resolves_dv01_dependency_alone() {
    use finstack_quant_valuations::instruments::Instrument;
    use finstack_quant_valuations::instruments::PricingOptions;

    let as_of = date!(2025 - 01 - 15);
    let market = MarketContext::new().insert(flat_discount_curve("USD-OIS", 0.04, as_of));
    let bond = breakeven_bond("YTM-DEP", BreakevenTarget::Ytm, BreakevenMode::Linear);

    // Requesting Breakeven alone must succeed: the dynamic dependency has to
    // resolve Dv01 (not Cs01) for a Ytm target. The registry returns only
    // the metrics the caller asked for, so compare against an explicit run.
    let alone = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Breakeven],
            PricingOptions::default(),
        )
        .expect("Ytm breakeven should resolve Dv01 without being asked")
        .measures
        .get(MetricId::Breakeven.as_str())
        .copied()
        .expect("breakeven");

    let explicit = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01, MetricId::CarryTotal, MetricId::Breakeven],
            PricingOptions::default(),
        )
        .expect("explicit run");
    let breakeven = explicit
        .measures
        .get(MetricId::Breakeven.as_str())
        .copied()
        .expect("breakeven");
    let dv01 = explicit
        .measures
        .get(MetricId::Dv01.as_str())
        .copied()
        .expect("dv01");
    let carry = explicit
        .measures
        .get(MetricId::CarryTotal.as_str())
        .copied()
        .expect("carry_total");

    assert_eq!(
        alone, breakeven,
        "Breakeven must not depend on request list"
    );
    assert!((breakeven - (-carry / dv01)).abs() < 1e-8);
}

/// The iterative path had no passing-case coverage — the mode with the
/// root-finder and the unit conversions. Pin it against the linear
/// estimate and verify it actually zeroes the P&L objective.
#[test]
fn test_iterative_breakeven_zeroes_the_horizon_pnl() {
    use finstack_quant_valuations::instruments::Instrument;
    use finstack_quant_valuations::instruments::PricingOptions;

    let as_of = date!(2025 - 01 - 15);
    let market = MarketContext::new().insert(flat_discount_curve("USD-OIS", 0.04, as_of));

    let ids = &[MetricId::Cs01, MetricId::CarryTotal, MetricId::Breakeven];
    let linear = breakeven_bond("BE-LIN", BreakevenTarget::ZSpread, BreakevenMode::Linear)
        .price_with_metrics(&market, as_of, ids, PricingOptions::default())
        .expect("linear");
    let iterative = breakeven_bond(
        "BE-ITER",
        BreakevenTarget::ZSpread,
        BreakevenMode::Iterative,
    )
    .price_with_metrics(&market, as_of, ids, PricingOptions::default())
    .expect("iterative");

    let be_lin = linear
        .measures
        .get(MetricId::Breakeven.as_str())
        .copied()
        .expect("linear breakeven");
    let be_iter = iterative
        .measures
        .get(MetricId::Breakeven.as_str())
        .copied()
        .expect("iterative breakeven");
    let carry = iterative
        .measures
        .get(MetricId::CarryTotal.as_str())
        .copied()
        .expect("carry_total");

    assert!(be_iter.is_finite(), "iterative breakeven must be finite");
    // Same sign and same order of magnitude as the linear estimate: this is
    // the guard that would have caught the historical 100x vol-units bug.
    assert_eq!(
        be_iter.signum(),
        be_lin.signum(),
        "iterative ({be_iter}) and linear ({be_lin}) must agree in sign"
    );
    let ratio = be_iter / be_lin;
    assert!(
        (0.5..2.0).contains(&ratio),
        "iterative ({be_iter}) vs linear ({be_lin}) ratio {ratio} is out of plausible range; \
         a units error would show up here as ~100x"
    );

    // The solved shift must actually zero the objective:
    //   carry + PV(bumped, horizon) - PV(base, horizon) ~= 0
    let bond = breakeven_bond("BE-CHK", BreakevenTarget::ZSpread, BreakevenMode::Iterative);
    let rolled =
        calculate_theta_date(as_of, Tenor::semi_annual(), bond.expiry()).expect("horizon date");
    let base_pv = bond.value(&market, rolled).expect("base pv").amount();
    let bumped = finstack_quant_valuations::metrics::bump_discount_curve_parallel(
        &market,
        &finstack_quant_core::types::CurveId::new("USD-OIS"),
        be_iter,
    )
    .expect("bump");
    let bumped_pv = bond.value(&bumped, rolled).expect("bumped pv").amount();

    let residual = carry + bumped_pv - base_pv;
    assert!(
        residual.abs() < 1e-6,
        "solved shift {be_iter} should zero the objective, residual = {residual}"
    );
}

/// On a credit-curve instrument, CS01 is a hazard-curve (par spread) bump
/// while the iterative solve shifts the discount curve. Reporting that as a
/// "ZSpread breakeven" would mislabel the risk factor, so it must be
/// rejected rather than silently returned.
#[test]
fn test_iterative_zspread_rejected_for_credit_curve_instrument() {
    use finstack_quant_core::market_data::term_structures::HazardCurve;
    use finstack_quant_core::types::CurveId;
    use finstack_quant_valuations::instruments::Instrument;
    use finstack_quant_valuations::instruments::MetricPricingOverrides;
    use finstack_quant_valuations::instruments::PricingOptions;

    let as_of = date!(2025 - 01 - 15);
    let mut bond = Bond::fixed(
        "CREDIT-BE",
        Money::from((100_i64, Currency::USD)),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        as_of,
        date!(2030 - 01 - 15),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond");
    bond.credit_curve_id = Some(CurveId::new("USD-CREDIT"));
    bond.metric_pricing_overrides = MetricPricingOverrides::default()
        .with_theta_period(Tenor::semi_annual())
        .with_breakeven_config(BreakevenConfig {
            target: BreakevenTarget::ZSpread,
            mode: BreakevenMode::Iterative,
        });

    let hazard = HazardCurve::builder("USD-CREDIT")
        .base_date(as_of)
        .recovery_rate(0.4)
        .knots([(0.0, 0.02), (5.0, 0.02)])
        .build()
        .expect("hazard curve");
    let market = MarketContext::new()
        .insert(flat_discount_curve("USD-OIS", 0.04, as_of))
        .insert(hazard);

    let err = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Breakeven],
            PricingOptions::default(),
        )
        .expect_err("iterative ZSpread on a credit bond should be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("hazard-curve") && msg.contains("BreakevenMode::Linear"),
        "error should explain the risk-factor mismatch and the remedy: {msg}"
    );
}

/// A near-zero sensitivity must explain itself rather than emitting the
/// context-free "Invalid input data".
#[test]
fn test_zero_sensitivity_error_names_the_metric_and_value() {
    let mut ctx = context_with_carry_and_sensitivity(
        0.50,
        0.0,
        BreakevenTarget::ZSpread,
        BreakevenMode::Linear,
    );
    let err = BreakevenCalculator
        .calculate(&mut ctx)
        .expect_err("zero sensitivity should error");
    let msg = err.to_string();
    assert!(
        msg.contains("ZSpread") && msg.contains("cs01") && msg.contains("undefined"),
        "error should name target, sensitivity metric and cause: {msg}"
    );
}

#[test]
fn test_breakeven_horizon_matches_carry_horizon() {
    use finstack_quant_valuations::instruments::Instrument;
    use finstack_quant_valuations::instruments::MetricPricingOverrides;
    use finstack_quant_valuations::instruments::PricingOptions;

    let as_of = date!(2025 - 01 - 15);

    // Compute with 1M horizon
    let mut bond_1m = Bond::fixed(
        "HORIZON-TEST",
        Money::from((100_i64, Currency::USD)),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        as_of,
        date!(2030 - 01 - 15),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond");

    bond_1m.metric_pricing_overrides = MetricPricingOverrides::default()
        .with_theta_period(Tenor::monthly())
        .with_breakeven_config(BreakevenConfig {
            target: BreakevenTarget::ZSpread,
            mode: BreakevenMode::Linear,
        });

    let market = MarketContext::new().insert(flat_discount_curve("USD-OIS", 0.04, as_of));

    let result_1m = bond_1m
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::CarryTotal, MetricId::Cs01, MetricId::Breakeven],
            PricingOptions::default(),
        )
        .expect("1m result");

    // Compute with 6M horizon
    let mut bond_6m = Bond::fixed(
        "HORIZON-TEST",
        Money::from((100_i64, Currency::USD)),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        as_of,
        date!(2030 - 01 - 15),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond");

    bond_6m.metric_pricing_overrides = MetricPricingOverrides::default()
        .with_theta_period(Tenor::semi_annual())
        .with_breakeven_config(BreakevenConfig {
            target: BreakevenTarget::ZSpread,
            mode: BreakevenMode::Linear,
        });

    let result_6m = bond_6m
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::CarryTotal, MetricId::Cs01, MetricId::Breakeven],
            PricingOptions::default(),
        )
        .expect("6m result");

    let be_1m = result_1m
        .measures
        .get(MetricId::Breakeven.as_str())
        .copied()
        .expect("be_1m");
    let be_6m = result_6m
        .measures
        .get(MetricId::Breakeven.as_str())
        .copied()
        .expect("be_6m");

    // 6M carry > 1M carry, so 6M breakeven should be larger (more room to widen)
    assert!(
        be_6m.abs() > be_1m.abs(),
        "6M breakeven ({be_6m}) should have larger magnitude than 1M ({be_1m})"
    );

    // Pin the relationship rather than only its direction: both are
    // `-carry / cs01` against the same CS01, so the breakeven ratio must
    // track the carry ratio. A bare inequality passes for a wide range of
    // wrong implementations.
    let carry_1m = result_1m
        .measures
        .get(MetricId::CarryTotal.as_str())
        .copied()
        .expect("carry_1m");
    let carry_6m = result_6m
        .measures
        .get(MetricId::CarryTotal.as_str())
        .copied()
        .expect("carry_6m");
    let cs01_1m = result_1m
        .measures
        .get(MetricId::Cs01.as_str())
        .copied()
        .expect("cs01_1m");
    let cs01_6m = result_6m
        .measures
        .get(MetricId::Cs01.as_str())
        .copied()
        .expect("cs01_6m");

    assert!((be_1m - (-carry_1m / cs01_1m)).abs() < 1e-8, "1M: {be_1m}");
    assert!((be_6m - (-carry_6m / cs01_6m)).abs() < 1e-8, "6M: {be_6m}");

    // CS01 is horizon-independent here (both measured at as_of), so the
    // breakeven ratio must equal the carry ratio.
    assert!(
        (cs01_1m - cs01_6m).abs() < 1e-12,
        "CS01 should not depend on the theta period: {cs01_1m} vs {cs01_6m}"
    );
    let carry_ratio = carry_6m / carry_1m;
    let breakeven_ratio = be_6m / be_1m;
    assert!(
        (carry_ratio - breakeven_ratio).abs() < 1e-8,
        "breakeven ratio ({breakeven_ratio}) should equal carry ratio ({carry_ratio})"
    );
}
