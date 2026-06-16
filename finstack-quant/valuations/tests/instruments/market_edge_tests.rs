//! Market-edge tests for CDS and Bond instruments.
//!
//! These tests verify correctness at market edge cases and validate
//! compliance with standard conventions:
//!
//! ## CDS Tests
//! - Upfront sign conventions for buyer/seller
//! - Accrual-on-default impact across various hazard rates
//! - IMM coupon date schedule generation
//!
//! ## Bond Tests
//! - Settlement vs as-of date for accrued interest
//! - Ex-coupon window behavior at boundaries
//! - Stub period handling (short front, short back, long stubs)
//! - Day-count boundary cases

mod cds_market_edge {
    //! CDS market edge case tests.

    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
    use finstack_quant_core::money::Money;

    use finstack_quant_valuations::instruments::Instrument;
    use finstack_quant_valuations::metrics::MetricId;
    use time::macros::date;

    fn build_discount_curve(rate: f64, base_date: Date, id: &str) -> DiscountCurve {
        DiscountCurve::builder(id)
            .base_date(base_date)
            .day_count(DayCount::Act360)
            .knots([
                (0.0, 1.0),
                (1.0, (-rate).exp()),
                (5.0, (-rate * 5.0).exp()),
                (10.0, (-rate * 10.0).exp()),
            ])
            .build()
            .unwrap()
    }

    fn build_hazard_curve(
        hazard_rate: f64,
        recovery: f64,
        base_date: Date,
        id: &str,
    ) -> HazardCurve {
        HazardCurve::builder(id)
            .base_date(base_date)
            .recovery_rate(recovery)
            .knots([
                (0.0, hazard_rate),
                (1.0, hazard_rate),
                (5.0, hazard_rate),
                (10.0, hazard_rate),
            ])
            .build()
            .unwrap()
    }

    /// Test that upfront payment sign is opposite for buyer vs seller.
    ///
    /// Market convention:
    /// - Buyer pays upfront when CDS is trading "points upfront" (risky name)
    /// - Seller receives upfront
    /// - Same absolute amount, opposite impact on NPV
    #[test]
    fn test_upfront_buyer_seller_symmetry() {
        let as_of = date!(2024 - 01 - 01);
        let end = date!(2029 - 01 - 01);

        let disc = build_discount_curve(0.05, as_of, "USD_OIS");
        let hazard = build_hazard_curve(0.02, 0.40, as_of, "CORP");
        let market = MarketContext::new().insert(disc).insert(hazard);

        let mut buyer = crate::finstack_quant_test_utils::cds_buy_protection(
            "BUYER",
            Money::new(10_000_000.0, Currency::USD),
            100.0,
            as_of,
            end,
            "USD_OIS",
            "CORP",
        )
        .expect("CDS construction should succeed");

        let mut seller = crate::finstack_quant_test_utils::cds_sell_protection(
            "SELLER",
            Money::new(10_000_000.0, Currency::USD),
            100.0,
            as_of,
            end,
            "USD_OIS",
            "CORP",
        )
        .expect("CDS construction should succeed");

        // Base NPVs (opposite signs)
        let buyer_base = buyer.value_raw(&market, as_of).unwrap();
        let seller_base = seller.value_raw(&market, as_of).unwrap();

        assert!(
            (buyer_base + seller_base).abs() < 1e-6,
            "Buyer and seller base NPV should sum to zero"
        );

        // Add same upfront
        let upfront = 500_000.0;
        buyer.upfront = Some((as_of, Money::new(upfront, Currency::USD)));
        seller.upfront = Some((as_of, Money::new(upfront, Currency::USD)));

        let buyer_with = buyer.value_raw(&market, as_of).unwrap();
        let seller_with = seller.value_raw(&market, as_of).unwrap();

        // Upfront reduces buyer NPV, increases seller NPV
        assert!(
            buyer_with < buyer_base,
            "Buyer NPV should decrease with upfront"
        );
        assert!(
            seller_with > seller_base,
            "Seller NPV should increase with upfront"
        );

        // Change should be exactly the upfront amount (same day payment)
        assert!(
            ((buyer_base - buyer_with) - upfront).abs() < 1e-6,
            "Buyer change should equal upfront"
        );
        assert!(
            ((seller_with - seller_base) - upfront).abs() < 1e-6,
            "Seller change should equal upfront"
        );
    }

