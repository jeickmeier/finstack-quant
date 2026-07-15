//! Comprehensive unit tests for basket/ETF instruments.
//!
//! This test suite follows market standards with:
//! - Arrange-Act-Assert (AAA) pattern
//! - Isolated, independent tests
//! - Comprehensive coverage (>80%) of functionality
//! - Clear naming conventions
//! - Edge case and error path testing

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::fx::{FxConversionPolicy, FxMatrix, FxProvider};
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::exotics::basket::BasketCalculator;
use finstack_quant_valuations::instruments::exotics::basket::{
    AssetExposureCalculator, ConstituentCountCalculator, ExpenseRatioCalculator,
};
use finstack_quant_valuations::instruments::exotics::basket::{
    AssetType, Basket, BasketConstituent, BasketPricingConfig, ConstituentReference,
};
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::{Attributes, Instrument};
use finstack_quant_valuations::metrics::{MetricCalculator, MetricContext};
use std::sync::Arc;
use time::Month;

// ============================================================================
// Test Fixtures and Helpers
// ============================================================================

fn date(year: i32, month: u8, day: u8) -> Date {
    Date::from_calendar_date(year, Month::try_from(month).unwrap(), day).unwrap()
}

fn usd(amount: f64) -> Money {
    Money::new(amount, Currency::USD)
}

fn eur(amount: f64) -> Money {
    Money::new(amount, Currency::EUR)
}

fn gbp(amount: f64) -> Money {
    Money::new(amount, Currency::GBP)
}

/// Create a minimal market context with discount curve
fn minimal_market_context() -> MarketContext {
    let base_date = date(2025, 1, 1);
    let discount_curve = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots([(0.0, 1.0), (1.0, 0.95), (5.0, 0.80)])
        .build()
        .unwrap();

    MarketContext::new().insert(discount_curve)
}

/// Create a market context with equity prices
fn equity_market_context() -> MarketContext {
    minimal_market_context()
        .insert_price("AAPL", MarketScalar::Unitless(150.0))
        .insert_price("MSFT", MarketScalar::Unitless(300.0))
        .insert_price("GOOGL", MarketScalar::Unitless(2800.0))
        .insert_price("AMZN", MarketScalar::Unitless(3200.0))
        .insert_price("CASH", MarketScalar::Unitless(1.0))
}

/// Create a market context with multi-currency FX
fn multi_currency_market_context() -> MarketContext {
    struct TestFxProvider;
    impl FxProvider for TestFxProvider {
        fn rate(
            &self,
            from: Currency,
            to: Currency,
            _on: Date,
            _policy: FxConversionPolicy,
        ) -> finstack_quant_core::Result<f64> {
            let rate_value = match (from, to) {
                (Currency::EUR, Currency::USD) => 1.1,
                (Currency::USD, Currency::EUR) => 1.0 / 1.1,
                (Currency::GBP, Currency::USD) => 1.25,
                (Currency::USD, Currency::GBP) => 1.0 / 1.25,
                (Currency::EUR, Currency::GBP) => 1.25 / 1.1,
                (Currency::GBP, Currency::EUR) => 1.1 / 1.25,
                _ => 1.0,
            };
            Ok(rate_value)
        }
    }

    let fx = FxMatrix::new(Arc::new(TestFxProvider));
    minimal_market_context()
        .insert_fx(fx)
        .insert_price("EUR_EQUITY", MarketScalar::Price(eur(100.0)))
        .insert_price("GBP_EQUITY", MarketScalar::Price(gbp(50.0)))
        .insert_price("USD_EQUITY", MarketScalar::Price(usd(200.0)))
}

/// Create a simple basket with market data constituents
fn simple_equity_basket() -> Basket {
    Basket {
        id: "SIMPLE_BASKET".into(),
        constituents: vec![
            BasketConstituent {
                id: "AAPL".to_string(),
                reference: ConstituentReference::MarketData {
                    price_id: "AAPL".into(),
                    asset_type: AssetType::Equity,
                },
                weight: 0.6,
                units: None,
                ticker: Some("AAPL".to_string()),
            },
            BasketConstituent {
                id: "MSFT".to_string(),
                reference: ConstituentReference::MarketData {
                    price_id: "MSFT".into(),
                    asset_type: AssetType::Equity,
                },
                weight: 0.4,
                units: None,
                ticker: Some("MSFT".to_string()),
            },
        ],
        expense_ratio: 0.001,
        currency: Currency::USD,
        notional: usd(1_000_000.0),
        discount_curve_id: "USD-OIS".into(),
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Attributes::new(),
        pricing_config: BasketPricingConfig::default(),
    }
}

// ============================================================================
// Type Construction and Validation Tests
// ============================================================================

#[test]
fn test_basket_creation_with_minimal_fields() {
    // Arrange & Act
    let basket = Basket {
        id: "TEST_BASKET".into(),
        constituents: vec![],
        expense_ratio: 0.0,
        currency: Currency::USD,
        notional: usd(1_000_000.0),
        discount_curve_id: "USD-OIS".into(),
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Attributes::new(),
        pricing_config: BasketPricingConfig::default(),
    };

    // Assert
    assert_eq!(basket.id.as_str(), "TEST_BASKET");
    assert_eq!(basket.expense_ratio, 0.0);
    assert_eq!(basket.currency, Currency::USD);
    assert_eq!(basket.constituents.len(), 0);
}

#[test]
fn test_basket_builder_pattern() {
    // Arrange & Act
    let basket = Basket::builder()
        .id("BUILDER_BASKET".into())
        .currency(Currency::USD)
        .notional(usd(1_000_000.0))
        .discount_curve_id("USD-OIS".into())
        .expense_ratio(0.0025)
        .pricing_config(BasketPricingConfig::default())
        .constituents(vec![BasketConstituent {
            id: "TEST".to_string(),
            reference: ConstituentReference::MarketData {
                price_id: "TEST".into(),
                asset_type: AssetType::Equity,
            },
            weight: 1.0,
            units: None,
            ticker: None,
        }])
        .build()
        .unwrap();

    // Assert
    assert_eq!(basket.id.as_str(), "BUILDER_BASKET");
    assert_eq!(basket.expense_ratio, 0.0025);
}

#[test]
fn test_basket_validation_valid_weights() {
    // Arrange
    let basket = simple_equity_basket();

    // Act
    let result = basket.validate();

    // Assert
    assert!(
        result.is_ok(),
        "Basket with weights summing to 1.0 should validate successfully"
    );
}

