//! Generic basket instrument for ETFs and equity/bond baskets.
//!
//! This module provides a unified basket instrument that can handle various asset types
//! including equities, bonds, ETFs, and other instruments by leveraging existing
//! pricing infrastructure.

use crate::instruments::common_impl::traits::{Attributes, Instrument};
use crate::instruments::common_impl::validation;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::{fx::FxConversionPolicy, Money};
use finstack_quant_core::types::{InstrumentId, PriceId};
use finstack_quant_core::Result;

use crate::instruments::json_loader::InstrumentJson;

use crate::impl_instrument_base;
use serde::{Deserialize, Serialize};

/// Type of asset in the basket
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AssetType {
    /// Equity security
    Equity,
    /// Fixed income security
    Bond,
    /// Exchange-traded fund
    ETF,
    /// Cash or cash equivalent
    Cash,
    /// Commodity
    Commodity,
    /// Derivative instrument
    Derivative,
}

/// Reference to a constituent asset in the basket
#[derive(Debug, Clone)]
pub enum ConstituentReference {
    /// Direct reference to an existing instrument (serializable via InstrumentJson)
    Instrument(Box<InstrumentJson>),
    /// Market data reference for simple price lookups
    MarketData {
        /// Price identifier in MarketContext
        price_id: PriceId,
        /// Type of asset for validation
        asset_type: AssetType,
    },
}

// Debug is now derived automatically on ConstituentReference

impl schemars::JsonSchema for ConstituentReference {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("ConstituentReference")
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "object"
        })
    }
}

impl Serialize for ConstituentReference {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            ConstituentReference::Instrument(instr_json) => {
                // Serialize as { "instrument": <InstrumentJson> }
                #[derive(Serialize)]
                struct InstrumentWrapper<'a> {
                    instrument: &'a InstrumentJson,
                }
                let wrapper = InstrumentWrapper {
                    instrument: instr_json,
                };
                wrapper.serialize(serializer)
            }
            ConstituentReference::MarketData {
                price_id,
                asset_type,
            } => {
                // Serialize as { "price_id": "...", "asset_type": "..." }
                #[derive(Serialize)]
                struct MarketDataRef<'a> {
                    price_id: &'a str,
                    asset_type: &'a AssetType,
                }
                let market_ref = MarketDataRef {
                    price_id: price_id.as_str(),
                    asset_type,
                };
                market_ref.serialize(serializer)
            }
        }
    }
}

impl<'de> Deserialize<'de> for ConstituentReference {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Use an untagged helper enum to disambiguate between the two shapes
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Helper {
            Instrument {
                instrument: Box<InstrumentJson>,
            },
            MarketData {
                price_id: PriceId,
                asset_type: AssetType,
            },
        }

        let helper = Helper::deserialize(deserializer)?;
        match helper {
            Helper::Instrument { instrument } => Ok(ConstituentReference::Instrument(instrument)),
            Helper::MarketData {
                price_id,
                asset_type,
            } => Ok(ConstituentReference::MarketData {
                price_id,
                asset_type,
            }),
        }
    }
}

/// Individual constituent in a basket
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BasketConstituent {
    /// Unique identifier for the constituent
    pub id: String,
    /// Reference to the underlying asset
    pub reference: ConstituentReference,
    /// Weight in the basket (as a fraction, e.g., 0.05 = 5%)
    pub weight: f64,
    /// Number of units for physical replication (optional)
    pub units: Option<f64>,
    /// Optional ticker symbol for reporting
    pub ticker: Option<String>,
}

/// Configuration for basket pricing behaviour.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BasketPricingConfig {
    /// Day basis used for fee accrual (e.g., 365.0 or 365.25). Avoid hardcoding in logic.
    pub days_in_year: f64,
    /// FX policy hint for conversions when constituent currency != basket currency.
    pub fx_policy: FxConversionPolicy,
}

impl Default for BasketPricingConfig {
    fn default() -> Self {
        Self {
            days_in_year: 365.25,
            fx_policy: FxConversionPolicy::CashflowDate,
        }
    }
}

/// Simplified basket instrument focused on pricing essentials.
///
/// This basket represents a collection of financial instruments or market data references
/// that can be valued as a portfolio. It focuses purely on pricing functionality without
/// ETF-specific operational features like creation/redemption mechanics.
#[derive(
    Debug,
    Clone,
    finstack_quant_valuations_macros::FinancialBuilder,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Basket {
    /// Unique instrument identifier
    pub id: InstrumentId,
    /// Basket constituents (the actual holdings)
    pub constituents: Vec<BasketConstituent>,
    /// Total expense ratio (as decimal, e.g., 0.0025 = 0.25%)
    /// This affects pricing through expense drag calculations
    pub expense_ratio: f64,
    /// Base currency of the basket
    pub currency: Currency,
    /// Position notional used to scale basket NAV to portfolio PV.
    pub notional: Money,
    /// Discount curve identifier for present value calculations
    pub discount_curve_id: finstack_quant_core::types::CurveId,
    /// Attributes for scenario selection and tagging
    #[serde(default)]
    #[builder(default)]
    pub pricing_overrides: crate::instruments::PricingOverrides,
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,
    /// Pricing configuration
    pub pricing_config: BasketPricingConfig,
}

