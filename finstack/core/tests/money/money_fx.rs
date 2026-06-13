use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::money::fx::{FxConfig, FxConversionPolicy, FxMatrix, FxProvider, FxQuery};
use finstack_core::money::Money;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

struct StaticFx {
    rate: f64,
}

impl FxProvider for StaticFx {
    fn rate(
        &self,
        _from: Currency,
        _to: Currency,
        _on: Date,
        _policy: FxConversionPolicy,
    ) -> finstack_core::Result<f64> {
        Ok(self.rate)
    }
}

#[test]
fn explicit_convert_and_add() {
    let usd = Money::new(100.0, Currency::USD);
    let eur = Money::new(90.0, Currency::EUR);
    let prov = StaticFx { rate: 1.2 }; // EUR→USD 1.2 for test
    let d = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();

    // Convert EUR to USD, then add
    let eur_in_usd = eur
        .convert(Currency::USD, d, &prov, FxConversionPolicy::CashflowDate)
        .unwrap();
    let sum = usd.checked_add(eur_in_usd).unwrap();
    // Expected: 100 + 90*1.2 = 208
    assert!((sum.amount() - 208.0).abs() < 1e-9);
}

#[test]
fn cross_currency_add_fails_without_convert() {
    let usd = Money::new(10.0, Currency::USD);
    let eur = Money::new(10.0, Currency::EUR);
    assert!(usd.checked_add(eur).is_err());
}

#[test]
fn closure_check_matrix() {
    // Market standard identity: cross rates must satisfy triangular consistency.
    // We force triangulation via USD pivot:
    // USD->EUR = 0.9, USD->GBP = 0.75, GBP->USD = 1/0.75
    // => GBP->EUR = GBP->USD * USD->EUR = 1.2
    struct Prov;
    impl FxProvider for Prov {
        fn rate(
            &self,
            from: Currency,
            to: Currency,
            _on: Date,
            _policy: FxConversionPolicy,
        ) -> finstack_core::Result<f64> {
            match (from, to) {
                (Currency::USD, Currency::EUR) => Ok(0.9),
                (Currency::USD, Currency::GBP) => Ok(0.75),
                (Currency::GBP, Currency::USD) => Ok(1.0 / 0.75),
                _ => Err(finstack_core::InputError::NotFound {
                    id: format!("FX:{from}->{to}"),
                }
                .into()),
            }
        }
    }
    let cfg = FxConfig {
        enable_triangulation: true,
        pivot_currency: Currency::USD,
        ..Default::default()
    };
    let m = FxMatrix::try_with_config(Arc::new(Prov), cfg).expect("valid FxConfig");
    let d = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();

    // Direct quote (not triangulated)
    let usd_eur = m
        .rate(FxQuery::new(Currency::USD, Currency::EUR, d))
        .unwrap();
    assert!(!usd_eur.triangulated);
    assert!((usd_eur.rate - 0.9).abs() < 1e-15);

    // Triangulated cross
    let gbp_eur = m
        .rate(FxQuery::new(Currency::GBP, Currency::EUR, d))
        .unwrap();
    assert!(gbp_eur.triangulated);
    assert!((gbp_eur.rate - 1.2).abs() < 1e-15);

    // Triangular consistency: USD->GBP * GBP->EUR == USD->EUR
    let usd_gbp = m
        .rate(FxQuery::new(Currency::USD, Currency::GBP, d))
        .unwrap();
    assert!(!usd_gbp.triangulated);
    let lhs = usd_gbp.rate * gbp_eur.rate;
    assert!((lhs - usd_eur.rate).abs() < 1e-12);
}

