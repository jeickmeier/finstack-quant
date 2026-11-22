use super::*;
use crate::structured_credit::AssetType;

mod pool_tests {
    use super::*;
    use finstack_quant_core::currency::Currency;

    #[test]
    fn test_pool_creation() {
        let pool = AssetPool::new("TEST_POOL", DealType::Clo, Currency::USD);
        assert_eq!(pool.id.as_str(), "TEST_POOL");
        assert_eq!(pool.deal_type, DealType::Clo);
        assert_eq!(pool.get_currency(), Currency::USD);
    }

    #[test]
    fn reset_lag_is_required_exactly_on_floating_rows() {
        use finstack_quant_core::dates::DayCount;
        let closing = Date::from_calendar_date(2025, time::Month::January, 1).expect("closing");
        let maturity = Date::from_calendar_date(2030, time::Month::January, 1).expect("maturity");
        let balance = Money::from((100_i64, Currency::USD));
        let validate = |asset: PoolAsset| {
            let mut pool = AssetPool::new("POOL", DealType::Clo, Currency::USD);
            pool.assets.push(asset);
            pool.normalized_view(closing).map(|_| ())
        };
        let floating = PoolAsset::floating_rate_loan(
            "L",
            balance,
            "SOFR-3M",
            300.0,
            maturity,
            DayCount::Act360,
            2,
        );
        let fixed = PoolAsset::fixed_rate_bond("B", balance, 0.06, maturity, DayCount::Thirty360);

        assert!(validate(floating.clone()).is_ok());
        assert!(validate(fixed.clone()).is_ok());
        let error = validate(PoolAsset {
            reset_lag_days: None,
            ..floating.clone()
        })
        .expect_err("floating row without a lag");
        assert!(error.to_string().contains("requires reset_lag_days"));
        let error = validate(PoolAsset {
            reset_lag_days: Some(-1),
            ..floating
        })
        .expect_err("negative lag");
        assert!(error.to_string().contains("negative reset_lag_days"));
        let error = validate(PoolAsset {
            reset_lag_days: Some(2),
            ..fixed
        })
        .expect_err("fixed row with a lag");
        assert!(error.to_string().contains("must not set reset_lag_days"));

        let mut line = RepLine::new(
            "RL",
            balance,
            0.03,
            maturity,
            DayCount::Act360,
            AssetType::FirstLienLoan {},
        );
        line.spread_bp = Some(300.0);
        line.forward_curve_id = Some("SOFR-3M".into());
        let mut pool = AssetPool::new("POOL", DealType::Clo, Currency::USD);
        pool.rep_lines = Some(vec![line.clone()]);
        let error = pool
            .normalized(closing)
            .expect_err("floating rep line without a lag");
        assert!(error.to_string().contains("requires reset_lag_days"));
        line.reset_lag_days = Some(2);
        pool.rep_lines = Some(vec![line]);
        let expanded = pool.normalized(closing).expect("lagged rep line expands");
        assert_eq!(expanded.assets[0].reset_lag_days, Some(2));
    }
}

#[cfg(test)]
mod market_standards_tests {
    use super::*;
    use finstack_quant_core::dates::DayCount;

    #[test]
    fn test_wam_mixed_day_counts() {
        let as_of = Date::from_calendar_date(2025, time::Month::January, 1).expect("Valid date");
        let maturity = Date::from_calendar_date(2026, time::Month::January, 1).expect("Valid date");

        // Asset A: Act365F (Standard) -> 1.0 years
        let asset_a = PoolAsset::fixed_rate_bond(
            "A",
            Money::from((100_i64, Currency::USD)),
            0.05,
            maturity,
            DayCount::Act365F,
        );

        // Asset B: Thirty360 -> 1.0 years (360/360)
        let asset_b = PoolAsset::fixed_rate_bond(
            "B",
            Money::from((100_i64, Currency::USD)),
            0.05,
            maturity,
            DayCount::Thirty360,
        );

        let mut pool = AssetPool::new("POOL", DealType::Abs, Currency::USD);
        pool.assets.push(asset_a);
        pool.assets.push(asset_b);

        let wam = pool
            .weighted_avg_maturity(as_of)
            .expect("wam")
            .expect("balance");

        // Both should be exactly 1.0
        assert!((wam - 1.0).abs() < 1e-10);
    }
}
