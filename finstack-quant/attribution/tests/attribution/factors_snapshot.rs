//! Tests for the surrounding crate component and its documented behavior.
//!
use finstack_quant_attribution::{MarketRestoreFlags, MarketSnapshot};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{
    BaseCorrelationCurve, DiscountCurve, ForwardCurve, HazardCurve, InflationCurve,
};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::fx::{FxConversionPolicy, FxMatrix, FxProvider};
use std::sync::Arc;
use time::macros::date;

struct StaticFxProvider;

impl FxProvider for StaticFxProvider {
    fn rate(
        &self,
        from: Currency,
        to: Currency,
        _on: Date,
        _policy: FxConversionPolicy,
    ) -> finstack_quant_core::Result<f64> {
        match (from, to) {
            (a, b) if a == b => Ok(1.0),
            (Currency::USD, Currency::EUR) => Ok(0.9),
            (Currency::EUR, Currency::USD) => Ok(1.0 / 0.9),
            _ => Ok(1.0),
        }
    }
}

fn sample_fx_matrix() -> FxMatrix {
    FxMatrix::new(Arc::new(StaticFxProvider))
}

fn create_test_discount_curve(id: &str, base_date: Date) -> DiscountCurve {
    DiscountCurve::builder(id)
        .base_date(base_date)
        .knots(vec![(0.0, 1.0), (1.0, 0.98), (5.0, 0.90)])
        .interp(InterpStyle::Linear)
        .build()
        .expect("DiscountCurve builder should succeed with valid test data")
}

fn create_test_forward_curve(id: &str, base_date: Date) -> ForwardCurve {
    ForwardCurve::builder(id, 0.25)
        .base_date(base_date)
        .knots(vec![(0.0, 0.02), (1.0, 0.025), (5.0, 0.03)])
        .build()
        .expect("ForwardCurve builder should succeed with valid test data")
}

fn create_test_hazard_curve(id: &str, base_date: Date) -> HazardCurve {
    HazardCurve::builder(id)
        .base_date(base_date)
        .knots(vec![(0.0, 0.0050), (1.0, 0.0055), (5.0, 0.0060)])
        .build()
        .expect("HazardCurve builder should succeed with valid test data")
}

fn create_test_inflation_curve(id: &str, _base_date: Date) -> InflationCurve {
    InflationCurve::builder(id)
        .base_date(_base_date)
        .base_cpi(100.0)
        .knots(vec![(0.0, 100.0), (1.0, 102.0), (5.0, 110.0)])
        .build()
        .expect("InflationCurve builder should succeed with valid test data")
}

fn create_test_base_correlation_curve(id: &str, _base_date: Date) -> BaseCorrelationCurve {
    BaseCorrelationCurve::builder(id)
        .knots(vec![
            (0.03, 0.30),
            (0.07, 0.40),
            (0.10, 0.50),
            (0.15, 0.60),
            (0.30, 0.70),
        ])
        .build()
        .expect("BaseCorrelationCurve builder should succeed with valid test data")
}

#[test]
fn test_extract_and_restore_rates_curves() {
    let base_date = date!(2025 - 01 - 15);
    let market = MarketContext::new()
        .insert(create_test_discount_curve("USD-OIS", base_date))
        .insert(create_test_discount_curve("EUR-OIS", base_date));

    let snapshot = MarketSnapshot::extract(&market, MarketRestoreFlags::RATES);
    assert_eq!(snapshot.discount_curves.len(), 2);

    let restored =
        MarketSnapshot::restore_market(&MarketContext::new(), &snapshot, MarketRestoreFlags::RATES);

    assert!(restored.get_discount("USD-OIS").is_ok());
    assert!(restored.get_discount("EUR-OIS").is_ok());
}

#[test]
fn test_market_restore_flags_individual() {
    assert_ne!(MarketRestoreFlags::DISCOUNT, MarketRestoreFlags::FORWARD);
    assert_ne!(MarketRestoreFlags::DISCOUNT, MarketRestoreFlags::HAZARD);
    assert_ne!(MarketRestoreFlags::HAZARD, MarketRestoreFlags::INFLATION);
    assert_ne!(
        MarketRestoreFlags::INFLATION,
        MarketRestoreFlags::CORRELATION
    );
}