#[test]
fn fx_matrix_cache_distinguishes_query_date_and_policy() {
    struct DatePolicyFx;

    impl FxProvider for DatePolicyFx {
        fn rate(
            &self,
            from: Currency,
            to: Currency,
            on: Date,
            policy: FxConversionPolicy,
        ) -> finstack_core::Result<f64> {
            assert_eq!(from, Currency::EUR);
            assert_eq!(to, Currency::USD);

            let jan_1 = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();
            let jan_2 = Date::from_calendar_date(2025, time::Month::January, 2).unwrap();

            match (on, policy) {
                (d, FxConversionPolicy::CashflowDate) if d == jan_1 => Ok(1.10),
                (d, FxConversionPolicy::CashflowDate) if d == jan_2 => Ok(1.20),
                (d, FxConversionPolicy::PeriodAverage) if d == jan_1 => Ok(1.15),
                (d, FxConversionPolicy::PeriodAverage) if d == jan_2 => Ok(1.25),
                _ => Err(finstack_core::InputError::NotFound {
                    id: format!("FX:{from}->{to}@{on:?}/{policy:?}"),
                }
                .into()),
            }
        }
    }

    let matrix = FxMatrix::try_with_config(
        Arc::new(DatePolicyFx),
        FxConfig {
            enable_triangulation: false,
            ..Default::default()
        },
    )
    .expect("valid FxConfig");
    let jan_1 = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();
    let jan_2 = Date::from_calendar_date(2025, time::Month::January, 2).unwrap();

    let cashflow_jan_1 = matrix
        .rate(FxQuery::new(Currency::EUR, Currency::USD, jan_1))
        .unwrap();
    let cashflow_jan_2 = matrix
        .rate(FxQuery::new(Currency::EUR, Currency::USD, jan_2))
        .unwrap();
    let avg_jan_1 = matrix
        .rate(FxQuery::with_policy(
            Currency::EUR,
            Currency::USD,
            jan_1,
            FxConversionPolicy::PeriodAverage,
        ))
        .unwrap();

    assert!((cashflow_jan_1.rate - 1.10).abs() < 1e-12);
    assert!((cashflow_jan_2.rate - 1.20).abs() < 1e-12);
    assert!((avg_jan_1.rate - 1.15).abs() < 1e-12);
}

#[test]
fn fx_matrix_set_quote_on_overrides_only_the_seeded_date() {
    // Date-aware provider: rate ramps by one cent per day from 2025-01-01.
    struct RampFx;
    impl FxProvider for RampFx {
        fn rate(
            &self,
            _from: Currency,
            _to: Currency,
            on: Date,
            _policy: FxConversionPolicy,
        ) -> finstack_core::Result<f64> {
            let base = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();
            Ok(1.10 + (on - base).whole_days() as f64 * 0.01)
        }
    }

    let matrix = FxMatrix::try_with_config(
        Arc::new(RampFx),
        FxConfig {
            enable_triangulation: false,
            ..Default::default()
        },
    )
    .expect("valid FxConfig");
    let jan_1 = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();
    let jan_2 = Date::from_calendar_date(2025, time::Month::January, 2).unwrap();

    // Seed only Jan 1; unlike set_quote this must NOT shadow the provider for
    // other dates.
    matrix
        .set_quote_on(
            Currency::EUR,
            Currency::USD,
            jan_1,
            FxConversionPolicy::CashflowDate,
            9.99,
        )
        .expect("date-scoped seed");

    let r1 = matrix
        .rate(FxQuery::new(Currency::EUR, Currency::USD, jan_1))
        .unwrap();
    let r2 = matrix
        .rate(FxQuery::new(Currency::EUR, Currency::USD, jan_2))
        .unwrap();

    assert!(
        (r1.rate - 9.99).abs() < 1e-12,
        "seeded date uses the override"
    );
    assert!(
        (r2.rate - 1.11).abs() < 1e-12,
        "other dates still use the date-aware provider, not the override"
    );
}

