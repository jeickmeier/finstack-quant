use super::test_helpers::{
    sample_base_correlation_curve, sample_base_date, sample_discount_curve, sample_forward_curve,
    sample_hazard_curve, sample_inflation_curve, sample_vol_surface,
};
use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::market_data::bumps::{BumpType, MarketBump};
use finstack_core::market_data::context::{BumpSpec, CurveStorage, MarketContext};
use finstack_core::market_data::dividends::DividendSchedule;
use finstack_core::market_data::scalars::{
    InflationIndex, InflationInterpolation, MarketScalar, ScalarTimeSeries, SeriesInterpolation,
};
use finstack_core::market_data::term_structures::CreditIndexData;
use finstack_core::money::fx::{FxConversionPolicy, FxMatrix, FxProvider};
use finstack_core::money::Money;
use finstack_core::types::CurveId;
use std::sync::Arc;
use time::Month;

// Simple static FX provider for testing
struct StaticFxProvider;
impl FxProvider for StaticFxProvider {
    fn rate(
        &self,
        from: Currency,
        to: Currency,
        _on: Date,
        _policy: FxConversionPolicy,
    ) -> finstack_core::Result<f64> {
        if from == Currency::USD && to == Currency::EUR {
            Ok(0.9)
        } else if from == to {
            Ok(1.0)
        } else {
            Ok(1.0 / 0.9)
        }
    }
}

fn sample_fx_matrix() -> FxMatrix {
    FxMatrix::new(Arc::new(StaticFxProvider))
}

#[test]
fn curve_storage_helpers() {
    let discount = CurveStorage::Discount(Arc::new(sample_discount_curve("USD-OIS")));
    assert!(discount.is_discount());
    assert_eq!(discount.curve_type(), "Discount");

    let forward = CurveStorage::Forward(Arc::new(sample_forward_curve("USD-LIBOR")));
    assert!(forward.is_forward());
    assert_eq!(forward.curve_type(), "Forward");

    let hazard = CurveStorage::Hazard(Arc::new(sample_hazard_curve("CDX")));
    assert!(hazard.is_hazard());

    let inflation = CurveStorage::Inflation(Arc::new(sample_inflation_curve("USD-CPI")));
    assert!(inflation.is_inflation());

    let base_corr = CurveStorage::BaseCorrelation(Arc::new(sample_base_correlation_curve("CDX")));
    assert!(base_corr.is_base_correlation());
}

#[test]
fn market_context_inserts_and_retrieves_curves() {
    let ctx = MarketContext::new()
        .insert(sample_discount_curve("USD-OIS"))
        .insert(sample_forward_curve("USD-LIBOR"))
        .insert(sample_hazard_curve("CDX"))
        .insert(sample_inflation_curve("USD-CPI"))
        .insert(sample_base_correlation_curve("CDX-BC"));

    let stats = ctx.stats();
    assert_eq!(stats.total_curves, 5);

    assert_eq!(
        ctx.get_discount("USD-OIS").unwrap().id().as_str(),
        "USD-OIS"
    );
    assert_eq!(
        ctx.get_forward("USD-LIBOR").unwrap().id().as_str(),
        "USD-LIBOR"
    );
    assert_eq!(ctx.get_hazard("CDX").unwrap().id().as_str(), "CDX");
    assert_eq!(
        ctx.get_inflation_curve("USD-CPI").unwrap().id().as_str(),
        "USD-CPI"
    );
    assert_eq!(
        ctx.get_base_correlation("CDX-BC").unwrap().id().as_str(),
        "CDX-BC"
    );
}

#[test]
fn market_context_getters_accept_preparsed_curve_ids() {
    let ctx = MarketContext::new()
        .insert(sample_discount_curve("USD-OIS"))
        .insert(sample_forward_curve("USD-LIBOR"))
        .insert(sample_hazard_curve("CDX"));

    assert_eq!(
        ctx.get_discount(CurveId::from("USD-OIS").as_str())
            .unwrap()
            .id()
            .as_str(),
        "USD-OIS"
    );
    assert_eq!(
        ctx.get_forward(CurveId::from("USD-LIBOR").as_str())
            .unwrap()
            .id()
            .as_str(),
        "USD-LIBOR"
    );
    assert_eq!(
        ctx.get_hazard(CurveId::from("CDX").as_str())
            .unwrap()
            .id()
            .as_str(),
        "CDX"
    );
}

#[test]
fn market_context_typed_mut_variants_store_curves() {
    let discount = sample_discount_curve("USD-OIS");
    let forward = sample_forward_curve("USD-LIBOR");

    let ctx = MarketContext::new().insert(discount).insert(forward);

    assert_eq!(
        ctx.get_discount("USD-OIS").unwrap().id().as_str(),
        "USD-OIS"
    );
    assert_eq!(
        ctx.get_forward("USD-LIBOR").unwrap().id().as_str(),
        "USD-LIBOR"
    );
}