#[test]
fn test_basket_validation_invalid_weights_sum() {
    // Arrange
    let mut basket = simple_equity_basket();
    basket.constituents[0].weight = 0.8;
    basket.constituents[1].weight = 0.3; // Sum = 1.1

    // Act
    let result = basket.validate();

    // Assert
    assert!(
        result.is_err(),
        "Basket with weights summing to >1.01 should fail validation"
    );
}

#[test]
fn test_basket_validation_empty_constituents() {
    // Arrange
    let basket = Basket {
        id: "EMPTY_BASKET".into(),
        constituents: vec![],
        expense_ratio: 0.001,
        currency: Currency::USD,
        notional: usd(1_000_000.0),
        discount_curve_id: "USD-OIS".into(),
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Attributes::new(),
        pricing_config: BasketPricingConfig::default(),
    };

    // Act - empty basket with no constituents should validate (zero weight sum is OK)
    // Note: The validate() method checks abs(total_weight - 1.0) > 0.01
    // For empty basket, total_weight = 0, so abs(0 - 1.0) = 1.0 > 0.01
    let result = basket.validate();

    // Assert
    assert!(
        result.is_err(),
        "Empty basket should fail validation due to zero weight sum"
    );
}

#[test]
fn test_constituent_count() {
    // Arrange
    let basket = simple_equity_basket();

    // Act
    let count = basket.constituent_count();

    // Assert
    assert_eq!(count, 2);
}

#[test]
fn test_get_constituent_by_id() {
    // Arrange
    let basket = simple_equity_basket();

    // Act
    let constituent = basket.get_constituent("AAPL");

    // Assert
    assert!(constituent.is_some());
    assert_eq!(constituent.unwrap().id, "AAPL");
}

#[test]
fn test_get_constituent_by_id_not_found() {
    // Arrange
    let basket = simple_equity_basket();

    // Act
    let constituent = basket.get_constituent("TSLA");

    // Assert
    assert!(constituent.is_none());
}

// ============================================================================
// Pricing Configuration Tests
// ============================================================================

#[test]
fn test_default_pricing_config() {
    // Arrange & Act
    let config = BasketPricingConfig::default();

    // Assert
    assert_eq!(config.days_in_year, 365.25);
    assert!(matches!(config.fx_policy, FxConversionPolicy::CashflowDate));
}

#[test]
fn test_custom_pricing_config() {
    // Arrange
    let config = BasketPricingConfig {
        days_in_year: 360.0,
        fx_policy: FxConversionPolicy::PeriodEnd,
    };

    // Act
    let basket = Basket::builder()
        .id("TEST".into())
        .currency(Currency::USD)
        .notional(usd(1_000_000.0))
        .discount_curve_id("USD-OIS".into())
        .expense_ratio(0.001)
        .pricing_config(BasketPricingConfig::default())
        .constituents(vec![BasketConstituent {
            id: "TEST".to_string(),
            reference: ConstituentReference::MarketData {
                price_id: "TEST".into(),
                asset_type: AssetType::Equity,
            },
            weight: 1.0,
            units: None,
            ticker: None,
        }])
        .build()
        .unwrap()
        .with_pricing_config(config);

    // Assert
    assert_eq!(basket.pricing_config.days_in_year, 360.0);
}

// ============================================================================
// Units-Based Pricing Tests
// ============================================================================

#[test]
fn test_pricing_with_explicit_units() {
    // Arrange
    let basket = Basket {
        id: "UNITS_BASKET".into(),
        constituents: vec![
            BasketConstituent {
                id: "AAPL".to_string(),
                reference: ConstituentReference::MarketData {
                    price_id: "AAPL".into(),
                    asset_type: AssetType::Equity,
                },
                weight: 0.0,
                units: Some(10.0),
                ticker: None,
            },
            BasketConstituent {
                id: "MSFT".to_string(),
                reference: ConstituentReference::MarketData {
                    price_id: "MSFT".into(),
                    asset_type: AssetType::Equity,
                },
                weight: 0.0,
                units: Some(5.0),
                ticker: None,
            },
        ],
        expense_ratio: 0.0,
        currency: Currency::USD,
        notional: usd(1_000_000.0),
        discount_curve_id: "USD-OIS".into(),
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Attributes::new(),
        pricing_config: BasketPricingConfig::default(),
    };
    let context = equity_market_context();
    let calc = basket.calculator();
    let as_of = date(2025, 1, 1);

    // Act
    let basket_value = calc
        .basket_value(&basket, &context, as_of, Some(100.0))
        .unwrap();

    // Assert
    // AAPL: 10 units × $150 = $1,500
    // MSFT: 5 units × $300 = $1,500
    // Total: $3,000
    assert_eq!(basket_value.amount(), 3000.0);
    assert_eq!(basket_value.currency(), Currency::USD);
}

#[test]
fn test_nav_calculation_with_units() {
    // Arrange
    let basket = Basket {
        id: "NAV_BASKET".into(),
        constituents: vec![BasketConstituent {
            id: "AAPL".to_string(),
            reference: ConstituentReference::MarketData {
                price_id: "AAPL".into(),
                asset_type: AssetType::Equity,
            },
            weight: 0.0,
            units: Some(100.0),
            ticker: None,
        }],
        expense_ratio: 0.0,
        currency: Currency::USD,
        notional: usd(1_000_000.0),
        discount_curve_id: "USD-OIS".into(),
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Attributes::new(),
        pricing_config: BasketPricingConfig::default(),
    };
    let context = equity_market_context();
    let calc = basket.calculator();
    let as_of = date(2025, 1, 1);
    let shares_outstanding = 1000.0;

    // Act
    let nav = calc
        .nav(&basket, &context, as_of, shares_outstanding)
        .unwrap();

    // Assert
    // Total value: 100 units × $150 = $15,000
    // NAV per share: $15,000 / 1000 shares = $15
    assert_eq!(nav.amount(), 15.0);
}

// ============================================================================
// Weight-Based Pricing Tests
// ============================================================================