#[test]
fn fx_matrix_pinned_quote_survives_cache_pressure() {
    // Date-aware provider so a pinned fixing is distinguishable from the
    // provider's answer on the same date.
    struct RampFx;
    impl FxProvider for RampFx {
        fn rate(
            &self,
            _from: Currency,
            _to: Currency,
            on: Date,
            _policy: FxConversionPolicy,
        ) -> finstack_core::Result<f64> {
            let base = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();
            Ok(1.10 + (on - base).whole_days() as f64 * 0.01)
        }
    }

    // Tiny LRU: any pinned fixing sharing the provider-observed cache would be
    // evicted after two unrelated lookups.
    let matrix = FxMatrix::try_with_config(
        Arc::new(RampFx),
        FxConfig {
            enable_triangulation: false,
            cache_capacity: 2,
            ..Default::default()
        },
    )
    .expect("valid FxConfig");

    let jan_1 = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();
    matrix
        .set_quote_on(
            Currency::EUR,
            Currency::USD,
            jan_1,
            FxConversionPolicy::CashflowDate,
            9.99,
        )
        .expect("pin a fixing");

    // Flood the observed cache far past its capacity with other dates.
    for day in 2..=28 {
        let d = Date::from_calendar_date(2025, time::Month::January, day).unwrap();
        let _ = matrix
            .rate(FxQuery::new(Currency::EUR, Currency::USD, d))
            .unwrap();
    }

    // The pinned fixing must still win for its own date — it is not evictable.
    let r = matrix
        .rate(FxQuery::new(Currency::EUR, Currency::USD, jan_1))
        .unwrap();
    assert!(
        (r.rate - 9.99).abs() < 1e-12,
        "pinned fixing must survive cache pressure, got {}",
        r.rate
    );

    // The reciprocal of the pinned fixing is served too.
    let rev = matrix
        .rate(FxQuery::new(Currency::USD, Currency::EUR, jan_1))
        .unwrap();
    assert!(
        (rev.rate - 1.0 / 9.99).abs() < 1e-12,
        "pinned reciprocal served, got {}",
        rev.rate
    );
}

#[test]
fn fx_matrix_explicit_quote_survives_cache_pressure() {
    // Regression: a pair-global `set_quote` (e.g. a pegged currency) must never
    // be evicted under cache pressure. It used to share the bounded provider
    // cache and could be silently dropped past `cache_capacity` distinct pairs,
    // after which the matrix would fall through to the provider and return a
    // *different* rate — a silent mispricing.
    struct RampFx;
    impl FxProvider for RampFx {
        fn rate(
            &self,
            _from: Currency,
            _to: Currency,
            on: Date,
            _policy: FxConversionPolicy,
        ) -> finstack_core::Result<f64> {
            let base = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();
            Ok(1.10 + (on - base).whole_days() as f64 * 0.01)
        }
    }

    // Tiny cache: a pair-global quote on the bounded store would be evicted by a
    // couple of unrelated lookups.
    let matrix = FxMatrix::try_with_config(
        Arc::new(RampFx),
        FxConfig {
            enable_triangulation: false,
            cache_capacity: 2,
            ..Default::default()
        },
    )
    .expect("valid FxConfig");

    // Pin a constant, date-independent peg.
    matrix
        .set_quote(Currency::EUR, Currency::USD, 9.99)
        .expect("pin a pair-global peg");

    // Flood the observed cache far past its capacity with other dates.
    for day in 2..=28 {
        let d = Date::from_calendar_date(2025, time::Month::January, day).unwrap();
        let _ = matrix
            .rate(FxQuery::new(Currency::EUR, Currency::USD, d))
            .unwrap();
    }

    // The peg must still win for every date — it is not evictable.
    let jan_1 = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();
    let r = matrix
        .rate(FxQuery::new(Currency::EUR, Currency::USD, jan_1))
        .unwrap();
    assert!(
        (r.rate - 9.99).abs() < 1e-12,
        "pair-global peg must survive cache pressure, got {}",
        r.rate
    );
}

#[test]
fn fx_matrix_try_with_config_rejects_zero_capacity() {
    let err = FxMatrix::try_with_config(
        Arc::new(StaticFx { rate: 1.0 }),
        FxConfig {
            cache_capacity: 0,
            ..Default::default()
        },
    )
    .err()
    .expect("zero-capacity cache should be rejected by the strict constructor");

    assert!(matches!(err, finstack_core::Error::Validation(_)));
}