impl Basket {
    // Builder provided by derive
    /// Create a canonical example basket with two market data constituents.
    pub fn example() -> finstack_quant_core::Result<Self> {
        use finstack_quant_core::currency::Currency;
        let constituents = vec![
            BasketConstituent {
                id: "EQ-AAPL".to_string(),
                reference: ConstituentReference::MarketData {
                    price_id: PriceId::new("AAPL-SPOT"),
                    asset_type: AssetType::Equity,
                },
                weight: 0.6,
                units: None,
                ticker: Some("AAPL".to_string()),
            },
            BasketConstituent {
                id: "BOND-UST10".to_string(),
                reference: ConstituentReference::MarketData {
                    price_id: PriceId::new("UST10Y-PRICE"),
                    asset_type: AssetType::Bond,
                },
                weight: 0.4,
                units: None,
                ticker: Some("UST10Y".to_string()),
            },
        ];
        Basket::builder()
            .id(InstrumentId::new("BASKET-60-40"))
            .constituents(constituents)
            .expense_ratio(0.0025)
            .currency(Currency::USD)
            .notional(Money::new(1_000_000.0, Currency::USD))
            .discount_curve_id(finstack_quant_core::types::CurveId::new("USD-OIS"))
            .attributes(Attributes::new())
            .pricing_config(BasketPricingConfig::default())
            .build()
    }

    /// Create an example basket with instrument-backed constituents.
    pub fn example_with_instruments() -> finstack_quant_core::Result<Self> {
        use finstack_quant_core::currency::Currency;
        use finstack_quant_core::money::Money;
        use time::macros::date;

        // Create a bond instrument
        let bond = crate::instruments::fixed_income::bond::Bond::fixed(
            "CORP-BOND-001",
            Money::new(1000.0, Currency::USD),
            0.05,
            date!(2024 - 01 - 01),
            date!(2034 - 01 - 01),
            "USD-OIS",
        )?;

        let constituents = vec![
            BasketConstituent {
                id: "BOND-CORP".to_string(),
                reference: ConstituentReference::Instrument(Box::new(
                    crate::instruments::json_loader::InstrumentJson::Bond(bond),
                )),
                weight: 0.0,
                units: Some(100.0),
                ticker: Some("CORP".to_string()),
            },
            BasketConstituent {
                id: "EQ-AAPL".to_string(),
                reference: ConstituentReference::MarketData {
                    price_id: PriceId::new("AAPL-SPOT"),
                    asset_type: AssetType::Equity,
                },
                weight: 1.0,
                units: None,
                ticker: Some("AAPL".to_string()),
            },
        ];

        Basket::builder()
            .id(InstrumentId::new("BASKET-MIXED"))
            .constituents(constituents)
            .expense_ratio(0.001)
            .currency(Currency::USD)
            .notional(Money::new(1_000_000.0, Currency::USD))
            .discount_curve_id(finstack_quant_core::types::CurveId::new("USD-OIS"))
            .attributes(Attributes::new())
            .pricing_config(BasketPricingConfig::default())
            .build()
    }

    /// Create a new basket with custom pricing configuration.
    pub fn with_pricing_config(mut self, config: BasketPricingConfig) -> Self {
        self.pricing_config = config;
        self
    }

    /// Get a configured calculator for this basket.
    ///
    /// This centralizes calculator creation and avoids duplication across
    /// metrics, pricers, and other components.
    pub fn calculator(&self) -> crate::instruments::exotics::basket::pricer::BasketCalculator {
        crate::instruments::exotics::basket::pricer::BasketCalculator::new(
            self.pricing_config.clone(),
        )
    }

    /// Get constituent by ID
    pub fn get_constituent(&self, id: &str) -> Option<&BasketConstituent> {
        self.constituents.iter().find(|c| c.id == id)
    }

    /// Get total number of constituents
    pub fn constituent_count(&self) -> usize {
        self.constituents.len()
    }

    /// Validate basket consistency (weights sum to ~1.0, currency consistency, etc.)
    ///
    /// Weight tolerance is 10bp (0.001), which is tighter than the common 1%
    /// tolerance to catch misconfigured baskets early. A basket with weights
    /// summing to 0.999 or 1.001 is accepted; 0.99 or 1.01 is rejected.
    pub fn validate(&self) -> Result<()> {
        // Check weight sum (10bp tolerance)
        let total_weight: f64 = self.constituents.iter().map(|c| c.weight).sum();
        validation::require_or(
            (total_weight - 1.0).abs() <= 0.001,
            finstack_quant_core::InputError::Invalid,
        )?;

        // Validate each constituent's currency compatibility would happen
        // during pricing through the existing instrument validation
        validation::require_or(
            self.notional.currency() == self.currency,
            finstack_quant_core::InputError::Invalid,
        )?;

        Ok(())
    }
}