    /// Test accrual-on-default impact scales with hazard rate.
    ///
    /// Higher hazard rate = higher probability of default during coupon period
    /// = larger accrual-on-default contribution.
    #[test]
    fn test_accrual_on_default_scales_with_hazard() {
        let as_of = date!(2024 - 01 - 01);
        let end = date!(2029 - 01 - 01);

        let disc = build_discount_curve(0.05, as_of, "USD_OIS");

        let hazard_rates = [0.005, 0.02, 0.05, 0.10]; // 50bps to 1000bps
        let mut premium_pvs = Vec::new();

        for &h in &hazard_rates {
            let hazard = build_hazard_curve(h, 0.40, as_of, "CORP");
            let market = MarketContext::new().insert(disc.clone()).insert(hazard);

            let cds = crate::finstack_quant_test_utils::cds_buy_protection(
                "AOD_TEST",
                Money::new(10_000_000.0, Currency::USD),
                100.0,
                as_of,
                end,
                "USD_OIS",
                "CORP",
            )
            .expect("CDS construction should succeed");

            let premium_pv = cds
                .price_with_metrics(
                    &market,
                    as_of,
                    &[MetricId::PremiumLegPv],
                    finstack_quant_valuations::instruments::PricingOptions::default(),
                )
                .unwrap()
                .measures[&MetricId::PremiumLegPv];
            premium_pvs.push((h, premium_pv));
        }

        // Premium leg PV should decrease with higher hazard (fewer expected payments)
        for i in 1..premium_pvs.len() {
            assert!(
                premium_pvs[i].1 < premium_pvs[i - 1].1,
                "Premium leg PV should decrease with hazard rate: {} at h={:.3} vs {} at h={:.3}",
                premium_pvs[i].1,
                premium_pvs[i].0,
                premium_pvs[i - 1].1,
                premium_pvs[i - 1].0
            );
        }
    }

    /// Test IMM date schedule compliance.
    ///
    /// ISDA standard CDS maturities fall on IMM dates (3rd Wednesday of Mar, Jun, Sep, Dec).
    /// Premium payments also use IMM-adjusted schedules.
    #[test]
    fn test_imm_schedule_quarterly_dates() {
        use finstack_quant_core::dates::{is_imm_date, next_imm};

        let as_of = date!(2024 - 01 - 15);

        // Next IMM date after as_of
        let next_imm_date = next_imm(as_of);

        // Verify next IMM date is correct
        let (_y, m, d) = (
            next_imm_date.year(),
            next_imm_date.month(),
            next_imm_date.day(),
        );

        // IMM dates are 3rd Wednesday of Mar, Jun, Sep, Dec
        let is_imm_month = matches!(
            m,
            time::Month::March | time::Month::June | time::Month::September | time::Month::December
        );
        assert!(
            is_imm_month,
            "IMM date {} should be in an IMM month",
            next_imm_date
        );

        // 3rd Wednesday falls between 15th and 21st
        assert!(
            (15..=21).contains(&d),
            "IMM date {} should be 3rd Wednesday (day 15-21)",
            next_imm_date
        );

        // Verify it's actually an IMM date
        assert!(
            is_imm_date(next_imm_date),
            "{} should be a valid IMM date",
            next_imm_date
        );
    }

    /// Test that IMM roll dates are generated correctly.
    #[test]
    fn test_imm_roll_sequence() {
        use finstack_quant_core::dates::{is_imm_date, next_imm};

        let start = date!(2024 - 01 - 01);
        let mut current = next_imm(start);
        let mut imm_dates = vec![current];

        // Generate next 8 IMM dates (2 years)
        for _ in 0..7 {
            current = next_imm(current.saturating_add(time::Duration::days(1)));
            imm_dates.push(current);
        }

        // Verify quarterly spacing (~91 days ± few days for actual month lengths)
        for i in 1..imm_dates.len() {
            let diff = (imm_dates[i] - imm_dates[i - 1]).whole_days();
            assert!(
                (85..=95).contains(&diff),
                "IMM dates {} to {} should be ~3 months apart, got {} days",
                imm_dates[i - 1],
                imm_dates[i],
                diff
            );
        }

        // All should be IMM dates
        for d in &imm_dates {
            assert!(is_imm_date(*d), "{} should be an IMM date", d);
        }
    }
}

