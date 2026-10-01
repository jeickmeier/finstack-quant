//! Tests for the surrounding crate component and its documented behavior.
//!
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::fx::{
    FxConfig, FxConversionPolicy, FxMatrix, FxProvider, FxQuery, SimpleFxProvider,
};
use finstack_quant_core::money::Money;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

struct StaticFx {
    rate: f64,
}

impl FxProvider for StaticFx {
    fn get_revision(&self) -> Option<u64> {
        Some(0)
    }

    fn rate(
        &self,
        _from: Currency,
        _to: Currency,
        _on: Date,
        _policy: FxConversionPolicy,
    ) -> finstack_quant_core::Result<f64> {
        Ok(self.rate)
    }
}

struct ReciprocalFx {
    eur_usd: f64,
}

#[test]
fn provider_cache_hits_refresh_recency_and_evict_the_least_recent_quote() {
    struct CountingFx(AtomicUsize);
    impl FxProvider for CountingFx {
        fn get_revision(&self) -> Option<u64> {
            Some(0)
        }

        fn rate(
            &self,
            _: Currency,
            _: Currency,
            _: Date,
            _: FxConversionPolicy,
        ) -> finstack_quant_core::Result<f64> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Ok(1.2)
        }
    }

    let provider = Arc::new(CountingFx(AtomicUsize::new(0)));
    let matrix = FxMatrix::try_with_config(
        Arc::clone(&provider) as Arc<dyn FxProvider>,
        FxConfig {
            cache_capacity: 2,
            ..Default::default()
        },
    )
    .expect("valid capacity");
    let query = |day| {
        FxQuery::new(
            Currency::EUR,
            Currency::USD,
            Date::from_calendar_date(2025, time::Month::January, day).expect("date"),
        )
    };
    for day in [1, 2, 1, 3, 1] {
        matrix.rate(query(day)).expect("provider rate");
    }
    assert_eq!(provider.0.load(Ordering::Relaxed), 3);
    assert_eq!(matrix.cache_stats(), 2);
    matrix
        .rate(query(2))
        .expect("evicted quote is fetched again");
    assert_eq!(provider.0.load(Ordering::Relaxed), 4);
    assert_eq!(matrix.cache_stats(), 2);
}

#[test]
fn provider_cache_allocates_for_observations_instead_of_the_capacity_limit() {
    let matrix = FxMatrix::try_with_config(
        Arc::new(StaticFx { rate: 1.2 }),
        FxConfig {
            cache_capacity: usize::MAX,
            ..Default::default()
        },
    )
    .expect("nonzero capacity is valid");
    assert_eq!(matrix.cache_stats(), 0);
    let date = Date::from_calendar_date(2025, time::Month::January, 1).expect("date");
    assert_eq!(
        matrix
            .rate(FxQuery::new(Currency::EUR, Currency::USD, date))
            .expect("provider rate")
            .rate,
        1.2
    );
    assert_eq!(matrix.cache_stats(), 1);
}

#[test]
fn mutable_provider_refreshes_warmed_base_and_bumped_context_snapshots() {
    use finstack_quant_core::market_data::context::MarketContext;

    let on = Date::from_ordinal_date(2025, 1).expect("date");
    let provider = Arc::new(SimpleFxProvider::new());
    provider
        .set_quote(Currency::EUR, Currency::USD, 1.1)
        .expect("initial rate");
    let matrix = FxMatrix::new(Arc::clone(&provider) as Arc<dyn FxProvider>);
    let query = FxQuery::new(Currency::EUR, Currency::USD, on);
    assert_eq!(matrix.rate(query).expect("warm base").rate, 1.1);
    let bumped = matrix
        .with_bumped_rate(Currency::EUR, Currency::USD, 0.01, on)
        .expect("one percent bump");
    assert!((bumped.rate(query).expect("warm bumped").rate - 1.111).abs() < 1e-12);

    provider
        .set_quote(Currency::EUR, Currency::USD, 1.2)
        .expect("updated rate");
    for (matrix, expected) in [(matrix, 1.2), (bumped, 1.212)] {
        let context = MarketContext::new().insert_fx(matrix);
        // Snapshot before another lookup: persistence and a warmed live matrix
        // must describe the same updated market state.
        let json = serde_json::to_string(&context).expect("snapshot");
        let restored: MarketContext = serde_json::from_str(&json).expect("restore");
        for context in [&context, &restored] {
            let fx = context.fx().expect("FX matrix");
            assert!((fx.rate(query).expect("updated direct").rate - expected).abs() < 1e-12);
            let inverse = fx
                .rate(FxQuery::new(Currency::USD, Currency::EUR, on))
                .expect("updated reciprocal");
            assert!((inverse.rate - 1.0 / expected).abs() < 1e-12);
        }
    }
}

#[test]
fn mutable_provider_refresh_preserves_global_and_scoped_overrides_and_bumps() {
    let on = Date::from_ordinal_date(2025, 1).expect("date");
    let later = Date::from_ordinal_date(2025, 2).expect("later date");
    let provider = Arc::new(SimpleFxProvider::new());
    provider
        .set_quotes(&[
            (Currency::EUR, Currency::USD, 1.1),
            (Currency::GBP, Currency::USD, 1.3),
        ])
        .expect("initial rates");
    let matrix = FxMatrix::new(Arc::clone(&provider) as Arc<dyn FxProvider>);
    matrix
        .set_quote(Currency::GBP, Currency::USD, 1.4)
        .expect("global override");
    matrix
        .set_quote_on(
            Currency::EUR,
            Currency::USD,
            on,
            FxConversionPolicy::CashflowDate,
            1.5,
        )
        .expect("scoped override");
    let eur_bumped = matrix
        .with_bumped_rate(Currency::EUR, Currency::USD, 0.01, on)
        .expect("bump scoped pair");
    let gbp_bumped = matrix
        .with_bumped_rate(Currency::GBP, Currency::USD, 0.01, on)
        .expect("bump global pair");
    let live_query = FxQuery::new(Currency::EUR, Currency::USD, later);
    for matrix in [&matrix, &eur_bumped, &gbp_bumped] {
        matrix.rate(live_query).expect("warm provider observation");
    }
    provider
        .set_quotes(&[
            (Currency::EUR, Currency::USD, 1.2),
            (Currency::GBP, Currency::USD, 1.6),
        ])
        .expect("publish batch");

    for (matrix, eur_factor, gbp_factor) in [
        (&matrix, 1.0, 1.0),
        (&eur_bumped, 1.01, 1.0),
        (&gbp_bumped, 1.0, 1.01),
    ] {
        assert!(
            (matrix.rate(live_query).expect("updated provider").rate - 1.2 * eur_factor).abs()
                < 1e-12
        );
        for date in [on, later] {
            for policy in [
                FxConversionPolicy::CashflowDate,
                FxConversionPolicy::PeriodAverage,
            ] {
                let eur_expected = if date == on && policy == FxConversionPolicy::CashflowDate {
                    1.5
                } else {
                    1.2
                } * eur_factor;
                for (from, expected) in [
                    (Currency::EUR, eur_expected),
                    (Currency::GBP, 1.4 * gbp_factor),
                ] {
                    let direct = matrix
                        .rate(FxQuery::with_policy(from, Currency::USD, date, policy))
                        .expect("preserved authority");
                    let reciprocal = matrix
                        .rate(FxQuery::with_policy(Currency::USD, from, date, policy))
                        .expect("preserved reciprocal");
                    assert!((direct.rate - expected).abs() < 1e-12);
                    assert!((reciprocal.rate - 1.0 / expected).abs() < 1e-12);
                }
            }
        }
    }
}

