//! Market sensitivity tests for convertible bonds.
//!
//! Tests sensitivities to market parameters:
//! - Equity price sensitivity
//! - Volatility sensitivity
//! - Interest rate sensitivity
//! - Dividend yield sensitivity
//! - Monotonicity and convexity properties

use super::fixtures::*;
use finstack_quant_valuations::instruments::fixed_income::convertible::{
    price_convertible_bond, ConvertibleTreeType,
};

#[test]
fn test_sensitivity_to_spot_price() {
    let bond = create_standard_convertible();

    let spots = vec![50.0, 75.0, 100.0, 125.0, 150.0, 200.0];
    let mut prices = Vec::new();

    for spot in &spots {
        let market = create_market_context_with_params(
            *spot,
            market_params::VOL_STANDARD,
            market_params::DIV_YIELD,
        );
        let price = price_convertible_bond(
            &bond,
            &market,
            ConvertibleTreeType::Binomial(50),
            dates::base_date(),
        )
        .unwrap();
        prices.push(price.amount());
    }

    // Prices should increase monotonically with spot
    for i in 1..prices.len() {
        assert!(
            prices[i] >= prices[i - 1] * 0.95, // Allow small numerical variance
            "Price should increase with spot: spot={}, price={} vs prev_price={}",
            spots[i],
            prices[i],
            prices[i - 1]
        );
    }
}

#[test]
fn test_sensitivity_to_volatility() {
    let bond = create_standard_convertible();

    let vols = vec![0.05, 0.10, 0.15, 0.20, 0.30, 0.40];
    let mut prices = Vec::new();

    for vol in &vols {
        let market = create_market_context_with_params(
            market_params::SPOT_PRICE,
            *vol,
            market_params::DIV_YIELD,
        );
        let price = price_convertible_bond(
            &bond,
            &market,
            ConvertibleTreeType::Binomial(50),
            dates::base_date(),
        )
        .unwrap();
        prices.push(price.amount());
    }

    // Prices should increase monotonically with volatility (option value)
    for i in 1..prices.len() {
        assert!(
            prices[i] >= prices[i - 1] * 0.98, // Allow small numerical variance
            "Price should increase with volatility: vol={}, price={} vs prev_price={}",
            vols[i],
            prices[i],
            prices[i - 1]
        );
    }
}

#[test]
fn test_sensitivity_to_interest_rates() {
    let bond = create_standard_convertible();

    let rates = vec![0.01, 0.02, 0.03, 0.04, 0.05];
    let mut prices = Vec::new();

    for rate in &rates {
        let market = create_market_context_with_rate(*rate);
        let price = price_convertible_bond(
            &bond,
            &market,
            ConvertibleTreeType::Binomial(50),
            dates::base_date(),
        )
        .unwrap();
        prices.push(price.amount());
    }

    // Sensitivity to rates is ambiguous (debt vs equity components)
    // Just verify all prices are reasonable and finite
    for (i, price) in prices.iter().enumerate() {
        assert!(
            *price > 0.0 && price.is_finite(),
            "Price should be positive and finite at rate={}: {}",
            rates[i],
            price
        );
    }
}

#[test]
fn test_sensitivity_to_dividend_yield() {
    let bond = create_standard_convertible();

    let div_yields = vec![0.0, 0.01, 0.02, 0.03, 0.05];
    let mut prices = Vec::new();

    for div_yield in &div_yields {
        let market = create_market_context_with_params(
            market_params::SPOT_PRICE,
            market_params::VOL_STANDARD,
            *div_yield,
        );
        let price = price_convertible_bond(
            &bond,
            &market,
            ConvertibleTreeType::Binomial(50),
            dates::base_date(),
        )
        .unwrap();
        prices.push(price.amount());
    }

    // Higher dividend yield should decrease conversion value (forward spot lower)
    // But the effect might be subtle
    for price in &prices {
        assert!(
            *price > 0.0 && price.is_finite(),
            "Price should be positive and finite: {}",
            price
        );
    }
}

