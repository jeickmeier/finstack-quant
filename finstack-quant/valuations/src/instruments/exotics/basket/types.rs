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
use std::sync::OnceLock;

/// Type of asset in the basket
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
// Distinct from the structured-credit collateral `AssetType`.
#[cfg_attr(feature = "json-schema", schemars(rename = "BasketAssetType"))]
pub enum AssetType {
    /// Equity security
    Equity,
    /// Fixed income security
    Bond,
    /// Exchange-traded fund
    #[serde(rename = "etf")]
    ETF,
    /// Cash or cash equivalent
    Cash,
    /// Commodity
    Commodity,
    /// Derivative instrument
    Derivative,
}

/// Reference to a constituent asset in the basket
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
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

/// Runtime cache of boxed instrument constituents. Not serialized.
#[derive(Default)]
pub(crate) struct BoxedConstituentCache(OnceLock<Vec<Option<Box<dyn Instrument>>>>);

impl Clone for BoxedConstituentCache {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl std::fmt::Debug for BoxedConstituentCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BoxedConstituentCache")
    }
}

/// Individual constituent in a basket
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
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
    serde::Serialize,
    serde::Deserialize,
)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::InstrumentPricingOverrides::is_empty"
    )]
    pub instrument_pricing_overrides: crate::instruments::InstrumentPricingOverrides,
    /// Metric-only pricing controls.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::MetricPricingOverrides::is_empty"
    )]
    pub metric_pricing_overrides: crate::instruments::MetricPricingOverrides,
    /// Scenario-only valuation adjustments.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::ScenarioPricingOverrides::is_empty"
    )]
    pub scenario_pricing_overrides: crate::instruments::ScenarioPricingOverrides,
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,
    /// Pricing configuration
    pub pricing_config: BasketPricingConfig,
    /// Boxed instrument constituents materialized once per instance.
    #[serde(skip)]
    #[cfg_attr(feature = "json-schema", schemars(skip))]
    #[builder(default)]
    pub(crate) boxed_constituents: BoxedConstituentCache,
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
            .notional(Money::from((1_000_000_i64, Currency::USD)))
            .discount_curve_id(finstack_quant_core::types::CurveId::new("USD-OIS"))
            .attributes(Attributes::new())
            .pricing_config(BasketPricingConfig::default())
            .build()
    }

    /// Boxed instrument for an instrument-backed constituent, materialized once.
    ///
    /// # Arguments
    ///
    /// * `index` - Zero-based constituent position, aligned with [`Self::constituents`].
    pub(crate) fn boxed_constituent_at(&self, index: usize) -> Result<Option<&dyn Instrument>> {
        let constituent_count = self.constituents.len();
        if index >= constituent_count {
            return Err(finstack_quant_core::Error::Validation(format!(
                "basket constituent index {index} is out of range for basket with {constituent_count} constituents"
            )));
        }
        let cache = self.ensure_boxed_constituents()?;
        Ok(cache[index].as_deref())
    }

    fn ensure_boxed_constituents(&self) -> Result<&[Option<Box<dyn Instrument>>]> {
        if let Some(cache) = self.boxed_constituents.0.get() {
            return Ok(cache.as_slice());
        }
        let cache = self
            .constituents
            .iter()
            .map(|c| match &c.reference {
                ConstituentReference::Instrument(json) => {
                    Ok(Some(json.as_ref().clone().into_boxed()?))
                }
                ConstituentReference::MarketData { .. } => Ok(None),
            })
            .collect::<Result<Vec<_>>>()?;
        let _ = self.boxed_constituents.0.set(cache);
        self.boxed_constituents
            .0
            .get()
            .map(Vec::as_slice)
            .ok_or_else(|| {
                finstack_quant_core::Error::Internal(
                    "boxed basket constituents cache missing after initialization".into(),
                )
            })
    }

    /// Create an example basket with instrument-backed constituents.
    pub fn example_with_instruments() -> finstack_quant_core::Result<Self> {
        use finstack_quant_core::currency::Currency;
        use finstack_quant_core::money::Money;
        use time::macros::date;

        // Create a bond instrument
        let bond = crate::instruments::fixed_income::bond::Bond::fixed(
            "CORP-BOND-001",
            Money::from((1000_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(0.05)?,
            date!(2024 - 01 - 01),
            date!(2034 - 01 - 01),
            finstack_quant_core::dates::StubKind::ShortFront,
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
            .notional(Money::from((1_000_000_i64, Currency::USD)))
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

    /// Validate basket consistency.
    ///
    /// Weight totals are intentionally unrestricted: the pricing contract
    /// supports partially invested and levered weight baskets, as well as
    /// unit-only and mixed unit/weight baskets. Validation instead enforces
    /// the structural invariants shared by all of those modes.
    pub fn validate(&self) -> Result<()> {
        validation::require(
            !self.constituents.is_empty(),
            "basket must contain at least one constituent",
        )?;
        validation::require(
            self.constituents.iter().all(|constituent| {
                constituent.weight.is_finite() && constituent.units.is_none_or(f64::is_finite)
            }),
            "basket constituent weights and units must be finite",
        )?;
        validation::require(
            self.pricing_config.days_in_year.is_finite() && self.pricing_config.days_in_year > 0.0,
            "basket pricing days_in_year must be finite and positive",
        )?;

        validation::require(
            self.notional.currency() == self.currency,
            "basket notional currency must match basket currency",
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
                    deps.add_market_scalar_id(price_id.as_str());
                }
            }
        }
        Ok(deps)
    }

    fn base_value(&self, curves: &MarketContext, as_of: Date) -> Result<Money> {
        self.validate()?;
        // Scale NAV-per-unit by explicit basket notional for portfolio PV.
        let nav_per_unit = self.calculator().nav(self, curves, as_of, 1.0)?;
        let scaled = nav_per_unit.amount() * self.notional.amount();
        Money::new(scaled, self.notional.currency())
    }

    fn effective_start_date(&self) -> Option<Date> {
        None
    }

    crate::impl_focused_pricing_overrides!();
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
            notional: Money::from((1_000_000_i64, Currency::USD)),
            discount_curve_id: "USD-OIS".into(),
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Attributes::new(),
            pricing_config: BasketPricingConfig::default(),
            boxed_constituents: BoxedConstituentCache::default(),
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
            notional: Money::from((1_000_000_i64, Currency::USD)),
            discount_curve_id: "USD-OIS".into(),
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Attributes::new(),
            pricing_config: BasketPricingConfig::default(),
            boxed_constituents: BoxedConstituentCache::default(),
        };

        // Fully invested, partially invested, and levered baskets are valid.
        assert!(basket.validate().is_ok());

        basket.constituents[0].weight = 0.8;
        assert!(basket.validate().is_ok());

        basket.constituents[0].weight = 1.2;
        assert!(basket.validate().is_ok());

        // Non-finite allocations cannot produce a meaningful value.
        basket.constituents[0].weight = f64::NAN;
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
            deps.market_scalar_ids,
            vec!["AAPL-SPOT".to_string(), "UST10Y-PRICE".to_string()]
        );
    }

    #[test]
    fn boxed_constituent_at_rejects_out_of_range_index() {
        let basket = Basket::example().expect("example");
        let Err(error) = basket.boxed_constituent_at(2) else {
            panic!("index beyond the constituents must fail")
        };

        assert!(matches!(error, finstack_quant_core::Error::Validation(_)));
        assert!(error.to_string().contains("index 2"));
        assert!(error.to_string().contains("2 constituents"));
    }
}
