//! Conversion policy tests for convertible bonds.
//!
//! Tests different conversion policies and their impact on valuation:
//! - Voluntary conversion
//! - Mandatory conversion on specific date
//! - Conversion windows
//! - Event-triggered conversion
//! - Policy impact on pricing

use super::fixtures::*;
use finstack_quant_core::dates::Date;
use finstack_quant_valuations::instruments::fixed_income::convertible::{
    price_convertible_bond, ConvertibleTreeType,
};
use finstack_quant_valuations::instruments::fixed_income::convertible::{
    ConversionEvent, ConversionPolicy,
};
use time::Month;

#[test]
fn test_voluntary_conversion_policy() {
    let bond = create_convertible_with_policy(ConversionPolicy::Voluntary);
    let market = create_market_context();

    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    // Voluntary conversion should work and price reasonably
    assert!(
        price.amount() > bond_params::NOTIONAL * 0.8,
        "Voluntary convertible should have reasonable price: {}",
        price.amount()
    );
}

#[test]
fn test_mandatory_conversion_policy() {
    let mandatory_date = dates::mid_date();
    let bond = create_convertible_with_policy(ConversionPolicy::MandatoryOn(mandatory_date));
    let market = create_market_context();

    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    // Mandatory conversion should still price successfully
    assert!(
        price.amount() > 0.0 && price.amount().is_finite(),
        "Mandatory convertible should price: {}",
        price.amount()
    );
}

#[test]
fn test_window_conversion_policy() {
    let window_start = Date::from_calendar_date(2027, Month::January, 1).unwrap();
    let window_end = Date::from_calendar_date(2029, Month::January, 1).unwrap();

    let bond = create_convertible_with_policy(ConversionPolicy::Window {
        start: window_start,
        end: window_end,
    });
    let market = create_market_context();

    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Trinomial(40),
        dates::base_date(),
    )
    .unwrap();

    // Window conversion should price successfully
    assert!(
        price.amount() > bond_params::NOTIONAL * 0.5,
        "Window convertible should have reasonable price: {}",
        price.amount()
    );
}

#[test]
fn test_window_vs_voluntary_pricing() {
    let bond_voluntary = create_convertible_with_policy(ConversionPolicy::Voluntary);

    let window_start = Date::from_calendar_date(2027, Month::January, 1).unwrap();
    let window_end = Date::from_calendar_date(2029, Month::January, 1).unwrap();
    let bond_window = create_convertible_with_policy(ConversionPolicy::Window {
        start: window_start,
        end: window_end,
    });

    let market = create_market_context();

    let price_voluntary = price_convertible_bond(
        &bond_voluntary,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    let price_window = price_convertible_bond(
        &bond_window,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    // Voluntary should be worth at least as much as windowed (more flexibility)
    assert!(
        price_voluntary.amount() >= price_window.amount() * 0.95,
        "Voluntary {} should be >= windowed {} (flexibility premium)",
        price_voluntary.amount(),
        price_window.amount()
    );
}

#[test]
fn test_event_triggered_conversion_qualified_ipo() {
    let bond =
        create_convertible_with_policy(ConversionPolicy::UponEvent(ConversionEvent::QualifiedIpo));
    let market = create_market_context();

    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(30),
        dates::base_date(),
    )
    .unwrap();

    // Event-triggered conversion is conservatively disabled in current implementation
    // Should still price successfully as a bond
    assert!(
        price.amount() > 0.0 && price.amount().is_finite(),
        "Event-triggered convertible should price: {}",
        price.amount()
    );
}

#[test]
fn test_event_triggered_conversion_change_of_control() {
    let bond = create_convertible_with_policy(ConversionPolicy::UponEvent(
        ConversionEvent::ChangeOfControl,
    ));
    let market = create_market_context();

    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(30),
        dates::base_date(),
    )
    .unwrap();

    assert!(
        price.amount() > 0.0 && price.amount().is_finite(),
        "Change of control convertible should price: {}",
        price.amount()
    );
}

#[test]
fn test_event_triggered_conversion_price_trigger() {
    let bond = create_convertible_with_policy(ConversionPolicy::UponEvent(
        ConversionEvent::PriceTrigger {
            threshold: 120.0,
            lookback_days: 20,
        },
    ));
    let market = create_market_context();

    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(30),
        dates::base_date(),
    )
    .unwrap();

    assert!(
        price.amount() > 0.0 && price.amount().is_finite(),
        "Price trigger convertible should price: {}",
        price.amount()
    );
}

#[test]
fn test_mandatory_conversion_at_maturity() {
    let maturity = dates::maturity_5y();
    let bond = create_convertible_with_policy(ConversionPolicy::MandatoryOn(maturity));
    let market = create_market_context();

    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    // Mandatory conversion at maturity should be close to forward conversion value
    let conversion_value =
        theoretical_conversion_value(market_params::SPOT_PRICE, bond_params::CONVERSION_RATIO);

    // Should be reasonably close to conversion value (discounted)
    let ratio = price.amount() / conversion_value;
    assert!(
        ratio > 0.7 && ratio < 1.2,
        "Mandatory conversion at maturity should be close to conversion value: {} vs {}",
        price.amount(),
        conversion_value
    );
}

#[test]
fn test_early_conversion_window() {
    // Early window (first 2 years)
    let window_start = dates::issue();
    let window_end = Date::from_calendar_date(2027, Month::January, 1).unwrap();

    let bond = create_convertible_with_policy(ConversionPolicy::Window {
        start: window_start,
        end: window_end,
    });
    let market = create_market_context();

    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(40),
        dates::base_date(),
    )
    .unwrap();

    assert!(
        price.amount() > bond_params::NOTIONAL * 0.8,
        "Early window convertible should have reasonable price: {}",
        price.amount()
    );
}

