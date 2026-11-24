use finstack_quant_core::FxHashMap;

/// The engine always supplies a claim map. Tests that want "every tranche
/// owes its full uncapped coupon" state that explicitly with this helper.
fn uncapped_claims(ids: &[&'static str]) -> FxHashMap<&'static str, Option<f64>> {
    ids.iter().map(|id| (*id, None)).collect()
}

mod coverage_tests {
    use super::super::evaluator::*;
    use super::uncapped_claims;
    use crate::structured_credit::types::{
        AssetPool, CoverageTestSpec, DealType, Tranche, TrancheSeniority, TrancheStructure,
    };
    use finstack_quant_cashflows::builder::RateSpec;
    use finstack_quant_core::FxHashMap;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::money::Money;

    use time::Month;

    #[test]
    fn test_oc_test_creation() {
        let test = CoverageTestSpec::oc("A", 1.15);
        assert_eq!(test.trigger_level, 1.15);
    }

    #[test]
    fn test_oc_test_calculation() {
        let pool = AssetPool::new("TEST", DealType::Clo, Currency::USD);
        let test = CoverageTestSpec::oc("TEST_TRANCHE", 1.25);

        let tranche = Tranche::new(
            "TEST_TRANCHE",
            0.0,
            100.0,
            TrancheSeniority::Senior,
            Money::from((100_000_i64, Currency::USD)),
            RateSpec::Fixed { rate: 0.05 },
            Date::from_calendar_date(2030, Month::January, 1).expect("Valid date"),
        )
        .expect("Valid tranche");

        let tranches = TrancheStructure::new(vec![tranche]).expect("Valid tranche structure");

        let context = TestContext {
            pool: &pool,
            tranches: &tranches,
            as_of: Date::from_calendar_date(2025, Month::January, 1).expect("Valid date"),
            valuation_date: Date::from_calendar_date(2025, Month::January, 1).expect("Valid date"),
            period_start: None,
            cash_balance: Money::from((0_i64, Currency::USD)),
            interest_collections: Money::from((0_i64, Currency::USD)),
            rules: None,
            market: None,
            tranche_balances: None,
            payable_principal_tranche_ids: None,
            asset_balances: None,
            live_collateral: None,
            current_pool_balance: None,
            senior_fees: Money::from((0_i64, Currency::USD)),
            restricted_cash: Money::from((0_i64, Currency::USD)),
            defaulted_collateral_value: Money::from((0_i64, Currency::USD)),
            interest_claim_caps: &uncapped_claims(&["TEST_TRANCHE"]),
            floating_rate_shift: 0.0,
            deferred_interest: None,
        };

        let result = test.evaluate(&context).expect("calculation should succeed");

        assert_eq!(result.ratio, 0.0);
        assert!(!result.passing);
    }

    #[test]
    fn test_ic_test_calculation() {
        let pool = AssetPool::new("TEST", DealType::Clo, Currency::USD);
        let test = CoverageTestSpec::ic("TEST_TRANCHE", 1.20);

        let tranche = Tranche::new(
            "TEST_TRANCHE",
            0.0,
            100.0,
            TrancheSeniority::Senior,
            Money::from((100_000_i64, Currency::USD)),
            RateSpec::Fixed { rate: 0.05 },
            Date::from_calendar_date(2030, Month::January, 1).expect("Valid date"),
        )
        .expect("Valid tranche");

        let tranches = TrancheStructure::new(vec![tranche]).expect("Valid tranche structure");

        let context = TestContext {
            pool: &pool,
            tranches: &tranches,
            as_of: Date::from_calendar_date(2025, Month::January, 1).expect("Valid date"),
            valuation_date: Date::from_calendar_date(2025, Month::January, 1).expect("Valid date"),
            period_start: None,
            cash_balance: Money::from((0_i64, Currency::USD)),
            interest_collections: Money::from((1_500_i64, Currency::USD)),
            rules: None,
            market: None,
            tranche_balances: None,
            payable_principal_tranche_ids: None,
            asset_balances: None,
            live_collateral: None,
            current_pool_balance: None,
            senior_fees: Money::from((0_i64, Currency::USD)),
            restricted_cash: Money::from((0_i64, Currency::USD)),
            defaulted_collateral_value: Money::from((0_i64, Currency::USD)),
            interest_claim_caps: &uncapped_claims(&["TEST_TRANCHE"]),
            floating_rate_shift: 0.0,
            deferred_interest: None,
        };

        let result = test.evaluate(&context).expect("calculation should succeed");

        assert!((result.ratio - 1.2).abs() < 0.01);
        assert!(result.passing);
    }

    /// IC must cover the same claim the waterfall pays: current coupon plus
    /// unpaid non-PIK arrears. Omitting deferred from the denominator makes a
    /// deal with carried shortfalls look healthier than the cash it owes.
    #[test]
    fn ic_denominator_includes_deferred_arrears() {
        let pool = AssetPool::new("TEST", DealType::Clo, Currency::USD);
        let test = CoverageTestSpec::ic("TEST_TRANCHE", 1.20);
        let tranche = Tranche::new(
            "TEST_TRANCHE",
            0.0,
            100.0,
            TrancheSeniority::Senior,
            Money::from((100_000_i64, Currency::USD)),
            RateSpec::Fixed { rate: 0.05 },
            Date::from_calendar_date(2030, Month::January, 1).expect("Valid date"),
        )
        .expect("Valid tranche");
        let tranches = TrancheStructure::new(vec![tranche]).expect("Valid tranche structure");
        let mut deferred = FxHashMap::default();
        deferred.insert(
            "TEST_TRANCHE".to_string(),
            Money::from((1_250_i64, Currency::USD)),
        );
        let context = TestContext {
            pool: &pool,
            tranches: &tranches,
            as_of: Date::from_calendar_date(2025, Month::January, 1).expect("Valid date"),
            valuation_date: Date::from_calendar_date(2025, Month::January, 1).expect("Valid date"),
            period_start: None,
            cash_balance: Money::from((0_i64, Currency::USD)),
            interest_collections: Money::from((1_500_i64, Currency::USD)),
            rules: None,
            market: None,
            tranche_balances: None,
            payable_principal_tranche_ids: None,
            asset_balances: None,
            live_collateral: None,
            current_pool_balance: None,
            senior_fees: Money::from((0_i64, Currency::USD)),
            restricted_cash: Money::from((0_i64, Currency::USD)),
            defaulted_collateral_value: Money::from((0_i64, Currency::USD)),
            interest_claim_caps: &uncapped_claims(&["TEST_TRANCHE"]),
            floating_rate_shift: 0.0,
            deferred_interest: Some(&deferred),
        };

        let result = test.evaluate(&context).expect("calculation should succeed");
        // Coupon ≈ 100k × 5% / 4 = 1,250; plus 1,250 deferred => 2,500 due.
        // Collections 1,500 / 2,500 = 0.60, which fails a 1.20 test.
        // Without deferred the ratio would be 1.20 and the test would pass.
        assert!(
            (result.ratio - 0.60).abs() < 0.02,
            "IC must include deferred arrears; got {}",
            result.ratio
        );
        assert!(!result.passing);
    }

    /// PIK shortfalls live in balance, not the deferred map. A stale deferred
    /// entry on a PIK tranche must not inflate the IC denominator.
    #[test]
    fn ic_denominator_ignores_deferred_on_pik_tranche() {
        let pool = AssetPool::new("TEST", DealType::Clo, Currency::USD);
        let test = CoverageTestSpec::ic("TEST_TRANCHE", 1.20);
        let mut tranche = Tranche::new(
            "TEST_TRANCHE",
            0.0,
            100.0,
            TrancheSeniority::Senior,
            Money::from((100_000_i64, Currency::USD)),
            RateSpec::Fixed { rate: 0.05 },
            Date::from_calendar_date(2030, Month::January, 1).expect("Valid date"),
        )
        .expect("Valid tranche");
        tranche.pik_enabled = true;
        let tranches = TrancheStructure::new(vec![tranche]).expect("Valid tranche structure");
        let mut deferred = FxHashMap::default();
        deferred.insert(
            "TEST_TRANCHE".to_string(),
            Money::from((1_250_i64, Currency::USD)),
        );
        let context = TestContext {
            pool: &pool,
            tranches: &tranches,
            as_of: Date::from_calendar_date(2025, Month::January, 1).expect("Valid date"),
            valuation_date: Date::from_calendar_date(2025, Month::January, 1).expect("Valid date"),
            period_start: None,
            cash_balance: Money::from((0_i64, Currency::USD)),
            interest_collections: Money::from((1_500_i64, Currency::USD)),
            rules: None,
            market: None,
            tranche_balances: None,
            payable_principal_tranche_ids: None,
            asset_balances: None,
            live_collateral: None,
            current_pool_balance: None,
            senior_fees: Money::from((0_i64, Currency::USD)),
            restricted_cash: Money::from((0_i64, Currency::USD)),
            defaulted_collateral_value: Money::from((0_i64, Currency::USD)),
            interest_claim_caps: &uncapped_claims(&["TEST_TRANCHE"]),
            floating_rate_shift: 0.0,
            deferred_interest: Some(&deferred),
        };

        let result = test.evaluate(&context).expect("calculation should succeed");
        // Coupon ≈ 100k × 5% / 4 = 1,250; stale deferred is ignored.
        // Collections 1,500 / 1,250 = 1.20.
        assert!(
            (result.ratio - 1.20).abs() < 0.02,
            "PIK IC denominator must be coupon only; got {}",
            result.ratio
        );
        assert!(result.passing);
    }

    /// W-22: the OC cure amount must account for the cash term leaving the
    /// numerator when it is diverted to pay down notes. Diverting the computed
    /// cure must bring the OC ratio to exactly the required ratio.
    #[test]
    fn test_oc_cure_with_cash_in_numerator_restores_exact_ratio() {
        // Numerator = collateral (90k, stays) + cash (30k, leaves on diversion).
        // Denominator = 100k. Ratio = 120k / 100k = 1.20, breaches a 1.25 trigger.
        let pool = AssetPool::new("TEST", DealType::Clo, Currency::USD);
        let trigger_level = 1.25_f64;
        let test = CoverageTestSpec::oc("SENIOR", trigger_level);

        let tranche = Tranche::new(
            "SENIOR",
            0.0,
            100.0,
            TrancheSeniority::Senior,
            Money::from((100_000_i64, Currency::USD)),
            RateSpec::Fixed { rate: 0.05 },
            Date::from_calendar_date(2030, Month::January, 1).expect("Valid date"),
        )
        .expect("Valid tranche");
        let tranches = TrancheStructure::new(vec![tranche]).expect("Valid tranche structure");

        let collateral = 90_000.0_f64;
        let cash = 30_000.0_f64;
        let context = TestContext {
            pool: &pool,
            tranches: &tranches,
            as_of: Date::from_calendar_date(2025, Month::January, 1).expect("Valid date"),
            valuation_date: Date::from_calendar_date(2025, Month::January, 1).expect("Valid date"),
            period_start: None,
            cash_balance: Money::new(cash, Currency::USD).expect("valid money fixture"),
            interest_collections: Money::from((0_i64, Currency::USD)),
            rules: None,
            market: None,
            tranche_balances: None,
            payable_principal_tranche_ids: None,
            asset_balances: None,
            live_collateral: None,
            current_pool_balance: Some(
                Money::new(collateral, Currency::USD).expect("valid money fixture"),
            ),
            senior_fees: Money::from((0_i64, Currency::USD)),
            restricted_cash: Money::from((0_i64, Currency::USD)),
            defaulted_collateral_value: Money::from((0_i64, Currency::USD)),
            interest_claim_caps: &uncapped_claims(&["SENIOR"]),
            floating_rate_shift: 0.0,
            deferred_interest: None,
        };

        let result = test.evaluate(&context).expect("calculation should succeed");
        assert!(!result.passing, "OC test should breach (ratio 1.20 < 1.25)");

        let cure = result.cure_amount.expect("breach must yield a cure amount");
        let x = cure.amount();
        assert!(
            x <= cash + 1e-6,
            "cure {x} should be fundable from available cash {cash}"
        );

        // numerator = collateral + cash, denominator = 100k.
        let numerator = collateral + cash;
        let denominator = 100_000.0_f64;
        // The cure is diverted interest, which is not in the numerator: it
        // only pays down the denominator. The cured ratio must equal the
        // required ratio exactly.
        let cured_ratio = numerator / (denominator - x);
        assert!(
            (cured_ratio - trigger_level).abs() < 1e-6,
            "cured ratio {cured_ratio} should equal required {trigger_level}; cure X={x}"
        );
    }

    /// Item 11 — a near-1.0 OC trigger must NOT produce an exploding cure.
    ///
    /// The cure formula carries a `1/(1 − trigger_level)` factor. With a
    /// trigger like 1.001 that factor is ~1000×, so an unbounded cure would be
    /// orders of magnitude larger than any principal the structure holds. The
    /// cure must be capped at the OC denominator (test tranche + senior
    /// balances) — the most a coverage diversion could ever pay down.
    #[test]
    fn test_oc_cure_is_capped_at_denominator_for_near_one_trigger() {
        let pool = AssetPool::new("TEST", DealType::Clo, Currency::USD);
        // Trigger just above 1.0 — the pathological regime for the cure.
        let trigger_level = 1.001_f64;
        let test = CoverageTestSpec::oc("SENIOR", trigger_level);

        let tranche = Tranche::new(
            "SENIOR",
            0.0,
            100.0,
            TrancheSeniority::Senior,
            Money::from((100_000_i64, Currency::USD)),
            RateSpec::Fixed { rate: 0.05 },
            Date::from_calendar_date(2030, Month::January, 1).expect("Valid date"),
        )
        .expect("Valid tranche");
        let tranches = TrancheStructure::new(vec![tranche]).expect("Valid tranche structure");

        // Collateral well below par so the OC ratio breaches 1.001.
        let collateral = 50_000.0_f64;
        let denominator = 100_000.0_f64; // single senior tranche balance
        let context = TestContext {
            pool: &pool,
            tranches: &tranches,
            as_of: Date::from_calendar_date(2025, Month::January, 1).expect("Valid date"),
            valuation_date: Date::from_calendar_date(2025, Month::January, 1).expect("Valid date"),
            period_start: None,
            cash_balance: Money::from((0_i64, Currency::USD)),
            interest_collections: Money::from((0_i64, Currency::USD)),
            rules: None,
            market: None,
            tranche_balances: None,
            payable_principal_tranche_ids: None,
            asset_balances: None,
            live_collateral: None,
            current_pool_balance: Some(
                Money::new(collateral, Currency::USD).expect("valid money fixture"),
            ),
            senior_fees: Money::from((0_i64, Currency::USD)),
            restricted_cash: Money::from((0_i64, Currency::USD)),
            defaulted_collateral_value: Money::from((0_i64, Currency::USD)),
            interest_claim_caps: &uncapped_claims(&["SENIOR"]),
            floating_rate_shift: 0.0,
            deferred_interest: None,
        };

        let result = test.evaluate(&context).expect("calculation should succeed");
        assert!(!result.passing, "OC test must breach (ratio 0.5 < 1.001)");

        let cure = result.cure_amount.expect("breach must yield a cure amount");
        // The raw formula would give a cure of roughly
        //   (50k − 1.001·100k)/(1 − 1.001) ≈ 50.1M — absurdly large.
        // The capped cure must never exceed the OC denominator.
        assert!(
            cure.amount() <= denominator + 1e-6,
            "near-1.0-trigger OC cure {} must be capped at the denominator \
             {denominator} — the raw formula explodes to tens of millions",
            cure.amount()
        );
        assert!(
            cure.amount() > 0.0,
            "a breaching OC test must still report a positive cure"
        );
    }

    /// C4 regression: a real CLO/ABS can carry several notes at one seniority
    /// level (Class A-1, A-2, A-3 all `Senior`). `payment_priority` must rank
    /// every note distinctly by structural seniority, NOT collapse them onto a
    /// per-`TrancheSeniority` constant. If they collapse, `senior_to` returns
    /// `[]` for same-seniority notes and the OC denominator omits them.
    #[test]
    fn test_same_seniority_notes_get_distinct_priorities_and_oc_denominator() {
        use finstack_quant_core::dates::Date;
        let mat = Date::from_calendar_date(2034, Month::January, 1).expect("date");
        let cpn = || RateSpec::Fixed { rate: 0.05 };
        // Three Senior notes at distinct attachment points + an Equity note.
        // Capital stack (most senior first, lowest attachment): A-1 0-25,
        // A-2 25-50, A-3 50-75, Equity 75-100. Passed in seniority order.
        let a1 = Tranche::new(
            "A-1",
            0.0,
            25.0,
            TrancheSeniority::Senior,
            Money::from((25_000_i64, Currency::USD)),
            cpn(),
            mat,
        )
        .expect("tranche");
        let a2 = Tranche::new(
            "A-2",
            25.0,
            50.0,
            TrancheSeniority::Senior,
            Money::from((25_000_i64, Currency::USD)),
            cpn(),
            mat,
        )
        .expect("tranche");
        let a3 = Tranche::new(
            "A-3",
            50.0,
            75.0,
            TrancheSeniority::Senior,
            Money::from((25_000_i64, Currency::USD)),
            cpn(),
            mat,
        )
        .expect("tranche");
        let equity = Tranche::new(
            "EQUITY",
            75.0,
            100.0,
            TrancheSeniority::Equity,
            Money::from((25_000_i64, Currency::USD)),
            cpn(),
            mat,
        )
        .expect("tranche");

        let tranches = TrancheStructure::new(vec![a1, a2, a3, equity]).expect("structure");

        // senior_to(A-2) must include A-1 only — not A-2 itself, not A-3/Equity.
        let senior_to_a2: Vec<&str> = tranches
            .senior_to("A-2")
            .iter()
            .map(|t| t.id.as_str())
            .collect();
        assert_eq!(
            senior_to_a2,
            vec!["A-1"],
            "A-1 is the only note senior to A-2"
        );

        // The OC denominator for a junior tranche (A-3) = its own balance plus
        // ALL senior notes (A-1 + A-2). All three senior notes must contribute.
        let oc_denominator = tranches.senior_balance("A-3").amount() + 25_000.0;
        assert_eq!(
            oc_denominator, 75_000.0,
            "A-3 OC denominator must include A-1 + A-2 + A-3 balances"
        );

        // A-2's OC denominator reflects A-1's balance (senior_balance non-zero).
        assert_eq!(
            tranches.senior_balance("A-2").amount(),
            25_000.0,
            "A-2 senior_balance must equal A-1's balance"
        );

        // Priorities must be distinct and strictly increasing by seniority.
        let p = |id: &str| {
            tranches
                .tranches
                .iter()
                .find(|t| t.id.as_str() == id)
                .expect("tranche")
                .payment_priority
        };
        assert!(p("A-1") < p("A-2"));
        assert!(p("A-2") < p("A-3"));
        assert!(p("A-3") < p("EQUITY"));
    }

    /// W-21: an IC-test breach must produce a non-`None` cure amount equal to
    /// the senior interest shortfall, so IC-only breaches actually divert cash.
    #[test]
    fn test_ic_breach_yields_senior_interest_shortfall_cure() {
        let pool = AssetPool::new("TEST", DealType::Clo, Currency::USD);
        let trigger_level = 1.20_f64;
        let test = CoverageTestSpec::ic("TEST_TRANCHE", trigger_level);

        let tranche = Tranche::new(
            "TEST_TRANCHE",
            0.0,
            100.0,
            TrancheSeniority::Senior,
            Money::from((100_000_i64, Currency::USD)),
            RateSpec::Fixed { rate: 0.05 },
            Date::from_calendar_date(2030, Month::January, 1).expect("Valid date"),
        )
        .expect("Valid tranche");
        let tranches = TrancheStructure::new(vec![tranche]).expect("Valid tranche structure");

        // Interest collections far below interest due => IC test breaches.
        let context = TestContext {
            pool: &pool,
            tranches: &tranches,
            as_of: Date::from_calendar_date(2025, Month::January, 1).expect("Valid date"),
            valuation_date: Date::from_calendar_date(2025, Month::January, 1).expect("Valid date"),
            period_start: None,
            cash_balance: Money::from((0_i64, Currency::USD)),
            interest_collections: Money::from((100_i64, Currency::USD)),
            rules: None,
            market: None,
            tranche_balances: None,
            payable_principal_tranche_ids: None,
            asset_balances: None,
            live_collateral: None,
            current_pool_balance: None,
            senior_fees: Money::from((0_i64, Currency::USD)),
            restricted_cash: Money::from((0_i64, Currency::USD)),
            defaulted_collateral_value: Money::from((0_i64, Currency::USD)),
            interest_claim_caps: &uncapped_claims(&["TEST_TRANCHE"]),
            floating_rate_shift: 0.0,
            deferred_interest: None,
        };

        let result = test.evaluate(&context).expect("calculation should succeed");
        assert!(!result.passing, "IC test should breach");
        let cure = result
            .cure_amount
            .expect("an IC breach must yield a non-None cure amount");
        assert!(
            cure.amount() > 0.0,
            "IC cure must be positive (the interest shortfall), got {}",
            cure.amount()
        );
    }
}

mod haircut_tests {
    use super::super::evaluator::*;
    use super::uncapped_claims;
    use crate::structured_credit::types::{
        AssetPool, CoverageRules, CoverageTestSpec, DealType, PoolAsset, Tranche, TrancheSeniority,
        TrancheStructure,
    };
    use finstack_quant_cashflows::builder::FloatingRateSpec;
    use finstack_quant_cashflows::builder::RateSpec;
    use finstack_quant_core::FxHashMap;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount, Tenor};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::fixings::fixing_series_id;
    use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
    use finstack_quant_core::market_data::term_structures::ForwardCurve;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::{CreditRating, CurveId};
    use rust_decimal::Decimal;
    use time::Month;

    fn maturity() -> Date {
        Date::from_calendar_date(2034, Month::January, 1).expect("date")
    }

    /// Pool of two 500k assets: one AAA, one CCC.
    fn rated_pool() -> AssetPool {
        let mut pool = AssetPool::new("POOL", DealType::Clo, Currency::USD);
        for (id, rating) in [("AAA1", CreditRating::AAA), ("CCC1", CreditRating::CCC)] {
            let mut asset = PoolAsset::fixed_rate_bond(
                id,
                Money::from((500_000_i64, Currency::USD)),
                0.07,
                maturity(),
                DayCount::Thirty360,
            );
            asset.rating = Some(rating);
            pool.assets.push(asset);
        }
        pool
    }

    fn single_tranche() -> TrancheStructure {
        TrancheStructure::new(vec![
            Tranche::new(
                "A",
                0.0,
                100.0,
                TrancheSeniority::Senior,
                Money::from((500_000_i64, Currency::USD)),
                RateSpec::Fixed { rate: 0.05 },
                maturity(),
            )
            .expect("tranche"),
        ])
        .expect("structure")
    }

    /// Carry CCC collateral at 50% of par — the standard CLO convention.
    fn ccc_haircuts() -> indexmap::IndexMap<CreditRating, f64> {
        let mut m = indexmap::IndexMap::new();
        m.insert(CreditRating::CCC, 0.5);
        m
    }

    /// Senior fees must be deducted from the IC numerator.
    ///
    /// Fees rank AHEAD of every note, so cash spent on them is not available to
    /// cover note interest. Market convention is
    /// `(collections − senior fees) / interest due`. Using raw collections
    /// overstates IC and lets a deal pass a test it should fail.
    ///
    /// With no fee tier modelled the deduction is zero, which hid the defect.
    #[test]
    fn senior_fees_tighten_the_ic_test() {
        let pool = rated_pool();
        let tranches = single_tranche();
        let market = MarketContext::new();
        let as_of = Date::from_calendar_date(2025, Month::April, 1).expect("date");
        let period_start = Date::from_calendar_date(2025, Month::January, 1).expect("date");
        let mut balances = FxHashMap::default();
        balances.insert("A".to_string(), Money::from((500_000_i64, Currency::USD)));

        let ratio_with_fees = |fees: f64| {
            let ctx = TestContext {
                pool: &pool,
                tranches: &tranches,
                as_of,
                valuation_date: as_of,
                period_start: Some(period_start),
                cash_balance: Money::from((0_i64, Currency::USD)),
                interest_collections: Money::from((10_000_i64, Currency::USD)),
                rules: None,
                market: Some(&market),
                tranche_balances: Some(&balances),
                payable_principal_tranche_ids: None,
                asset_balances: None,
                live_collateral: None,
                current_pool_balance: Some(Money::from((400_000_i64, Currency::USD))),
                senior_fees: Money::new(fees, Currency::USD).expect("valid money fixture"),
                restricted_cash: Money::from((0_i64, Currency::USD)),
                defaulted_collateral_value: Money::from((0_i64, Currency::USD)),
                interest_claim_caps: &uncapped_claims(&["A"]),
                floating_rate_shift: 0.0,
                deferred_interest: None,
            };
            CoverageTestSpec::ic("A", 1.20)
                .evaluate(&ctx)
                .expect("ic test")
                .ratio
        };

        let without = ratio_with_fees(0.0);
        let with = ratio_with_fees(4_000.0);

        assert!(
            with < without,
            "senior fees must REDUCE the IC ratio: {with:.4} with a 4,000 fee \
             vs {without:.4} without. Equal values mean the numerator is still \
             raw collections."
        );
        // 10,000 collections less 4,000 of fees is 60% of the un-netted figure.
        assert!(
            (with / without - 0.6).abs() < 1e-9,
            "the ratio must fall exactly in proportion to the fee deduction: \
             expected 0.60x, got {:.4}x",
            with / without
        );
    }

    /// The IC cure must be a PRINCIPAL paydown, since that is how the
    /// diversion applies it.
    ///
    /// The old cure was the cash shortfall `R*I_due - I_coll`, which answers
    /// "how much extra interest cash would clear the test" — but the diversion
    /// pays down senior PRINCIPAL, which adds nothing to interest collections.
    /// Restoring IC by de-levering needs `X >= (I_due - I_coll/R) / (r*tau)`,
    /// which is larger by roughly `1/(r*tau)` — two orders of magnitude at a
    /// quarterly 5% coupon.
    #[test]
    fn ic_cure_is_a_delevering_paydown_not_a_cash_shortfall() {
        let pool = rated_pool();
        let market = MarketContext::new();
        let as_of = Date::from_calendar_date(2025, Month::April, 1).expect("date");
        let period_start = Date::from_calendar_date(2025, Month::January, 1).expect("date");

        // Single senior note, 5% coupon, quarterly.
        let tranches = single_tranche();
        let mut balances = FxHashMap::default();
        balances.insert("A".to_string(), Money::from((500_000_i64, Currency::USD)));

        let ctx = TestContext {
            pool: &pool,
            tranches: &tranches,
            as_of,
            valuation_date: as_of,
            period_start: Some(period_start),
            cash_balance: Money::from((0_i64, Currency::USD)),
            // Collections far below what the coupon demands => a hard breach.
            interest_collections: Money::from((100_i64, Currency::USD)),
            rules: None,
            market: Some(&market),
            tranche_balances: Some(&balances),
            payable_principal_tranche_ids: None,
            asset_balances: None,
            live_collateral: None,
            current_pool_balance: Some(Money::from((400_000_i64, Currency::USD))),
            senior_fees: Money::from((0_i64, Currency::USD)),
            restricted_cash: Money::from((0_i64, Currency::USD)),
            defaulted_collateral_value: Money::from((0_i64, Currency::USD)),
            interest_claim_caps: &uncapped_claims(&["A"]),
            floating_rate_shift: 0.0,
            deferred_interest: None,
        };

        let result = CoverageTestSpec::ic("A", 1.20)
            .evaluate(&ctx)
            .expect("ic test");
        assert!(!result.passing, "the IC test must breach");

        let cure = result
            .cure_amount
            .expect("a breach must produce a cure")
            .amount();

        // The cash shortfall for comparison: R*I_due - I_coll. With a 500k
        // balance at 5% over ~a quarter, I_due is on the order of 6k, so the
        // shortfall is a few thousand. De-levering must be far larger, because
        // it has to retire principal whose COUPON is the problem.
        let interest_due = 500_000.0 * 0.05 * 0.25;
        let cash_shortfall = 1.20 * interest_due - 100.0;
        assert!(
            cure > cash_shortfall * 10.0,
            "the de-levering cure {cure:.2} must be far larger than the cash \
             shortfall {cash_shortfall:.2} — they are different quantities, and \
             using the shortfall under-cures an IC breach by roughly 1/(r*tau) \
            "
        );

        // Sanity: paying down exactly `cure` of principal must clear the test.
        let remaining_balance = (500_000.0 - cure).max(0.0);
        let due_after = remaining_balance * 0.05 * 0.25;
        assert!(
            due_after <= 100.0 / 1.20 + 1e-6,
            "after paying down the cure the interest due {due_after:.4} must be \
             coverable by collections at the required ratio"
        );
    }

    #[test]
    fn coverage_economics_ic_cure_crosses_into_second_rate_piecewise() {
        let pool = rated_pool();
        let as_of = Date::from_calendar_date(2025, Month::April, 1).expect("date");
        let period_start = Date::from_calendar_date(2025, Month::January, 1).expect("date");
        let maturity = maturity();
        let senior = Tranche::new(
            "A",
            0.0,
            50.0,
            TrancheSeniority::Senior,
            Money::from((100_000_i64, Currency::USD)),
            RateSpec::Fixed { rate: 0.04 },
            maturity,
        )
        .expect("senior tranche");
        let test_tranche = Tranche::new(
            "B",
            50.0,
            100.0,
            TrancheSeniority::Subordinated,
            Money::from((100_000_i64, Currency::USD)),
            RateSpec::Fixed { rate: 0.16 },
            maturity,
        )
        .expect("test tranche");
        let tranches =
            TrancheStructure::new(vec![senior, test_tranche]).expect("tranche structure");
        let collections = 100.0;
        let trigger_level = 1.20;
        let ctx = TestContext {
            pool: &pool,
            tranches: &tranches,
            as_of,
            valuation_date: as_of,
            period_start: Some(period_start),
            cash_balance: Money::from((0_i64, Currency::USD)),
            interest_collections: Money::new(collections, Currency::USD)
                .expect("valid money fixture"),
            rules: None,
            market: None,
            tranche_balances: None,
            payable_principal_tranche_ids: None,
            asset_balances: None,
            live_collateral: None,
            current_pool_balance: None,
            senior_fees: Money::from((0_i64, Currency::USD)),
            restricted_cash: Money::from((0_i64, Currency::USD)),
            defaulted_collateral_value: Money::from((0_i64, Currency::USD)),
            interest_claim_caps: &uncapped_claims(&["B"]),
            floating_rate_shift: 0.0,
            deferred_interest: None,
        };

        let result = CoverageTestSpec::ic("B", trigger_level)
            .evaluate(&ctx)
            .expect("IC test");
        let actual = result.cure_amount.expect("breach cure").amount();
        let senior = &tranches.tranches[0];
        let junior = &tranches.tranches[1];
        let senior_tau = senior
            .day_count
            .year_fraction(
                period_start,
                as_of,
                finstack_quant_core::dates::DayCountContext::default(),
            )
            .expect("senior accrual");
        let junior_tau = junior
            .day_count
            .year_fraction(
                period_start,
                as_of,
                finstack_quant_core::dates::DayCountContext::default(),
            )
            .expect("junior accrual");
        let total_due = 100_000.0 * 0.04 * senior_tau + 100_000.0 * 0.16 * junior_tau;
        let target_due = collections / trigger_level;
        let reduction_after_senior = total_due - target_due - 100_000.0 * 0.04 * senior_tau;
        let expected = 100_000.0 + reduction_after_senior / (0.16 * junior_tau);

        assert!(
            (actual - expected).abs() < 1e-6,
            "cure must exhaust the 100k senior balance, then use the junior \
             tranche's 16% rate × accrual for the remaining reduction: \
             expected {expected:.6}, got {actual:.6}"
        );
    }

    #[test]
    fn coverage_economics_floating_cure_uses_denominator_market_rate() {
        let pool = rated_pool();
        let as_of = Date::from_calendar_date(2025, Month::April, 1).expect("date");
        let forward_curve_id = CurveId::new("USD-3M");
        let forward = ForwardCurve::builder(forward_curve_id.clone())
            .base_date(as_of)
            .day_count(DayCount::Act360)
            .index_tenor(Tenor::quarterly())
            .knots([(0.0, 1.0), (1.0, (-0.04_f64).exp())])
            .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .expect("forward curve");
        let fixing = ScalarTimeSeries::new(
            fixing_series_id("USD-3M"),
            vec![(as_of, 0.04)],
            None,
            finstack_quant_core::market_data::scalars::SeriesInterpolation::Step,
            finstack_quant_core::market_data::scalars::SeriesExtrapolation::Error,
        )
        .expect("fixing series");
        let market = MarketContext::new()
            .try_insert(forward)
            .expect("insert market curve")
            .insert_series(fixing);
        let floating_coupon = RateSpec::Floating(FloatingRateSpec {
            index_id: "USD-3M".into(),
            forward_curve_id,
            spread_bp: Decimal::from(100),
            gearing: Decimal::ONE,
            gearing_includes_spread: true,
            index_floor_bp: None,
            all_in_cap_bp: None,
            all_in_floor_bp: None,
            index_cap_bp: None,
            overnight_index_constraints: Default::default(),
            reset_frequency: Tenor::quarterly(),
            index_tenor: None,
            reset_lag_days: 0,
            fixing_calendar_id: None,
            compounding: None,
            overnight_basis: None,
            fallback: Default::default(),
        });
        let tranche = Tranche::new(
            "A",
            0.0,
            100.0,
            TrancheSeniority::Senior,
            Money::from((100_000_i64, Currency::USD)),
            floating_coupon,
            maturity(),
        )
        .expect("floating tranche");
        let tranches = TrancheStructure::new(vec![tranche]).expect("tranche structure");
        let collections = 100.0;
        let trigger_level = 1.20;
        let ctx = TestContext {
            pool: &pool,
            tranches: &tranches,
            as_of,
            valuation_date: as_of,
            period_start: None,
            cash_balance: Money::from((0_i64, Currency::USD)),
            interest_collections: Money::new(collections, Currency::USD)
                .expect("valid money fixture"),
            rules: None,
            market: Some(&market),
            tranche_balances: None,
            payable_principal_tranche_ids: None,
            asset_balances: None,
            live_collateral: None,
            current_pool_balance: None,
            senior_fees: Money::from((0_i64, Currency::USD)),
            restricted_cash: Money::from((0_i64, Currency::USD)),
            defaulted_collateral_value: Money::from((0_i64, Currency::USD)),
            interest_claim_caps: &uncapped_claims(&["A"]),
            floating_rate_shift: 0.0,
            deferred_interest: None,
        };

        let result = CoverageTestSpec::ic("A", trigger_level)
            .evaluate(&ctx)
            .expect("IC test");
        let actual = result.cure_amount.expect("breach cure").amount();
        let coupon_end = tranches.tranches[0]
            .frequency
            .add_to_date(
                as_of,
                None,
                finstack_quant_core::dates::BusinessDayConvention::Unadjusted,
            )
            .expect("next coupon boundary");
        let coupon_accrual = tranches.tranches[0]
            .day_count
            .year_fraction(
                as_of,
                coupon_end,
                finstack_quant_core::dates::DayCountContext::default(),
            )
            .expect("coupon accrual");
        let all_in_rate = tranches.tranches[0]
            .coupon
            .try_rate_for_period(as_of, coupon_end, coupon_accrual, as_of, &market)
            .expect("market-aware coupon");
        let tau = 1.0
            / (tranches.tranches[0].frequency)
                .payments_per_year()
                .expect("monthly tranche frequency");
        let total_due = 100_000.0 * all_in_rate * tau;
        let expected = (total_due - collections / trigger_level) / (all_in_rate * tau);

        assert!(
            (actual - expected).abs() < 1e-6,
            "floating cure must use the denominator's market-aware all-in rate: \
             expected {expected:.6}, got {actual:.6}"
        );

        let payment = Date::from_calendar_date(2025, Month::August, 1).unwrap();
        let start = Date::from_calendar_date(2025, Month::May, 1).unwrap();
        let projected = TestContext {
            as_of: payment,
            valuation_date: as_of,
            period_start: Some(start),
            ..ctx
        };
        let result = CoverageTestSpec::ic("A", trigger_level)
            .evaluate(&projected)
            .expect("future coverage dates must not require future fixings");
        let accrual = tranches.tranches[0]
            .day_count
            .year_fraction(
                start,
                payment,
                finstack_quant_core::dates::DayCountContext::default(),
            )
            .unwrap();
        let rate = tranches.tranches[0]
            .coupon
            .try_rate_for_period(start, payment, accrual, as_of, &market)
            .unwrap();
        assert!((result.ratio - collections / (100_000.0 * rate * accrual)).abs() < 1e-12);
        let expected_cure = 100_000.0 - collections / (trigger_level * rate * accrual);
        assert!((result.cure_amount.unwrap().amount() - expected_cure).abs() < 1e-6);

        let missing_fixing = TestContext {
            valuation_date: start,
            ..projected
        };
        let error = CoverageTestSpec::ic("A", trigger_level)
            .evaluate(&missing_fixing)
            .expect_err("a reset on the valuation date still requires its exact fixing");
        assert!(error.to_string().contains("2025-05-01"), "{error}");
    }

    /// A configured haircut must track the CURRENT pool balance, not
    /// freeze at the closing-date snapshot.
    ///
    /// `context.pool` is `&instrument.pool`, never mutated during simulation.
    /// Before this fix, setting any haircut bypassed the `current_pool_balance`
    /// override entirely, so the OC numerator stayed at its closing value while
    /// the denominator amortized — the ratio inflated monotonically and the
    /// test could never breach.
    #[test]
    fn haircut_applies_to_the_current_pool_balance() {
        let pool = rated_pool();
        let tranches = single_tranche();
        let market = MarketContext::new();
        let rules = CoverageRules {
            rating_haircuts: ccc_haircuts(),
            ..Default::default()
        };
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("date");

        // The pool has amortized from 1,000,000 to 400,000.
        let current = Money::from((400_000_i64, Currency::USD));
        let ctx = TestContext {
            pool: &pool,
            tranches: &tranches,
            as_of,
            valuation_date: as_of,
            period_start: None,
            cash_balance: Money::from((0_i64, Currency::USD)),
            interest_collections: Money::from((0_i64, Currency::USD)),
            rules: Some(&rules),
            market: Some(&market),
            tranche_balances: None,
            payable_principal_tranche_ids: None,
            asset_balances: None,
            live_collateral: None,
            current_pool_balance: Some(current),
            senior_fees: Money::from((0_i64, Currency::USD)),
            restricted_cash: Money::from((0_i64, Currency::USD)),
            defaulted_collateral_value: Money::from((0_i64, Currency::USD)),
            interest_claim_caps: &uncapped_claims(&["A"]),
            floating_rate_shift: 0.0,
            deferred_interest: None,
        };

        let result = CoverageTestSpec::oc("A", 1.0)
            .evaluate(&ctx)
            .expect("oc test");

        // Haircut factor: (500k + 0.5*500k) / 1,000k = 0.75.
        // Numerator must be 400k * 0.75 = 300k against a 500k tranche => 0.60.
        assert!(
            (result.ratio - 0.60).abs() < 1e-9,
            "the haircut must scale the CURRENT balance: expected 0.60 \
             (400k x 0.75 / 500k), got {}. A ratio near 1.5 means the numerator \
             is still the frozen closing pool.",
            result.ratio
        );
        assert!(
            !result.passing,
            "a 0.60 OC ratio against a 1.0 requirement must breach"
        );
    }

    #[test]
    fn coverage_economics_oc_haircuts_use_uneven_live_asset_balances() {
        let pool = rated_pool();
        let tranches = single_tranche();
        let rules = CoverageRules {
            rating_haircuts: ccc_haircuts(),
            ..Default::default()
        };
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("date");
        // Closing balances are 500k AAA / 500k CCC. Live balances have become
        // 100k AAA / 300k CCC, so a 50% CCC haircut produces 250k, not the
        // 300k implied by scaling the 400k aggregate with the stale 75% mix.
        let live_asset_balances = [100_000.0, 300_000.0];
        let ctx = TestContext {
            pool: &pool,
            tranches: &tranches,
            as_of,
            valuation_date: as_of,
            period_start: None,
            cash_balance: Money::from((0_i64, Currency::USD)),
            interest_collections: Money::from((0_i64, Currency::USD)),
            rules: Some(&rules),
            market: None,
            tranche_balances: None,
            payable_principal_tranche_ids: None,
            asset_balances: Some(&live_asset_balances),
            live_collateral: None,
            current_pool_balance: Some(Money::from((400_000_i64, Currency::USD))),
            senior_fees: Money::from((0_i64, Currency::USD)),
            restricted_cash: Money::from((0_i64, Currency::USD)),
            defaulted_collateral_value: Money::from((0_i64, Currency::USD)),
            interest_claim_caps: &uncapped_claims(&["A"]),
            floating_rate_shift: 0.0,
            deferred_interest: None,
        };

        let result = CoverageTestSpec::oc("A", 1.0)
            .evaluate(&ctx)
            .expect("OC test");
        assert!(
            (result.ratio - 0.50).abs() < 1e-9,
            "live per-asset haircut should be (100k + 50% × 300k) / 500k = \
             0.50, got {}",
            result.ratio
        );
    }

    /// Without haircuts the current-balance override is used directly,
    /// so this path is unchanged.
    #[test]
    fn absent_haircuts_use_the_current_balance_unscaled() {
        let pool = rated_pool();
        let tranches = single_tranche();
        let market = MarketContext::new();
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("date");

        let ctx = TestContext {
            pool: &pool,
            tranches: &tranches,
            as_of,
            valuation_date: as_of,
            period_start: None,
            cash_balance: Money::from((0_i64, Currency::USD)),
            interest_collections: Money::from((0_i64, Currency::USD)),
            rules: None,
            market: Some(&market),
            tranche_balances: None,
            payable_principal_tranche_ids: None,
            asset_balances: None,
            live_collateral: None,
            current_pool_balance: Some(Money::from((400_000_i64, Currency::USD))),
            senior_fees: Money::from((0_i64, Currency::USD)),
            restricted_cash: Money::from((0_i64, Currency::USD)),
            defaulted_collateral_value: Money::from((0_i64, Currency::USD)),
            interest_claim_caps: &uncapped_claims(&["A"]),
            floating_rate_shift: 0.0,
            deferred_interest: None,
        };

        let result = CoverageTestSpec::oc("A", 1.0)
            .evaluate(&ctx)
            .expect("oc test");
        assert!(
            (result.ratio - 0.80).abs() < 1e-9,
            "without haircuts the ratio is 400k / 500k = 0.80, got {}",
            result.ratio
        );
    }
}
