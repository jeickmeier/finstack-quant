//! Usage examples for market-standard structured credit implementation.
//!
//! This module provides practical examples demonstrating:
//! - Proper spread tracking for WAS calculations
//! - Cashflow-based WAL calculations
//! - Rating factor consistency

#[cfg(test)]
mod tests {
    use finstack_quant_core::{currency::Currency, dates::Date, money::Money, types::CreditRating};
    use finstack_quant_models::credit::moodys_warf_factor;
    use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
        AssetPool, DealType, PoolAsset,
    };
    use time::Month;

    #[test]
    fn example_creating_pool_with_spreads() {
        // Example: Creating a CLO pool with proper spread tracking for WAS calculation

        let maturity = Date::from_calendar_date(2030, Month::December, 31).unwrap();

        // Create floating rate loans with explicit spreads
        let loan1 = PoolAsset::floating_rate_loan(
            "LOAN001",
            Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"),
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
            Money::new(15_000_000.0, Currency::USD).expect("valid money fixture"),
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
            Money::new(5_000_000.0, Currency::USD).expect("valid money fixture"),
            0.09, // 9% fixed
            maturity,
            finstack_quant_core::dates::DayCount::Thirty360,
        )
        .with_rating(CreditRating::BB)
        .with_industry("Energy");

        // Build pool
        let mut pool = AssetPool::new("CLO_2024_1", DealType::Clo, Currency::USD);
        pool.assets.push(loan1);
        pool.assets.push(loan2);
        pool.assets.push(bond1);

        // Calculate WAS - spread component only, over assets that carry one.
        let was = pool.weighted_avg_spread();

        // Expected WAS calculation (market convention):
        // Loan1: 10M × 425bps = 4,250M·bp
        // Loan2: 15M × 475bps = 7,125M·bp
        // Bond1: fixed-rate with no explicit spread → EXCLUDED (no
        //        rate × 10⁴ fallback; the all-in coupon is not a spread)
        // Total: (4,250 + 7,125) / 25M = 11,375 / 25 = 455.0 bp

        assert!((was - 455.0).abs() < 0.01);

        // Verify individual spread access
        assert_eq!(pool.assets[0].spread_bp(), 425.0);
        assert_eq!(pool.assets[1].spread_bp(), 475.0);
        assert_eq!(pool.assets[2].spread_bp(), 900.0); // Accessor still falls back to rate
    }

    #[test]
    fn example_warf_calculation_with_shared_factors() {
        // Example: WARF calculation using shared rating factors

        let maturity = Date::from_calendar_date(2030, Month::December, 31).unwrap();

        let mut pool = AssetPool::new("CLO_WARF_DEMO", DealType::Clo, Currency::USD);

        // Add assets with various ratings
        pool.assets.push(
            PoolAsset::floating_rate_loan(
                "ASSET_AAA",
                Money::new(50_000_000.0, Currency::USD).expect("valid money fixture"),
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
                Money::new(100_000_000.0, Currency::USD).expect("valid money fixture"),
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
                Money::new(150_000_000.0, Currency::USD).expect("valid money fixture"),
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
                Money::new(200_000_000.0, Currency::USD).expect("valid money fixture"),
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
                Money::new(20_000_000.0, Currency::USD).expect("valid money fixture"),
            ), // 1 year
            (
                Date::from_calendar_date(2027, Month::January, 1).unwrap(),
                Money::new(30_000_000.0, Currency::USD).expect("valid money fixture"),
            ), // 2 years
            (
                Date::from_calendar_date(2028, Month::January, 1).unwrap(),
                Money::new(30_000_000.0, Currency::USD).expect("valid money fixture"),
            ), // 3 years
            (
                Date::from_calendar_date(2029, Month::January, 1).unwrap(),
                Money::new(20_000_000.0, Currency::USD).expect("valid money fixture"),
            ), // 4 years
        ];

        let pool = AssetPool::new("DEMO_POOL", DealType::Clo, Currency::USD);

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
    fn example_complete_clo_setup() {
        // Example: Complete CLO setup using all new market-standard features

        let maturity = Date::from_calendar_date(2031, Month::December, 15).unwrap();

        // 1. Create pool with proper spread tracking
        let mut pool = AssetPool::new("CLO_2024_1A", DealType::Clo, Currency::USD);

        // Add diversified loan portfolio
        pool.assets.push(
            PoolAsset::floating_rate_loan(
                "LOAN_TECH_001",
                Money::new(15_000_000.0, Currency::USD).expect("valid money fixture"),
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
                Money::new(20_000_000.0, Currency::USD).expect("valid money fixture"),
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
                Money::new(15_000_000.0, Currency::USD).expect("valid money fixture"),
                "SOFR-3M",
                500.0,
                maturity,
                finstack_quant_core::dates::DayCount::Act360,
            )
            .with_rating(CreditRating::B)
            .with_industry("Consumer")
            .with_obligor("CONSUMER_CORP_C"),
        );

        // 3. Calculate pool metrics using market-standard methods

        // WAS - now correctly uses spread only
        let was = pool.weighted_avg_spread();
        // Expected: (15M×425 + 20M×450 + 15M×500) / 50M
        //         = (6,375 + 9,000 + 7,500) / 50
        //         = 22,875 / 50 = 457.5 bp
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
        let _wam = pool
            .weighted_avg_maturity(Date::from_calendar_date(2025, Month::January, 1).unwrap())
            .expect("wam");
    }

    #[test]
    fn example_wal_vs_wam_difference() {
        // Example: Demonstrating the difference between WAL and WAM

        let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

        let pool = AssetPool::new("DEMO", DealType::Rmbs, Currency::USD);

        // Asset with 5-year maturity but principal amortizes over time
        let amortizing_cashflows = vec![
            (
                Date::from_calendar_date(2026, Month::January, 1).unwrap(),
                Money::new(25_000_000.0, Currency::USD).expect("valid money fixture"),
            ), // Year 1: 25%
            (
                Date::from_calendar_date(2027, Month::January, 1).unwrap(),
                Money::new(30_000_000.0, Currency::USD).expect("valid money fixture"),
            ), // Year 2: 30%
            (
                Date::from_calendar_date(2028, Month::January, 1).unwrap(),
                Money::new(25_000_000.0, Currency::USD).expect("valid money fixture"),
            ), // Year 3: 25%
            (
                Date::from_calendar_date(2029, Month::January, 1).unwrap(),
                Money::new(15_000_000.0, Currency::USD).expect("valid money fixture"),
            ), // Year 4: 15%
            (
                Date::from_calendar_date(2030, Month::January, 1).unwrap(),
                Money::new(5_000_000.0, Currency::USD).expect("valid money fixture"),
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
}