#[test]
fn test_late_conversion_window() {
    // Late window (last 2 years)
    let window_start = Date::from_calendar_date(2028, Month::January, 1).unwrap();
    let window_end = dates::maturity_5y();

    let bond = create_convertible_with_policy(ConversionPolicy::Window {
        start: window_start,
        end: window_end,
    });
    let market = create_market_context();

    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(40),
        dates::base_date(),
    )
    .unwrap();

    assert!(
        price.amount() > bond_params::NOTIONAL * 0.8,
        "Late window convertible should have reasonable price: {}",
        price.amount()
    );
}

#[test]
fn test_narrow_conversion_window() {
    // Very narrow window (1 month)
    let window_start = Date::from_calendar_date(2027, Month::June, 1).unwrap();
    let window_end = Date::from_calendar_date(2027, Month::July, 1).unwrap();

    let bond = create_convertible_with_policy(ConversionPolicy::Window {
        start: window_start,
        end: window_end,
    });
    let market = create_market_context();

    let price = price_convertible_bond(
        &bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    // Narrow window should be worth less than voluntary
    let bond_voluntary = create_convertible_with_policy(ConversionPolicy::Voluntary);
    let price_voluntary = price_convertible_bond(
        &bond_voluntary,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    assert!(
        price.amount() < price_voluntary.amount() * 1.05,
        "Narrow window {} should be worth less than voluntary {}",
        price.amount(),
        price_voluntary.amount()
    );
}

#[test]
fn test_conversion_policy_with_itm_bond() {
    let market = create_market_context(); // ITM scenario

    let policies = vec![
        ("Voluntary", ConversionPolicy::Voluntary),
        (
            "Mandatory",
            ConversionPolicy::MandatoryOn(dates::mid_date()),
        ),
        (
            "Window",
            ConversionPolicy::Window {
                start: dates::issue(),
                end: dates::maturity_5y(),
            },
        ),
    ];

    for (name, policy) in policies {
        let bond = create_convertible_with_policy(policy);
        let price = price_convertible_bond(
            &bond,
            &market,
            ConvertibleTreeType::Binomial(50),
            dates::base_date(),
        )
        .unwrap();

        let conversion_value =
            theoretical_conversion_value(market_params::SPOT_PRICE, bond_params::CONVERSION_RATIO);

        assert!(
            price.amount() >= conversion_value * 0.95,
            "{} policy ITM bond should be close to conversion value: {} vs {}",
            name,
            price.amount(),
            conversion_value
        );
    }
}

#[test]
fn test_conversion_policy_with_otm_bond() {
    let market = create_market_context_with_params(
        market_params::SPOT_LOW,
        market_params::VOL_STANDARD,
        market_params::DIV_YIELD,
    );

    // Voluntary and Window policies: OTM bond should have bond floor support
    // because conversion is optional.
    let optional_policies = vec![
        ("Voluntary", ConversionPolicy::Voluntary),
        (
            "Window",
            ConversionPolicy::Window {
                start: dates::issue(),
                end: dates::maturity_5y(),
            },
        ),
    ];

    let approx_bond_floor =
        calculate_bond_floor(bond_params::COUPON_RATE, 5.0, market_params::RISK_FREE_RATE)
            * bond_params::NOTIONAL;

    for (name, policy) in optional_policies {
        let bond = create_convertible_with_policy(policy);
        let price = price_convertible_bond(
            &bond,
            &market,
            ConvertibleTreeType::Binomial(50),
            dates::base_date(),
        )
        .unwrap();

        assert!(
            price.amount() >= approx_bond_floor * 0.90,
            "{} policy OTM bond should have bond floor support: {} vs {}",
            name,
            price.amount(),
            approx_bond_floor
        );
    }

    // Mandatory conversion: the holder is FORCED to convert even at a loss.
    // OTM mandatory bonds trade below the straight bond floor because the
    // holder bears equity downside risk (this is the defining feature of
    // PERCS/DECS structures).
    let mandatory_bond =
        create_convertible_with_policy(ConversionPolicy::MandatoryOn(dates::mid_date()));
    let mandatory_price = price_convertible_bond(
        &mandatory_bond,
        &market,
        ConvertibleTreeType::Binomial(50),
        dates::base_date(),
    )
    .unwrap();

    assert!(
        mandatory_price.amount() < approx_bond_floor,
        "Mandatory OTM bond should price BELOW bond floor (forced conversion at a loss): {} vs {}",
        mandatory_price.amount(),
        approx_bond_floor
    );
    assert!(
        mandatory_price.amount() > 0.0,
        "Mandatory OTM bond should still have positive value: {}",
        mandatory_price.amount()
    );
}

#[test]
fn test_all_conversion_policies_price_successfully() {
    let market = create_market_context();

    let policies = vec![
        ConversionPolicy::Voluntary,
        ConversionPolicy::MandatoryOn(dates::mid_date()),
        ConversionPolicy::Window {
            start: dates::issue(),
            end: dates::maturity_5y(),
        },
        ConversionPolicy::UponEvent(ConversionEvent::QualifiedIpo),
        ConversionPolicy::UponEvent(ConversionEvent::ChangeOfControl),
        ConversionPolicy::UponEvent(ConversionEvent::PriceTrigger {
            threshold: 120.0,
            lookback_days: 20,
        }),
    ];

    for policy in policies {
        let bond = create_convertible_with_policy(policy);
        let result = price_convertible_bond(
            &bond,
            &market,
            ConvertibleTreeType::Binomial(30),
            dates::base_date(),
        );

        assert!(
            result.is_ok(),
            "All conversion policies should price successfully"
        );

        let price = result.unwrap();
        assert!(
            price.amount() > 0.0 && price.amount().is_finite(),
            "All prices should be positive and finite"
        );
    }
}