#[test]
fn market_context_manages_fx_and_scalars() {
    let vol_surface = sample_vol_surface();
    let series = ScalarTimeSeries::new(
        "CPI",
        vec![
            (sample_base_date(), 100.0),
            (
                Date::from_calendar_date(2024, Month::February, 1).unwrap(),
                101.0,
            ),
        ],
        Some(Currency::USD),
    )
    .unwrap()
    .with_interpolation(SeriesInterpolation::Linear);

    let index = InflationIndex::new(
        "US-CPI",
        vec![
            (
                Date::from_calendar_date(2024, Month::January, 31).unwrap(),
                100.0,
            ),
            (
                Date::from_calendar_date(2024, Month::February, 29).unwrap(),
                101.0,
            ),
        ],
        Currency::USD,
    )
    .unwrap()
    .with_interpolation(InflationInterpolation::Linear);

    let dividends = DividendSchedule::new("AAPL-DIVS")
        .add_cash(sample_base_date(), Money::new(1.0, Currency::USD));
    let credit_index = CreditIndexData::builder()
        .num_constituents(1)
        .index_credit_curve(Arc::new(sample_hazard_curve("CDX")))
        .base_correlation_curve(Arc::new(sample_base_correlation_curve("CDX-BC")))
        .build()
        .unwrap();

    let ctx = MarketContext::new()
        .insert_fx(sample_fx_matrix())
        .insert(sample_discount_curve("USD-OIS"))
        .insert_series(series)
        .insert_inflation_index("US-CPI", index)
        .insert_dividends(dividends)
        .insert_credit_index("CDX", credit_index)
        .insert_surface(vol_surface.clone())
        .insert_price("USD-PRIME", MarketScalar::Unitless(0.05));

    assert!(ctx.get_surface(vol_surface.id()).is_ok());
    match ctx.get_price("USD-PRIME").unwrap() {
        MarketScalar::Unitless(v) => {
            assert!((v - 0.05).abs() < 1e-12);
        }
        MarketScalar::Price(other) => panic!("unexpected scalar variant: {other:?}"),
    }

    assert_eq!(
        ctx.get_series("CPI").unwrap().currency(),
        Some(Currency::USD)
    );
    assert!(ctx.get_inflation_index("US-CPI").is_ok());
    assert!(ctx.get_dividend_schedule("AAPL-DIVS").is_ok());
    assert!(ctx.get_credit_index("CDX").is_ok());

    let ids: Vec<_> = ctx.curve_ids().map(|c| c.as_str().to_string()).collect();
    assert!(ids.contains(&"USD-OIS".to_string()));

    let counts = ctx.count_by_type();
    assert_eq!(counts.get(&"Discount"), Some(&1));
}

#[test]
fn market_context_supports_curve_bumps() {
    let ctx = MarketContext::new()
        .insert(sample_discount_curve("USD-OIS"))
        .insert(sample_forward_curve("USD-LIBOR"));

    // Get the original discount factor for verification
    let orig_disc = ctx.get_discount("USD-OIS").unwrap();
    let orig_df_5y = orig_disc.df(5.0);

    let bumped = ctx
        .bump([MarketBump::Curve {
            id: CurveId::new("USD-OIS"),
            spec: finstack_core::market_data::context::BumpSpec::parallel_bp(50.0),
        }])
        .expect("bump should succeed");

    // Original context is unchanged
    let orig_disc_after = ctx.get_discount("USD-OIS").unwrap();
    assert_eq!(
        orig_disc_after.df(5.0),
        orig_df_5y,
        "original context unchanged"
    );

    // Bumped context has the curve under the same ID, but with bumped values
    let bumped_disc = bumped.get_discount("USD-OIS").unwrap();
    assert_ne!(
        bumped_disc.df(5.0),
        orig_df_5y,
        "bumped curve has different values"
    );
    assert!(
        bumped_disc.df(5.0) < orig_df_5y,
        "bumped curve has lower discount factors (higher rates)"
    );
}

#[test]
fn market_context_bumps_surfaces_and_scalars() {
    let surface = sample_vol_surface();
    let price = MarketScalar::Price(Money::new(100.0, Currency::USD));
    let series = ScalarTimeSeries::new(
        "TS",
        vec![
            (sample_base_date(), 10.0),
            (sample_base_date() + time::Duration::days(30), 12.0),
        ],
        None,
    )
    .unwrap();

    let ctx = MarketContext::new()
        .insert_surface(surface.clone())
        .insert_price("EQ-SPOT", price)
        .insert_series(series);

    let original_vol = surface
        .value_checked(0.5, 1.0)
        .expect("vol lookup should succeed in test");
    let original_price = match ctx.get_price("EQ-SPOT").unwrap() {
        MarketScalar::Price(m) => m.amount(),
        MarketScalar::Unitless(_) => panic!("unexpected scalar variant"),
    };
    let original_series = ctx
        .get_series("TS")
        .unwrap()
        .value_on(sample_base_date())
        .unwrap();

    let bumped = ctx
        .bump([
            MarketBump::Curve {
                id: CurveId::from("EQ-VOL"),
                spec: BumpSpec::multiplier(1.10),
            },
            MarketBump::Curve {
                id: CurveId::from("EQ-SPOT"),
                spec: BumpSpec {
                    mode: finstack_core::market_data::context::BumpMode::Multiplicative,
                    units: finstack_core::market_data::context::BumpUnits::Factor,
                    value: 1.05,
                    bump_type: BumpType::Parallel,
                },
            },
            MarketBump::Curve {
                id: CurveId::from("TS"),
                spec: BumpSpec {
                    mode: finstack_core::market_data::context::BumpMode::Additive,
                    units: finstack_core::market_data::context::BumpUnits::Percent,
                    value: 10.0,
                    bump_type: BumpType::Parallel,
                },
            },
        ])
        .expect("bump should succeed");

    let bumped_vol = bumped
        .get_surface("EQ-VOL")
        .unwrap()
        .value_checked(0.5, 1.0)
        .unwrap();
    assert!(bumped_vol > original_vol);

    let bumped_price = match bumped.get_price("EQ-SPOT").unwrap() {
        MarketScalar::Price(m) => m.amount(),
        MarketScalar::Unitless(_) => panic!("unexpected scalar variant"),
    };
    assert!((bumped_price - original_price * 1.05).abs() < 1e-9);

    let bumped_series_value = bumped
        .get_series("TS")
        .unwrap()
        .value_on(sample_base_date())
        .unwrap();
    assert!((bumped_series_value - (original_series + 0.10)).abs() < 1e-9);
}