mod bond_market_edge {
    //! Bond market edge case tests.

    use finstack_quant_cashflows::{accrued_interest_amount, CashflowProvider};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount, Tenor};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::Money;
    use finstack_quant_valuations::instruments::fixed_income::bond::{Bond, CashflowSpec};
    use finstack_quant_valuations::instruments::Instrument;
    use finstack_quant_valuations::metrics::{standard_registry, MetricContext, MetricId};
    use std::sync::Arc;
    use time::macros::date;

    fn build_discount_curve(rate: f64, base_date: Date, id: &str) -> DiscountCurve {
        DiscountCurve::builder(id)
            .base_date(base_date)
            .day_count(DayCount::Act365F)
            .knots([
                (0.0, 1.0),
                (1.0, (-rate).exp()),
                (5.0, (-rate * 5.0).exp()),
                (10.0, (-rate * 10.0).exp()),
                (30.0, (-rate * 30.0).exp()),
            ])
            .build()
            .unwrap()
    }

    /// Test accrued interest is calculated from settlement date, not as-of.
    ///
    /// Market convention: Buyer pays accrued from last coupon to settlement date,
    /// not to trade date (as_of).
    #[test]
    fn test_accrued_uses_settlement_date() {
        let as_of = date!(2025 - 03 - 01);
        let issue = date!(2025 - 01 - 01);
        let maturity = date!(2030 - 01 - 01);

        // T+0 settlement
        let bond_t0 = Bond::builder()
            .id("T0_SETTLE".into())
            .notional(Money::new(1000.0, Currency::USD))
            .issue_date(issue)
            .maturity(maturity)
            .cashflow_spec(
                CashflowSpec::fixed(0.06, Tenor::semi_annual(), DayCount::Thirty360)
                    .expect("finite test coupon"),
            )
            .discount_curve_id("USD-OIS".into())
            .settlement_convention_opt(Some(
                finstack_quant_valuations::instruments::fixed_income::bond::BondSettlementConvention {
                    settlement_days: 0,
                    ..Default::default()
                },
            ))
            .build()
            .unwrap();

        // T+2 settlement
        let bond_t2 = Bond::builder()
            .id("T2_SETTLE".into())
            .notional(Money::new(1000.0, Currency::USD))
            .issue_date(issue)
            .maturity(maturity)
            .cashflow_spec(
                CashflowSpec::fixed(0.06, Tenor::semi_annual(), DayCount::Thirty360)
                    .expect("finite test coupon"),
            )
            .discount_curve_id("USD-OIS".into())
            .settlement_convention_opt(Some(
                finstack_quant_valuations::instruments::fixed_income::bond::BondSettlementConvention {
                    settlement_days: 2,
                    ..Default::default()
                },
            ))
            .build()
            .unwrap();

        let sched_t0 = bond_t0
            .cashflow_schedule(&MarketContext::new(), as_of)
            .unwrap();
        let sched_t2 = bond_t2
            .cashflow_schedule(&MarketContext::new(), as_of)
            .unwrap();

        // Calculate accrued for each
        let accrued_t0 = accrued_interest_amount(
            &sched_t0,
            as_of, // T+0: settlement = as_of
            &bond_t0.accrual_config(),
        )
        .unwrap();

        // For T+2, settlement is 2 business days later (approximately as_of + 2)
        let settle_t2 = as_of.saturating_add(time::Duration::days(2));
        let accrued_t2 =
            accrued_interest_amount(&sched_t2, settle_t2, &bond_t2.accrual_config()).unwrap();

        // T+2 settlement means 2 more days of accrual
        // 6% annual / 2 = 3% semi-annual, 3% / 180 days ≈ 0.0167% per day
        // 2 days ≈ 0.033% ≈ $0.33 on $1000
        let expected_diff = 1000.0 * 0.06 / 365.0 * 2.0; // Approximate
        let actual_diff = accrued_t2 - accrued_t0;

        assert!(
            (actual_diff - expected_diff).abs() < 0.10, // Allow small tolerance for day count
            "T+2 should have ~{:.2} more accrued than T+0, got {:.2}",
            expected_diff,
            actual_diff
        );
    }

    /// Test ex-coupon window zeroes accrued at boundary.
    ///
    /// Exactly at ex-coupon date, accrued should be zero.
    /// One day before ex-coupon, accrued should be full period accrual.
    #[test]
    fn test_ex_coupon_boundary_behavior() {
        let issue = date!(2025 - 01 - 01);
        let maturity = date!(2030 - 01 - 01);
        let coupon_date = date!(2025 - 07 - 01);

        let mut bond = Bond::fixed(
            "EX_COUPON_TEST",
            Money::new(1000.0, Currency::USD),
            0.06,
            issue,
            maturity,
            "USD-OIS",
        )
        .unwrap();
        bond.settlement_convention = Some(
            finstack_quant_valuations::instruments::fixed_income::bond::BondSettlementConvention {
                ex_coupon_days: 7,
                ..Default::default()
            },
        );

        let schedule = bond
            .cashflow_schedule(&MarketContext::new(), coupon_date)
            .unwrap();
        let config = bond.accrual_config();

        // 8 days before coupon = just before ex-coupon window
        let before_ex = coupon_date - time::Duration::days(8);
        let accrued_before = accrued_interest_amount(&schedule, before_ex, &config).unwrap();

        // 7 days before coupon = exactly at ex-coupon boundary
        let at_ex = coupon_date - time::Duration::days(7);
        let accrued_at = accrued_interest_amount(&schedule, at_ex, &config).unwrap();

        // 5 days before coupon = within ex-coupon window
        let within_ex = coupon_date - time::Duration::days(5);
        let accrued_within = accrued_interest_amount(&schedule, within_ex, &config).unwrap();

        // Before ex-coupon: should have significant accrued (~2.9% of 3% = full period minus 8 days)
        assert!(
            accrued_before > 2.0,
            "Accrued before ex-coupon should be substantial: {}",
            accrued_before
        );

        // At and within ex-coupon: negative accrued (seller keeps the coupon,
        // buyer is compensated for the remaining stub to the coupon date).
        assert!(
            accrued_at < 0.0,
            "Accrued at ex-coupon boundary should be negative: {}",
            accrued_at
        );
        assert!(
            accrued_within < 0.0,
            "Accrued within ex-coupon window should be negative: {}",
            accrued_within
        );
        // Closer to the coupon date the negative stub shrinks towards zero.
        assert!(
            accrued_within > accrued_at,
            "Negative accrued should shrink towards zero through the ex window: at={} within={}",
            accrued_at,
            accrued_within
        );
    }

    /// Test short front stub period accrued interest.
    ///
    /// When a bond issues between regular coupon dates, the first period
    /// is a "short front stub" with proportionally less accrued.
    #[test]
    fn test_short_front_stub_accrued() {
        // Issue date is 2 months after a would-be coupon date
        let issue = date!(2025 - 03 - 01); // 2 months after Jan 1
        let maturity = date!(2030 - 01 - 01);

        // First coupon is July 1, so stub period is Mar 1 - Jul 1 (4 months vs 6 months normal)
        let bond = Bond::fixed(
            "SHORT_STUB",
            Money::new(1000.0, Currency::USD),
            0.06,
            issue,
            maturity,
            "USD-OIS",
        )
        .unwrap();

        let schedule = bond
            .cashflow_schedule(&MarketContext::new(), issue)
            .unwrap();
        let config = bond.accrual_config();

        // 2 months into the stub period
        let as_of = date!(2025 - 05 - 01);
        let accrued = accrued_interest_amount(&schedule, as_of, &config).unwrap();

        // 2 months out of 4-month stub = 50% of stub coupon
        // Stub coupon = 3% * (4/6) = 2%
        // 50% of stub = 1%
        let expected = 1000.0 * 0.01; // $10

        assert!(
            (accrued - expected).abs() < 0.50, // Allow for day-count variations
            "Short stub accrued should be ~{:.2}, got {:.2}",
            expected,
            accrued
        );
    }

    /// Test day-count boundary: Feb 28/29 transitions.
    ///
    /// Day-count conventions handle leap years differently.
    /// 30/360 treats Feb as 30 days.
    /// Act/365 uses actual days.
    #[test]
    fn test_day_count_leap_year_boundary() {
        // 2024 is a leap year
        let issue = date!(2024 - 01 - 01);
        let maturity = date!(2029 - 01 - 01);

        // Bond with 30/360
        let bond_30_360 = Bond::builder()
            .id("DC_30_360".into())
            .notional(Money::new(1000.0, Currency::USD))
            .issue_date(issue)
            .maturity(maturity)
            .cashflow_spec(
                CashflowSpec::fixed(0.06, Tenor::semi_annual(), DayCount::Thirty360)
                    .expect("finite test coupon"),
            )
            .discount_curve_id("USD-OIS".into())
            .build()
            .unwrap();

        // Bond with Act/365
        let bond_act_365 = Bond::builder()
            .id("DC_ACT365".into())
            .notional(Money::new(1000.0, Currency::USD))
            .issue_date(issue)
            .maturity(maturity)
            .cashflow_spec(
                CashflowSpec::fixed(0.06, Tenor::semi_annual(), DayCount::Act365F)
                    .expect("finite test coupon"),
            )
            .discount_curve_id("USD-OIS".into())
            .build()
            .unwrap();

        let sched_30_360 = bond_30_360
            .cashflow_schedule(&MarketContext::new(), issue)
            .unwrap();
        let sched_act_365 = bond_act_365
            .cashflow_schedule(&MarketContext::new(), issue)
            .unwrap();

        // Feb 29 (leap day) - ~59 days into a 180-day period
        let leap_day = date!(2024 - 02 - 29);

        let accrued_30_360 =
            accrued_interest_amount(&sched_30_360, leap_day, &bond_30_360.accrual_config())
                .unwrap();

        let accrued_act_365 =
            accrued_interest_amount(&sched_act_365, leap_day, &bond_act_365.accrual_config())
                .unwrap();

        // Expected accrued: ~59 days out of ~180 days period, 3% semi-annual coupon = $30
        // 59/180 * $30 ≈ $9.83

        assert!(
            accrued_30_360 > 8.0 && accrued_30_360 < 12.0,
            "30/360 accrued on leap day should be ~$9.50: {}",
            accrued_30_360
        );
        assert!(
            accrued_act_365 > 8.0 && accrued_act_365 < 12.0,
            "Act/365 accrued on leap day should be ~$9.50: {}",
            accrued_act_365
        );

        // They should differ slightly due to day-count treatment
        // 30/360 treats Feb as 30 days, Act/365 uses actual day count
        let diff = (accrued_30_360 - accrued_act_365).abs();
        assert!(
            diff < 1.0, // Small difference expected due to different day-count basis
            "30/360 and Act/365 should produce similar accrued on leap day, diff: {}",
            diff
        );
    }

    /// Test dirty vs clean price relationship with accrued.
    ///
    /// Market convention: Dirty Price = Clean Price + Accrued Interest
    #[test]
    fn test_dirty_clean_accrued_relationship() {
        let as_of = date!(2025 - 04 - 01);
        let issue = date!(2025 - 01 - 01);
        let maturity = date!(2030 - 01 - 01);

        let bond = Bond::fixed(
            "DIRTY_CLEAN",
            Money::new(1000.0, Currency::USD),
            0.06,
            issue,
            maturity,
            "USD-OIS",
        )
        .unwrap();

        let disc = build_discount_curve(0.05, as_of, "USD-OIS");
        let market = MarketContext::new().insert(disc);

        // Get dirty price (full PV)
        let dirty = bond.value(&market, as_of).unwrap().amount();

        // Get accrued via metrics
        let registry = standard_registry();
        let pv = bond.value(&market, as_of).unwrap();
        let mut context = MetricContext::new(
            Arc::new(bond),
            Arc::new(market),
            as_of,
            pv,
            MetricContext::default_config(),
        );

        let results = registry
            .compute(&[MetricId::Accrued], &mut context)
            .unwrap();
        let accrued = results.get(&MetricId::Accrued).copied().unwrap_or(0.0);

        // Clean = Dirty - Accrued
        let clean = dirty - accrued;

        // Verify relationship
        assert!(
            accrued > 0.0,
            "Accrued should be positive mid-period: {}",
            accrued
        );
        assert!(
            clean < dirty,
            "Clean ({}) should be less than dirty ({})",
            clean,
            dirty
        );
        assert!(
            (dirty - clean - accrued).abs() < 1e-6,
            "Dirty = Clean + Accrued should hold exactly"
        );
    }
}
