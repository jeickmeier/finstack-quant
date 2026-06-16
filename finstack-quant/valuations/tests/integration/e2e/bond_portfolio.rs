//! Bond Portfolio End-to-End Integration Tests
//!
//! Tests complete workflows for multi-currency bond portfolios including:
//! - Multi-currency market context creation
//! - Large portfolio pricing (100 bonds)
//! - Comprehensive metric computation in strict mode
//! - DataFrame export with correct metric key mapping
//!
//! # Test Coverage
//!
//! - **Portfolio Scale**: 100 bonds across USD/EUR/GBP
//! - **Metrics**: 9 metrics per bond (CleanPrice, DirtyPrice, Accrued, YTM,
//!   DurationMod, DurationMac, Convexity, DV01, Theta)
//! - **Strict Mode**: Verifies no silent failures or zero values
//! - **Export**: Validates MetricId → DataFrame column mapping

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::{standard_registry, MetricContext, MetricId};
use finstack_quant_valuations::results::ValuationResult;
use indexmap::IndexMap;
use std::sync::Arc;
use time::macros::date;

/// Standard set of metrics to compute for each bond.
fn standard_bond_metrics() -> Vec<MetricId> {
    vec![
        MetricId::CleanPrice,
        MetricId::DirtyPrice,
        MetricId::Accrued,
        MetricId::Ytm,
        MetricId::DurationMod,
        MetricId::DurationMac,
        MetricId::Convexity,
        MetricId::Dv01,
        MetricId::Theta,
    ]
}

/// Create a standard USD OIS discount curve for testing.
fn create_usd_discount_curve(as_of: Date) -> DiscountCurve {
    DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([
            (0.0f64, 1.0f64),
            (0.25f64, 0.987f64),
            (0.5f64, 0.974f64),
            (1.0f64, 0.948f64),
            (2.0f64, 0.898f64),
            (3.0f64, 0.850f64),
            (5.0f64, 0.760f64),
            (7.0f64, 0.680f64),
            (10.0f64, 0.580f64),
        ])
        .build()
        .expect("Curve should build")
}

/// Build a multi-currency bond portfolio for testing.
///
/// Creates bonds in USD, EUR, and GBP to test cross-currency workflows.
fn build_bond_portfolio(as_of: Date) -> Vec<Bond> {
    let mut bonds = Vec::new();

    // USD bonds (60% of portfolio)
    for i in 0..60 {
        let currency = Currency::try_from("USD").unwrap();
        let notional = Money::new(1_000_000.0, currency);
        let maturity_years = 1 + (i % 10); // 1-10 year maturities
        let maturity = as_of + time::Duration::days((maturity_years * 365) as i64);
        let coupon = 0.04 + (i as f64 * 0.001); // 4.0% - 6.9%

        let bond = Bond::fixed(
            format!("USD-BOND-{:03}", i + 1),
            notional,
            coupon,
            as_of,
            maturity,
            "USD-OIS",
        )
        .unwrap();

        bonds.push(bond);
    }

    // EUR bonds (25% of portfolio)
    for i in 0..25 {
        let currency = Currency::try_from("EUR").unwrap();
        let notional = Money::new(1_000_000.0, currency);
        let maturity_years = 1 + (i % 10);
        let maturity = as_of + time::Duration::days((maturity_years * 365) as i64);
        let coupon = 0.03 + (i as f64 * 0.001); // 3.0% - 5.4%

        let bond = Bond::fixed(
            format!("EUR-BOND-{:03}", i + 1),
            notional,
            coupon,
            as_of,
            maturity,
            "EUR-OIS",
        )
        .unwrap();

        bonds.push(bond);
    }

    // GBP bonds (15% of portfolio)
    for i in 0..15 {
        let currency = Currency::try_from("GBP").unwrap();
        let notional = Money::new(1_000_000.0, currency);
        let maturity_years = 1 + (i % 10);
        let maturity = as_of + time::Duration::days((maturity_years * 365) as i64);
        let coupon = 0.045 + (i as f64 * 0.001); // 4.5% - 6.4%

        let bond = Bond::fixed(
            format!("GBP-BOND-{:03}", i + 1),
            notional,
            coupon,
            as_of,
            maturity,
            "GBP-OIS",
        )
        .unwrap();

        bonds.push(bond);
    }

    bonds
}

/// Create market context with discount curves for multi-currency portfolio.
fn create_multi_currency_market(as_of: Date) -> MarketContext {
    let mut market = MarketContext::new();

    // USD curve
    let usd_curve = create_usd_discount_curve(as_of);
    market = market.insert(usd_curve);

    // EUR curve
    let eur_curve = DiscountCurve::builder("EUR-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .knots([
            (0.0f64, 1.0f64),
            (1.0f64, 0.97f64),
            (2.0f64, 0.94f64),
            (5.0f64, 0.85f64),
            (10.0f64, 0.70f64),
        ])
        .build()
        .expect("EUR curve should build");
    market = market.insert(eur_curve);

    // GBP curve
    let gbp_curve = DiscountCurve::builder("GBP-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([
            (0.0f64, 1.0f64),
            (1.0f64, 0.955f64),
            (2.0f64, 0.91f64),
            (5.0f64, 0.82f64),
            (10.0f64, 0.67f64),
        ])
        .build()
        .expect("GBP curve should build");
    market = market.insert(gbp_curve);

    market
}