#[test]
fn market_context_roll_forward_keeps_credit_index_curves_consistent() {
    let hazard = sample_hazard_curve("CDX");
    let base_corr = sample_base_correlation_curve("CDX-BC");
    let credit_index = CreditIndexData::builder()
        .num_constituents(125)
        .recovery_rate(0.4)
        .index_credit_curve(Arc::new(hazard.clone()))
        .base_correlation_curve(Arc::new(base_corr.clone()))
        .build()
        .unwrap();

    let rolled = MarketContext::new()
        .insert(hazard)
        .insert(base_corr)
        .insert_credit_index("CDX-IG", credit_index)
        .roll_forward(30)
        .unwrap();

    let direct_hazard = rolled.get_hazard("CDX").unwrap();
    let bundled_credit_index = rolled.get_credit_index("CDX-IG").unwrap();

    assert_eq!(
        bundled_credit_index.index_credit_curve.base_date(),
        direct_hazard.base_date(),
        "credit index bundle should stay aligned with rolled hazard curve"
    );
}

#[test]
fn update_base_correlation_curve_keeps_direct_lookup_in_sync() {
    let original_curve = sample_base_correlation_curve("CDX-BC");
    let hazard = sample_hazard_curve("CDX");
    let credit_index = CreditIndexData::builder()
        .num_constituents(125)
        .recovery_rate(0.4)
        .index_credit_curve(Arc::new(hazard.clone()))
        .base_correlation_curve(Arc::new(original_curve.clone()))
        .build()
        .unwrap();

    let mut ctx = MarketContext::new()
        .insert(hazard)
        .insert(original_curve)
        .insert_credit_index("CDX-IG", credit_index);

    let updated_curve = Arc::new(
        finstack_core::market_data::term_structures::BaseCorrelationCurve::builder("CDX-BC")
            .knots([(3.0, 0.30), (7.0, 0.45), (10.0, 0.60)])
            .build()
            .unwrap(),
    );

    assert!(ctx.update_base_correlation_curve("CDX-IG", Arc::clone(&updated_curve)));
    let direct_curve = ctx.get_base_correlation("CDX-BC").unwrap();
    assert!((direct_curve.correlation(7.0) - 0.45).abs() < 1e-12);
}

#[test]
fn generic_curve_replace_rebinds_credit_index_dependencies() {
    let original_hazard = sample_hazard_curve("CDX");
    let base_corr = sample_base_correlation_curve("CDX-BC");
    let credit_index = CreditIndexData::builder()
        .num_constituents(125)
        .recovery_rate(0.4)
        .index_credit_curve(Arc::new(original_hazard.clone()))
        .base_correlation_curve(Arc::new(base_corr.clone()))
        .build()
        .unwrap();

    let replacement_hazard =
        finstack_core::market_data::term_structures::HazardCurve::builder("CDX")
            .base_date(sample_base_date() + time::Duration::days(30))
            .knots([(1.0, 0.02), (3.0, 0.025), (5.0, 0.03)])
            .build()
            .unwrap();

    let ctx = MarketContext::new()
        .insert(original_hazard)
        .insert(base_corr)
        .insert_credit_index("CDX-IG", credit_index)
        .insert(replacement_hazard);

    let direct_hazard = ctx.get_hazard("CDX").unwrap();
    let bundled_credit_index = ctx.get_credit_index("CDX-IG").unwrap();
    assert_eq!(
        bundled_credit_index.index_credit_curve.base_date(),
        direct_hazard.base_date()
    );
}

#[test]
fn cross_type_curve_replacement_invalidates_credit_index_dependency() {
    let original_hazard = sample_hazard_curve("CDX");
    let replacement_discount = sample_discount_curve("CDX");
    let base_corr = sample_base_correlation_curve("CDX-BC");
    let credit_index = CreditIndexData::builder()
        .num_constituents(125)
        .recovery_rate(0.4)
        .index_credit_curve(Arc::new(original_hazard.clone()))
        .base_correlation_curve(Arc::new(base_corr.clone()))
        .build()
        .unwrap();

    let ctx = MarketContext::new()
        .insert(original_hazard)
        .insert(base_corr)
        .insert_credit_index("CDX-IG", credit_index)
        .insert(replacement_discount);

    assert!(ctx.get_discount("CDX").is_ok());
    assert!(ctx.get_hazard("CDX").is_err());
    assert!(
        ctx.get_credit_index("CDX-IG").is_err(),
        "credit index should not keep stale hazard references after same-ID cross-type replacement"
    );
}