#[test]
fn test_pricing_with_weights_and_aum() {
    // Arrange
    let basket = simple_equity_basket();
    let context = equity_market_context();
    let calc = basket.calculator();
    let as_of = date(2025, 1, 1);
    let aum = usd(1_000_000.0);

    // Act
    let basket_value = calc
        .basket_value_with_aum(&basket, &context, as_of, aum)
        .unwrap();

    // Assert
    // Since all constituents are weight-based and expense_ratio is 0.001:
    // Daily drag = 1,000,000 × (0.001 / 365.25) ≈ 2.738
    // Value ≈ 1,000,000 - 2.738 ≈ 999,997.26
    let expected_drag = 1_000_000.0 * (0.001 / 365.25);
    assert!((basket_value.amount() - (1_000_000.0 - expected_drag)).abs() < 0.01);
}

#[test]
fn test_nav_with_aum_calculation() {
    // Arrange
    let basket = simple_equity_basket();
    let context = equity_market_context();
    let calc = basket.calculator();
    let as_of = date(2025, 1, 1);
    let aum = usd(1_000_000.0);
    let shares = 50_000.0;

    // Act
    let nav = calc
        .nav_with_aum(&basket, &context, as_of, aum, shares)
        .unwrap();

    // Assert
    // NAV = (AUM - expense drag) / shares
    let expected_drag = 1_000_000.0 * (0.001 / 365.25);
    let expected_nav = (1_000_000.0 - expected_drag) / shares;
    assert!((nav.amount() - expected_nav).abs() < 0.001);
}

// ============================================================================
// Expense Ratio Tests
// ============================================================================

#[test]
fn test_expense_ratio_zero_has_no_impact() {
    // Arrange
    let mut basket = simple_equity_basket();
    basket.expense_ratio = 0.0;
    let context = equity_market_context();
    let calc = basket.calculator();
    let as_of = date(2025, 1, 1);
    let aum = usd(1_000_000.0);

    // Act
    let basket_value = calc
        .basket_value_with_aum(&basket, &context, as_of, aum)
        .unwrap();

    // Assert
    assert_eq!(basket_value.amount(), 1_000_000.0);
}

#[test]
fn test_expense_ratio_reduces_basket_value() {
    // Arrange
    let mut basket = simple_equity_basket();
    basket.expense_ratio = 0.365; // Large ratio for clear test signal
    let context = equity_market_context();
    let calc = basket.calculator();
    let as_of = date(2025, 1, 1);
    let aum = usd(1_000_000.0);

    // Act
    let basket_value = calc
        .basket_value_with_aum(&basket, &context, as_of, aum)
        .unwrap();

    // Assert
    // Daily expense = 1,000,000 × (0.365 / 365.25) = 1,000
    // Value = 1,000,000 - 1,000 = 999,000
    let expected_drag = 1_000_000.0 * (0.365 / 365.25);
    assert!((basket_value.amount() - (1_000_000.0 - expected_drag)).abs() < 1.0);
}

#[test]
fn test_expense_ratio_with_custom_days_in_year() {
    // Arrange
    let config = BasketPricingConfig {
        days_in_year: 360.0,
        fx_policy: FxConversionPolicy::CashflowDate,
    };
    let mut basket = simple_equity_basket().with_pricing_config(config);
    basket.expense_ratio = 0.36; // 0.1% daily with 360 day basis
    let context = equity_market_context();
    let calc = basket.calculator();
    let as_of = date(2025, 1, 1);
    let aum = usd(1_000_000.0);

    // Act
    let basket_value = calc
        .basket_value_with_aum(&basket, &context, as_of, aum)
        .unwrap();

    // Assert
    // Daily expense = 1,000,000 × (0.36 / 360) = 1,000
    let expected_drag = 1_000_000.0 * (0.36 / 360.0);
    assert!((basket_value.amount() - (1_000_000.0 - expected_drag)).abs() < 1.0);
}

// ============================================================================
// Multi-Currency and FX Conversion Tests
// ============================================================================

#[test]
fn test_fx_conversion_eur_to_usd() {
    // Arrange
    let basket = Basket {
        id: "FX_BASKET".into(),
        constituents: vec![BasketConstituent {
            id: "EUR_EQUITY".to_string(),
            reference: ConstituentReference::MarketData {
                price_id: "EUR_EQUITY".into(),
                asset_type: AssetType::Equity,
            },
            weight: 0.0,
            units: Some(10.0),
            ticker: None,
        }],
        expense_ratio: 0.0,
        currency: Currency::USD,
        notional: usd(1_000_000.0),
        discount_curve_id: "USD-OIS".into(),
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Attributes::new(),
        pricing_config: BasketPricingConfig::default(),
    };
    let context = multi_currency_market_context();
    let calc = basket.calculator();
    let as_of = date(2025, 1, 1);

    // Act
    let basket_value = calc
        .basket_value(&basket, &context, as_of, Some(100.0))
        .unwrap();

    // Assert
    // EUR price: €100 × 10 units = €1,000
    // FX rate: 1.1 (EUR/USD)
    // USD value: €1,000 × 1.1 = $1,100
    assert!((basket_value.amount() - 1100.0).abs() < 0.01);
    assert_eq!(basket_value.currency(), Currency::USD);
}

#[test]
fn test_fx_conversion_multiple_currencies() {
    // Arrange
    let basket = Basket {
        id: "MULTI_CCY_BASKET".into(),
        constituents: vec![
            BasketConstituent {
                id: "EUR_EQUITY".to_string(),
                reference: ConstituentReference::MarketData {
                    price_id: "EUR_EQUITY".into(),
                    asset_type: AssetType::Equity,
                },
                weight: 0.0,
                units: Some(5.0),
                ticker: None,
            },
            BasketConstituent {
                id: "GBP_EQUITY".to_string(),
                reference: ConstituentReference::MarketData {
                    price_id: "GBP_EQUITY".into(),
                    asset_type: AssetType::Equity,
                },
                weight: 0.0,
                units: Some(10.0),
                ticker: None,
            },
            BasketConstituent {
                id: "USD_EQUITY".to_string(),
                reference: ConstituentReference::MarketData {
                    price_id: "USD_EQUITY".into(),
                    asset_type: AssetType::Equity,
                },
                weight: 0.0,
                units: Some(2.0),
                ticker: None,
            },
        ],
        expense_ratio: 0.0,
        currency: Currency::USD,
        notional: usd(1_000_000.0),
        discount_curve_id: "USD-OIS".into(),
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Attributes::new(),
        pricing_config: BasketPricingConfig::default(),
    };
    let context = multi_currency_market_context();
    let calc = basket.calculator();
    let as_of = date(2025, 1, 1);

    // Act
    let basket_value = calc
        .basket_value(&basket, &context, as_of, Some(100.0))
        .unwrap();

    // Assert
    // EUR: €100 × 5 × 1.1 = $550
    // GBP: £50 × 10 × 1.25 = $625
    // USD: $200 × 2 = $400
    // Total: $1,575
    assert!((basket_value.amount() - 1575.0).abs() < 0.01);
}