#[test]
fn test_market_restore_flags_union() {
    let discount_forward = MarketRestoreFlags::DISCOUNT | MarketRestoreFlags::FORWARD;
    assert!(discount_forward.contains(MarketRestoreFlags::DISCOUNT));
    assert!(discount_forward.contains(MarketRestoreFlags::FORWARD));
    assert!(!discount_forward.contains(MarketRestoreFlags::HAZARD));
    assert_eq!(MarketRestoreFlags::RATES, discount_forward);
}

#[test]
fn test_market_restore_flags_intersection() {
    let rates = MarketRestoreFlags::RATES;
    let discount_hazard = MarketRestoreFlags::DISCOUNT | MarketRestoreFlags::HAZARD;

    let intersection = rates & discount_hazard;
    assert!(intersection.contains(MarketRestoreFlags::DISCOUNT));
    assert!(!intersection.contains(MarketRestoreFlags::FORWARD));
    assert!(!intersection.contains(MarketRestoreFlags::HAZARD));
}

#[test]
fn test_market_restore_flags_complement() {
    let not_discount = MarketRestoreFlags::all() & !MarketRestoreFlags::DISCOUNT;
    assert!(!not_discount.contains(MarketRestoreFlags::DISCOUNT));
    assert!(not_discount.contains(MarketRestoreFlags::FORWARD));
    assert!(not_discount.contains(MarketRestoreFlags::HAZARD));
    assert!(not_discount.contains(MarketRestoreFlags::INFLATION));
    assert!(not_discount.contains(MarketRestoreFlags::CORRELATION));
}

#[test]
fn test_market_restore_flags_all_and_empty() {
    let all = MarketRestoreFlags::all();
    assert!(all.contains(MarketRestoreFlags::DISCOUNT));
    assert!(all.contains(MarketRestoreFlags::FORWARD));
    assert!(all.contains(MarketRestoreFlags::HAZARD));
    assert!(all.contains(MarketRestoreFlags::INFLATION));
    assert!(all.contains(MarketRestoreFlags::CORRELATION));
    assert!(all.contains(MarketRestoreFlags::RATES));
    assert!(all.contains(MarketRestoreFlags::CREDIT));

    let empty = MarketRestoreFlags::empty();
    assert!(!empty.contains(MarketRestoreFlags::DISCOUNT));
    assert!(!empty.contains(MarketRestoreFlags::FORWARD));
}

#[test]
fn test_market_snapshot_extract_single_discount() {
    let base_date = date!(2025 - 01 - 15);
    let market = MarketContext::new()
        .insert(create_test_discount_curve("USD-OIS", base_date))
        .insert(create_test_forward_curve("USD-SOFR", base_date))
        .insert(create_test_hazard_curve("CORP-A", base_date));

    let snapshot = MarketSnapshot::extract(&market, MarketRestoreFlags::DISCOUNT);

    assert_eq!(snapshot.discount_curves.len(), 1);
    assert!(snapshot.discount_curves.contains_key("USD-OIS"));
    assert!(snapshot.forward_curves.is_empty());
    assert!(snapshot.hazard_curves.is_empty());
}

#[test]
fn test_market_snapshot_extract_all_curve_types() {
    let base_date = date!(2025 - 01 - 15);
    let market = MarketContext::new()
        .insert(create_test_discount_curve("USD-OIS", base_date))
        .insert(create_test_forward_curve("USD-SOFR", base_date))
        .insert(create_test_hazard_curve("CORP-A", base_date))
        .insert(create_test_inflation_curve("US-CPI", base_date))
        .insert(create_test_base_correlation_curve("CDX-IG", base_date));

    let snapshot = MarketSnapshot::extract(&market, MarketRestoreFlags::all());

    assert_eq!(snapshot.discount_curves.len(), 1);
    assert_eq!(snapshot.forward_curves.len(), 1);
    assert_eq!(snapshot.hazard_curves.len(), 1);
    assert_eq!(snapshot.inflation_curves.len(), 1);
    assert_eq!(snapshot.base_correlation_curves.len(), 1);
}

#[test]
fn test_market_snapshot_extract_empty_flags_and_empty_market() {
    let base_date = date!(2025 - 01 - 15);
    let market = MarketContext::new().insert(create_test_discount_curve("USD-OIS", base_date));

    let snapshot = MarketSnapshot::extract(&market, MarketRestoreFlags::empty());
    assert!(snapshot.discount_curves.is_empty());

    let empty_snap = MarketSnapshot::extract(&MarketContext::new(), MarketRestoreFlags::all());
    assert!(empty_snap.discount_curves.is_empty());
}