// Implement traits manually to handle InstrumentId properly
impl Instrument for Basket {
    impl_instrument_base!(crate::pricer::InstrumentType::Basket);

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        self.validate()
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<
        crate::instruments::common_impl::dependencies::MarketDependencies,
    > {
        let mut deps = crate::instruments::common_impl::dependencies::MarketDependencies::new();
        deps.add_discount_curve(self.discount_curve_id.clone());
        for constituent in &self.constituents {
            match &constituent.reference {
                ConstituentReference::Instrument(instrument) => deps.merge(
                    crate::instruments::common_impl::dependencies::MarketDependencies::from_instrument_json(
                        instrument,
                    )?,
                ),
                ConstituentReference::MarketData { price_id, .. } => {
                    deps.add_spot_id(price_id.as_str());
                }
            }
        }
        Ok(deps)
    }

    // === Pricing Methods ===

    fn base_value(&self, curves: &MarketContext, as_of: Date) -> Result<Money> {
        self.validate()?;
        // Scale NAV-per-unit by explicit basket notional for portfolio PV.
        let nav_per_unit = self.calculator().nav(self, curves, as_of, 1.0)?;
        let scaled = nav_per_unit.amount() * self.notional.amount();
        Ok(Money::new(scaled, self.notional.currency()))
    }

    fn effective_start_date(&self) -> Option<Date> {
        None
    }

    fn pricing_overrides_mut(
        &mut self,
    ) -> Option<&mut crate::instruments::pricing_overrides::PricingOverrides> {
        Some(&mut self.pricing_overrides)
    }

    fn pricing_overrides(
        &self,
    ) -> Option<&crate::instruments::pricing_overrides::PricingOverrides> {
        Some(&self.pricing_overrides)
    }
}

// Declare canonical market dependencies for the DV01 calculator.
crate::impl_empty_cashflow_provider!(
    Basket,
    crate::cashflow::builder::CashflowRepresentation::Placeholder
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basket_creation() {
        let basket = Basket {
            id: InstrumentId::new("TEST_BASKET"),
            constituents: vec![],
            expense_ratio: 0.001,
            currency: Currency::USD,
            notional: Money::new(1_000_000.0, Currency::USD),
            discount_curve_id: "USD-OIS".into(),
            pricing_overrides: crate::instruments::PricingOverrides::default(),
            attributes: Attributes::new(),
            pricing_config: BasketPricingConfig::default(),
        };

        assert_eq!(basket.id.as_str(), "TEST_BASKET");
        assert_eq!(basket.expense_ratio, 0.001);
    }

    #[test]
    fn test_validate_weights() {
        let mut basket = Basket {
            id: InstrumentId::new("TEST"),
            constituents: vec![
                BasketConstituent {
                    id: "CONST1".to_string(),
                    reference: ConstituentReference::MarketData {
                        price_id: "AAPL".to_string().into(),
                        asset_type: AssetType::Equity,
                    },
                    weight: 0.6,
                    units: None,
                    ticker: Some("AAPL".to_string()),
                },
                BasketConstituent {
                    id: "CONST2".to_string(),
                    reference: ConstituentReference::MarketData {
                        price_id: "MSFT".to_string().into(),
                        asset_type: AssetType::Equity,
                    },
                    weight: 0.4,
                    units: None,
                    ticker: Some("MSFT".to_string()),
                },
            ],
            expense_ratio: 0.001,
            currency: Currency::USD,
            notional: Money::new(1_000_000.0, Currency::USD),
            discount_curve_id: "USD-OIS".into(),
            pricing_overrides: crate::instruments::PricingOverrides::default(),
            attributes: Attributes::new(),
            pricing_config: BasketPricingConfig::default(),
        };

        // Should pass with weights summing to 1.0
        assert!(basket.validate().is_ok());

        // Should fail with weights not summing to ~1.0 (10bp tolerance)
        basket.constituents[0].weight = 0.8;
        assert!(basket.validate().is_err());

        // Edge: just within 10bp tolerance should pass
        basket.constituents[0].weight = 0.6005;
        assert!(basket.validate().is_ok());

        // Edge: just outside 10bp tolerance should fail
        basket.constituents[0].weight = 0.602;
        assert!(basket.validate().is_err());
    }

    #[test]
    fn canonical_dependencies_include_constituent_prices() {
        let basket = Basket::example().expect("example");
        let deps =
            crate::instruments::Instrument::market_dependencies(&basket).expect("dependencies");

        assert_eq!(
            deps.curves.discount_curves.as_slice(),
            &[basket.discount_curve_id]
        );
        assert_eq!(
            deps.spot_ids,
            vec!["AAPL-SPOT".to_string(), "UST10Y-PRICE".to_string()]
        );
    }
}