#[test]
fn test_fx_conversion_error_without_fx_provider() {
    // Arrange
    let basket = Basket {
        id: "NO_FX_BASKET".into(),
        constituents: vec![BasketConstituent {
            id: "EUR_EQUITY".to_string(),
            reference: ConstituentReference::MarketData {
                price_id: "EUR_EQUITY".into(),
                asset_type: AssetType::Equity,
            },
            weight: 0.0,
            units: Some(10.0),
            ticker: None,
        }],
        expense_ratio: 0.0,
        currency: Currency::USD,
        notional: usd(1_000_000.0),
        discount_curve_id: "USD-OIS".into(),
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Attributes::new(),
        pricing_config: BasketPricingConfig::default(),
    };
    // Context without FX provider
    let context =
        minimal_market_context().insert_price("EUR_EQUITY", MarketScalar::Price(eur(100.0)));
    let calc = basket.calculator();
    let as_of = date(2025, 1, 1);

    // Act
    let result = calc.basket_value(&basket, &context, as_of, Some(100.0));

    // Assert
    assert!(
        result.is_err(),
        "Should fail when FX conversion needed but no FX provider available"
    );
}

// ============================================================================
// Mixed Units and Weights Tests
// ============================================================================

#[test]
fn test_mixed_units_and_weights_with_aum() {
    // Arrange
    let basket = Basket {
        id: "MIXED_BASKET".into(),
        constituents: vec![
            BasketConstituent {
                id: "AAPL".to_string(),
                reference: ConstituentReference::MarketData {
                    price_id: "AAPL".into(),
                    asset_type: AssetType::Equity,
                },
                weight: 0.0,
                units: Some(10.0), // Explicit units
                ticker: None,
            },
            BasketConstituent {
                id: "MSFT".to_string(),
                reference: ConstituentReference::MarketData {
                    price_id: "MSFT".into(),
                    asset_type: AssetType::Equity,
                },
                weight: 0.5, // Weight-based
                units: None,
                ticker: None,
            },
        ],
        expense_ratio: 0.0,
        currency: Currency::USD,
        notional: usd(1_000_000.0),
        discount_curve_id: "USD-OIS".into(),
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Attributes::new(),
        pricing_config: BasketPricingConfig::default(),
    };
    let context = equity_market_context();
    let calc = basket.calculator();
    let as_of = date(2025, 1, 1);
    let aum = usd(10_000.0);

    // Act
    let basket_value = calc
        .basket_value_with_aum(&basket, &context, as_of, aum)
        .unwrap();

    // Assert
    // AAPL: 10 × $150 = $1,500 (units-based)
    // MSFT: 0.5 × $10,000 = $5,000 (weight of AUM)
    // Total: $6,500
    assert!((basket_value.amount() - 6500.0).abs() < 0.01);
}

// ============================================================================
// Instrument Reference Tests
// ============================================================================

#[test]
fn test_constituent_reference_with_bond_instrument() {
    use finstack_quant_valuations::instruments::json_loader::InstrumentJson;

    // Arrange
    let base_date = date(2025, 1, 1);
    let maturity = date(2030, 1, 1);
    let bond = Bond::fixed(
        "CORP_BOND",
        usd(1000.0),
        0.05,
        base_date,
        maturity,
        "USD-OIS",
    )
    .unwrap();

    let basket = Basket {
        id: "BOND_BASKET".into(),
        constituents: vec![BasketConstituent {
            id: "CORP_BOND".to_string(),
            reference: ConstituentReference::Instrument(Box::new(InstrumentJson::Bond(bond))),
            weight: 0.0,
            units: Some(10.0),
            ticker: None,
        }],
        expense_ratio: 0.0,
        currency: Currency::USD,
        notional: usd(1_000_000.0),
        discount_curve_id: "USD-OIS".into(),
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Attributes::new(),
        pricing_config: BasketPricingConfig::default(),
    };
    let context = minimal_market_context();
    let calc = basket.calculator();
    let as_of = base_date;

    // Act
    let result = calc.basket_value(&basket, &context, as_of, Some(100.0));

    // Assert
    // Should successfully price using the bond's value() method
    assert!(result.is_ok());
    let value = result.unwrap();
    assert!(value.amount() > 0.0);
    assert_eq!(value.currency(), Currency::USD);
}

// ============================================================================
// Asset Type Tests
// ============================================================================

#[test]
fn test_basket_with_multiple_asset_types() {
    // Arrange
    let basket = Basket {
        id: "MULTI_ASSET".into(),
        constituents: vec![
            BasketConstituent {
                id: "EQUITY".to_string(),
                reference: ConstituentReference::MarketData {
                    price_id: "AAPL".into(),
                    asset_type: AssetType::Equity,
                },
                weight: 0.4,
                units: None,
                ticker: None,
            },
            BasketConstituent {
                id: "BOND".to_string(),
                reference: ConstituentReference::MarketData {
                    price_id: "BOND_AAA".into(),
                    asset_type: AssetType::Bond,
                },
                weight: 0.3,
                units: None,
                ticker: None,
            },
            BasketConstituent {
                id: "CASH".to_string(),
                reference: ConstituentReference::MarketData {
                    price_id: "CASH".into(),
                    asset_type: AssetType::Cash,
                },
                weight: 0.3,
                units: None,
                ticker: None,
            },
        ],
        expense_ratio: 0.0,
        currency: Currency::USD,
        notional: usd(1_000_000.0),
        discount_curve_id: "USD-OIS".into(),
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Attributes::new(),
        pricing_config: BasketPricingConfig::default(),
    };

    // Act & Assert
    assert_eq!(basket.constituents.len(), 3);
    assert!(basket.validate().is_ok());
}

// ============================================================================
// Metrics Tests
// ============================================================================

#[test]
fn test_constituent_count_metric() {
    // Arrange
    let basket = simple_equity_basket();
    let context = equity_market_context();
    let instrument: Arc<dyn Instrument> = Arc::new(basket);
    let mut metric_context = MetricContext::new(
        instrument,
        Arc::new(context),
        date(2025, 1, 1),
        usd(0.0),
        MetricContext::default_config(),
    );
    let calculator = ConstituentCountCalculator;

    // Act
    let result = calculator.calculate(&mut metric_context).unwrap();

    // Assert
    assert_eq!(result, 2.0);
}