#[test]
fn test_restore_market_unified_discount_only() {
    let base_date = date!(2025 - 01 - 15);
    let current_market = MarketContext::new()
        .insert(create_test_discount_curve("USD-OIS", base_date))
        .insert(create_test_forward_curve("USD-SOFR", base_date))
        .insert(create_test_hazard_curve("CORP-A", base_date));

    let snapshot = MarketSnapshot {
        discount_curves: vec![(
            "EUR-OIS".into(),
            Arc::new(create_test_discount_curve("EUR-OIS", base_date)),
        )]
        .into_iter()
        .collect(),
        ..Default::default()
    };

    let restored =
        MarketSnapshot::restore_market(&current_market, &snapshot, MarketRestoreFlags::DISCOUNT);

    assert!(restored.get_discount("EUR-OIS").is_ok());
    assert!(restored.get_discount("USD-OIS").is_err());
    assert!(restored.get_forward("USD-SOFR").is_ok());
    assert!(restored.get_hazard("CORP-A").is_ok());
}

#[test]
fn test_restore_market_unified_rates() {
    let base_date = date!(2025 - 01 - 15);
    let current_market = MarketContext::new()
        .insert(create_test_discount_curve("USD-OIS", base_date))
        .insert(create_test_forward_curve("USD-SOFR", base_date))
        .insert(create_test_hazard_curve("CORP-A", base_date));

    let snapshot = MarketSnapshot {
        discount_curves: vec![(
            "EUR-OIS".into(),
            Arc::new(create_test_discount_curve("EUR-OIS", base_date)),
        )]
        .into_iter()
        .collect(),
        forward_curves: vec![(
            "EUR-ESTR".into(),
            Arc::new(create_test_forward_curve("EUR-ESTR", base_date)),
        )]
        .into_iter()
        .collect(),
        ..Default::default()
    };

    let restored =
        MarketSnapshot::restore_market(&current_market, &snapshot, MarketRestoreFlags::RATES);

    assert!(restored.get_discount("EUR-OIS").is_ok());
    assert!(restored.get_forward("EUR-ESTR").is_ok());
    assert!(restored.get_discount("USD-OIS").is_err());
    assert!(restored.get_hazard("CORP-A").is_ok());
}

#[test]
fn test_restore_market_empty_snapshot_and_empty_market() {
    let base_date = date!(2025 - 01 - 15);
    let market = MarketContext::new().insert(create_test_discount_curve("USD-OIS", base_date));

    let restored = MarketSnapshot::restore_market(
        &market,
        &MarketSnapshot::default(),
        MarketRestoreFlags::RATES,
    );
    assert!(restored.get_discount("USD-OIS").is_err());

    let snapshot = MarketSnapshot {
        discount_curves: vec![(
            "USD-OIS".into(),
            Arc::new(create_test_discount_curve("USD-OIS", base_date)),
        )]
        .into_iter()
        .collect(),
        ..Default::default()
    };
    let restored2 =
        MarketSnapshot::restore_market(&MarketContext::new(), &snapshot, MarketRestoreFlags::RATES);
    assert!(restored2.get_discount("USD-OIS").is_ok());
}