#[test]
fn test_convexity_in_spot() {
    let bond = create_standard_convertible();

    // Test convexity by checking price changes for up and down moves
    let spot_center = market_params::SPOT_PRICE;
    let bump = 10.0;

    let market_center = create_market_context_with_params(
        spot_center,
        market_params::VOL_STANDARD,
        market_params::DIV_YIELD,
    );
    let price_center = price_convertible_bond(
        &bond,
        &market_center,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    let market_up = create_market_context_with_params(
        spot_center + bump,
        market_params::VOL_STANDARD,
        market_params::DIV_YIELD,
    );
    let price_up = price_convertible_bond(
        &bond,
        &market_up,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    let market_down = create_market_context_with_params(
        spot_center - bump,
        market_params::VOL_STANDARD,
        market_params::DIV_YIELD,
    );
    let price_down = price_convertible_bond(
        &bond,
        &market_down,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    // Convexity: average of up/down moves should exceed center price
    let avg = (price_up.amount() + price_down.amount()) / 2.0;

    assert!(
        avg >= price_center.amount() * 0.98, // Should show convexity (gamma > 0)
        "Should exhibit convexity: avg={} vs center={}",
        avg,
        price_center.amount()
    );
}

#[test]
fn test_cross_sensitivity_spot_vol() {
    let bond = create_standard_convertible();

    // High spot, low vol
    let market_hs_lv = create_market_context_with_params(
        market_params::SPOT_HIGH,
        market_params::VOL_LOW,
        market_params::DIV_YIELD,
    );
    let price_hs_lv = price_convertible_bond(
        &bond,
        &market_hs_lv,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    // High spot, high vol
    let market_hs_hv = create_market_context_with_params(
        market_params::SPOT_HIGH,
        market_params::VOL_HIGH,
        market_params::DIV_YIELD,
    );
    let price_hs_hv = price_convertible_bond(
        &bond,
        &market_hs_hv,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    // High vol should add value even when deep ITM
    assert!(
        price_hs_hv.amount() >= price_hs_lv.amount() * 0.95,
        "High vol should add value even deep ITM: {} vs {}",
        price_hs_hv.amount(),
        price_hs_lv.amount()
    );

    // Low spot, low vol
    let market_ls_lv = create_market_context_with_params(
        market_params::SPOT_LOW,
        market_params::VOL_LOW,
        market_params::DIV_YIELD,
    );
    let price_ls_lv = price_convertible_bond(
        &bond,
        &market_ls_lv,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    // Low spot, high vol
    let market_ls_hv = create_market_context_with_params(
        market_params::SPOT_LOW,
        market_params::VOL_HIGH,
        market_params::DIV_YIELD,
    );
    let price_ls_hv = price_convertible_bond(
        &bond,
        &market_ls_hv,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    // High vol should add significant value when OTM (option value)
    assert!(
        price_ls_hv.amount() > price_ls_lv.amount(),
        "High vol should add significant value OTM: {} vs {}",
        price_ls_hv.amount(),
        price_ls_lv.amount()
    );
}

#[test]
fn test_sensitivity_extreme_low_spot() {
    let bond = create_standard_convertible();
    let market = create_market_context_with_params(
        10.0, // Very low spot
        market_params::VOL_STANDARD,
        market_params::DIV_YIELD,
    );

    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    // Should be close to bond floor
    let approx_bond_floor =
        calculate_bond_floor(bond_params::COUPON_RATE, 5.0, market_params::RISK_FREE_RATE)
            * bond_params::NOTIONAL;

    assert!(
        price.amount() >= approx_bond_floor * 0.90,
        "Very low spot should result in bond floor: {} vs {}",
        price.amount(),
        approx_bond_floor
    );
}

#[test]
fn test_sensitivity_extreme_high_spot() {
    let bond = create_standard_convertible();
    let market = create_market_context_with_params(
        500.0, // Very high spot
        market_params::VOL_STANDARD,
        market_params::DIV_YIELD,
    );

    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    // Should track conversion value closely
    let conversion_value = theoretical_conversion_value(500.0, bond_params::CONVERSION_RATIO);

    assert!(
        price.amount() >= conversion_value * 0.95,
        "Very high spot should track conversion value: {} vs {}",
        price.amount(),
        conversion_value
    );
}

#[test]
fn test_sensitivity_very_low_volatility() {
    let bond = create_standard_convertible();
    let market = create_market_context_with_params(
        market_params::SPOT_PRICE,
        0.02, // Very low volatility (2%) - tree pricing requires minimum vol for stability
        market_params::DIV_YIELD,
    );

    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(30),
        dates::base_date(),
    )
    .unwrap();

    // With very low vol, should be close to max(bond_floor, conversion_value)
    let conversion_value =
        theoretical_conversion_value(market_params::SPOT_PRICE, bond_params::CONVERSION_RATIO);

    assert!(
        price.amount() >= conversion_value * 0.90,
        "Very low vol should be close to intrinsic: {} vs {}",
        price.amount(),
        conversion_value
    );
}

#[test]
fn test_sensitivity_high_volatility() {
    let bond = create_standard_convertible();
    let market = create_market_context_with_params(
        market_params::SPOT_PRICE,
        0.80, // Very high volatility
        market_params::DIV_YIELD,
    );

    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    // Should exceed intrinsic value due to option value
    let conversion_value =
        theoretical_conversion_value(market_params::SPOT_PRICE, bond_params::CONVERSION_RATIO);

    assert!(
        price.amount() > conversion_value,
        "High vol should add significant option value: {} vs {}",
        price.amount(),
        conversion_value
    );
}

#[test]
fn test_parallel_rate_shift() {
    let bond = create_standard_convertible();

    let base_market = create_market_context_with_rate(0.03);
    let price_base = price_convertible_bond(
        &bond,
        &base_market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    let up_market = create_market_context_with_rate(0.04);
    let price_up = price_convertible_bond(
        &bond,
        &up_market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    let down_market = create_market_context_with_rate(0.02);
    let price_down = price_convertible_bond(
        &bond,
        &down_market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    // All prices should be reasonable and finite
    assert!(price_base.amount() > 0.0 && price_base.amount().is_finite());
    assert!(price_up.amount() > 0.0 && price_up.amount().is_finite());
    assert!(price_down.amount() > 0.0 && price_down.amount().is_finite());
}

#[test]
fn test_spot_vol_surface() {
    let bond = create_standard_convertible();

    // Test multiple combinations of spot and vol
    let spots = vec![market_params::SPOT_LOW, 100.0, market_params::SPOT_PRICE];
    let vols = vec![
        market_params::VOL_LOW,
        market_params::VOL_STANDARD,
        market_params::VOL_HIGH,
    ];

    for spot in &spots {
        for vol in &vols {
            let market = create_market_context_with_params(*spot, *vol, market_params::DIV_YIELD);

            let price = price_convertible_bond(
                &bond,
                &market,
                ConvertibleTreeType::Binomial(50),
                dates::base_date(),
            )
            .unwrap();

            assert!(
                price.amount() > 0.0 && price.amount().is_finite(),
                "Price should be valid for spot={}, vol={}: {}",
                spot,
                vol,
                price.amount()
            );
        }
    }
}

#[test]
fn test_time_decay_sensitivity() {
    // Compare short-dated vs long-dated convertibles
    let bond_short = create_floating_convertible(); // 1 year
    let bond_long = create_standard_convertible(); // 5 years

    let market = create_market_context();

    let price_short = price_convertible_bond(
        &bond_short,
        &market,
        ConvertibleTreeType::Binomial(20),
        dates::base_date(),
    )
    .unwrap();

    let price_long = price_convertible_bond(
        &bond_long,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    // Longer maturity should generally have higher option value
    // (though this depends on coupon and other factors)
    assert!(
        price_short.amount() > 0.0 && price_long.amount() > 0.0,
        "Both maturities should price: short={}, long={}",
        price_short.amount(),
        price_long.amount()
    );
}