#[test]
fn test_expense_ratio_metric() {
    // Arrange
    let basket = simple_equity_basket();
    let context = equity_market_context();
    let instrument: Arc<dyn Instrument> = Arc::new(basket);
    let mut metric_context = MetricContext::new(
        instrument,
        Arc::new(context),
        date(2025, 1, 1),
        usd(0.0),
        MetricContext::default_config(),
    );
    let calculator = ExpenseRatioCalculator;

    // Act
    let result = calculator.calculate(&mut metric_context).unwrap();

    // Assert
    // Expense ratio is stored as decimal (0.001), returned as percentage (0.1)
    assert!((result - 0.1).abs() < 0.001);
}

#[test]
fn test_asset_exposure_metric_equity() {
    // Arrange
    let basket = Basket {
        id: "EXPOSURE_TEST".into(),
        constituents: vec![
            BasketConstituent {
                id: "AAPL".to_string(),
                reference: ConstituentReference::MarketData {
                    price_id: "AAPL".into(),
                    asset_type: AssetType::Equity,
                },
                weight: 0.6,
                units: None,
                ticker: None,
            },
            BasketConstituent {
                id: "MSFT".to_string(),
                reference: ConstituentReference::MarketData {
                    price_id: "MSFT".into(),
                    asset_type: AssetType::Equity,
                },
                weight: 0.3,
                units: None,
                ticker: None,
            },
            BasketConstituent {
                id: "BOND".to_string(),
                reference: ConstituentReference::MarketData {
                    price_id: "BOND_AAA".into(),
                    asset_type: AssetType::Bond,
                },
                weight: 0.1,
                units: None,
                ticker: None,
            },
        ],
        expense_ratio: 0.0,
        currency: Currency::USD,
        notional: usd(1_000_000.0),
        discount_curve_id: "USD-OIS".into(),
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Attributes::new(),
        pricing_config: BasketPricingConfig::default(),
    };
    let context = equity_market_context();
    let instrument: Arc<dyn Instrument> = Arc::new(basket);
    let mut metric_context = MetricContext::new(
        instrument,
        Arc::new(context),
        date(2025, 1, 1),
        usd(0.0),
        MetricContext::default_config(),
    );
    let calculator = AssetExposureCalculator::new(AssetType::Equity);

    // Act
    let result = calculator.calculate(&mut metric_context).unwrap();

    // Assert
    // Equity exposure: 0.6 + 0.3 = 0.9 = 90%
    assert!((result - 90.0).abs() < 0.01);
}

#[test]
fn test_asset_exposure_metric_bond() {
    // Arrange
    let basket = Basket {
        id: "BOND_EXPOSURE".into(),
        constituents: vec![
            BasketConstituent {
                id: "BOND1".to_string(),
                reference: ConstituentReference::MarketData {
                    price_id: "BOND_AAA".into(),
                    asset_type: AssetType::Bond,
                },
                weight: 0.5,
                units: None,
                ticker: None,
            },
            BasketConstituent {
                id: "BOND2".to_string(),
                reference: ConstituentReference::MarketData {
                    price_id: "BOND_AA".into(),
                    asset_type: AssetType::Bond,
                },
                weight: 0.3,
                units: None,
                ticker: None,
            },
            BasketConstituent {
                id: "EQUITY".to_string(),
                reference: ConstituentReference::MarketData {
                    price_id: "AAPL".into(),
                    asset_type: AssetType::Equity,
                },
                weight: 0.2,
                units: None,
                ticker: None,
            },
        ],
        expense_ratio: 0.0,
        currency: Currency::USD,
        notional: usd(1_000_000.0),
        discount_curve_id: "USD-OIS".into(),
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Attributes::new(),
        pricing_config: BasketPricingConfig::default(),
    };
    // Need to add bond prices to context
    let context = equity_market_context()
        .insert_price("BOND_AAA", MarketScalar::Unitless(98.5))
        .insert_price("BOND_AA", MarketScalar::Unitless(101.2));
    let instrument: Arc<dyn Instrument> = Arc::new(basket);
    let mut metric_context = MetricContext::new(
        instrument,
        Arc::new(context),
        date(2025, 1, 1),
        usd(0.0),
        MetricContext::default_config(),
    );
    let calculator = AssetExposureCalculator::new(AssetType::Bond);

    // Act
    let result = calculator.calculate(&mut metric_context).unwrap();

    // Assert
    // Bond exposure: 0.5 + 0.3 = 0.8 = 80%
    assert!((result - 80.0).abs() < 0.01);
}

// ============================================================================
// Instrument Trait Implementation Tests
// ============================================================================

#[test]
fn test_instrument_trait_id() {
    // Arrange
    let basket = simple_equity_basket();
    let instrument: &dyn Instrument = &basket;

    // Act
    let id = instrument.id();

    // Assert
    assert_eq!(id, "SIMPLE_BASKET");
}

#[test]
fn test_instrument_trait_key() {
    // Arrange
    let basket = simple_equity_basket();
    let instrument: &dyn Instrument = &basket;

    // Act
    let key = instrument.key();

    // Assert
    assert_eq!(
        key,
        finstack_quant_valuations::pricer::InstrumentType::Basket
    );
}

#[test]
fn test_instrument_trait_value() {
    // Arrange
    let basket = simple_equity_basket();
    let context = equity_market_context();
    let as_of = date(2025, 1, 1);
    let instrument: &dyn Instrument = &basket;

    // Act
    let value = instrument.value(&context, as_of);

    // Assert
    assert!(value.is_ok());
    let money = value.unwrap();
    assert!(money.amount() > 0.0);
    assert_eq!(money.currency(), Currency::USD);
}

#[test]
fn test_instrument_trait_attributes() {
    // Arrange
    let mut basket = simple_equity_basket();
    basket
        .attributes
        .meta
        .insert("sector".to_string(), "technology".to_string());

    // Act
    let attrs = basket.attributes();

    // Assert
    assert!(attrs.meta.contains_key("sector"));
}

#[test]
fn test_instrument_trait_clone_box() {
    // Arrange
    let basket = simple_equity_basket();
    let instrument: &dyn Instrument = &basket;

    // Act
    let cloned = instrument.clone_box();

    // Assert
    assert_eq!(cloned.id(), "SIMPLE_BASKET");
}