#[test]
fn fx_matrix_set_quote_rejects_invalid_rates_without_mutating_state() {
    struct MissingFx;
    impl FxProvider for MissingFx {
        fn rate(
            &self,
            from: Currency,
            to: Currency,
            _on: Date,
            _policy: FxConversionPolicy,
        ) -> finstack_core::Result<f64> {
            Err(finstack_core::InputError::NotFound {
                id: format!("FX:{from}->{to}"),
            }
            .into())
        }
    }

    let matrix = FxMatrix::new(Arc::new(MissingFx));

    let err = matrix
        .set_quote(Currency::GBP, Currency::USD, 0.0)
        .expect_err("non-positive FX rate should be rejected");
    assert!(matches!(err, finstack_core::Error::Input(_)));

    let jan_1 = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();
    let lookup = matrix.rate(FxQuery::new(Currency::GBP, Currency::USD, jan_1));
    assert!(
        lookup.is_err(),
        "rejecting an explicit quote should leave the matrix without that quote"
    );
}

#[test]
fn with_bumped_rate_invalidates_cached_crosses() {
    struct PivotFx;
    impl FxProvider for PivotFx {
        fn rate(
            &self,
            from: Currency,
            to: Currency,
            _on: Date,
            _policy: FxConversionPolicy,
        ) -> finstack_core::Result<f64> {
            match (from, to) {
                (Currency::GBP, Currency::USD) => Ok(1.25),
                (Currency::USD, Currency::EUR) => Ok(0.90),
                _ => Err(finstack_core::InputError::NotFound {
                    id: format!("FX:{from}->{to}"),
                }
                .into()),
            }
        }
    }

    let matrix = FxMatrix::try_with_config(
        Arc::new(PivotFx),
        FxConfig {
            enable_triangulation: true,
            pivot_currency: Currency::USD,
            ..Default::default()
        },
    )
    .expect("valid FxConfig");
    let as_of = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();

    let original_cross = matrix
        .rate(FxQuery::new(Currency::GBP, Currency::EUR, as_of))
        .unwrap()
        .rate;
    let bumped = matrix
        .with_bumped_rate(Currency::USD, Currency::EUR, 0.10, as_of)
        .unwrap();
    let bumped_cross = bumped
        .rate(FxQuery::new(Currency::GBP, Currency::EUR, as_of))
        .unwrap()
        .rate;

    assert!(bumped_cross > original_cross);
}

#[test]
fn validate_triangular_flags_inconsistent_crosses() {
    struct MissingFx;
    impl FxProvider for MissingFx {
        fn rate(
            &self,
            from: Currency,
            to: Currency,
            _on: Date,
            _policy: FxConversionPolicy,
        ) -> finstack_core::Result<f64> {
            Err(finstack_core::InputError::NotFound {
                id: format!("FX:{from}->{to}"),
            }
            .into())
        }
    }

    let matrix = FxMatrix::new(Arc::new(MissingFx));
    matrix
        .set_quotes(&[
            (Currency::EUR, Currency::USD, 1.10),
            (Currency::USD, Currency::GBP, 0.80),
            (Currency::GBP, Currency::EUR, 1.20),
        ])
        .expect("valid quotes");

    let err = matrix
        .validate_triangular(5.0)
        .expect_err("inconsistent triangle should be rejected");
    assert!(matches!(err, finstack_core::Error::Validation(_)));
}

#[test]
fn triangulation_missing_leg_only_queries_provider_once_per_leg() {
    struct CountingMissingFx {
        calls: AtomicUsize,
    }

    impl FxProvider for CountingMissingFx {
        fn rate(
            &self,
            from: Currency,
            to: Currency,
            _on: Date,
            _policy: FxConversionPolicy,
        ) -> finstack_core::Result<f64> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            Err(finstack_core::InputError::NotFound {
                id: format!("FX:{from}->{to}"),
            }
            .into())
        }
    }

    let provider = Arc::new(CountingMissingFx {
        calls: AtomicUsize::new(0),
    });
    let matrix = FxMatrix::try_with_config(
        Arc::<CountingMissingFx>::clone(&provider),
        FxConfig {
            enable_triangulation: true,
            pivot_currency: Currency::USD,
            ..Default::default()
        },
    )
    .expect("valid FxConfig");
    let as_of = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();

    let result = matrix.rate(FxQuery::new(Currency::GBP, Currency::EUR, as_of));
    assert!(
        result.is_err(),
        "missing triangulation legs should still error"
    );
    assert_eq!(
        provider.calls.load(Ordering::Relaxed),
        2,
        "lookup should perform one direct probe and one first-leg probe, without a duplicate retry"
    );
}