#[test]
fn insert_inflation_index_rejects_mismatched_storage_key() {
    let index = InflationIndex::new(
        "US-CPI",
        vec![
            (
                Date::from_calendar_date(2024, Month::January, 31).unwrap(),
                100.0,
            ),
            (
                Date::from_calendar_date(2024, Month::February, 29).unwrap(),
                101.0,
            ),
        ],
        Currency::USD,
    )
    .unwrap();

    let result = std::panic::catch_unwind(|| {
        let _ = MarketContext::new().insert_inflation_index("ALIAS", index);
    });
    assert!(
        result.is_err(),
        "mismatched inflation index keys should be rejected when inserted"
    );
}

#[test]
fn inflation_key_rate_bump_preserves_curve_metadata() {
    let curve = finstack_core::market_data::term_structures::InflationCurve::builder("US-CPI")
        .base_cpi(100.0)
        .base_date(sample_base_date())
        .day_count(finstack_core::dates::DayCount::Act360)
        .indexation_lag_months(6)
        .interp(finstack_core::math::interp::InterpStyle::Linear)
        .knots([(0.0, 100.0), (1.0, 102.0), (2.0, 104.0)])
        .build()
        .unwrap();

    let bumped = MarketContext::new()
        .insert(curve)
        .bump([MarketBump::Curve {
            id: CurveId::from("US-CPI"),
            spec: BumpSpec::triangular_key_rate_bp(0.0, 1.0, 2.0, 25.0),
        }])
        .unwrap();

    let bumped_curve = bumped.get_inflation_curve("US-CPI").unwrap();
    assert_eq!(bumped_curve.base_date(), sample_base_date());
    assert_eq!(
        bumped_curve.day_count(),
        finstack_core::dates::DayCount::Act360
    );
    assert_eq!(bumped_curve.indexation_lag_months(), 6);
    assert_eq!(
        bumped_curve.interp_style(),
        finstack_core::math::interp::InterpStyle::Linear
    );
}

#[test]
fn market_context_handles_additional_introspection() {
    let mut ctx = MarketContext::new();
    assert!(ctx.is_empty());

    ctx = ctx.insert(sample_discount_curve("USD-OIS"));
    assert!(!ctx.is_empty());
    assert!(ctx.curve("USD-OIS").is_some());

    let counts = ctx.count_by_type();
    assert_eq!(counts.get("Discount"), Some(&1));

    let collected: Vec<_> = ctx.curves_of_type("Discount").collect();
    assert_eq!(collected.len(), 1);

    let stats = ctx.stats();
    assert_eq!(stats.total_curves, 1);
    assert!(!stats.has_fx);
}

#[test]
fn market_context_update_and_bump_failures() {
    let base_corr = Arc::new(sample_base_correlation_curve("CDX-BC"));
    let hazard = Arc::new(sample_hazard_curve("CDX"));
    let credit_index = CreditIndexData::builder()
        .num_constituents(1)
        .index_credit_curve(Arc::clone(&hazard))
        .base_correlation_curve(Arc::clone(&base_corr))
        .build()
        .unwrap();

    let mut ctx = MarketContext::new()
        .insert(hazard.as_ref().clone())
        .insert(base_corr.as_ref().clone())
        .insert_credit_index("CDX", credit_index);
    let new_curve = Arc::new(sample_base_correlation_curve("CDX-NEW"));
    assert!(ctx.update_base_correlation_curve("CDX", Arc::clone(&new_curve)));
    assert_eq!(
        ctx.get_credit_index("CDX")
            .unwrap()
            .base_correlation_curve
            .id(),
        new_curve.id()
    );
    assert!(!ctx.update_base_correlation_curve("UNKNOWN", new_curve));

    assert!(ctx
        .bump([MarketBump::Curve {
            id: CurveId::new("MISSING"),
            spec: BumpSpec::parallel_bp(10.0),
        }])
        .is_err());
}

#[test]
fn market_context_collateral_and_stats() {
    let ctx = MarketContext::new()
        .insert(sample_discount_curve("USD-OIS"))
        .map_collateral("USD-CSA", CurveId::from("USD-OIS"));

    assert!(!ctx.is_empty());
    assert_eq!(ctx.total_objects(), 1);

    let stats = ctx.stats();
    assert_eq!(stats.total_curves, 1);
    assert_eq!(stats.collateral_mapping_count, 1);

    let collateral = ctx.get_collateral("USD-CSA").unwrap();
    assert!(collateral.df(0.5) < 1.0);
    let collateral_ref = ctx.get_collateral("USD-CSA").unwrap();
    assert!(collateral_ref.df(1.0) < 1.0);
}

#[test]
fn market_context_getters_type_mismatch_and_not_found() {
    let ctx = MarketContext::new()
        .insert(sample_discount_curve("USD-OIS"))
        .insert(sample_forward_curve("USD-LIBOR"))
        .insert_surface(sample_vol_surface())
        .insert_price("EQ-SPOT", MarketScalar::Unitless(1.0));

    // Not found cases
    assert!(ctx.get_discount("MISSING").is_err());
    assert!(ctx.get_forward("MISSING").is_err());
    assert!(ctx.get_surface("MISSING").is_err());
    assert!(ctx.get_price("MISSING").is_err());
    assert!(ctx.get_series("MISSING").is_err());
    assert!(ctx.get_credit_index("MISSING").is_err());
    assert!(ctx.get_inflation_index("MISSING").is_err());
    assert!(ctx.get_inflation_index("MISSING").is_err());
    assert!(ctx.get_dividend_schedule("MISSING").is_err());
    assert!(ctx.get_dividend_schedule("MISSING").is_err());

    // Type mismatch cases (ensure the "mismatch" branches are exercised)
    assert!(ctx.get_forward("USD-OIS").is_err());
    assert!(ctx.get_discount("USD-LIBOR").is_err());
    assert!(ctx.get_forward("USD-OIS").is_err());
    assert!(ctx.get_discount("USD-LIBOR").is_err());
}