/// Tests pricing a 100-bond portfolio across USD/EUR/GBP with strict metric computation.
///
/// # Validates
///
/// - Portfolio construction with 100 bonds across 3 currencies
/// - All 9 metrics compute successfully for each bond
/// - No silent failures (all metrics finite and non-zero where expected)
/// - Strict mode error propagation works correctly
#[test]
fn test_100_bond_portfolio_pricing() {
    let as_of = date!(2024 - 01 - 15);

    // Step 1: Create market context (wrap in Arc once for efficient sharing)
    let market = Arc::new(create_multi_currency_market(as_of));

    // Step 2: Build bond portfolio (100 bonds across 3 currencies)
    let portfolio = build_bond_portfolio(as_of);
    assert_eq!(portfolio.len(), 100, "Portfolio should contain 100 bonds");

    // Step 3: Create metrics registry
    let metrics_registry = standard_registry();

    // Step 4: Price all bonds with full metrics (strict mode)
    let metric_ids = standard_bond_metrics();
    let mut all_metrics_count = 0;

    for bond in &portfolio {
        // Get PV first
        let pv = bond.value(&market, as_of).expect("Bond should price");

        // Create metric context (cheap Arc::clone instead of full market clone)
        let mut context = MetricContext::new(
            Arc::new(bond.clone()),
            Arc::clone(&market),
            as_of,
            pv,
            MetricContext::default_config(),
        );

        // Compute metrics in strict mode (no silent failures)
        let result = metrics_registry
            .compute(&metric_ids, &mut context)
            .expect("All metrics should succeed in strict mode");

        // Verify all metrics were computed
        assert_eq!(
            result.len(),
            metric_ids.len(),
            "All {} metrics should be computed for bond {}",
            metric_ids.len(),
            bond.id()
        );

        // Verify no zero values from silent failures
        for (metric_id, value) in &result {
            assert!(
                value.is_finite(),
                "Metric {} for bond {} should be finite, got {}",
                metric_id.as_str(),
                bond.id(),
                value
            );

            // DV01 and Theta should never be exactly 0.0 for a bond
            if metric_id == &MetricId::Dv01 || metric_id == &MetricId::Theta {
                assert!(
                    value.abs() > 1e-10,
                    "Metric {} for bond {} should not be zero (silent failure check), got {}",
                    metric_id.as_str(),
                    bond.id(),
                    value
                );
            }
        }

        all_metrics_count += result.len();
    }

    // Verify total metrics computed
    assert_eq!(
        all_metrics_count,
        100 * metric_ids.len(),
        "Should have computed {} metrics total",
        100 * metric_ids.len()
    );
}

/// Tests DataFrame export with correct MetricId → column key mapping.
///
/// # Validates
///
/// - `duration_mod` → `row.duration`
/// - `dv01` → `row.dv01`
/// - `convexity` → `row.convexity`
/// - `ytm` → `row.ytm`
/// - Values are reasonable (non-zero, finite, expected ranges)
#[test]
fn test_dataframe_export_metric_keys() {
    let as_of = date!(2024 - 01 - 15);
    let market = Arc::new(create_multi_currency_market(as_of));

    let currency = Currency::try_from("USD").unwrap();
    let notional = Money::new(1_000_000.0, currency);
    let bond = Bond::fixed(
        "TEST-BOND-EXPORT",
        notional,
        0.05,
        as_of,
        as_of + time::Duration::days(5 * 365),
        "USD-OIS",
    )
    .unwrap();

    // Compute metrics
    let metric_ids = vec![
        MetricId::DurationMod,
        MetricId::Dv01,
        MetricId::Convexity,
        MetricId::Ytm,
        MetricId::CleanPrice,
    ];

    // Compute PV and metrics
    let pv = bond.value(&market, as_of).expect("Should price");
    let mut context = MetricContext::new(
        Arc::new(bond),
        Arc::clone(&market),
        as_of,
        pv,
        MetricContext::default_config(),
    );
    let metrics_registry = standard_registry();
    let computed_metrics = metrics_registry
        .compute(&metric_ids, &mut context)
        .expect("Metrics should compute");

    // Build a ValuationResult for testing export
    let mut measures = IndexMap::new();
    for (metric_id, value) in computed_metrics {
        measures.insert(metric_id, value);
    }

    let result = ValuationResult::stamped("TEST-BOND-EXPORT", as_of, pv).with_measures(measures);

    // Export to row
    let row = result.to_row();

    // Verify identity fields propagate.
    assert_eq!(row.instrument_id, "TEST-BOND-EXPORT");
    assert_eq!(row.currency, "USD");

    // Every requested metric appears as a named column, keyed by its
    // canonical MetricId string. No metric is hard-coded by the row type.
    let duration = row
        .measures
        .get("duration_mod")
        .copied()
        .expect("duration_mod should be populated");
    assert!(
        duration > 0.0,
        "Duration should be positive, got {duration}"
    );

    let dv01 = row
        .measures
        .get("dv01")
        .copied()
        .expect("dv01 should be populated");
    assert!(dv01.abs() > 1e-10, "DV01 should be non-zero, got {dv01}");

    let convexity = row
        .measures
        .get("convexity")
        .copied()
        .expect("convexity should be populated");
    assert!(
        convexity.is_finite(),
        "Convexity should be finite, got {convexity}"
    );

    let ytm = row
        .measures
        .get("ytm")
        .copied()
        .expect("ytm should be populated");
    assert!(
        ytm > 0.0 && ytm < 1.0,
        "YTM should be a reasonable rate (0-100%), got {ytm}"
    );
}