/// Provider for triangulation tests: serves only the listed legs, errors on
/// everything else (in particular the EUR→GBP cross itself).
struct LegFx {
    /// `(from, to, rate)` legs the provider can serve.
    legs: Vec<(Currency, Currency, f64)>,
}

impl FxProvider for LegFx {
    fn rate(
        &self,
        from: Currency,
        to: Currency,
        _on: Date,
        _policy: FxConversionPolicy,
    ) -> finstack_core::Result<f64> {
        self.legs
            .iter()
            .find(|(f, t, _)| *f == from && *t == to)
            .map(|(_, _, r)| *r)
            .ok_or_else(|| {
                finstack_core::InputError::NotFound {
                    id: format!("FX:{from}->{to}"),
                }
                .into()
            })
    }
}

#[test]
fn fx_triangulation_honors_pinned_leg() {
    // Provider knows both pivot legs; a pinned EUR→USD fixing must override the
    // provider's leg inside triangulation, exactly as it does for direct
    // lookups — otherwise the cross contradicts the pinned fixing (internal
    // triangular arbitrage on the same date/policy).
    let matrix = FxMatrix::try_with_config(
        Arc::new(LegFx {
            legs: vec![
                (Currency::EUR, Currency::USD, 1.10),
                (Currency::USD, Currency::GBP, 0.80),
            ],
        }),
        FxConfig {
            enable_triangulation: true,
            pivot_currency: Currency::USD,
            ..Default::default()
        },
    )
    .expect("valid FxConfig");
    let jan_1 = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();

    matrix
        .set_quote_on(
            Currency::EUR,
            Currency::USD,
            jan_1,
            FxConversionPolicy::CashflowDate,
            1.25,
        )
        .expect("pin EUR->USD fixing");

    let cross = matrix
        .rate(FxQuery::new(Currency::EUR, Currency::GBP, jan_1))
        .unwrap();

    let pinned_product = 1.25 * 0.80;
    let provider_product = 1.10 * 0.80;
    assert!(
        (cross.rate - pinned_product).abs() < 1e-12,
        "cross must use the pinned leg: expected {pinned_product}, got {}",
        cross.rate
    );
    assert!(
        (cross.rate - provider_product).abs() > 1e-6,
        "cross must not silently use the provider leg over the pinned fixing"
    );
    assert!(cross.triangulated, "cross is derived via the pivot");

    // Direct lookup of the pinned leg agrees with the leg used in the cross.
    let leg = matrix
        .rate(FxQuery::new(Currency::EUR, Currency::USD, jan_1))
        .unwrap();
    assert!((cross.rate - leg.rate * 0.80).abs() < 1e-12);
}

#[test]
fn fx_triangulation_succeeds_when_leg_exists_only_as_pinned_quote() {
    // Provider has no EUR→USD leg at all; the pinned fixing must be enough for
    // triangulation to succeed.
    let matrix = FxMatrix::try_with_config(
        Arc::new(LegFx {
            legs: vec![(Currency::USD, Currency::GBP, 0.80)],
        }),
        FxConfig {
            enable_triangulation: true,
            pivot_currency: Currency::USD,
            ..Default::default()
        },
    )
    .expect("valid FxConfig");
    let jan_1 = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();

    matrix
        .set_quote_on(
            Currency::EUR,
            Currency::USD,
            jan_1,
            FxConversionPolicy::CashflowDate,
            1.25,
        )
        .expect("pin EUR->USD fixing");

    let cross = matrix
        .rate(FxQuery::new(Currency::EUR, Currency::GBP, jan_1))
        .unwrap();
    assert!(
        (cross.rate - 1.25 * 0.80).abs() < 1e-12,
        "triangulation must succeed via the pinned leg, got {}",
        cross.rate
    );
    assert!(cross.triangulated);
}