// ============================================================================
// Edge Cases and Error Handling Tests
// ============================================================================

#[test]
fn test_basket_value_with_zero_shares() {
    // Arrange
    let basket = Basket {
        id: "ZERO_SHARES_TEST".into(),
        constituents: vec![BasketConstituent {
            id: "AAPL".to_string(),
            reference: ConstituentReference::MarketData {
                price_id: "AAPL".into(),
                asset_type: AssetType::Equity,
            },
            weight: 0.0,
            units: Some(10.0),
            ticker: None,
        }],
        expense_ratio: 0.0,
        currency: Currency::USD,
        notional: usd(1_000_000.0),
        discount_curve_id: "USD-OIS".into(),
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Attributes::new(),
        pricing_config: BasketPricingConfig::default(),
    };
    let context = equity_market_context();
    let calc = basket.calculator();
    let as_of = date(2025, 1, 1);

    // Act - passing 0 shares with units-based constituent should error
    let result = calc.nav(&basket, &context, as_of, 0.0);

    // Assert
    assert!(
        result.is_err(),
        "NAV calculation with zero shares and units should fail"
    );
}

#[test]
fn test_basket_value_with_negative_shares() {
    // Arrange
    let basket = Basket {
        id: "NEG_SHARES_TEST".into(),
        constituents: vec![BasketConstituent {
            id: "AAPL".to_string(),
            reference: ConstituentReference::MarketData {
                price_id: "AAPL".into(),
                asset_type: AssetType::Equity,
            },
            weight: 0.0,
            units: Some(10.0),
            ticker: None,
        }],
        expense_ratio: 0.0,
        currency: Currency::USD,
        notional: usd(1_000_000.0),
        discount_curve_id: "USD-OIS".into(),
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Attributes::new(),
        pricing_config: BasketPricingConfig::default(),
    };
    let context = equity_market_context();
    let calc = basket.calculator();
    let as_of = date(2025, 1, 1);

    // Act
    let result = calc.nav(&basket, &context, as_of, -100.0);

    // Assert
    assert!(
        result.is_err(),
        "NAV calculation with negative shares and units should fail"
    );
}

#[test]
fn test_missing_price_data_error() {
    // Arrange
    let basket = simple_equity_basket();
    let context = minimal_market_context(); // No price data
    let calc = basket.calculator();
    let as_of = date(2025, 1, 1);

    // Act
    let result = calc.basket_value(&basket, &context, as_of, Some(100.0));

    // Assert
    assert!(
        result.is_err(),
        "Should fail when required price data is missing"
    );
}

#[test]
fn test_basket_value_weight_without_aum_or_shares_errors() {
    // Arrange
    let basket = simple_equity_basket();
    let context = equity_market_context();
    let calc = basket.calculator();
    let as_of = date(2025, 1, 1);

    // Act - weight-based basket needs shares or AUM
    let result = calc.basket_value(&basket, &context, as_of, None);

    // Assert
    assert!(
        result.is_err(),
        "Weight-based basket without shares or AUM should error"
    );
}

#[test]
fn test_very_small_expense_ratio() {
    // Arrange
    let mut basket = simple_equity_basket();
    basket.expense_ratio = 0.000001; // 0.0001 bps
    let context = equity_market_context();
    let calc = basket.calculator();
    let as_of = date(2025, 1, 1);
    let aum = usd(1_000_000.0);

    // Act
    let basket_value = calc
        .basket_value_with_aum(&basket, &context, as_of, aum)
        .unwrap();

    // Assert
    // Very small expense should have minimal impact
    assert!((basket_value.amount() - 1_000_000.0).abs() < 1.0);
}

#[test]
fn test_very_large_expense_ratio() {
    // Arrange
    let mut basket = simple_equity_basket();
    basket.expense_ratio = 10.0; // 1000% - unrealistic but tests math
    let context = equity_market_context();
    let calc = basket.calculator();
    let as_of = date(2025, 1, 1);
    let aum = usd(1_000_000.0);

    // Act
    let basket_value = calc
        .basket_value_with_aum(&basket, &context, as_of, aum)
        .unwrap();

    // Assert
    // Should handle large expense ratios without panic
    assert!(basket_value.amount() >= 0.0);
}

#[test]
fn test_single_constituent_basket() {
    // Arrange
    let basket = Basket {
        id: "SINGLE".into(),
        constituents: vec![BasketConstituent {
            id: "AAPL".to_string(),
            reference: ConstituentReference::MarketData {
                price_id: "AAPL".into(),
                asset_type: AssetType::Equity,
            },
            weight: 1.0,
            units: None,
            ticker: None,
        }],
        expense_ratio: 0.0,
        currency: Currency::USD,
        notional: usd(1_000_000.0),
        discount_curve_id: "USD-OIS".into(),
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Attributes::new(),
        pricing_config: BasketPricingConfig::default(),
    };

    // Act & Assert
    assert!(basket.validate().is_ok());
    assert_eq!(basket.constituent_count(), 1);
}

#[test]
fn test_unitless_scalar_defaults_to_basket_currency() {
    // Arrange
    let basket = Basket {
        id: "UNITLESS_TEST".into(),
        constituents: vec![BasketConstituent {
            id: "ASSET".to_string(),
            reference: ConstituentReference::MarketData {
                price_id: "UNITLESS_PRICE".into(),
                asset_type: AssetType::Equity,
            },
            weight: 0.0,
            units: Some(10.0),
            ticker: None,
        }],
        expense_ratio: 0.0,
        currency: Currency::USD,
        notional: usd(1_000_000.0),
        discount_curve_id: "USD-OIS".into(),
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Attributes::new(),
        pricing_config: BasketPricingConfig::default(),
    };
    let context =
        minimal_market_context().insert_price("UNITLESS_PRICE", MarketScalar::Unitless(50.0));
    let calc = basket.calculator();
    let as_of = date(2025, 1, 1);

    // Act
    let basket_value = calc
        .basket_value(&basket, &context, as_of, Some(100.0))
        .unwrap();

    // Assert
    // 10 units × 50 (unitless, interpreted as USD) = 500 USD
    assert_eq!(basket_value.amount(), 500.0);
    assert_eq!(basket_value.currency(), Currency::USD);
}

// ============================================================================
// Serialization Tests (if serde feature enabled)
// ============================================================================