#[test]
fn mutable_provider_refreshes_triangulated_crosses_and_preexisting_cross_bumps() {
    let on = Date::from_ordinal_date(2025, 1).expect("date");
    let provider = Arc::new(SimpleFxProvider::new());
    provider
        .set_quotes(&[
            (Currency::EUR, Currency::USD, 1.1),
            (Currency::USD, Currency::GBP, 0.8),
        ])
        .expect("initial pivot legs");
    let matrix = FxMatrix::new(Arc::clone(&provider) as Arc<dyn FxProvider>);
    let bumped = matrix
        .with_bumped_rate(Currency::EUR, Currency::GBP, 0.01, on)
        .expect("cross bump");
    for matrix in [&matrix, &bumped] {
        for (from, to) in [
            (Currency::EUR, Currency::GBP),
            (Currency::GBP, Currency::EUR),
            (Currency::USD, Currency::EUR),
        ] {
            matrix
                .rate(FxQuery::new(from, to, on))
                .expect("warm cross and reciprocal");
        }
    }
    provider
        .set_quote(Currency::EUR, Currency::USD, 1.2)
        .expect("update first pivot leg");
    for (matrix, factor) in [(&matrix, 1.0), (&bumped, 1.01)] {
        let expected = 1.2 * 0.8 * factor;
        let cross = matrix
            .rate(FxQuery::new(Currency::EUR, Currency::GBP, on))
            .expect("updated cross");
        let inverse = matrix
            .rate(FxQuery::new(Currency::GBP, Currency::EUR, on))
            .expect("updated inverse cross");
        assert!((cross.rate - expected).abs() < 1e-12);
        assert!((inverse.rate - 1.0 / expected).abs() < 1e-12);
        assert!(
            (matrix
                .rate(FxQuery::new(Currency::USD, Currency::EUR, on))
                .expect("updated inverse leg")
                .rate
                - 1.0 / 1.2)
                .abs()
                < 1e-12
        );
    }
}

#[test]
fn providers_without_revisions_do_not_cache_observations() {
    struct LiveFx(AtomicUsize);
    impl FxProvider for LiveFx {
        fn rate(
            &self,
            _: Currency,
            _: Currency,
            _: Date,
            _: FxConversionPolicy,
        ) -> finstack_quant_core::Result<f64> {
            Ok(self.0.load(Ordering::Relaxed) as f64 / 100.0)
        }
    }

    let on = Date::from_ordinal_date(2025, 1).expect("date");
    let provider = Arc::new(LiveFx(AtomicUsize::new(110)));
    assert_eq!(provider.get_revision(), None);
    let matrix = FxMatrix::new(Arc::clone(&provider) as Arc<dyn FxProvider>);
    let query = FxQuery::new(Currency::EUR, Currency::USD, on);
    assert_eq!(matrix.rate(query).expect("initial live rate").rate, 1.1);
    let bumped = matrix
        .with_bumped_rate(Currency::EUR, Currency::USD, 0.01, on)
        .expect("live provider bump");
    assert!((bumped.rate(query).expect("initial bumped rate").rate - 1.111).abs() < 1e-12);
    provider.0.store(120, Ordering::Relaxed);
    assert_eq!(matrix.rate(query).expect("updated live rate").rate, 1.2);
    assert!((bumped.rate(query).expect("updated bumped rate").rate - 1.212).abs() < 1e-12);
    assert_eq!(matrix.cache_stats(), 0);
    assert_eq!(bumped.cache_stats(), 0);
}

#[test]
fn provider_update_between_cross_legs_retries_the_complete_rate() {
    struct PublishBetweenLegs(AtomicUsize);
    impl FxProvider for PublishBetweenLegs {
        fn get_revision(&self) -> Option<u64> {
            Some(self.0.load(Ordering::SeqCst) as u64)
        }

        fn rate(
            &self,
            from: Currency,
            to: Currency,
            _: Date,
            _: FxConversionPolicy,
        ) -> finstack_quant_core::Result<f64> {
            let revision = self.0.load(Ordering::SeqCst);
            match (from, to) {
                (Currency::EUR, Currency::USD) => {
                    self.0.store(1, Ordering::SeqCst);
                    Ok(if revision == 0 { 1.1 } else { 1.2 })
                }
                (Currency::USD, Currency::GBP) => Ok(if revision == 0 { 0.8 } else { 0.9 }),
                _ => Err(finstack_quant_core::InputError::NotFound {
                    id: format!("FX:{from}->{to}"),
                }
                .into()),
            }
        }
    }

    let matrix = FxMatrix::new(Arc::new(PublishBetweenLegs(AtomicUsize::new(0))));
    let on = Date::from_ordinal_date(2025, 1).expect("date");
    let query = FxQuery::new(Currency::EUR, Currency::GBP, on);
    // Old market: 1.1 * 0.8 = 0.88. New market: 1.2 * 0.9 = 1.08.
    // The interleaved 1.1 * 0.9 = 0.99 must never escape as a valid cross.
    for _ in 0..2 {
        assert!((matrix.rate(query).expect("coherent cross").rate - 1.08).abs() < 1e-12);
    }
}