#[test]
fn market_context_surface_and_dividends_arc_variants_preserve_identity() {
    let surface = Arc::new(sample_vol_surface());
    let dividends = Arc::new(
        DividendSchedule::new("AAPL-DIVS")
            .add_cash(sample_base_date(), Money::new(1.0, Currency::USD)),
    );

    let ctx = MarketContext::new()
        .insert_surface(Arc::clone(&surface))
        .insert_dividends(Arc::clone(&dividends));

    let got_surface = ctx.get_surface("EQ-VOL").unwrap();
    assert_eq!(got_surface.id(), surface.id());

    let got_divs = ctx.get_dividend_schedule("AAPL-DIVS").unwrap();
    assert_eq!(got_divs.id, dividends.id);
}

#[test]
fn market_context_collateral_error_paths() {
    // Missing mapping
    let ctx = MarketContext::new().insert(sample_discount_curve("USD-OIS"));
    assert!(ctx.get_collateral("MISSING-CSA").is_err());
    assert!(ctx.get_collateral("MISSING-CSA").is_err());

    // Mapping exists but curve is missing
    let ctx = MarketContext::new().map_collateral("USD-CSA", CurveId::from("USD-OIS"));
    assert!(ctx.get_collateral("USD-CSA").is_err());
    assert!(ctx.get_collateral("USD-CSA").is_err());
}

#[test]
fn market_context_roll_forward_preserves_ids_and_clones_non_curves() {
    let surface = sample_vol_surface();
    let ctx = MarketContext::new()
        .insert(sample_discount_curve("USD-OIS"))
        .insert(sample_forward_curve("USD-LIBOR"))
        .insert(sample_hazard_curve("CDX"))
        .insert(sample_inflation_curve("USD-CPI"))
        .insert(sample_base_correlation_curve("CDX-BC"))
        .insert_surface(surface)
        .insert_price("EQ-SPOT", MarketScalar::Unitless(1.0))
        .map_collateral("USD-CSA", CurveId::from("USD-OIS"))
        .insert_fx(sample_fx_matrix());

    let before_base = ctx.get_discount("USD-OIS").unwrap().base_date();
    let rolled = ctx.roll_forward(30).expect("roll_forward should succeed");

    // Original unchanged
    assert_eq!(
        ctx.get_discount("USD-OIS").unwrap().base_date(),
        before_base
    );

    // IDs preserved and base date advanced in rolled context
    assert_eq!(
        rolled.get_discount("USD-OIS").unwrap().id().as_str(),
        "USD-OIS"
    );
    assert_ne!(
        rolled.get_discount("USD-OIS").unwrap().base_date(),
        before_base
    );

    // Base correlation curves are passed through without "rolling" logic (still available, same id)
    assert_eq!(
        rolled.get_base_correlation("CDX-BC").unwrap().id().as_str(),
        "CDX-BC"
    );

    // Non-curve data is preserved
    assert!(rolled.fx().is_some());
    assert!(rolled.get_surface("EQ-VOL").is_ok());
    assert!(rolled.get_price("EQ-SPOT").is_ok());
    assert!(rolled.get_collateral("USD-CSA").is_ok());

    // Total object count should be unchanged across roll
    assert_eq!(rolled.total_objects(), ctx.total_objects());
}

#[test]
fn market_context_bump_fx_spot_error_and_success_paths() {
    let date = sample_base_date();

    // Error when no FX matrix is present
    let ctx = MarketContext::new();
    assert!(ctx
        .bump([MarketBump::FxPct {
            base: Currency::USD,
            quote: Currency::EUR,
            pct: 1.0,
            as_of: date,
        }])
        .is_err());

    // Success path with a static provider
    let ctx = MarketContext::new().insert_fx(sample_fx_matrix());
    let before = ctx
        .fx()
        .unwrap()
        .rate(finstack_core::money::fx::FxQuery::new(
            Currency::USD,
            Currency::EUR,
            date,
        ))
        .unwrap()
        .rate;

    let bumped = ctx
        .bump([MarketBump::FxPct {
            base: Currency::USD,
            quote: Currency::EUR,
            pct: 10.0,
            as_of: date,
        }])
        .unwrap();
    let after = bumped
        .fx()
        .unwrap()
        .rate(finstack_core::money::fx::FxQuery::new(
            Currency::USD,
            Currency::EUR,
            date,
        ))
        .unwrap()
        .rate;

    assert!((after - before * 1.10).abs() < 1e-12);
}