#[test]
fn test_basket_serialization_roundtrip() {
    // Arrange
    let basket = simple_equity_basket();

    // Act
    let json = serde_json::to_string(&basket).unwrap();
    let deserialized: Basket = serde_json::from_str(&json).unwrap();

    // Assert
    assert_eq!(basket.id, deserialized.id);
    assert_eq!(basket.expense_ratio, deserialized.expense_ratio);
    assert_eq!(basket.currency, deserialized.currency);
    assert_eq!(basket.constituents.len(), deserialized.constituents.len());
}

#[test]
fn test_asset_type_serialization() {
    // Arrange
    let types = vec![
        AssetType::Equity,
        AssetType::Bond,
        AssetType::ETF,
        AssetType::Cash,
        AssetType::Commodity,
        AssetType::Derivative,
    ];

    // Act & Assert
    for asset_type in types {
        let json = serde_json::to_string(&asset_type).unwrap();
        let deserialized: AssetType = serde_json::from_str(&json).unwrap();
        // Can't directly compare due to no PartialEq, but serialization should succeed
        let _ = deserialized;
    }
}

#[test]
fn test_basket_with_mixed_constituents_serialization() {
    use finstack_quant_valuations::instruments::json_loader::InstrumentJson;

    // Arrange - create a basket with both MarketData and Instrument constituents
    let bond = Bond::fixed(
        "CORP_BOND",
        usd(1000.0),
        0.05,
        date(2025, 1, 1),
        date(2030, 1, 1),
        "USD-OIS",
    )
    .unwrap();

    let basket = Basket {
        id: "MIXED_BASKET".into(),
        constituents: vec![
            BasketConstituent {
                id: "MARKET_DATA_EQUITY".to_string(),
                reference: ConstituentReference::MarketData {
                    price_id: "AAPL".into(),
                    asset_type: AssetType::Equity,
                },
                weight: 0.5,
                units: None,
                ticker: Some("AAPL".to_string()),
            },
            BasketConstituent {
                id: "INSTRUMENT_BOND".to_string(),
                reference: ConstituentReference::Instrument(Box::new(InstrumentJson::Bond(bond))),
                weight: 0.0,
                units: Some(10.0),
                ticker: Some("CORP".to_string()),
            },
        ],
        expense_ratio: 0.001,
        currency: Currency::USD,
        notional: usd(1_000_000.0),
        discount_curve_id: "USD-OIS".into(),
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Attributes::new(),
        pricing_config: BasketPricingConfig::default(),
    };

    // Act
    let json = serde_json::to_string_pretty(&basket).unwrap();
    let deserialized: Basket = serde_json::from_str(&json).unwrap();

    // Assert
    assert_eq!(basket.id, deserialized.id);
    assert_eq!(basket.expense_ratio, deserialized.expense_ratio);
    assert_eq!(basket.currency, deserialized.currency);
    assert_eq!(basket.constituents.len(), deserialized.constituents.len());
    assert_eq!(basket.constituents.len(), 2);

    // Verify first constituent is MarketData
    match &deserialized.constituents[0].reference {
        ConstituentReference::MarketData { price_id, .. } => {
            assert_eq!(price_id.as_str(), "AAPL");
        }
        ConstituentReference::Instrument(_) => panic!("Expected MarketData constituent"),
    }

    // Verify second constituent is Instrument
    match &deserialized.constituents[1].reference {
        ConstituentReference::Instrument(instr_json) => match instr_json.as_ref() {
            InstrumentJson::Bond(b) => {
                assert_eq!(b.id.as_str(), "CORP_BOND");
            }
            _ => panic!("Expected Bond instrument"),
        },
        ConstituentReference::MarketData { .. } => panic!("Expected Instrument constituent"),
    }
}

#[test]
fn test_constituent_reference_market_data_roundtrip() {
    // Arrange
    let reference = ConstituentReference::MarketData {
        price_id: "AAPL-SPOT".into(),
        asset_type: AssetType::Equity,
    };

    // Act
    let json = serde_json::to_string(&reference).unwrap();
    let deserialized: ConstituentReference = serde_json::from_str(&json).unwrap();

    // Assert
    match deserialized {
        ConstituentReference::MarketData { price_id, .. } => {
            assert_eq!(price_id.as_str(), "AAPL-SPOT");
        }
        ConstituentReference::Instrument(_) => panic!("Expected MarketData variant"),
    }
}

#[test]
fn test_constituent_reference_instrument_roundtrip() {
    use finstack_quant_valuations::instruments::json_loader::InstrumentJson;

    // Arrange
    let bond = Bond::fixed(
        "TEST_BOND",
        usd(1000.0),
        0.05,
        date(2025, 1, 1),
        date(2030, 1, 1),
        "USD-OIS",
    )
    .unwrap();
    let reference = ConstituentReference::Instrument(Box::new(InstrumentJson::Bond(bond)));

    // Act
    let json = serde_json::to_string(&reference).unwrap();
    let deserialized: ConstituentReference = serde_json::from_str(&json).unwrap();

    // Assert
    match deserialized {
        ConstituentReference::Instrument(instr_json) => match instr_json.as_ref() {
            InstrumentJson::Bond(b) => {
                assert_eq!(b.id.as_str(), "TEST_BOND");
            }
            _ => panic!("Expected Bond instrument"),
        },
        ConstituentReference::MarketData { .. } => panic!("Expected Instrument variant"),
    }
}

#[test]
fn test_basket_envelope_roundtrip_with_instruments() {
    use finstack_quant_valuations::instruments::json_loader::{InstrumentEnvelope, InstrumentJson};

    // Arrange
    let bond = Bond::fixed(
        "ENVELOPE_BOND",
        usd(1000.0),
        0.05,
        date(2025, 1, 1),
        date(2030, 1, 1),
        "USD-OIS",
    )
    .unwrap();

    let basket = Basket {
        id: "ENVELOPE_BASKET".into(),
        constituents: vec![BasketConstituent {
            id: "BOND_CONSTITUENT".to_string(),
            reference: ConstituentReference::Instrument(Box::new(InstrumentJson::Bond(bond))),
            weight: 0.0,
            units: Some(5.0),
            ticker: None,
        }],
        expense_ratio: 0.001,
        currency: Currency::USD,
        notional: usd(1_000_000.0),
        discount_curve_id: "USD-OIS".into(),
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Attributes::new(),
        pricing_config: BasketPricingConfig::default(),
    };

    let envelope = InstrumentEnvelope {
        schema: "finstack_quant.instrument/1".to_string(),
        instrument: InstrumentJson::Basket(basket.clone()),
    };

    // Act
    let json = serde_json::to_string_pretty(&envelope).unwrap();
    let deserialized: InstrumentEnvelope = serde_json::from_str(&json).unwrap();

    // Assert
    assert_eq!(deserialized.schema, envelope.schema);
    match deserialized.instrument {
        InstrumentJson::Basket(b) => {
            assert_eq!(b.id, basket.id);
            assert_eq!(b.expense_ratio, basket.expense_ratio);
            assert_eq!(b.constituents.len(), 1);
        }
        _ => panic!("Expected Basket variant"),
    }
}