#[test]
fn provider_update_between_snapshot_hooks_retries_the_complete_snapshot() {
    struct PublishBetweenHooks(AtomicUsize);
    impl FxProvider for PublishBetweenHooks {
        fn get_revision(&self) -> Option<u64> {
            Some(self.0.load(Ordering::SeqCst) as u64)
        }

        fn rate(
            &self,
            _: Currency,
            _: Currency,
            _: Date,
            _: FxConversionPolicy,
        ) -> finstack_quant_core::Result<f64> {
            Ok(1.2)
        }

        fn snapshot_quotes(&self) -> Vec<(Currency, Currency, f64)> {
            let prior = self.0.swap(1, Ordering::SeqCst);
            vec![(
                Currency::EUR,
                Currency::USD,
                if prior == 0 { 1.1 } else { 1.2 },
            )]
        }

        fn snapshot_pinned_quotes(
            &self,
        ) -> Vec<(Currency, Currency, Date, FxConversionPolicy, f64)> {
            vec![(
                Currency::EUR,
                Currency::USD,
                Date::from_ordinal_date(2025, 1).expect("date"),
                FxConversionPolicy::CashflowDate,
                if self.0.load(Ordering::SeqCst) == 0 {
                    1.3
                } else {
                    1.4
                },
            )]
        }
    }

    let matrix = FxMatrix::new(Arc::new(PublishBetweenHooks(AtomicUsize::new(0))));
    let state = matrix.get_serializable_state().expect("coherent snapshot");
    assert_eq!(
        state.provider_quotes,
        vec![(Currency::EUR, Currency::USD, 1.2)]
    );
    assert_eq!(state.provider_pinned_quotes[0].4, 1.4);
}

#[test]
fn repeated_provider_updates_fail_rate_and_snapshot_after_bounded_retries() {
    struct ChangingFx(AtomicUsize);
    impl FxProvider for ChangingFx {
        fn get_revision(&self) -> Option<u64> {
            Some(self.0.load(Ordering::SeqCst) as u64)
        }

        fn rate(
            &self,
            _: Currency,
            _: Currency,
            _: Date,
            _: FxConversionPolicy,
        ) -> finstack_quant_core::Result<f64> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(1.2)
        }

        fn snapshot_quotes(&self) -> Vec<(Currency, Currency, f64)> {
            self.0.fetch_add(1, Ordering::SeqCst);
            vec![(Currency::EUR, Currency::USD, 1.2)]
        }
    }

    let provider = Arc::new(ChangingFx(AtomicUsize::new(0)));
    let matrix = FxMatrix::new(Arc::clone(&provider) as Arc<dyn FxProvider>);
    let on = Date::from_ordinal_date(2025, 1).expect("date");
    assert!(matrix
        .rate(FxQuery::new(Currency::EUR, Currency::USD, on))
        .is_err());
    assert_eq!(provider.0.load(Ordering::SeqCst), 3);
    assert!(matrix.get_serializable_state().is_err());
    assert_eq!(provider.0.load(Ordering::SeqCst), 6);
}

#[test]
fn unversioned_provider_snapshot_quotes_are_rejected() {
    struct UnversionedSnapshot;
    impl FxProvider for UnversionedSnapshot {
        fn rate(
            &self,
            _: Currency,
            _: Currency,
            _: Date,
            _: FxConversionPolicy,
        ) -> finstack_quant_core::Result<f64> {
            Ok(1.2)
        }

        fn snapshot_quotes(&self) -> Vec<(Currency, Currency, f64)> {
            vec![(Currency::EUR, Currency::USD, 1.2)]
        }
    }
    let matrix = FxMatrix::new(Arc::new(UnversionedSnapshot));
    assert!(matrix.get_serializable_state().is_err());
}

impl FxProvider for ReciprocalFx {
    fn get_revision(&self) -> Option<u64> {
        Some(0)
    }

    fn rate(
        &self,
        from: Currency,
        to: Currency,
        _on: Date,
        _policy: FxConversionPolicy,
    ) -> finstack_quant_core::Result<f64> {
        match (from, to) {
            (Currency::EUR, Currency::USD) => Ok(self.eur_usd),
            (Currency::USD, Currency::EUR) => Ok(self.eur_usd.recip()),
            _ => Err(finstack_quant_core::InputError::NotFound {
                id: format!("FX:{from}->{to}"),
            }
            .into()),
        }
    }
}