#[test]
fn market_context_bumps_inflation_triangular_key_rate_branch() {
    let ctx = MarketContext::new().insert(sample_inflation_curve("USD-CPI"));

    let orig = ctx.get_inflation_curve("USD-CPI").unwrap();
    let orig_levels = orig.cpi_levels().to_vec();

    let bumped = ctx
        .bump([MarketBump::Curve {
            id: CurveId::from("USD-CPI"),
            spec: BumpSpec::triangular_key_rate_bp(0.0, 1.0, 2.0, 1.0), // 1bp => +0.0001 fraction
        }])
        .unwrap();
    let bumped_inf = bumped.get_inflation_curve("USD-CPI").unwrap();
    let bumped_levels = bumped_inf.cpi_levels();

    // Closest knot to 1.0y should be bumped multiplicatively
    assert!((bumped_levels[1] - orig_levels[1] * 1.0001).abs() < 1e-9);
    // Other points unchanged
    assert!((bumped_levels[0] - orig_levels[0]).abs() < 1e-12);
    assert!((bumped_levels[2] - orig_levels[2]).abs() < 1e-12);
}

#[test]
fn market_context_apply_bumps_exercises_all_variants() {
    let date = sample_base_date();

    let ctx = MarketContext::new()
        .insert(sample_discount_curve("USD-OIS"))
        .insert(sample_base_correlation_curve("CDX-BC"))
        .insert_surface(sample_vol_surface())
        .insert_fx(sample_fx_matrix());

    let df_before = ctx.get_discount("USD-OIS").unwrap().df(2.0);
    let fx_before = ctx
        .fx()
        .unwrap()
        .rate(finstack_core::money::fx::FxQuery::new(
            Currency::USD,
            Currency::EUR,
            date,
        ))
        .unwrap()
        .rate;

    let vol_before = ctx
        .get_surface("EQ-VOL")
        .unwrap()
        .value_checked(0.5, 1.0)
        .unwrap();
    let bc_before = ctx.get_base_correlation("CDX-BC").unwrap().correlations()[0];

    let bumps = vec![
        // Curve bump
        MarketBump::Curve {
            id: CurveId::from("USD-OIS"),
            spec: BumpSpec::parallel_bp(25.0),
        },
        // FX bump (pct)
        MarketBump::FxPct {
            base: Currency::USD,
            quote: Currency::EUR,
            pct: 10.0,
            as_of: date,
        },
        // Vol surface bucket bump: filter a single bucket so we hit the filtered path
        MarketBump::VolBucketPct {
            surface_id: CurveId::from("EQ-VOL"),
            expiries: Some(vec![0.5]),
            strikes: Some(vec![1.0]),
            pct: 10.0,
        },
        // Base correlation bucket bump
        MarketBump::BaseCorrBucketPts {
            surface_id: CurveId::from("CDX-BC"),
            detachments: Some(vec![3.0]),
            points: 0.02,
        },
    ];

    let bumped = ctx.bump(bumps).unwrap();

    // Curve bumped
    let df_after = bumped.get_discount("USD-OIS").unwrap().df(2.0);
    assert_ne!(df_after, df_before);

    // FX bumped
    let fx_after = bumped
        .fx()
        .unwrap()
        .rate(finstack_core::money::fx::FxQuery::new(
            Currency::USD,
            Currency::EUR,
            date,
        ))
        .unwrap()
        .rate;
    assert!((fx_after - fx_before * 1.10).abs() < 1e-12);

    // Vol bucket bumped at (0.5, 1.0); non-bucket cells should remain unchanged.
    let vol_after = bumped
        .get_surface("EQ-VOL")
        .unwrap()
        .value_checked(0.5, 1.0)
        .unwrap();
    assert!((vol_after - vol_before * 1.10).abs() < 1e-12);
    let vol_other_before = ctx
        .get_surface("EQ-VOL")
        .unwrap()
        .value_checked(0.25, 0.9)
        .unwrap();
    let vol_other_after = bumped
        .get_surface("EQ-VOL")
        .unwrap()
        .value_checked(0.25, 0.9)
        .unwrap();
    assert!((vol_other_after - vol_other_before).abs() < 1e-12);

    // Base correlation bumped (first point matches 3.0 detachment in test helper curve)
    let bc_after = bumped
        .get_base_correlation("CDX-BC")
        .unwrap()
        .correlations()[0];
    assert!((bc_after - (bc_before + 0.02)).abs() < 1e-12);

    // Stats display covered
    let _ = format!("{}", bumped.stats());
}

