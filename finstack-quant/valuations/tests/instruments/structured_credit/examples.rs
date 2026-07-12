//! Usage examples for market-standard structured credit implementation.
//!
//! This module provides practical examples demonstrating:
//! - DealConfig usage for eliminating hardcoded values
//! - Proper spread tracking for WAS calculations
//! - Cashflow-based WAL calculations
//! - Rating factor consistency

#[cfg(test)]
mod tests {
    use finstack_quant_core::{
        currency::Currency,
        dates::{Date, Tenor},
        money::Money,
        types::{moodys_warf_factor, CreditRating},
    };
    use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
        AssetPool, DealConfig, DealDates, DealType, DefaultAssumptions, PoolAsset,
    };
    use time::Month;

    #[test]
    fn example_deal_config_usage() {
        // Example: Creating a CLO with market-standard configuration
        // instead of hardcoded values

        // Step 1: Define deal-specific dates
        let dates = DealDates::new(
            Date::from_calendar_date(2024, Month::March, 15).unwrap(), // Actual closing
            Date::from_calendar_date(2024, Month::June, 15).unwrap(),  // First payment
            Date::from_calendar_date(2031, Month::March, 15).unwrap(), // Maturity (7yr)
            Tenor::quarterly(),
        )
        .with_reinvestment_end(Date::from_calendar_date(2026, Month::March, 15).unwrap()); // 2yr reinvestment

        // Step 2: Get standard CLO configuration
        let config = DealConfig::clo_standard(dates, Currency::USD);

        // Step 3: Customize as needed for this specific deal
        let mut custom_config = config;
        custom_config.fees.trustee_fee_annual = Money::new(75_000.0, Currency::USD); // Higher fee
        custom_config.fees.senior_mgmt_fee_bps = 35.0; // 35bps instead of default 40bps
        custom_config.default_assumptions.base_cdr_annual = 0.025; // 2.5% CDR assumption

        // Add coverage test requirements
        custom_config
            .coverage_tests
            .add_oc_test("CLASS_A", 1.25)
            .expect("valid OC trigger");
        custom_config
            .coverage_tests
            .add_oc_test("CLASS_B", 1.15)
            .expect("valid OC trigger");
        custom_config
            .coverage_tests
            .add_ic_test("CLASS_A", 1.20)
            .expect("valid IC trigger");
        custom_config
            .coverage_tests
            .add_ic_test("CLASS_B", 1.10)
            .expect("valid IC trigger");

        // Verify configuration
        assert_eq!(
            custom_config.dates.closing_date,
            Date::from_calendar_date(2024, Month::March, 15).unwrap()
        );
        assert_eq!(custom_config.fees.trustee_fee_annual.amount(), 75_000.0);
        assert_eq!(custom_config.fees.senior_mgmt_fee_bps, 35.0);
        assert_eq!(custom_config.default_assumptions.base_recovery_rate, 0.40); // CLO standard

        // This config can now be used in instrument construction
        // (future enhancement: StructuredCredit::with_config(pool, tranches, custom_config))
    }

    #[test]
    fn example_creating_pool_with_spreads() {
        // Example: Creating a CLO pool with proper spread tracking for WAS calculation

        let maturity = Date::from_calendar_date(2030, Month::December, 31).unwrap();

        // Create floating rate loans with explicit spreads
        let loan1 = PoolAsset::floating_rate_loan(
            "LOAN001",
            Money::new(10_000_000.0, Currency::USD),
            "SOFR-3M",
            425.0, // SOFR + 425bps
            maturity,
            finstack_quant_core::dates::DayCount::Act360,
        )
        .with_rating(CreditRating::BB)
        .with_industry("Technology")
        .with_obligor("OBLIGOR001");

        let loan2 = PoolAsset::floating_rate_loan(
            "LOAN002",
            Money::new(15_000_000.0, Currency::USD),
            "SOFR-3M",
            475.0, // SOFR + 475bps
            maturity,
            finstack_quant_core::dates::DayCount::Act360,
        )
        .with_rating(CreditRating::B)
        .with_industry("Healthcare")
        .with_obligor("OBLIGOR002");

        // Create fixed rate bond (no separate spread)
        let bond1 = PoolAsset::fixed_rate_bond(
            "BOND001",
            Money::new(5_000_000.0, Currency::USD),
            0.09, // 9% fixed
            maturity,
            finstack_quant_core::dates::DayCount::Thirty360,
        )
        .with_rating(CreditRating::BB)
        .with_industry("Energy");

        // Build pool
        let mut pool = AssetPool::new("CLO_2024_1", DealType::CLO, Currency::USD);
        pool.assets.push(loan1);
        pool.assets.push(loan2);
        pool.assets.push(bond1);

        // Calculate WAS - spread component only, over assets that carry one.
        let was = pool.weighted_avg_spread();

        // Expected WAS calculation (market convention):
        // Loan1: 10M × 425bps = 4,250M·bps
        // Loan2: 15M × 475bps = 7,125M·bps
        // Bond1: fixed-rate with no explicit spread → EXCLUDED (no
        //        rate × 10⁴ fallback; the all-in coupon is not a spread)
        // Total: (4,250 + 7,125) / 25M = 11,375 / 25 = 455.0 bps

        assert!((was - 455.0).abs() < 0.01);

        // Verify individual spread access
        assert_eq!(pool.assets[0].spread_bps(), 425.0);
        assert_eq!(pool.assets[1].spread_bps(), 475.0);
        assert_eq!(pool.assets[2].spread_bps(), 900.0); // Accessor still falls back to rate
    }

    #[test]
    fn example_warf_calculation_with_shared_factors() {
        // Example: WARF calculation using shared rating factors

        let maturity = Date::from_calendar_date(2030, Month::December, 31).unwrap();

        let mut pool = AssetPool::new("CLO_WARF_DEMO", DealType::CLO, Currency::USD);

        // Add assets with various ratings
        pool.assets.push(
            PoolAsset::floating_rate_loan(
                "ASSET_AAA",
                Money::new(50_000_000.0, Currency::USD),
                "SOFR-3M",
                200.0,
                maturity,
                finstack_quant_core::dates::DayCount::Act360,
            )
            .with_rating(CreditRating::AAA),
        );

        pool.assets.push(
            PoolAsset::floating_rate_loan(
                "ASSET_A",
                Money::new(100_000_000.0, Currency::USD),
                "SOFR-3M",
                350.0,
                maturity,
                finstack_quant_core::dates::DayCount::Act360,
            )
            .with_rating(CreditRating::A),
        );

        pool.assets.push(
            PoolAsset::floating_rate_loan(
                "ASSET_BB",
                Money::new(150_000_000.0, Currency::USD),
                "SOFR-3M",
                450.0,
                maturity,
                finstack_quant_core::dates::DayCount::Act360,
            )
            .with_rating(CreditRating::BB),
        );

        pool.assets.push(
            PoolAsset::floating_rate_loan(
                "ASSET_B",
                Money::new(200_000_000.0, Currency::USD),
                "SOFR-3M",
                550.0,
                maturity,
                finstack_quant_core::dates::DayCount::Act360,
            )
            .with_rating(CreditRating::B),
        );

        // Calculate WARF using shared rating factors
        let mut weighted_sum: f64 = 0.0;
        let mut total_balance: f64 = 0.0;

        for asset in &pool.assets {
            let balance = asset.balance.amount();
            let rating_factor = asset
                .credit_quality
                .map(moodys_warf_factor)
                .transpose()
                .expect("rating factor lookup should succeed")
                .unwrap_or(3650.0);

            weighted_sum += balance * rating_factor;
            total_balance += balance;
        }

        let warf = weighted_sum / total_balance;

        // Expected WARF:
        // AAA: 50M × 1 = 50
        // A:   100M × 120 = 12,000
        // BB:  150M × 1,350 = 202,500
        // B:   200M × 2,720 = 544,000
        // Total: 758,550 / 500M = 1,517.1

        assert!((warf - 1517.1).abs() < 0.1);

        // Verify individual rating factors
        assert_eq!(moodys_warf_factor(CreditRating::AAA).unwrap(), 1.0);
        assert_eq!(moodys_warf_factor(CreditRating::A).unwrap(), 120.0);
        assert_eq!(moodys_warf_factor(CreditRating::BB).unwrap(), 1350.0);
        assert_eq!(moodys_warf_factor(CreditRating::B).unwrap(), 2720.0);
    }

    #[test]
    fn example_wal_from_cashflows() {
        // Example: Calculating true WAL from a cashflow schedule

        let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

        // Simulated principal cashflow schedule (Date, Amount)
        let cashflows = vec![
            (
                Date::from_calendar_date(2026, Month::January, 1).unwrap(),
                Money::new(20_000_000.0, Currency::USD),
            ), // 1 year
            (
                Date::from_calendar_date(2027, Month::January, 1).unwrap(),
                Money::new(30_000_000.0, Currency::USD),
            ), // 2 years
            (
                Date::from_calendar_date(2028, Month::January, 1).unwrap(),
                Money::new(30_000_000.0, Currency::USD),
            ), // 3 years
            (
                Date::from_calendar_date(2029, Month::January, 1).unwrap(),
                Money::new(20_000_000.0, Currency::USD),
            ), // 4 years
        ];

        let pool = AssetPool::new("DEMO_POOL", DealType::CLO, Currency::USD);

        // Calculate WAL using market-standard cashflow-based method
        let wal = pool
            .weighted_avg_life_from_cashflows(&cashflows, as_of)
            .expect("WAL");

        // Expected WAL:
        // (20M×1 + 30M×2 + 30M×3 + 20M×4) / 100M
        // = (20 + 60 + 90 + 80) / 100
        // = 250 / 100 = 2.5 years

        assert!((wal - 2.5).abs() < 0.01);

        // Compare to WAM (which would be much different if maturities vary)
        // This demonstrates why WAL ≠ WAM
    }

    #[test]
    fn example_auto_abs_with_updated_recovery() {
        // Example: Auto ABS using updated 45% recovery rate

        let assumptions = DefaultAssumptions::abs_auto_standard();

        // Verify we're using market-standard recovery rate
        assert_eq!(assumptions.base_recovery_rate, 0.45);
        assert_eq!(assumptions.abs_speed_monthly, Some(0.015));
        assert_eq!(assumptions.base_cdr_annual, 0.02);

        // Calculate expected loss on $10M defaults
        let default_amount = 10_000_000.0;
        let expected_recovery = default_amount * assumptions.base_recovery_rate;
        let expected_loss = default_amount - expected_recovery;

        assert_eq!(expected_recovery, 4_500_000.0); // $4.5M recovery
        assert_eq!(expected_loss, 5_500_000.0); // $5.5M loss

        // Compare to old (incorrect) 35% recovery:
        // Old recovery: $3.5M
        // Old loss: $6.5M
        // Difference: $1M ($2M total swing)
    }

    #[test]
    fn example_complete_clo_setup() {
        // Example: Complete CLO setup using all new market-standard features

        let maturity = Date::from_calendar_date(2031, Month::December, 15).unwrap();

        // 1. Create pool with proper spread tracking
        let mut pool = AssetPool::new("CLO_2024_1A", DealType::CLO, Currency::USD);

        // Add diversified loan portfolio
        pool.assets.push(
            PoolAsset::floating_rate_loan(
                "LOAN_TECH_001",
                Money::new(15_000_000.0, Currency::USD),
                "SOFR-3M",
                425.0,
                maturity,
                finstack_quant_core::dates::DayCount::Act360,
            )
            .with_rating(CreditRating::BB)
            .with_industry("Technology")
            .with_obligor("TECH_CORP_A"),
        );

        pool.assets.push(
            PoolAsset::floating_rate_loan(
                "LOAN_HEALTH_001",
                Money::new(20_000_000.0, Currency::USD),
                "SOFR-3M",
                450.0,
                maturity,
                finstack_quant_core::dates::DayCount::Act360,
            )
            .with_rating(CreditRating::B)
            .with_industry("Healthcare")
            .with_obligor("HEALTH_CORP_B"),
        );

        pool.assets.push(
            PoolAsset::floating_rate_loan(
                "LOAN_CONSUMER_001",
                Money::new(15_000_000.0, Currency::USD),
                "SOFR-3M",
                500.0,
                maturity,
                finstack_quant_core::dates::DayCount::Act360,
            )
            .with_rating(CreditRating::B)
            .with_industry("Consumer")
            .with_obligor("CONSUMER_CORP_C"),
        );

        // 2. Set up deal configuration
        let dates = DealDates::new(
            Date::from_calendar_date(2024, Month::March, 15).unwrap(),
            Date::from_calendar_date(2024, Month::June, 15).unwrap(),
            maturity,
            Tenor::quarterly(),
        );

        let mut config = DealConfig::clo_standard(dates, Currency::USD);

        // Customize for this specific deal
        config.default_assumptions.base_cdr_annual = 0.025; // Custom CDR assumption

        config
            .coverage_tests
            .add_oc_test("CLASS_A", 1.27)
            .expect("valid OC trigger"); // AA rated tranche
        config
            .coverage_tests
            .add_oc_test("CLASS_B", 1.17)
            .expect("valid OC trigger"); // A rated tranche
        config
            .coverage_tests
            .add_ic_test("CLASS_A", 1.22)
            .expect("valid IC trigger");
        config
            .coverage_tests
            .add_ic_test("CLASS_B", 1.12)
            .expect("valid IC trigger");

        // 3. Calculate pool metrics using market-standard methods

        // WAS - now correctly uses spread only
        let was = pool.weighted_avg_spread();
        // Expected: (15M×425 + 20M×450 + 15M×500) / 50M
        //         = (6,375 + 9,000 + 7,500) / 50
        //         = 22,875 / 50 = 457.5 bps
        assert!((was - 457.5).abs() < 0.01);

        // WARF - using shared rating factors
        let mut warf_sum: f64 = 0.0;
        let mut total_bal: f64 = 0.0;
        for asset in &pool.assets {
            let bal = asset.balance.amount();
            let factor = asset
                .credit_quality
                .map(moodys_warf_factor)
                .transpose()
                .expect("rating factor lookup should succeed")
                .unwrap_or(3650.0);
            warf_sum += bal * factor;
            total_bal += bal;
        }
        let warf = warf_sum / total_bal;

        // Expected: (15M×1350 + 20M×2720 + 15M×2720) / 50M
        //         = (20,250 + 54,400 + 40,800) / 50
        //         = 115,450 / 50 = 2,309
        assert!((warf - 2309.0).abs() < 0.1);

        // WAC - unchanged
        let _wac = pool.weighted_avg_coupon();
        // Would be calculated from all-in rates after index fixing

        // WAM (not WAL)
        let _wam =
            pool.weighted_avg_maturity(Date::from_calendar_date(2025, Month::January, 1).unwrap());

        // Verify configuration is ready for use
        assert!(config.coverage_tests.oc_triggers.contains_key("CLASS_A"));
        // Verify customized CDR assumption (was set to 2.5% above, default is 2%)
        assert_eq!(config.default_assumptions.base_cdr_annual, 0.025);
    }

    #[test]
    fn example_wal_vs_wam_difference() {
        // Example: Demonstrating the difference between WAL and WAM

        let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

        let pool = AssetPool::new("DEMO", DealType::RMBS, Currency::USD);

        // Asset with 5-year maturity but principal amortizes over time
        let amortizing_cashflows = vec![
            (
                Date::from_calendar_date(2026, Month::January, 1).unwrap(),
                Money::new(25_000_000.0, Currency::USD),
            ), // Year 1: 25%
            (
                Date::from_calendar_date(2027, Month::January, 1).unwrap(),
                Money::new(30_000_000.0, Currency::USD),
            ), // Year 2: 30%
            (
                Date::from_calendar_date(2028, Month::January, 1).unwrap(),
                Money::new(25_000_000.0, Currency::USD),
            ), // Year 3: 25%
            (
                Date::from_calendar_date(2029, Month::January, 1).unwrap(),
                Money::new(15_000_000.0, Currency::USD),
            ), // Year 4: 15%
            (
                Date::from_calendar_date(2030, Month::January, 1).unwrap(),
                Money::new(5_000_000.0, Currency::USD),
            ), // Year 5: 5%
        ];

        // Calculate true WAL from cashflows
        let wal = pool
            .weighted_avg_life_from_cashflows(&amortizing_cashflows, as_of)
            .expect("WAL");

        // Expected WAL:
        // (25M×1 + 30M×2 + 25M×3 + 15M×4 + 5M×5) / 100M
        // = (25 + 60 + 75 + 60 + 25) / 100
        // = 245 / 100 = 2.45 years

        assert!((wal - 2.45).abs() < 0.01);

        // WAM would be 5 years (maturity date) - very different!
        // This shows why WAL is critical for prepaying/amortizing assets
    }

    #[test]
    fn example_rmbs_deal_config() {
        // Example: RMBS with PSA/SDA assumptions

        let dates = DealDates::new(
            Date::from_calendar_date(2024, Month::June, 1).unwrap(),
            Date::from_calendar_date(2024, Month::July, 1).unwrap(),
            Date::from_calendar_date(2054, Month::June, 1).unwrap(), // 30-year
            Tenor::monthly(),
        );

        let config = DealConfig::rmbs_standard(dates, Currency::USD);

        // Verify RMBS-specific assumptions
        assert_eq!(config.default_assumptions.psa_speed, Some(1.0)); // 100% PSA
        assert_eq!(config.default_assumptions.sda_speed, Some(1.0)); // 100% SDA
        assert_eq!(config.default_assumptions.base_recovery_rate, 0.60); // 60% mortgage recovery
        assert_eq!(config.default_assumptions.base_cpr_annual, 0.06); // 6% CPR at 100% PSA

        // Monthly payment frequency for RMBS
        assert_eq!(config.dates.frequency.months(), Some(1));

        // Lower servicing fees than CLO
        assert_eq!(config.fees.servicing_fee_bps, 25.0); // 25bps vs 50bps for ABS
    }

    #[test]
    fn example_abs_deal_config() {
        // Example: Auto ABS with updated recovery rate

        let dates = DealDates::new(
            Date::from_calendar_date(2024, Month::September, 1).unwrap(),
            Date::from_calendar_date(2024, Month::October, 1).unwrap(),
            Date::from_calendar_date(2029, Month::September, 1).unwrap(), // 5-year
            Tenor::monthly(),
        );

        let config = DealConfig::abs_standard(dates, Currency::USD);

        // Verify updated auto recovery rate
        assert_eq!(config.default_assumptions.base_recovery_rate, 0.45); // Updated!
        assert_eq!(config.default_assumptions.abs_speed_monthly, Some(0.015)); // 1.5% ABS

        // ABS has higher servicing fees than RMBS
        assert_eq!(config.fees.servicing_fee_bps, 50.0); // 50bps

        // No management fees (unlike CLO)
        assert_eq!(config.fees.senior_mgmt_fee_bps, 0.0);
    }

    #[test]
    fn example_cmbs_deal_config() {
        // Example: CMBS commercial mortgage setup

        let dates = DealDates::new(
            Date::from_calendar_date(2024, Month::November, 1).unwrap(),
            Date::from_calendar_date(2024, Month::December, 1).unwrap(),
            Date::from_calendar_date(2034, Month::November, 1).unwrap(), // 10-year
            Tenor::monthly(),
        );

        let config = DealConfig::cmbs_standard(dates, Currency::USD);

        // Verify CMBS-specific settings
        assert_eq!(config.fees.master_servicer_fee_bps, Some(25.0)); // Master servicer
        assert_eq!(config.fees.special_servicer_fee_bps, Some(25.0)); // Special servicer
        assert_eq!(config.default_assumptions.base_recovery_rate, 0.65); // Higher CRE recovery

        // Commercial properties have lower default rates
        assert_eq!(config.default_assumptions.base_cdr_annual, 0.005); // 0.5% CDR
    }
}