#[test]
fn explicit_convert_and_add() {
    let usd = Money::new(100.0, Currency::USD).expect("valid money fixture");
    let eur = Money::new(90.0, Currency::EUR).expect("valid money fixture");
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
        ) -> finstack_quant_core::Result<f64> {
            match (from, to) {
                (Currency::USD, Currency::EUR) => Ok(0.9),
                (Currency::USD, Currency::GBP) => Ok(0.75),
                (Currency::GBP, Currency::USD) => Ok(1.0 / 0.75),
                _ => Err(finstack_quant_core::InputError::NotFound {
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
fn fx_quote_updates_invalidate_cached_crosses() {
    let date = Date::from_calendar_date(2025, time::Month::January, 2).unwrap();
    let other_date = Date::from_calendar_date(2025, time::Month::January, 3).unwrap();
    let policy = FxConversionPolicy::CashflowDate;
    for update in ["single", "batch", "pinned"] {
        let matrix = FxMatrix::new(Arc::new(SimpleFxProvider::new()));
        matrix
            .set_quote(Currency::GBP, Currency::USD, 1.25)
            .unwrap();
        if update == "pinned" {
            for on in [date, other_date] {
                matrix
                    .set_quote_on(Currency::EUR, Currency::USD, on, policy, 1.10)
                    .unwrap();
            }
        } else {
            matrix
                .set_quote(Currency::EUR, Currency::USD, 1.10)
                .unwrap();
        }
        for on in [date, other_date] {
            let before = matrix
                .rate(FxQuery::new(Currency::EUR, Currency::GBP, on))
                .unwrap();
            assert!((before.rate - 0.88).abs() < 1e-12);
        }
        match update {
            "single" => matrix
                .set_quote(Currency::EUR, Currency::USD, 1.20)
                .unwrap(),
            "batch" => matrix
                .set_quotes(&[(Currency::EUR, Currency::USD, 1.20)])
                .unwrap(),
            _ => matrix
                .set_quote_on(Currency::EUR, Currency::USD, date, policy, 1.20)
                .unwrap(),
        }
        for (from, to, expected) in [
            (Currency::EUR, Currency::GBP, 0.96),
            (Currency::GBP, Currency::EUR, 1.0 / 0.96),
        ] {
            let after = matrix.rate(FxQuery::new(from, to, date)).unwrap();
            assert!((after.rate - expected).abs() < 1e-12, "{update}: {after:?}");
            assert!(after.triangulated);
        }
        let other = matrix
            .rate(FxQuery::new(Currency::EUR, Currency::GBP, other_date))
            .unwrap();
        let expected = if update == "pinned" { 0.88 } else { 0.96 };
        assert!((other.rate - expected).abs() < 1e-12);
        assert_eq!(
            matrix
                .rate(FxQuery::new(Currency::GBP, Currency::USD, date))
                .unwrap()
                .rate,
            1.25
        );
    }
}

#[test]
fn fx_quote_update_prevents_inflight_cross_from_repopulating_cache() {
    use std::sync::Barrier;

    struct BlockingFx {
        entered: Arc<Barrier>,
        resume: Arc<Barrier>,
        calls: AtomicUsize,
    }

    impl FxProvider for BlockingFx {
        fn get_revision(&self) -> Option<u64> {
            Some(0)
        }

        fn rate(
            &self,
            from: Currency,
            to: Currency,
            _: Date,
            _: FxConversionPolicy,
        ) -> finstack_quant_core::Result<f64> {
            if (from, to) != (Currency::USD, Currency::GBP) {
                return Err(finstack_quant_core::InputError::NotFound {
                    id: format!("FX:{from}->{to}"),
                }
                .into());
            }
            if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
                self.entered.wait();
                self.resume.wait();
            }
            Ok(0.8)
        }
    }

    let entered = Arc::new(Barrier::new(2));
    let resume = Arc::new(Barrier::new(2));
    let matrix = Arc::new(FxMatrix::new(Arc::new(BlockingFx {
        entered: Arc::clone(&entered),
        resume: Arc::clone(&resume),
        calls: AtomicUsize::new(0),
    })));
    let date = Date::from_calendar_date(2025, time::Month::January, 2).unwrap();
    let query = FxQuery::new(Currency::EUR, Currency::GBP, date);
    matrix
        .set_quote(Currency::EUR, Currency::USD, 1.10)
        .unwrap();
    let reader = Arc::clone(&matrix);
    let task = std::thread::spawn(move || reader.rate(query).unwrap());
    entered.wait();
    matrix
        .set_quote(Currency::EUR, Currency::USD, 1.20)
        .unwrap();
    resume.wait();
    task.join().unwrap();
    let after = matrix.rate(query).unwrap();
    assert!((after.rate - 0.96).abs() < 1e-12, "{after:?}");
}

#[test]
fn fx_matrix_cache_distinguishes_query_date_and_policy() {
    struct DatePolicyFx;

    impl FxProvider for DatePolicyFx {
        fn get_revision(&self) -> Option<u64> {
            Some(0)
        }

        fn rate(
            &self,
            from: Currency,
            to: Currency,
            on: Date,
            policy: FxConversionPolicy,
        ) -> finstack_quant_core::Result<f64> {
            assert_eq!(from, Currency::EUR);
            assert_eq!(to, Currency::USD);

            let jan_1 = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();
            let jan_2 = Date::from_calendar_date(2025, time::Month::January, 2).unwrap();

            match (on, policy) {
                (d, FxConversionPolicy::CashflowDate) if d == jan_1 => Ok(1.10),
                (d, FxConversionPolicy::CashflowDate) if d == jan_2 => Ok(1.20),
                (d, FxConversionPolicy::PeriodAverage) if d == jan_1 => Ok(1.15),
                (d, FxConversionPolicy::PeriodAverage) if d == jan_2 => Ok(1.25),
                _ => Err(finstack_quant_core::InputError::NotFound {
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
        ) -> finstack_quant_core::Result<f64> {
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
        fn get_revision(&self) -> Option<u64> {
            Some(0)
        }

        fn rate(
            &self,
            _from: Currency,
            _to: Currency,
            on: Date,
            _policy: FxConversionPolicy,
        ) -> finstack_quant_core::Result<f64> {
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
        fn get_revision(&self) -> Option<u64> {
            Some(0)
        }

        fn rate(
            &self,
            _from: Currency,
            _to: Currency,
            on: Date,
            _policy: FxConversionPolicy,
        ) -> finstack_quant_core::Result<f64> {
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

    assert!(matches!(err, finstack_quant_core::Error::Validation(_)));
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
        ) -> finstack_quant_core::Result<f64> {
            Err(finstack_quant_core::InputError::NotFound {
                id: format!("FX:{from}->{to}"),
            }
            .into())
        }
    }

    let matrix = FxMatrix::new(Arc::new(MissingFx));

    let err = matrix
        .set_quote(Currency::GBP, Currency::USD, 0.0)
        .expect_err("non-positive FX rate should be rejected");
    assert!(matches!(err, finstack_quant_core::Error::Input(_)));

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
        fn get_revision(&self) -> Option<u64> {
            Some(0)
        }

        fn rate(
            &self,
            from: Currency,
            to: Currency,
            _on: Date,
            _policy: FxConversionPolicy,
        ) -> finstack_quant_core::Result<f64> {
            match (from, to) {
                (Currency::GBP, Currency::USD) => Ok(1.25),
                (Currency::USD, Currency::EUR) => Ok(0.90),
                _ => Err(finstack_quant_core::InputError::NotFound {
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
fn bumped_quote_providers_survive_context_snapshots_in_both_directions_and_crosses() {
    use finstack_quant_core::market_data::context::MarketContext;

    let on = Date::from_ordinal_date(2025, 1).expect("date");
    for (from, to) in [
        (Currency::EUR, Currency::USD),
        (Currency::USD, Currency::EUR),
        (Currency::EUR, Currency::GBP),
    ] {
        let provider = SimpleFxProvider::new();
        provider
            .set_quotes(&[
                (Currency::EUR, Currency::USD, 1.1),
                (Currency::GBP, Currency::USD, 1.3),
            ])
            .expect("snapshot quotes");
        let matrix = FxMatrix::new(Arc::new(provider));
        let bumped = matrix
            .with_bumped_rate(from, to, 0.1, on)
            .expect("FX shock");
        let expected = bumped.rate(FxQuery::new(from, to, on)).expect("rate").rate;
        let context = MarketContext::new().insert_fx(bumped);
        let json = serde_json::to_string(&context).expect("context snapshot");
        let restored: MarketContext = serde_json::from_str(&json).expect("restore context");
        let fx = restored.fx_required().expect("restored FX");
        for date in [on, Date::from_ordinal_date(2025, 182).expect("later date")] {
            assert!(
                (fx.rate(FxQuery::new(from, to, date)).expect("rate").rate - expected).abs()
                    < 1e-12
            );
            assert!(
                (fx.rate(FxQuery::new(to, from, date)).expect("inverse").rate - 1.0 / expected)
                    .abs()
                    < 1e-12
            );
        }
        assert!(
            (fx.rate(FxQuery::new(Currency::GBP, Currency::USD, on))
                .expect("untouched pair")
                .rate
                - 1.3)
                .abs()
                < 1e-12
        );

        // Restored provider snapshots must remain lower authority than new
        // scoped fixings and pair-global caller overrides.
        fx.set_quote_on(from, to, on, FxConversionPolicy::CashflowDate, 2.0)
            .expect("pinned fixing");
        assert!(
            (fx.rate(FxQuery::new(from, to, on))
                .expect("pinned rate")
                .rate
                - 2.0)
                .abs()
                < 1e-12
        );
        fx.set_quote(from, to, 3.0).expect("global override");
        assert!(
            (fx.rate(FxQuery::new(from, to, on))
                .expect("global rate")
                .rate
                - 3.0)
                .abs()
                < 1e-12
        );
    }
}

#[test]
fn derived_bump_snapshot_respects_explicit_source_legs() {
    use finstack_quant_core::market_data::context::MarketContext;

    let on = Date::from_ordinal_date(2025, 1).expect("date");
    let provider = SimpleFxProvider::new();
    provider
        .set_quotes(&[
            (Currency::EUR, Currency::USD, 1.1),
            (Currency::GBP, Currency::USD, 1.3),
        ])
        .expect("provider quotes");
    let matrix = FxMatrix::new(Arc::new(provider));
    matrix
        .set_quote(Currency::EUR, Currency::USD, 1.2)
        .expect("authoritative pivot leg");
    let bumped = matrix
        .with_bumped_rate(Currency::EUR, Currency::GBP, 0.1, on)
        .expect("cross shock");
    let context = MarketContext::new().insert_fx(bumped);
    let json = serde_json::to_string(&context).expect("snapshot");
    let restored: MarketContext = serde_json::from_str(&json).expect("restore");
    let actual = restored
        .fx_required()
        .expect("FX")
        .rate(FxQuery::new(Currency::EUR, Currency::GBP, on))
        .expect("restored cross")
        .rate;
    assert!((actual - 1.2 / 1.3 * 1.1).abs() < 1e-12);
}

#[test]
fn bumped_global_quotes_and_derived_crosses_survive_context_snapshots() {
    use finstack_quant_core::market_data::context::MarketContext;

    let on = Date::from_ordinal_date(2025, 1).expect("date");
    for (from, to) in [
        (Currency::EUR, Currency::USD),
        (Currency::USD, Currency::EUR),
        (Currency::EUR, Currency::GBP),
    ] {
        let matrix = FxMatrix::new(Arc::new(SimpleFxProvider::new()));
        matrix
            .set_quotes(&[
                (Currency::EUR, Currency::USD, 1.1),
                (Currency::GBP, Currency::USD, 1.3),
            ])
            .expect("authoritative quotes");
        let expected = matrix.rate(FxQuery::new(from, to, on)).expect("rate").rate * 1.1;
        let bumped = matrix.with_bumped_rate(from, to, 0.1, on).expect("shock");
        let context = MarketContext::new().insert_fx(bumped);
        let json = serde_json::to_string(&context).expect("snapshot");
        let restored: MarketContext = serde_json::from_str(&json).expect("restore");
        let fx = restored.fx_required().expect("FX");
        assert!(
            (fx.rate(FxQuery::new(from, to, on))
                .expect("restored rate")
                .rate
                - expected)
                .abs()
                < 1e-12
        );
        assert!(
            (fx.rate(FxQuery::new(to, from, on)).expect("inverse").rate - 1.0 / expected).abs()
                < 1e-12
        );
    }
}

#[test]
fn global_cross_bump_persists_when_reference_date_has_a_direct_pinned_target() {
    use finstack_quant_core::market_data::context::MarketContext;

    let reference = Date::from_ordinal_date(2025, 1).expect("reference date");
    let later = Date::from_ordinal_date(2025, 182).expect("later date");
    let matrix = FxMatrix::new(Arc::new(SimpleFxProvider::new()));
    matrix
        .set_quotes(&[
            (Currency::EUR, Currency::USD, 1.1),
            (Currency::USD, Currency::GBP, 0.8),
        ])
        .expect("static global pivot legs");
    matrix
        .set_quote_on(
            Currency::EUR,
            Currency::GBP,
            reference,
            FxConversionPolicy::CashflowDate,
            2.0,
        )
        .expect("direct reference fixing");
    let bumped = matrix
        .with_bumped_rate(Currency::EUR, Currency::GBP, 0.1, reference)
        .expect("shock validated against the direct fixing");
    assert!(
        (bumped
            .rate(FxQuery::new(Currency::EUR, Currency::GBP, later))
            .expect("live global cross")
            .rate
            - 0.968)
            .abs()
            < 1e-12
    );
    let json = serde_json::to_string(&MarketContext::new().insert_fx(bumped)).expect("snapshot");
    let restored: MarketContext = serde_json::from_str(&json).expect("restore");
    let fx = restored.fx_required().expect("FX");
    for on in [reference, later] {
        for policy in [
            FxConversionPolicy::CashflowDate,
            FxConversionPolicy::PeriodEnd,
            FxConversionPolicy::PeriodAverage,
        ] {
            let expected = if on == reference && policy == FxConversionPolicy::CashflowDate {
                2.2
            } else {
                0.968
            };
            assert!(
                (fx.rate(FxQuery::with_policy(
                    Currency::EUR,
                    Currency::GBP,
                    on,
                    policy
                ))
                .expect("restored cross")
                .rate
                    - expected)
                    .abs()
                    < 1e-12
            );
            assert!(
                (fx.rate(FxQuery::with_policy(
                    Currency::GBP,
                    Currency::EUR,
                    on,
                    policy
                ))
                .expect("restored inverse")
                .rate
                    - 1.0 / expected)
                    .abs()
                    < 1e-12
            );
        }
    }
}

#[test]
fn bumped_pinned_crosses_retain_each_date_policy_and_quote_authority() {
    use finstack_quant_core::market_data::context::MarketContext;

    let dates = [
        Date::from_ordinal_date(2025, 1).expect("first date"),
        Date::from_ordinal_date(2025, 182).expect("second date"),
    ];
    let policies = [
        FxConversionPolicy::CashflowDate,
        FxConversionPolicy::PeriodEnd,
        FxConversionPolicy::PeriodAverage,
    ];
    for first_leg_source in ["global", "provider", "pinned", "direct_provider"] {
        for (from, to) in [
            (Currency::EUR, Currency::GBP),
            (Currency::GBP, Currency::EUR),
        ] {
            let provider = SimpleFxProvider::new();
            if matches!(first_leg_source, "provider" | "direct_provider") {
                provider
                    .set_quotes(&[
                        (Currency::EUR, Currency::USD, 1.1),
                        (Currency::USD, Currency::GBP, 0.8),
                    ])
                    .expect("provider pivot legs");
            }
            if first_leg_source == "direct_provider" {
                provider
                    .set_quote(Currency::EUR, Currency::GBP, 1.5)
                    .expect("direct provider cross outranks scoped pivot legs");
            }
            let matrix = FxMatrix::new(Arc::new(provider));
            if first_leg_source == "global" {
                matrix
                    .set_quote(Currency::EUR, Currency::USD, 1.1)
                    .expect("global pivot leg");
            }
            for (date_index, &on) in dates.iter().enumerate() {
                for (policy_index, &policy) in policies.iter().enumerate() {
                    let offset = date_index as f64 * 0.1 + policy_index as f64 * 0.01;
                    if first_leg_source != "global" {
                        matrix
                            .set_quote_on(Currency::EUR, Currency::USD, on, policy, 1.2 + offset)
                            .expect("scoped first leg");
                    }
                    matrix
                        .set_quote_on(Currency::USD, Currency::GBP, on, policy, 0.7 + offset)
                        .expect("scoped second leg");
                }
            }
            // The reference date is a direct pinned target rather than a
            // triangulated cross. Other scopes must still retain their shock,
            // and the opposite stored orientation must retain its authority.
            matrix
                .set_quote_on(Currency::GBP, Currency::EUR, dates[0], policies[0], 2.0)
                .expect("direct scoped cross");
            let expected: Vec<_> = dates
                .iter()
                .flat_map(|&on| policies.iter().map(move |&policy| (on, policy)))
                .map(|(on, policy)| {
                    let rate = matrix
                        .rate(FxQuery::with_policy(from, to, on, policy))
                        .expect("source scoped rate")
                        .rate;
                    (on, policy, rate * 1.1)
                })
                .collect();
            let bumped = matrix
                .with_bumped_rate(from, to, 0.1, dates[0])
                .expect("scoped cross shock");
            let context = MarketContext::new().insert_fx(bumped);
            let json = serde_json::to_string(&context).expect("snapshot");
            let restored: MarketContext = serde_json::from_str(&json).expect("restore");
            let fx = restored.fx_required().expect("restored FX");
            for (on, policy, expected) in expected {
                let query = FxQuery::with_policy(from, to, on, policy);
                let inverse = FxQuery::with_policy(to, from, on, policy);
                assert!((fx.rate(query).expect("scoped rate").rate - expected).abs() < 1e-12);
                assert!(
                    (fx.rate(inverse).expect("scoped reciprocal").rate - 1.0 / expected).abs()
                        < 1e-12
                );
            }
            fx.set_quote_on(to, from, dates[1], policies[0], 4.0)
                .expect("new scoped inverse override");
            assert_eq!(
                fx.rate(FxQuery::with_policy(to, from, dates[1], policies[0]))
                    .expect("new scoped override")
                    .rate,
                4.0
            );
            assert_eq!(
                fx.rate(FxQuery::with_policy(from, to, dates[1], policies[0]))
                    .expect("new scoped override reciprocal")
                    .rate,
                0.25
            );
            fx.set_quote(to, from, 5.0)
                .expect("new global inverse override");
            assert_eq!(
                fx.rate(FxQuery::new(from, to, dates[1]))
                    .expect("global")
                    .rate,
                0.2
            );
        }
    }
}

#[test]
fn snapshot_only_scoped_crosses_do_not_pin_the_live_bumped_provider() {
    let on = Date::from_ordinal_date(2025, 1).expect("date");
    let provider = Arc::new(SimpleFxProvider::new());
    let matrix = FxMatrix::new(Arc::clone(&provider) as Arc<dyn FxProvider>);
    matrix
        .set_quote(Currency::EUR, Currency::USD, 1.1)
        .expect("global first leg");
    matrix
        .set_quote_on(
            Currency::USD,
            Currency::GBP,
            on,
            FxConversionPolicy::CashflowDate,
            0.8,
        )
        .expect("scoped second leg");
    let bumped = matrix
        .with_bumped_rate(Currency::EUR, Currency::GBP, 0.1, on)
        .expect("cross shock");
    let state = bumped.get_serializable_state().expect("stable FX state");
    assert!(state
        .provider_pinned_quotes
        .iter()
        .any(|&(from, to, date, policy, rate)| {
            from == Currency::EUR
                && to == Currency::GBP
                && date == on
                && policy == FxConversionPolicy::CashflowDate
                && (rate - 0.968).abs() < 1e-12
        }));
    // A newly published direct provider quote still outranks triangulation.
    provider
        .set_quote(Currency::EUR, Currency::GBP, 2.0)
        .expect("new direct provider quote");
    assert_eq!(
        bumped
            .rate(FxQuery::new(Currency::EUR, Currency::GBP, on))
            .expect("live direct rate")
            .rate,
        2.2
    );
    assert!(bumped
        .get_serializable_state()
        .expect("stable FX state")
        .provider_pinned_quotes
        .iter()
        .all(|&(from, to, _, _, _)| !(from == Currency::EUR && to == Currency::GBP)));
}

#[test]
fn public_bumped_provider_snapshot_normalizes_the_shocked_orientation() {
    use finstack_quant_core::money::fx::BumpedFxProvider;

    let on = Date::from_ordinal_date(2025, 1).expect("date");
    let provider = SimpleFxProvider::new();
    provider
        .set_quotes(&[
            (Currency::EUR, Currency::USD, 1.1),
            (Currency::USD, Currency::EUR, 0.9),
            (Currency::GBP, Currency::USD, 1.3),
        ])
        .expect("positive direction-sensitive quotes");
    let bumped =
        BumpedFxProvider::new(Arc::new(provider), Currency::EUR, Currency::USD, 0.1).expect("bump");
    let restored = SimpleFxProvider::new();
    restored
        .set_quotes(&bumped.snapshot_quotes())
        .expect("snapshot quotes");
    for (from, to) in [
        (Currency::EUR, Currency::USD),
        (Currency::USD, Currency::EUR),
        (Currency::GBP, Currency::USD),
    ] {
        assert_eq!(
            restored
                .rate(from, to, on, FxConversionPolicy::CashflowDate)
                .expect("snapshot rate"),
            bumped
                .rate(from, to, on, FxConversionPolicy::CashflowDate)
                .expect("live rate")
        );
    }
}

#[test]
fn fx_bumps_reject_overflowing_static_scopes_outside_the_reference_date() {
    let reference = Date::from_ordinal_date(2025, 1).expect("reference date");
    let later = Date::from_ordinal_date(2025, 182).expect("later scope");
    for (first, second) in [(1e300, 1e300), (f64::MAX / 2.0, 0.8)] {
        // Provider baseline legs remain below the exceptional pinned scope.
        let provider = SimpleFxProvider::new();
        provider
            .set_quotes(&[
                (Currency::EUR, Currency::USD, 1.1),
                (Currency::USD, Currency::GBP, 0.8),
            ])
            .expect("valid reference legs");
        let matrix = FxMatrix::new(Arc::new(provider));
        matrix
            .set_quote_on(
                Currency::EUR,
                Currency::USD,
                later,
                FxConversionPolicy::PeriodEnd,
                first,
            )
            .expect("finite first scoped leg");
        matrix
            .set_quote_on(
                Currency::USD,
                Currency::GBP,
                later,
                FxConversionPolicy::PeriodEnd,
                second,
            )
            .expect("finite second scoped leg");
        assert!(
            (matrix
                .rate(FxQuery::new(Currency::EUR, Currency::GBP, reference))
                .expect("valid reference")
                .rate
                - 0.88)
                .abs()
                < 1e-12
        );
        assert!(matrix
            .with_bumped_rate(Currency::EUR, Currency::GBP, 9.0, reference)
            .is_err());
        assert!(
            serde_json::to_string(&matrix.get_serializable_state().expect("stable FX state"))
                .is_ok()
        );
    }
}

#[test]
fn scoped_provider_snapshot_field_is_required() {
    let matrix = FxMatrix::new(Arc::new(SimpleFxProvider::new()));
    let mut state = serde_json::to_value(matrix.get_serializable_state().expect("stable FX state"))
        .expect("snapshot");
    state
        .as_object_mut()
        .expect("object")
        .remove("provider_pinned_quotes");
    assert!(
        serde_json::from_value::<finstack_quant_core::money::fx::FxMatrixState>(state).is_err()
    );
}

#[test]
fn mutable_provider_overflow_fails_snapshot_serialization_without_emitting_null() {
    use finstack_quant_core::market_data::context::MarketContext;

    let on = Date::from_ordinal_date(2025, 1).expect("date");
    let provider = Arc::new(SimpleFxProvider::new());
    provider
        .set_quote(Currency::EUR, Currency::USD, 1.1)
        .expect("initial quote");
    let matrix = FxMatrix::new(Arc::clone(&provider) as Arc<dyn FxProvider>);
    let bumped = matrix
        .with_bumped_rate(Currency::EUR, Currency::USD, 0.1, on)
        .expect("initially valid shock");
    provider
        .set_quote(Currency::EUR, Currency::USD, f64::MAX)
        .expect("finite positive provider update");
    assert!(bumped
        .rate(FxQuery::new(Currency::EUR, Currency::USD, on))
        .is_err());
    let captured = bumped.get_serializable_state().expect("stable FX state");
    assert!(serde_json::to_string(&captured).is_err());
    assert!(serde_json::to_value(&captured).is_err());
    let context = MarketContext::new().insert_fx(bumped);
    assert!(serde_json::to_string(&context).is_err());
}

#[test]
fn mutable_provider_cross_shock_overflow_is_not_replaced_by_unshocked_triangulation() {
    use finstack_quant_core::market_data::context::MarketContext;

    let on = Date::from_ordinal_date(2025, 1).expect("date");
    let provider = Arc::new(SimpleFxProvider::new());
    for (from, to, rate) in [
        (Currency::EUR, Currency::GBP, 1.1),
        (Currency::EUR, Currency::USD, 1.1),
        (Currency::USD, Currency::GBP, 0.8),
    ] {
        provider.set_quote(from, to, rate).expect("initial quote");
    }
    let matrix = FxMatrix::new(Arc::clone(&provider) as Arc<dyn FxProvider>);
    let bumped = matrix
        .with_bumped_rate(Currency::EUR, Currency::GBP, 0.1, on)
        .expect("initially valid cross shock");
    provider
        .set_quote(Currency::EUR, Currency::GBP, f64::MAX)
        .expect("finite positive provider update");
    let error = bumped
        .rate(FxQuery::new(Currency::EUR, Currency::GBP, on))
        .expect_err("invalid direct shocked rate must not fall back to unshocked pivot legs");
    assert!(matches!(
        error,
        finstack_quant_core::Error::Input(finstack_quant_core::InputError::InvalidFxRate { .. })
    ));
    assert!(
        serde_json::to_string(&bumped.get_serializable_state().expect("stable FX state")).is_err()
    );
    assert!(serde_json::to_string(&MarketContext::new().insert_fx(bumped)).is_err());
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
        ) -> finstack_quant_core::Result<f64> {
            Err(finstack_quant_core::InputError::NotFound {
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
    assert!(matches!(err, finstack_quant_core::Error::Validation(_)));
}

#[test]
fn validate_triangular_accepts_consistent_quotes_and_rejects_bad_tolerance() {
    struct MissingFx;
    impl FxProvider for MissingFx {
        fn rate(
            &self,
            from: Currency,
            to: Currency,
            _on: Date,
            _policy: FxConversionPolicy,
        ) -> finstack_quant_core::Result<f64> {
            Err(finstack_quant_core::InputError::NotFound {
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
            (Currency::GBP, Currency::EUR, 1.0 / (1.10 * 0.80)),
        ])
        .expect("valid quotes");

    assert!(matrix.validate_triangular(5.0).is_ok());
    assert!(matches!(
        matrix.validate_triangular(-1.0),
        Err(finstack_quant_core::Error::Validation(_))
    ));
    assert!(matches!(
        matrix.validate_triangular(f64::NAN),
        Err(finstack_quant_core::Error::Validation(_))
    ));
}

#[test]
fn validate_triangular_handles_normal_market_quote_directions() {
    let matrix = FxMatrix::new(Arc::new(SimpleFxProvider::new()));
    matrix
        .set_quotes(&[
            (Currency::EUR, Currency::USD, 1.10),
            (Currency::USD, Currency::JPY, 150.0),
            (Currency::EUR, Currency::JPY, 160.0),
        ])
        .expect("valid quotes");

    matrix
        .validate_triangular(5.0)
        .expect_err("EUR/JPY should be compared with EUR/USD x USD/JPY");
}

#[test]
fn validate_triangular_does_not_mix_dates() {
    let matrix = FxMatrix::new(Arc::new(SimpleFxProvider::new()));
    let jan = Date::from_calendar_date(2025, time::Month::January, 2).unwrap();
    let feb = Date::from_calendar_date(2025, time::Month::February, 3).unwrap();
    let mar = Date::from_calendar_date(2025, time::Month::March, 3).unwrap();
    let policy = FxConversionPolicy::CashflowDate;

    matrix
        .set_quote_on(Currency::EUR, Currency::USD, jan, policy, 1.10)
        .unwrap();
    matrix
        .set_quote_on(Currency::USD, Currency::JPY, feb, policy, 150.0)
        .unwrap();
    matrix
        .set_quote_on(Currency::EUR, Currency::JPY, mar, policy, 160.0)
        .unwrap();

    assert!(matrix.validate_triangular(5.0).is_ok());
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
        ) -> finstack_quant_core::Result<f64> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            Err(finstack_quant_core::InputError::NotFound {
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
    fn get_revision(&self) -> Option<u64> {
        Some(0)
    }

    fn rate(
        &self,
        from: Currency,
        to: Currency,
        _on: Date,
        _policy: FxConversionPolicy,
    ) -> finstack_quant_core::Result<f64> {
        self.legs
            .iter()
            .find(|(f, t, _)| *f == from && *t == to)
            .map(|(_, _, r)| *r)
            .ok_or_else(|| {
                finstack_quant_core::InputError::NotFound {
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
        ) -> finstack_quant_core::Result<f64> {
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
fn pair_global_reciprocal_outranks_pinned_quote() {
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
        (rate - 1.25).abs() < 1e-12,
        "pair-global source priority must apply in either orientation"
    );
}

#[test]
fn fx_matrix_state_round_trips_pinned_quotes() {
    // After snapshot + restore, a pinned fixing must still win over the
    // provider for its (on, policy).
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

    let state = matrix.get_serializable_state().expect("stable FX state");
    assert_eq!(state.pinned_quotes.len(), 1);

    // Serde round-trip too (the state is the persistence format).
    let json = serde_json::to_string(&state).unwrap();
    let state: finstack_quant_core::money::fx::FxMatrixState = serde_json::from_str(&json).unwrap();

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

    // A snapshot that omits `pinned_quotes` is rejected rather than silently
    // restoring a matrix with no pinned fixings.
    let without_pinned = r#"{"config":{"pivot_currency":"USD","enable_triangulation":true,"cache_capacity":256},"quotes":[],"provider_quotes":[],"provider_pinned_quotes":[]}"#;
    assert!(
        serde_json::from_str::<finstack_quant_core::money::fx::FxMatrixState>(without_pinned)
            .is_err(),
        "a state payload missing `pinned_quotes` must fail closed"
    );
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
        ) -> finstack_quant_core::Result<f64> {
            Err(finstack_quant_core::InputError::NotFound {
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

/// Provider-observed rates must not depend on which pair direction warmed the
/// bounded LRU first. Assertions are bit-exact because a tolerance would hide
/// the one-ulp nondeterminism this test guards.
#[test]
fn fx_observed_cache_is_order_independent_across_directions() {
    let date = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();
    let rate = 31.528_877_897_691_935;

    let first = FxMatrix::new(Arc::new(ReciprocalFx { eur_usd: rate }));
    let _ = first
        .rate(FxQuery::new(Currency::EUR, Currency::USD, date))
        .unwrap();
    let first_forward = first
        .rate(FxQuery::new(Currency::EUR, Currency::USD, date))
        .unwrap();
    let first_reverse = first
        .rate(FxQuery::new(Currency::USD, Currency::EUR, date))
        .unwrap();

    let second = FxMatrix::new(Arc::new(ReciprocalFx { eur_usd: rate }));
    let _ = second
        .rate(FxQuery::new(Currency::USD, Currency::EUR, date))
        .unwrap();
    let second_forward = second
        .rate(FxQuery::new(Currency::EUR, Currency::USD, date))
        .unwrap();
    let second_reverse = second
        .rate(FxQuery::new(Currency::USD, Currency::EUR, date))
        .unwrap();

    assert_eq!(first_forward.rate, second_forward.rate);
    assert_eq!(first_reverse.rate, second_reverse.rate);
    assert_eq!(first_forward.triangulated, second_forward.triangulated);
    assert_eq!(first_reverse.triangulated, second_reverse.triangulated);
}

/// A cold provider lookup must return exactly what its subsequent cache hit
/// returns in both pair directions.
#[test]
fn fx_cold_and_warm_lookups_agree_bit_exactly() {
    let date = Date::from_calendar_date(2025, time::Month::January, 1).unwrap();
    let rate = 31.528_877_897_691_935;

    for (from, to) in [
        (Currency::EUR, Currency::USD),
        (Currency::USD, Currency::EUR),
    ] {
        let matrix = FxMatrix::new(Arc::new(ReciprocalFx { eur_usd: rate }));
        let cold = matrix.rate(FxQuery::new(from, to, date)).unwrap();
        let warm = matrix.rate(FxQuery::new(from, to, date)).unwrap();
        assert_eq!(cold.rate, warm.rate, "{from}->{to}");
        assert_eq!(cold.triangulated, warm.triangulated, "{from}->{to}");
    }
}