#[test]
fn market_context_insert_and_stats_setters_cover_remaining_paths() {
    // Cover insert_surface(by value), insert_dividends(by value), insert_fx/insert_fx_mut,
    // insert_credit_index_mut, map_collateral_mut and
    // the mutable setters/iterators in stats.rs.
    let date = sample_base_date();

    let surface_by_value = finstack_core::market_data::surfaces::VolSurface::builder("IR-VOL")
        .expiries(&[1.0, 2.0])
        .strikes(&[90.0, 100.0])
        .row(&[0.2, 0.2])
        .row(&[0.2, 0.2])
        .build()
        .unwrap();

    let dividends_by_value =
        DividendSchedule::new("MSFT-DIVS").add_cash(date, Money::new(0.5, Currency::USD));

    let hazard = Arc::new(sample_hazard_curve("CDX"));
    let base_corr = Arc::new(sample_base_correlation_curve("CDX-BC"));
    let credit_index = CreditIndexData::builder()
        .num_constituents(2)
        .index_credit_curve(hazard)
        .base_correlation_curve(base_corr)
        .build()
        .unwrap();

    let mut ctx = MarketContext::new()
        .insert(sample_discount_curve("USD-OIS"))
        .insert_surface(surface_by_value)
        .insert_dividends(dividends_by_value)
        .insert_fx(sample_fx_matrix());

    // insert_credit_index + insert_fx
    ctx = ctx
        .insert_credit_index("CDX", credit_index)
        .insert_fx(sample_fx_matrix());

    // map_collateral
    ctx = ctx.map_collateral("USD-CSA", CurveId::from("USD-OIS"));
    assert!(ctx.get_collateral("USD-CSA").is_ok());

    // scalars / series / indices / dividends
    ctx = ctx.insert_price("PX", MarketScalar::Unitless(42.0));

    let series = ScalarTimeSeries::new(
        "TS-2",
        vec![(date, 1.0), (date + time::Duration::days(1), 2.0)],
        None,
    )
    .unwrap()
    .with_interpolation(SeriesInterpolation::Linear);
    ctx = ctx.insert_series(series);

    let idx = Arc::new(
        InflationIndex::new("US-CPI", vec![(date, 100.0)], Currency::USD)
            .unwrap()
            .with_interpolation(InflationInterpolation::Linear),
    );
    ctx = ctx.insert_inflation_index("US-CPI", idx);

    let divs =
        Arc::new(DividendSchedule::new("AAPL-DIVS").add_cash(date, Money::new(1.0, Currency::USD)));
    ctx = ctx.insert_dividends(divs);

    assert_eq!(ctx.prices_iter().count(), 1);
    assert_eq!(ctx.series_iter().count(), 1);
    assert_eq!(ctx.inflation_indices_iter().count(), 1);
    assert_eq!(ctx.dividends_iter().count(), 2); // MSFT-DIVS + AAPL-DIVS

    let stats = ctx.stats();
    assert!(stats.total_curves >= 1);
    assert!(stats.has_fx);
    assert!(stats.surface_count >= 1);
    assert_eq!(stats.collateral_mapping_count, 1);
}

#[test]
fn market_context_bump_more_curve_types_and_error_paths() {
    // Cover additional bump branches: forward multiplicative, hazard, inflation parallel, base correlation.
    let ctx = MarketContext::new()
        .insert(sample_discount_curve("USD-OIS"))
        .insert(sample_forward_curve("USD-LIBOR"))
        .insert(sample_hazard_curve("CDX"))
        .insert(sample_inflation_curve("USD-CPI"))
        .insert(sample_base_correlation_curve("CDX-BC"))
        .insert_surface(sample_vol_surface());

    let df_before = ctx.get_discount("USD-OIS").unwrap().df(2.0);
    let fwd_before = ctx.get_forward("USD-LIBOR").unwrap().rate(2.0);
    let haz_before = ctx.get_hazard("CDX").unwrap().hazard_rate(5.0);
    let cpi_before = ctx.get_inflation_curve("USD-CPI").unwrap().cpi_levels()[1];
    let bc_before = ctx.get_base_correlation("CDX-BC").unwrap().correlations()[0];

    let bumped = ctx
        .bump([
            MarketBump::Curve {
                id: CurveId::from("USD-OIS"),
                spec: BumpSpec::parallel_bp(10.0),
            },
            MarketBump::Curve {
                id: CurveId::from("USD-LIBOR"),
                spec: BumpSpec::multiplier(1.10),
            },
            MarketBump::Curve {
                id: CurveId::from("CDX"),
                spec: BumpSpec::parallel_bp(5.0),
            },
            MarketBump::Curve {
                id: CurveId::from("USD-CPI"),
                spec: BumpSpec::inflation_shift_pct(2.0),
            },
            MarketBump::Curve {
                id: CurveId::from("CDX-BC"),
                spec: BumpSpec::correlation_shift_pct(10.0),
            },
        ])
        .unwrap();

    assert_ne!(bumped.get_discount("USD-OIS").unwrap().df(2.0), df_before);
    assert_ne!(
        bumped.get_forward("USD-LIBOR").unwrap().rate(2.0),
        fwd_before
    );
    assert_ne!(
        bumped.get_hazard("CDX").unwrap().hazard_rate(5.0),
        haz_before
    );
    assert_ne!(
        bumped.get_inflation_curve("USD-CPI").unwrap().cpi_levels()[1],
        cpi_before
    );
    assert_ne!(
        bumped
            .get_base_correlation("CDX-BC")
            .unwrap()
            .correlations()[0],
        bc_before
    );

    // Error path: attempting to apply a key-rate bump to a VolSurface via ctx.bump should fail
    assert!(ctx
        .bump([MarketBump::Curve {
            id: CurveId::from("EQ-VOL"),
            spec: BumpSpec::triangular_key_rate_bp(0.25, 0.5, 1.0, 1.0),
        }])
        .is_err());
}

#[test]
fn curve_storage_from_impls_and_variant_accessors() {
    let disc = sample_discount_curve("USD-OIS");
    let fwd = sample_forward_curve("USD-LIBOR");
    let haz = sample_hazard_curve("CDX");
    let inf = sample_inflation_curve("USD-CPI");
    let bc = sample_base_correlation_curve("CDX-BC");

    let s_disc: CurveStorage = disc.into();
    assert_eq!(s_disc.id().as_str(), "USD-OIS");
    assert!(s_disc.discount().is_some());
    assert!(s_disc.forward().is_none());
    assert!(s_disc.hazard().is_none());
    assert!(s_disc.inflation().is_none());
    assert!(s_disc.base_correlation().is_none());

    let s_fwd: CurveStorage = Arc::new(fwd).into();
    assert_eq!(s_fwd.id().as_str(), "USD-LIBOR");
    assert!(s_fwd.discount().is_none());
    assert!(s_fwd.forward().is_some());

    let s_haz: CurveStorage = haz.into();
    assert_eq!(s_haz.id().as_str(), "CDX");
    assert!(s_haz.hazard().is_some());

    let s_inf: CurveStorage = Arc::new(inf).into();
    assert_eq!(s_inf.id().as_str(), "USD-CPI");
    assert!(s_inf.inflation().is_some());

    let s_bc: CurveStorage = bc.into();
    assert_eq!(s_bc.id().as_str(), "CDX-BC");
    assert!(s_bc.base_correlation().is_some());
}