#[test]
fn fx_triangulated_flag_is_stable_across_repeat_queries() {
    // Regression: the first lookup of a missing cross returned
    // `triangulated: true` and cached the derived rate; the second identical
    // query hit the observed cache and flipped to `triangulated: false`.
    // Stamped metadata must not depend on call history.
    let matrix = FxMatrix::try_with_config(
        Arc::new(LegFx {
            legs: vec![
                (Currency::EUR, Currency::USD, 1.10),
                (Currency::USD, Currency::GBP, 0.80),
            ],
        }),
        FxConfig {
            enable_triangulation: true,
            pivot_currency: Currency::USD,
            ..Default::default()
        },
    )
    .expect("valid FxConfig");
    let jan_1 = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();
    let query = FxQuery::new(Currency::EUR, Currency::GBP, jan_1);

    let first = matrix.rate(query).unwrap();
    let second = matrix.rate(query).unwrap();

    assert!(first.triangulated, "first lookup is derived via the pivot");
    assert!(
        second.triangulated,
        "repeat lookup must stamp the same provenance as the first"
    );
    assert!((first.rate - second.rate).abs() < 1e-15);
    assert!((first.rate - 1.10 * 0.80).abs() < 1e-12);
}

#[test]
fn with_bumped_rate_preserves_fx_term_structure() {
    // `with_bumped_rate` previously froze one
    // absolute rate for every date, flattening a date-aware provider's term
    // structure. The bump must be relative and per-date.
    struct DateAwareFx;
    impl FxProvider for DateAwareFx {
        fn rate(
            &self,
            _from: Currency,
            _to: Currency,
            on: Date,
            _policy: FxConversionPolicy,
        ) -> finstack_core::Result<f64> {
            if on == Date::from_calendar_date(2025, time::Month::January, 1).unwrap() {
                Ok(1.10)
            } else {
                Ok(1.20)
            }
        }
    }

    let matrix = FxMatrix::new(Arc::new(DateAwareFx));
    let d1 = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();
    let d2 = Date::from_calendar_date(2025, time::Month::June, 1).unwrap();

    let bumped = matrix
        .with_bumped_rate(Currency::EUR, Currency::USD, 0.01, d1)
        .expect("valid bump");

    let r1 = bumped
        .rate(FxQuery::new(Currency::EUR, Currency::USD, d1))
        .unwrap()
        .rate;
    let r2 = bumped
        .rate(FxQuery::new(Currency::EUR, Currency::USD, d2))
        .unwrap()
        .rate;

    assert!((r1 - 1.10 * 1.01).abs() < 1e-12, "d1 bumped 1%, got {r1}");
    assert!((r2 - 1.20 * 1.01).abs() < 1e-12, "d2 bumped 1%, got {r2}");
    assert!(
        (r1 - r2).abs() > 1e-6,
        "bump must not flatten the FX term structure"
    );
}

#[test]
fn with_bumped_rate_rejects_invalid_bumps() {
    let matrix = FxMatrix::new(Arc::new(StaticFx { rate: 1.1 }));
    let d = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();
    for bad in [f64::NAN, f64::INFINITY, -1.0, -2.0] {
        assert!(
            matrix
                .with_bumped_rate(Currency::EUR, Currency::USD, bad, d)
                .is_err(),
            "bump_pct {bad} must be rejected"
        );
    }
}

#[test]
fn set_quotes_is_atomic_on_invalid_entry() {
    let matrix = FxMatrix::new(Arc::new(StaticFx { rate: 1.10 }));
    let d = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();

    let err = matrix.set_quotes(&[
        (Currency::EUR, Currency::USD, 1.25),
        (Currency::GBP, Currency::USD, 0.0),
    ]);
    assert!(err.is_err(), "invalid batch quote should fail");

    let rate = matrix
        .rate(FxQuery::new(Currency::EUR, Currency::USD, d))
        .unwrap()
        .rate;
    assert!(
        (rate - 1.10).abs() < 1e-12,
        "failed set_quotes batch must not insert earlier valid entries"
    );
}