#[test]
fn test_restore_equivalence_mixed_curve_types() {
    let base_date = date!(2025 - 01 - 15);

    let market = MarketContext::new()
        .insert(create_test_discount_curve("USD-OIS", base_date))
        .insert(create_test_discount_curve("EUR-OIS", base_date))
        .insert(create_test_forward_curve("USD-SOFR", base_date))
        .insert(create_test_hazard_curve("CORP-A", base_date))
        .insert(create_test_inflation_curve("US-CPI", base_date))
        .insert(create_test_base_correlation_curve("CDX-IG", base_date));

    let rates_snap = MarketSnapshot::extract(&market, MarketRestoreFlags::RATES);
    let credit_snap = MarketSnapshot::extract(&market, MarketRestoreFlags::CREDIT);
    let inflation_snap = MarketSnapshot::extract(&market, MarketRestoreFlags::INFLATION);
    let corr_snap = MarketSnapshot::extract(&market, MarketRestoreFlags::CORRELATION);

    let target = MarketContext::new()
        .insert(create_test_discount_curve("GBP-OIS", base_date))
        .insert(create_test_hazard_curve("CORP-B", base_date));

    let after_rates =
        MarketSnapshot::restore_market(&target, &rates_snap, MarketRestoreFlags::RATES);
    assert!(after_rates.get_hazard("CORP-B").is_ok());

    let after_credit =
        MarketSnapshot::restore_market(&after_rates, &credit_snap, MarketRestoreFlags::CREDIT);
    assert!(after_credit.get_discount("USD-OIS").is_ok());
    assert!(after_credit.get_hazard("CORP-A").is_ok());

    let after_inflation = MarketSnapshot::restore_market(
        &after_credit,
        &inflation_snap,
        MarketRestoreFlags::INFLATION,
    );
    assert!(after_inflation.get_inflation_curve("US-CPI").is_ok());

    let final_market = MarketSnapshot::restore_market(
        &after_inflation,
        &corr_snap,
        MarketRestoreFlags::CORRELATION,
    );
    assert!(final_market.get_base_correlation("CDX-IG").is_ok());
    assert!(final_market.get_discount("GBP-OIS").is_err());
}

#[test]
fn test_combined_restore_matches_stacked_restore_for_cross_factor_flags() {
    let base_date = date!(2025 - 01 - 15);
    let market_t0 = MarketContext::new()
        .insert(create_test_discount_curve("USD-OIS", base_date))
        .insert(create_test_forward_curve("USD-SOFR", base_date))
        .insert(create_test_hazard_curve("CORP-A", base_date))
        .insert_fx(sample_fx_matrix());

    let market_t1 = MarketContext::new()
        .insert(create_test_discount_curve("EUR-OIS", base_date))
        .insert(create_test_forward_curve("EUR-ESTR", base_date))
        .insert(create_test_hazard_curve("CORP-B", base_date));

    let flags = MarketRestoreFlags::RATES | MarketRestoreFlags::CREDIT | MarketRestoreFlags::FX;
    let combined_snapshot = MarketSnapshot::extract(&market_t0, flags);
    let combined = MarketSnapshot::restore_market(&market_t1, &combined_snapshot, flags);

    let rates_snapshot = MarketSnapshot::extract(&market_t0, MarketRestoreFlags::RATES);
    let credit_snapshot = MarketSnapshot::extract(&market_t0, MarketRestoreFlags::CREDIT);
    let fx_snapshot = MarketSnapshot::extract(&market_t0, MarketRestoreFlags::FX);
    let stacked_rates =
        MarketSnapshot::restore_market(&market_t1, &rates_snapshot, MarketRestoreFlags::RATES);
    let stacked_credit = MarketSnapshot::restore_market(
        &stacked_rates,
        &credit_snapshot,
        MarketRestoreFlags::CREDIT,
    );
    let stacked =
        MarketSnapshot::restore_market(&stacked_credit, &fx_snapshot, MarketRestoreFlags::FX);

    assert_eq!(
        combined
            .get_discount("USD-OIS")
            .expect("combined discount")
            .df(1.0),
        stacked
            .get_discount("USD-OIS")
            .expect("stacked discount")
            .df(1.0)
    );
    assert_eq!(
        combined
            .get_forward("USD-SOFR")
            .expect("combined forward")
            .rate(1.0),
        stacked
            .get_forward("USD-SOFR")
            .expect("stacked forward")
            .rate(1.0)
    );
    assert_eq!(
        combined
            .get_hazard("CORP-A")
            .expect("combined hazard")
            .hazard_rate(1.0),
        stacked
            .get_hazard("CORP-A")
            .expect("stacked hazard")
            .hazard_rate(1.0)
    );
    assert!(combined.fx().is_some());
    assert!(stacked.fx().is_some());
}

#[test]
fn test_restore_fx_with_none_snapshot_clears_current_fx() {
    let market_with_fx = MarketContext::new().insert_fx(sample_fx_matrix());
    let snapshot_without_fx = MarketSnapshot::default();

    let restored = MarketSnapshot::restore_market(
        &market_with_fx,
        &snapshot_without_fx,
        MarketRestoreFlags::FX,
    );

    assert!(restored.fx().is_none());
}