// ============================================================================
// Calculator Reusability Tests
// ============================================================================

#[test]
fn test_calculator_can_be_reused() {
    // Arrange
    let basket1 = simple_equity_basket();
    let basket2 = simple_equity_basket();
    let context = equity_market_context();
    let calc = BasketCalculator::with_defaults();
    let as_of = date(2025, 1, 1);

    // Act
    let value1 = calc
        .basket_value(&basket1, &context, as_of, Some(100.0))
        .unwrap();
    let value2 = calc
        .basket_value(&basket2, &context, as_of, Some(100.0))
        .unwrap();

    // Assert
    assert_eq!(value1.amount(), value2.amount());
}

// ============================================================================
// Integration Tests
// ============================================================================

#[test]
fn test_full_pricing_workflow_units_based() {
    // Arrange
    let basket = Basket {
        id: "WORKFLOW_UNITS".into(),
        constituents: vec![
            BasketConstituent {
                id: "AAPL".to_string(),
                reference: ConstituentReference::MarketData {
                    price_id: "AAPL".into(),
                    asset_type: AssetType::Equity,
                },
                weight: 0.0,
                units: Some(50.0),
                ticker: Some("AAPL".to_string()),
            },
            BasketConstituent {
                id: "MSFT".to_string(),
                reference: ConstituentReference::MarketData {
                    price_id: "MSFT".into(),
                    asset_type: AssetType::Equity,
                },
                weight: 0.0,
                units: Some(30.0),
                ticker: Some("MSFT".to_string()),
            },
        ],
        expense_ratio: 0.01, // 1% annual
        currency: Currency::USD,
        notional: usd(1_000_000.0),
        discount_curve_id: "USD-OIS".into(),
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Attributes::new(),
        pricing_config: BasketPricingConfig::default(),
    };
    let context = equity_market_context();
    let as_of = date(2025, 1, 1);
    let shares = 1000.0;

    // Act - Calculate NAV
    let calc = basket.calculator();
    let nav = calc.nav(&basket, &context, as_of, shares).unwrap();

    // Assert
    // AAPL: 50 × $150 = $7,500
    // MSFT: 30 × $300 = $9,000
    // Total: $16,500
    // Expense: $16,500 × (0.01 / 365.25) ≈ $0.45
    // After fees: $16,499.55
    // NAV: $16,499.55 / 1000 = $16.50 (approximately)
    assert!((nav.amount() - 16.5).abs() < 0.01);
}

#[test]
fn test_full_pricing_workflow_weight_based() {
    // Arrange
    let basket = simple_equity_basket();
    let context = equity_market_context();
    let as_of = date(2025, 1, 1);
    let aum = usd(5_000_000.0);
    let shares = 100_000.0;

    // Act
    let calc = basket.calculator();
    let nav = calc
        .nav_with_aum(&basket, &context, as_of, aum, shares)
        .unwrap();

    // Assert
    // AUM: $5,000,000
    // Expense: $5,000,000 × (0.001 / 365.25) ≈ $13.69
    // After fees: $4,999,986.31
    // NAV: $4,999,986.31 / 100,000 ≈ $50.00
    assert!((nav.amount() - 50.0).abs() < 0.01);
}

#[test]
fn test_real_world_etf_scenario() {
    // Arrange - Simulate a tech-heavy ETF
    let basket = Basket {
        id: "TECH_ETF".into(),
        constituents: vec![
            BasketConstituent {
                id: "AAPL".to_string(),
                reference: ConstituentReference::MarketData {
                    price_id: "AAPL".into(),
                    asset_type: AssetType::Equity,
                },
                weight: 0.25,
                units: None,
                ticker: Some("AAPL".to_string()),
            },
            BasketConstituent {
                id: "MSFT".to_string(),
                reference: ConstituentReference::MarketData {
                    price_id: "MSFT".into(),
                    asset_type: AssetType::Equity,
                },
                weight: 0.25,
                units: None,
                ticker: Some("MSFT".to_string()),
            },
            BasketConstituent {
                id: "GOOGL".to_string(),
                reference: ConstituentReference::MarketData {
                    price_id: "GOOGL".into(),
                    asset_type: AssetType::Equity,
                },
                weight: 0.25,
                units: None,
                ticker: Some("GOOGL".to_string()),
            },
            BasketConstituent {
                id: "AMZN".to_string(),
                reference: ConstituentReference::MarketData {
                    price_id: "AMZN".into(),
                    asset_type: AssetType::Equity,
                },
                weight: 0.20,
                units: None,
                ticker: Some("AMZN".to_string()),
            },
            BasketConstituent {
                id: "CASH".to_string(),
                reference: ConstituentReference::MarketData {
                    price_id: "CASH".into(),
                    asset_type: AssetType::Cash,
                },
                weight: 0.05,
                units: None,
                ticker: None,
            },
        ],
        expense_ratio: 0.0009, // 9 bps (typical for equity ETF)
        currency: Currency::USD,
        notional: usd(1_000_000.0),
        discount_curve_id: "USD-OIS".into(),
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Attributes::new(),
        pricing_config: BasketPricingConfig::default(),
    };
    let context = equity_market_context();
    let as_of = date(2025, 1, 1);

    // Act
    assert!(basket.validate().is_ok());
    let aum = usd(10_000_000_000.0); // $10B AUM
    let shares = 200_000_000.0; // 200M shares
    let calc = basket.calculator();
    let nav = calc
        .nav_with_aum(&basket, &context, as_of, aum, shares)
        .unwrap();

    // Assert
    assert!(nav.amount() > 0.0);
    assert_eq!(nav.currency(), Currency::USD);
    // NAV should be around $50 ($10B / 200M shares)
    assert!((nav.amount() - 50.0).abs() < 1.0);
}