#[test]
fn pinned_quote_outranks_pair_global_reciprocal() {
    let matrix = FxMatrix::new(Arc::new(StaticFx { rate: 1.10 }));
    let d = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();

    matrix
        .set_quote(Currency::USD, Currency::EUR, 0.80)
        .expect("valid reciprocal global quote");
    matrix
        .set_quote_on(
            Currency::EUR,
            Currency::USD,
            d,
            FxConversionPolicy::CashflowDate,
            1.30,
        )
        .expect("valid pinned quote");

    let rate = matrix
        .rate(FxQuery::new(Currency::EUR, Currency::USD, d))
        .unwrap()
        .rate;

    assert!(
        (rate - 1.30).abs() < 1e-12,
        "pinned fixing should win over an opposite-direction pair-global quote"
    );
}

#[test]
fn fx_matrix_state_round_trips_pinned_quotes() {
    // persistence previously dropped pinned
    // (date/policy-scoped) quotes. After snapshot + restore, a pinned fixing
    // must still win over the provider for its (on, policy).
    let matrix = FxMatrix::new(Arc::new(StaticFx { rate: 1.10 }));
    let fixing_date = Date::from_calendar_date(2025, time::Month::March, 14).unwrap();
    matrix
        .set_quote_on(
            Currency::EUR,
            Currency::USD,
            fixing_date,
            FxConversionPolicy::CashflowDate,
            1.2345,
        )
        .expect("valid pinned fixing");
    matrix
        .set_quote(Currency::GBP, Currency::USD, 1.25)
        .expect("valid explicit quote");

    let state = matrix.get_serializable_state();
    assert_eq!(state.pinned_quotes.len(), 1);

    // Serde round-trip too (the state is the persistence format).
    let json = serde_json::to_string(&state).unwrap();
    let state: finstack_core::money::fx::FxMatrixState = serde_json::from_str(&json).unwrap();

    let restored = FxMatrix::new(Arc::new(StaticFx { rate: 1.10 }));
    restored.load_from_state(&state).expect("restore");

    let pinned = restored
        .rate(FxQuery::new(Currency::EUR, Currency::USD, fixing_date))
        .unwrap()
        .rate;
    assert!(
        (pinned - 1.2345).abs() < 1e-12,
        "restored pinned fixing must win over the provider, got {pinned}"
    );

    // Other dates still come from the provider.
    let other = Date::from_calendar_date(2025, time::Month::March, 17).unwrap();
    let provider_rate = restored
        .rate(FxQuery::new(Currency::EUR, Currency::USD, other))
        .unwrap()
        .rate;
    assert!((provider_rate - 1.10).abs() < 1e-12);

    // Older payloads without the new field still deserialize (serde-additive).
    let legacy = r#"{"config":{"pivot_currency":"USD","enable_triangulation":true,"cache_capacity":256},"quotes":[]}"#;
    let legacy_state: finstack_core::money::fx::FxMatrixState =
        serde_json::from_str(legacy).unwrap();
    assert!(legacy_state.pinned_quotes.is_empty());
}

#[test]
fn reciprocal_of_subnormal_rate_is_rejected() {
    // a pinned 1e-320 passed input checks but
    // its reciprocal overflowed to +inf. The reciprocal OUTPUT must be
    // validated (finite, positive).
    struct MissingFx;
    impl FxProvider for MissingFx {
        fn rate(
            &self,
            from: Currency,
            to: Currency,
            _on: Date,
            _policy: FxConversionPolicy,
        ) -> finstack_core::Result<f64> {
            Err(finstack_core::InputError::NotFound {
                id: format!("FX:{from}->{to}"),
            }
            .into())
        }
    }

    let matrix = FxMatrix::try_with_config(
        Arc::new(MissingFx),
        FxConfig {
            enable_triangulation: false,
            ..Default::default()
        },
    )
    .expect("valid FxConfig");
    matrix
        .set_quote(Currency::EUR, Currency::USD, 1e-320)
        .expect("subnormal but positive quote is accepted at insert");
    let d = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();

    // Reverse-direction lookup goes through the reciprocal: 1/1e-320 = +inf,
    // which must now be rejected rather than served.
    let result = matrix.rate(FxQuery::new(Currency::USD, Currency::EUR, d));
    assert!(
        result.is_err(),
        "infinite reciprocal must be rejected, got {result:?}"
    );
}