#[test]
fn test_volatility_snapshot_extract() {
    let market = MarketContext::new();
    let snapshot = MarketSnapshot::extract(&market, MarketRestoreFlags::VOL);
    assert!(snapshot.surfaces.is_empty());
}

#[test]
fn test_scalars_snapshot_extract() {
    let market = MarketContext::new();
    let snapshot = MarketSnapshot::extract(&market, MarketRestoreFlags::SCALARS);
    assert_eq!(snapshot.prices.len(), 0);
    assert_eq!(snapshot.series.len(), 0);
    assert_eq!(snapshot.inflation_indices.len(), 0);
    assert_eq!(snapshot.dividends.len(), 0);
}

/// Quant review B2: `restore_market` must preserve every market-data family
/// the snapshot does not model. The previous implementation rebuilt the
/// context from scratch and silently dropped price / vol-index curves, SABR
/// vol cubes, collateral CSA mappings, etc., hard-failing (or silently
/// corrupting) every factor reprice for instruments that depend on them.
#[test]
fn test_restore_preserves_families_outside_the_snapshot_model() {
    use finstack_quant_core::market_data::surfaces::VolCube;
    use finstack_quant_core::market_data::term_structures::{PriceCurve, VolatilityIndexCurve};
    use finstack_quant_core::math::volatility::sabr::SabrParams;
    use finstack_quant_core::types::CurveId;

    let as_of = date!(2025 - 01 - 15);
    let price_curve = PriceCurve::builder("WTI")
        .base_date(as_of)
        .spot_price(70.0)
        .knots([(0.0, 70.0), (1.0, 72.0)])
        .build()
        .unwrap();
    let vol_index = VolatilityIndexCurve::builder("VIX")
        .base_date(as_of)
        .spot_level(15.0)
        .knots([(0.0, 15.0), (1.0, 18.0)])
        .build()
        .unwrap();
    let sabr = SabrParams::new(0.035, 0.5, -0.2, 0.4).unwrap();
    let cube = VolCube::builder("USD-SWAPTION")
        .expiries(&[1.0])
        .tenors(&[5.0])
        .node(sabr, 0.03)
        .build()
        .unwrap();

    let market_t1 = MarketContext::new()
        .insert(create_test_discount_curve("USD-OIS", as_of))
        .insert(create_test_hazard_curve("ACME-HAZ", as_of))
        .insert(price_curve)
        .insert(vol_index)
        .insert_vol_cube(cube)
        .map_collateral("CSA-USD", CurveId::new("USD-OIS"));

    let market_t0 = MarketContext::new().insert(create_test_discount_curve("USD-OIS", as_of));

    // Restore the rates family to T0: everything else must survive.
    let snapshot = MarketSnapshot::extract(&market_t0, MarketRestoreFlags::RATES);
    let restored = MarketSnapshot::restore_market(&market_t1, &snapshot, MarketRestoreFlags::RATES);

    assert!(restored.get_discount("USD-OIS").is_ok(), "rates restored");
    assert!(restored.get_hazard("ACME-HAZ").is_ok(), "hazard preserved");
    assert!(
        restored.get_price_curve("WTI").is_ok(),
        "price curve must survive a rates restore"
    );
    assert!(
        restored.get_vol_index_curve("VIX").is_ok(),
        "vol-index curve must survive a rates restore"
    );
    assert!(
        restored.get_vol_cube("USD-SWAPTION").is_ok(),
        "SABR vol cube must survive a rates restore"
    );
    assert!(
        restored.get_collateral("CSA-USD").is_ok(),
        "collateral CSA mapping must survive a rates restore"
    );

    // A VOL-flagged restore replaces cubes wholesale: T0 has none, so the
    // cube must be gone afterwards (vol cubes belong to the VOL family now).
    let vol_snapshot = MarketSnapshot::extract(&market_t0, MarketRestoreFlags::VOL);
    let vol_restored =
        MarketSnapshot::restore_market(&market_t1, &vol_snapshot, MarketRestoreFlags::VOL);
    assert!(
        vol_restored.get_vol_cube("USD-SWAPTION").is_err(),
        "VOL restore must replace vol cubes from the snapshot"
    );
    assert!(
        vol_restored.get_price_curve("WTI").is_ok(),
        "price curve must survive a vol restore"
    );
}