#[test]
fn curve_storage_arc_from_impls_and_curve_type_coverage() {
    let disc = Arc::new(sample_discount_curve("USD-OIS"));
    let fwd = Arc::new(sample_forward_curve("USD-LIBOR"));
    let haz = Arc::new(sample_hazard_curve("CDX"));
    let inf = Arc::new(sample_inflation_curve("USD-CPI"));
    let bc = Arc::new(sample_base_correlation_curve("CDX-BC"));

    let s_disc: CurveStorage = Arc::clone(&disc).into();
    assert!(Arc::ptr_eq(s_disc.discount().unwrap(), &disc));
    assert_eq!(s_disc.curve_type(), "Discount");

    let s_fwd: CurveStorage = Arc::clone(&fwd).into();
    assert!(Arc::ptr_eq(s_fwd.forward().unwrap(), &fwd));
    assert_eq!(s_fwd.curve_type(), "Forward");

    let s_haz: CurveStorage = Arc::clone(&haz).into();
    assert!(Arc::ptr_eq(s_haz.hazard().unwrap(), &haz));
    assert_eq!(s_haz.curve_type(), "Hazard");

    let s_inf: CurveStorage = Arc::clone(&inf).into();
    assert!(Arc::ptr_eq(s_inf.inflation().unwrap(), &inf));
    assert_eq!(s_inf.curve_type(), "Inflation");

    let s_bc: CurveStorage = Arc::clone(&bc).into();
    assert!(Arc::ptr_eq(s_bc.base_correlation().unwrap(), &bc));
    assert_eq!(s_bc.curve_type(), "BaseCorrelation");
}

#[test]
fn market_context_apply_bumps_additional_branches_and_errors() {
    let date = sample_base_date();

    // FxPct error branch (missing FX)
    let ctx = MarketContext::new().insert(sample_discount_curve("USD-OIS"));
    assert!(ctx
        .bump([MarketBump::FxPct {
            base: Currency::USD,
            quote: Currency::EUR,
            pct: 1.0,
            as_of: date,
        }])
        .is_err());

    // VolBucketPct with no filters ("all buckets") path
    let surface = sample_vol_surface();
    let base_vol = surface.value_checked(0.5, 1.0).unwrap();
    let ctx = MarketContext::new().insert_surface(surface);
    let bumped = ctx
        .bump([MarketBump::VolBucketPct {
            surface_id: CurveId::from("EQ-VOL"),
            expiries: None,
            strikes: None,
            pct: 10.0,
        }])
        .unwrap();
    let bumped_vol = bumped
        .get_surface("EQ-VOL")
        .unwrap()
        .value_checked(0.5, 1.0)
        .unwrap();
    // Canonical VolBucketPct semantics are MULTIPLICATIVE (vol × (1 + pct/100)),
    // matching the filtered path and the scenarios adapter preview
    // (2026-06-09 core quant review, Blocker #3).
    assert!((bumped_vol - base_vol * 1.10).abs() < 1e-12);

    // VolBucketPct missing surface error branch
    let ctx = MarketContext::new();
    assert!(ctx
        .bump([MarketBump::VolBucketPct {
            surface_id: CurveId::from("MISSING-VOL"),
            expiries: Some(vec![0.5]),
            strikes: Some(vec![1.0]),
            pct: 1.0,
        }])
        .is_err());

    // BaseCorrBucketPts "all buckets" (detachments None) path
    let ctx = MarketContext::new().insert(sample_base_correlation_curve("CDX-BC"));
    let before = ctx.get_base_correlation("CDX-BC").unwrap().correlations()[0];
    let bumped = ctx
        .bump([MarketBump::BaseCorrBucketPts {
            surface_id: CurveId::from("CDX-BC"),
            detachments: None,
            points: 0.01,
        }])
        .unwrap();
    let after = bumped
        .get_base_correlation("CDX-BC")
        .unwrap()
        .correlations()[0];
    assert!((after - (before + 0.01)).abs() < 1e-12);

    // BaseCorrBucketPts missing curve error branch
    let ctx = MarketContext::new();
    assert!(ctx
        .bump([MarketBump::BaseCorrBucketPts {
            surface_id: CurveId::from("MISSING-BC"),
            detachments: None,
            points: 0.01,
        }])
        .is_err());
}

#[test]
fn market_context_clone_thread_safe_smoke() {
    // MarketContext is cheap to clone and safe to use across threads when wrapped in Arc.
    let ctx = MarketContext::new().insert(sample_discount_curve("USD-OIS"));
    let cloned = ctx;

    let handle = std::thread::spawn(move || {
        // basic lookup on another thread
        cloned.get_discount("USD-OIS").is_ok()
    });

    assert!(handle.join().expect("thread should join"));
}
