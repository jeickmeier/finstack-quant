//! Bond future core types.
//!
//! This module defines the data structures for bond futures, including
//! the deliverable basket, contract specifications, and the main BondFuture type.

use crate::cashflow::builder::CashFlowSchedule;
use crate::contract_specs::{embedded_registry, ContractSpecRegistry};
use crate::impl_instrument_base;
use crate::instruments::common_impl::dependencies::MarketDependencies;
use crate::instruments::common_impl::traits::Attributes;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};

// Re-export the canonical position type from the historical module path.
pub use crate::instruments::Position;

/// Day-count basis used to annualize implied repo rates.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum RepoDayCountBasis {
    /// ACT/360 convention (common in USD and EUR money markets)
    Act360,
    /// ACT/365 convention (common in GBP money markets)
    Act365,
}

impl RepoDayCountBasis {
    fn annualization_denominator(self) -> f64 {
        match self {
            Self::Act360 => 360.0,
            Self::Act365 => 365.0,
        }
    }
}

/// A bond in the deliverable basket with its conversion factor.
///
/// Each bond future contract has a basket of deliverable bonds that can be delivered
/// to satisfy the contract. The conversion factor normalizes bonds with different
/// coupons and maturities to a standard notional bond.
///
/// # Conversion Factor Requirements
///
/// **IMPORTANT**: Conversion factors must match the values published by the exchange
/// (CME for UST, Eurex for Bund, ICE for Gilt). The implementation does **not**
/// calculate CFs internally - they must be sourced from official exchange publications.
///
/// The CF calculation formula (using 6% notional yield for UST/Bund, 4% for Gilt)
/// is documented in exchange rulebooks:
/// - **CME**: CBOT US Treasury Futures Contract Specifications
/// - **Eurex**: Euro-Bund Futures Contract Specifications
/// - **ICE**: Long Gilt Futures Contract Specifications
///
/// Using incorrect CFs will result in material pricing errors for invoice price,
/// CTD determination, and basis calculations.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations::instruments::fixed_income::bond_future::DeliverableBond;
/// use finstack_quant_core::types::InstrumentId;
///
/// // CF from CME publication for a specific deliverable bond
/// let deliverable = DeliverableBond {
///     bond_id: InstrumentId::new("US912828XG33"),
///     conversion_factor: 0.8234,  // Must match CME-published value
/// };
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct DeliverableBond {
    /// Identifier of the deliverable bond
    pub bond_id: InstrumentId,
    /// Conversion factor for this bond.
    ///
    /// **Must match exchange-published value**. CFs are not calculated internally.
    /// See struct-level documentation for exchange references.
    pub conversion_factor: f64,
}

/// Contract specifications for bond futures.
///
/// Defines the standard parameters for a bond future contract including
/// contract size, tick size, and the notional bond parameters used for
/// conversion factor calculations.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations::instruments::fixed_income::bond_future::BondFutureSpecs;
///
/// // UST 10-year contract specs
/// let specs = BondFutureSpecs::default(); // UST 10Y defaults
/// assert_eq!(specs.contract_size, 100_000.0);
/// assert_eq!(specs.standard_coupon, 0.06);
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct BondFutureSpecs {
    /// Face value of a single contract (e.g., $100,000 for UST)
    pub contract_size: f64,
    /// Minimum price increment (e.g., 1/32 = 0.03125 for UST)
    pub tick_size: f64,
    /// Value of one tick in currency units (e.g., $31.25 for UST)
    pub tick_value: f64,
    /// Standard coupon rate for conversion factor calculation (e.g., 0.06 for 6%)
    pub standard_coupon: f64,
    /// Standard maturity in years for conversion factor calculation
    pub standard_maturity_years: f64,
    /// Number of business days for settlement after expiry
    pub settlement_days: u32,
    /// Holiday calendar identifier for business day calculations.
    ///
    /// Defaults to "nyse" for US Treasury futures.
    /// Use "target2" for European government bond futures.
    #[serde(default = "default_calendar_id")]
    pub calendar_id: String,
    /// Day-count basis for implied repo rate annualization.
    #[serde(default = "default_repo_day_count_basis")]
    pub repo_day_count_basis: RepoDayCountBasis,
}

fn default_calendar_id() -> String {
    "nyse".to_string()
}

fn default_repo_day_count_basis() -> RepoDayCountBasis {
    RepoDayCountBasis::Act360
}

#[allow(clippy::expect_used)]
fn contract_spec_registry() -> &'static ContractSpecRegistry {
    embedded_registry().expect("embedded contract-spec registry should load")
}

#[allow(clippy::expect_used)]
fn bond_future_specs_from_registry(id: &str) -> BondFutureSpecs {
    contract_spec_registry()
        .bond_future_specs(id)
        .expect("embedded bond future contract spec should exist")
}

impl Default for BondFutureSpecs {
    /// Default specifications for UST 10-year futures.
    ///
    /// Standard parameters:
    /// - Contract size: $100,000
    /// - Tick size: 1/2 of 1/32 (half-32nd, 0.015625)
    /// - Tick value: $15.625
    /// - Standard coupon: 6% (0.06)
    /// - Standard maturity: 10 years
    /// - Settlement: 2 business days
    /// - Calendar: NYSE (New York Stock Exchange)
    fn default() -> Self {
        Self::ust_10y()
    }
}

impl BondFutureSpecs {
    /// UST 10-year futures contract specifications.
    ///
    /// **Market**: U.S. Treasury
    /// **Exchange**: Chicago Board of Trade (CBOT)
    /// **Contract**: 10-Year T-Note Futures
    ///
    /// # Specifications
    ///
    /// - Contract size: $100,000
    /// - Tick size: 1/2 of 1/32 (half-32nd, 0.015625)
    /// - Tick value: $15.625 per tick
    /// - Standard coupon: 6% annual
    /// - Standard maturity: 10 years
    /// - Settlement: T+2 business days
    /// - Day count: Actual/Actual (ISDA)
    /// - Deliverable: U.S. Treasury notes with at least 6.5 years remaining maturity
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_valuations::instruments::fixed_income::bond_future::BondFutureSpecs;
    ///
    /// let specs = BondFutureSpecs::ust_10y();
    /// assert_eq!(specs.contract_size, 100_000.0);
    /// assert_eq!(specs.standard_coupon, 0.06);
    /// ```
    pub fn ust_10y() -> Self {
        bond_future_specs_from_registry("cme.ust_10y")
    }

    /// UST 5-year futures contract specifications.
    ///
    /// **Market**: U.S. Treasury
    /// **Exchange**: Chicago Board of Trade (CBOT)
    /// **Contract**: 5-Year T-Note Futures
    ///
    /// # Specifications
    ///
    /// - Contract size: $100,000
    /// - Tick size: 1/4 of 1/32 of a point (0.0078125)
    /// - Tick value: $7.8125 per tick
    /// - Standard coupon: 6% annual
    /// - Standard maturity: 5 years
    /// - Settlement: T+2 business days
    /// - Day count: Actual/Actual (ISDA)
    /// - Deliverable: U.S. Treasury notes with at least 4 years, 2 months remaining maturity
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_valuations::instruments::fixed_income::bond_future::BondFutureSpecs;
    ///
    /// let specs = BondFutureSpecs::ust_5y();
    /// assert_eq!(specs.contract_size, 100_000.0);
    /// assert_eq!(specs.tick_size, 1.0 / 128.0);
    /// assert_eq!(specs.standard_maturity_years, 5.0);
    /// ```
    pub fn ust_5y() -> Self {
        bond_future_specs_from_registry("cme.ust_5y")
    }

    /// UST 2-year futures contract specifications.
    ///
    /// **Market**: U.S. Treasury
    /// **Exchange**: Chicago Board of Trade (CBOT)
    /// **Contract**: 2-Year T-Note Futures
    ///
    /// # Specifications
    ///
    /// - Contract size: $200,000 (note: double the 5Y/10Y contracts)
    /// - Tick size: 1/8 of 1/32 of a point (0.00390625)
    /// - Tick value: $7.8125 per tick
    /// - Standard coupon: 6% annual
    /// - Standard maturity: 2 years
    /// - Settlement: T+2 business days
    /// - Day count: Actual/Actual (ISDA)
    /// - Deliverable: U.S. Treasury notes with at least 1 year, 9 months remaining maturity
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_valuations::instruments::fixed_income::bond_future::BondFutureSpecs;
    ///
    /// let specs = BondFutureSpecs::ust_2y();
    /// assert_eq!(specs.contract_size, 200_000.0);
    /// assert_eq!(specs.tick_size, 1.0 / 256.0);
    /// assert_eq!(specs.standard_maturity_years, 2.0);
    /// ```
    pub fn ust_2y() -> Self {
        bond_future_specs_from_registry("cme.ust_2y")
    }

    /// German Bund futures contract specifications.
    ///
    /// **Market**: Germany (Eurex)
    /// **Exchange**: Eurex Exchange
    /// **Contract**: Euro-Bund Futures
    ///
    /// # Specifications
    ///
    /// - Contract size: €100,000
    /// - Tick size: 0.01 (1 basis point)
    /// - Tick value: €10 per tick
    /// - Standard coupon: 6% annual
    /// - Standard maturity: 10 years
    /// - Settlement: T+2 business days
    /// - Day count: Actual/Actual (ISDA)
    /// - Deliverable: German Federal bonds with 8.5 to 10.5 years remaining maturity
    ///
    /// # Notes
    ///
    /// - Quoted in percentage points (e.g., 125.50 = 125.50%)
    /// - Different tick size from UST (decimal vs. 32nds)
    /// - Settlement follows TARGET2 calendar
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_valuations::instruments::fixed_income::bond_future::BondFutureSpecs;
    ///
    /// let specs = BondFutureSpecs::bund();
    /// assert_eq!(specs.contract_size, 100_000.0);
    /// assert_eq!(specs.tick_size, 0.01);
    /// assert_eq!(specs.tick_value, 10.0);
    /// ```
    pub fn bund() -> Self {
        bond_future_specs_from_registry("eurex.bund")
    }

    /// UK Gilt futures contract specifications.
    ///
    /// **Market**: United Kingdom
    /// **Exchange**: ICE Futures Europe (LIFFE)
    /// **Contract**: Long Gilt Futures
    ///
    /// # Specifications
    ///
    /// - Contract size: £100,000
    /// - Tick size: 0.01 (1 basis point)
    /// - Tick value: £10 per tick
    /// - Standard coupon: 4% annual (note: different from UST/Bund 6%)
    /// - Standard maturity: 10 years
    /// - Settlement: T+2 business days
    /// - Day count: Actual/Actual (ISDA)
    /// - Deliverable: UK Gilts with 8.75 to 13 years remaining maturity
    ///
    /// # Notes
    ///
    /// - Quoted in percentage points (e.g., 125.50 = 125.50%)
    /// - Standard coupon is 4%, not 6% like other major markets
    /// - Settlement follows UK bank holidays
    /// - Long Gilt contract covers 8.75-13 year maturity range
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_valuations::instruments::fixed_income::bond_future::BondFutureSpecs;
    ///
    /// let specs = BondFutureSpecs::gilt();
    /// assert_eq!(specs.contract_size, 100_000.0);
    /// assert_eq!(specs.tick_size, 0.01);
    /// assert_eq!(specs.standard_coupon, 0.04);  // 4%, not 6%
    /// ```
    pub fn gilt() -> Self {
        bond_future_specs_from_registry("ice.gilt")
    }
}

/// Bond future instrument.
///
/// A bond future is a standardized contract to buy or sell a government bond at a
/// specified price on a future date. The contract has a basket of deliverable bonds,
/// each with a conversion factor. The holder of the short position chooses which
/// bond to deliver (typically the Cheapest-to-Deliver or CTD bond).
///
/// # Contract Mechanics
///
/// - **Deliverable Basket**: Multiple bonds eligible for delivery
/// - **Conversion Factors**: Published by exchange to normalize different bonds
/// - **CTD Selection**: Short side chooses which bond to deliver (user-specified in this implementation)
/// - **Invoice Price**: (Futures Price × Conversion Factor) + Accrued Interest
///
/// # Examples
///
/// ```ignore
/// use finstack_quant_valuations::instruments::fixed_income::bond_future::{
///     BondFuture, BondFutureBuilder, BondFutureSpecs, DeliverableBond, Position,
/// };
/// use finstack_quant_core::money::Money;
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::dates::Date;
/// use finstack_quant_core::types::{InstrumentId, CurveId};
/// use time::Month;
///
/// // Create a UST 10-year future
/// let future = BondFuture::builder()
///     .id(InstrumentId::new("TYH5"))
///     .notional(Money::new(1_000_000.0, Currency::USD))
///     .expiry(Date::from_calendar_date(2025, Month::March, 20).unwrap())
///     .delivery_start(Date::from_calendar_date(2025, Month::March, 21).unwrap())
///     .delivery_end(Date::from_calendar_date(2025, Month::March, 31).unwrap())
///     .quoted_price(125.50)
///     .position(Position::Long)
///     .contract_specs(BondFutureSpecs::ust_10y())
///     .deliverable_basket(vec![
///         DeliverableBond {
///             bond_id: InstrumentId::new("US912828XG33"),
///             conversion_factor: 0.8234,
///         },
///     ])
///     .ctd_bond_id(InstrumentId::new("US912828XG33"))
///     .discount_curve_id(CurveId::new("USD-TREASURY"))
///     .build()
///     .expect("Valid bond future");
/// ```
#[derive(
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    finstack_quant_valuations_macros::FocusedPricingOverrides,
)]
#[serde(deny_unknown_fields)]
pub struct BondFuture {
    /// Unique identifier for the contract
    pub id: InstrumentId,

    /// Notional exposure in currency units.
    /// For multiple contracts, use notional = contract_specs.contract_size × num_contracts
    pub notional: Money,

    /// Future expiry date (last trading day)
    #[schemars(with = "String")]
    pub expiry: Date,

    /// First delivery date
    #[schemars(with = "String")]
    pub delivery_start: Date,

    /// Last delivery date
    #[schemars(with = "String")]
    pub delivery_end: Date,

    /// Contract/entry futures price (e.g., 125.50 for 125-16/32).
    ///
    /// `base_value` returns model-minus-contract value for a long position.
    /// Current-settlement variation margin is a separate cash-P&L workflow.
    pub quoted_price: f64,

    /// Position side (Long or Short)
    pub position: Position,

    /// Contract specifications (tick size, standard coupon, etc.)
    pub contract_specs: BondFutureSpecs,

    /// Basket of deliverable bonds with conversion factors
    pub deliverable_basket: Vec<DeliverableBond>,

    /// Cheapest-to-Deliver (CTD) bond identifier.
    ///
    /// The CTD bond is the deliverable bond that maximizes the implied repo rate
    /// (or equivalently, minimizes the basis). Users can:
    ///
    /// 1. **Specify directly**: Set this to a known CTD bond ID
    /// 2. **Calculate automatically**: Use [`BondFuture::determine_ctd`] with bond clean prices
    ///
    /// # CTD Selection Methodology
    ///
    /// The CTD is determined by comparing the **net basis** for each deliverable bond:
    ///
    /// ```text
    /// Net Basis = Clean Price - (Futures Price × Conversion Factor)
    /// ```
    ///
    /// The bond with the **lowest net basis** (or highest implied repo) is the CTD.
    /// In practice, this is usually the bond with:
    /// - Highest duration (in a rising rate environment)
    /// - Lowest duration (in a falling rate environment)
    /// - Lowest coupon (when yields are above the notional coupon)
    ///
    /// # Production Note
    ///
    /// For production systems, use [`BondFuture::determine_ctd`] or integrate with
    /// a real-time CTD analysis service. The CTD can change throughout the day as
    /// bond prices and repo rates fluctuate.
    ///
    /// Optional to support workflow where CTD is selected by pricing logic.
    /// If omitted, the engine resolves CTD as:
    /// 1) `ctd_bond.id` when embedded CTD bond is provided
    /// 2) single deliverable bond in basket when basket length is 1
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ctd_bond_id: Option<InstrumentId>,

    /// Optional embedded CTD bond definition.
    ///
    /// When present, the bond future can be priced without relying on any external
    /// instrument registry. This keeps `finstack_quant_core::market_data::MarketContext`
    /// purely market-data focused (curves/surfaces/scalars) and fully serializable.
    ///
    /// If `None`, pricing will return a validation error instructing callers to
    /// provide the CTD bond at the pricing boundary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    pub ctd_bond: Option<crate::instruments::fixed_income::bond::Bond>,

    /// Discount curve identifier for present value calculations
    pub discount_curve_id: CurveId,

    /// Optional repo/financing curve identifier.
    ///
    /// When set, this curve is used for implied repo rate calculations and
    /// carry analysis instead of the general discount curve. This allows
    /// capturing repo specials, where specific collateral (e.g., on-the-run
    /// Treasuries) trades at rates different from the general funding curve.
    ///
    /// If `None`, the `discount_curve_id` is used for financing calculations.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo_curve_id: Option<CurveId>,

    /// Attributes for scenario selection and tagging
    #[serde(default)]
    #[builder(default)]
    /// Instrument-owned pricing inputs.
    pub instrument_pricing_overrides: crate::instruments::InstrumentPricingOverrides,
    /// Metric-time pricing configuration.
    #[serde(default)]
    #[builder(default)]
    pub metric_pricing_overrides: crate::instruments::MetricPricingOverrides,
    /// Scenario-only pricing adjustments.
    #[serde(default)]
    #[builder(default)]
    pub scenario_pricing_overrides: crate::instruments::ScenarioPricingOverrides,
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,
}

impl BondFuture {
    /// Create a representative UST 10Y bond future example.
    ///
    /// Long position, 10 contracts ($1M notional), 2 deliverable bonds
    /// with published conversion factors.
    pub fn example() -> finstack_quant_core::Result<Self> {
        use finstack_quant_core::currency::Currency;

        let expiry = time::macros::date!(2025 - 09 - 19);
        let delivery_start = time::macros::date!(2025 - 09 - 22);
        let delivery_end = time::macros::date!(2025 - 09 - 30);

        let bond1_id = InstrumentId::new("US91282CJL54");
        let bond2_id = InstrumentId::new("US91282CHT18");

        Self::builder()
            .id(InstrumentId::new("TYU5"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .expiry(expiry)
            .delivery_start(delivery_start)
            .delivery_end(delivery_end)
            .quoted_price(112.25)
            .position(Position::Long)
            .contract_specs(BondFutureSpecs::ust_10y())
            .deliverable_basket(vec![
                DeliverableBond {
                    bond_id: bond1_id.clone(),
                    conversion_factor: 0.8234,
                },
                DeliverableBond {
                    bond_id: bond2_id,
                    conversion_factor: 0.7915,
                },
            ])
            .ctd_bond_id(bond1_id)
            .discount_curve_id(CurveId::new("USD-TREASURY"))
            .build_validated()
    }

    fn resolve_ctd_bond_id(&self) -> finstack_quant_core::Result<InstrumentId> {
        if let Some(id) = &self.ctd_bond_id {
            return Ok(id.clone());
        }
        if let Some(ctd_bond) = &self.ctd_bond {
            return Ok(ctd_bond.id.clone());
        }
        if self.deliverable_basket.len() == 1 {
            return Ok(self.deliverable_basket[0].bond_id.clone());
        }
        Err(finstack_quant_core::Error::Validation(
            "ctd_bond_id is required when deliverable_basket has multiple bonds and no ctd_bond is embedded"
                .to_string(),
        ))
    }

    /// Validate the BondFuture parameters.
    ///
    /// This method checks the following invariants:
    /// - Date ordering: expiry < delivery_start < delivery_end
    /// - Deliverable basket is non-empty
    /// - CTD bond exists in deliverable basket
    /// - All conversion factors are positive
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`](finstack_quant_core::Error::Validation) if any validation fails.
    fn validate(&self) -> finstack_quant_core::Result<()> {
        // Date ordering validation
        if self.expiry >= self.delivery_start {
            return Err(finstack_quant_core::Error::Validation(format!(
                "expiry ({}) must be before delivery_start ({})",
                self.expiry, self.delivery_start
            )));
        }
        if self.delivery_start >= self.delivery_end {
            return Err(finstack_quant_core::Error::Validation(format!(
                "delivery_start ({}) must be before delivery_end ({})",
                self.delivery_start, self.delivery_end
            )));
        }

        // Deliverable basket validation
        if self.deliverable_basket.is_empty() {
            return Err(finstack_quant_core::Error::Validation(
                "deliverable_basket cannot be empty".to_string(),
            ));
        }

        // CTD bond exists in basket validation when we can resolve CTD id.
        let resolved_ctd_id = self.resolve_ctd_bond_id()?;
        let ctd_exists = self
            .deliverable_basket
            .iter()
            .any(|bond| bond.bond_id == resolved_ctd_id);
        if !ctd_exists {
            return Err(finstack_quant_core::Error::Validation(format!(
                "resolved ctd_bond_id ({}) not found in deliverable_basket",
                resolved_ctd_id.as_str()
            )));
        }

        // If an embedded CTD bond is provided, it must match the CTD id.
        if let Some(bond) = &self.ctd_bond {
            if bond.id != resolved_ctd_id {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "ctd_bond.id ({}) must match ctd_bond_id ({})",
                    bond.id.as_str(),
                    resolved_ctd_id.as_str()
                )));
            }
        }

        // Conversion factors validation
        for deliverable in &self.deliverable_basket {
            if !deliverable.conversion_factor.is_finite() || deliverable.conversion_factor <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "conversion_factor must be positive and finite for bond {}, got {}",
                    deliverable.bond_id.as_str(),
                    deliverable.conversion_factor
                )));
            }
        }

        if !self.quoted_price.is_finite() || self.quoted_price < 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "quoted_price must be finite and non-negative, got {}",
                self.quoted_price
            )));
        }
        if !self.notional.amount().is_finite() || self.notional.amount() == 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "bond future notional must be finite and non-zero, got {}",
                self.notional.amount()
            )));
        }

        Ok(())
    }

    /// Calculate the invoice price for settlement of the bond future.
    ///
    /// The invoice price is the amount the buyer pays to the seller when taking delivery
    /// of the underlying bond at expiry. It consists of:
    /// - The futures price scaled by the conversion factor (bringing CTD bond to standard terms)
    /// - Plus accrued interest on the CTD bond at settlement
    ///
    /// Formula: `Invoice = (Futures_Price × Conversion_Factor) + Accrued_Interest`
    ///
    /// # Arguments
    ///
    /// * `ctd_bond` - The cheapest-to-deliver bond reference
    /// * `market` - Market context containing curves for accrued interest calculation
    /// * `settlement_date` - Settlement date (typically T+2 after expiry)
    ///
    /// # Returns
    ///
    /// Invoice price in the same currency as the futures contract notional.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - CTD bond is not found in the deliverable basket
    /// - Cashflow schedule building fails
    /// - Accrued interest calculation fails
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::market_data::context::MarketContext;
    /// use finstack_quant_core::money::Money;
    /// use finstack_quant_core::types::{CurveId, InstrumentId};
    /// use finstack_quant_valuations::instruments::Bond;
    /// use finstack_quant_valuations::instruments::fixed_income::bond_future::{
    ///     BondFuture, BondFutureSpecs, DeliverableBond, Position,
    /// };
    /// use time::macros::date;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let ctd_bond_id = InstrumentId::new("US912828XG33");
    /// let future = BondFuture::builder()
    ///     .id(InstrumentId::new("TYH5"))
    ///     .notional(Money::new(1_000_000.0, Currency::USD))
    ///     .expiry(date!(2025-03-20))
    ///     .delivery_start(date!(2025-03-21))
    ///     .delivery_end(date!(2025-03-31))
    ///     .quoted_price(125.50)
    ///     .position(Position::Long)
    ///     .contract_specs(BondFutureSpecs::ust_10y())
    ///     .deliverable_basket(vec![DeliverableBond {
    ///         bond_id: ctd_bond_id.clone(),
    ///         conversion_factor: 0.8234,
    ///     }])
    ///     .ctd_bond_id(ctd_bond_id.clone())
    ///     .discount_curve_id(CurveId::new("USD-TREASURY"))
    ///     .build()
    ///     .expect("Valid bond future");
    /// let ctd_bond = Bond::fixed(
    ///     ctd_bond_id.as_str(),
    ///     Money::new(100_000.0, Currency::USD),
    ///     0.05,
    ///     date!(2020-01-15),
    ///     date!(2030-01-15),
    ///     "USD-OIS",
    /// )?;
    /// let market = MarketContext::new();
    ///
    /// // Calculate invoice price for settlement 2 days after expiry
    /// let settlement = date!(2025-03-23);
    /// let invoice = future.invoice_price(&ctd_bond, &market, settlement)?;
    ///
    /// // For futures price 125.50 and CF 0.8234:
    /// // Invoice = (125.50 × 0.8234) + accrued
    /// # let _ = invoice;
    /// # Ok(())
    /// # }
    /// ```
    pub fn invoice_price(
        &self,
        ctd_bond: &crate::instruments::fixed_income::bond::Bond,
        market: &finstack_quant_core::market_data::context::MarketContext,
        settlement_date: Date,
    ) -> finstack_quant_core::Result<Money> {
        let ctd_bond_id = self.resolve_ctd_bond_id()?;
        // Find the conversion factor for the CTD bond
        let conversion_factor = self
            .deliverable_basket
            .iter()
            .find(|db| db.bond_id == ctd_bond_id)
            .ok_or_else(|| finstack_quant_core::InputError::NotFound {
                id: format!(
                    "CTD bond {} not found in deliverable basket",
                    ctd_bond_id.as_str()
                ),
            })?
            .conversion_factor;

        // Get the CTD bond's cashflow schedule
        let schedule = ctd_bond.full_cashflow_schedule(market)?;

        // Calculate accrued interest at settlement date
        use crate::cashflow::accrual::accrued_interest_amount;

        let accrual_config = ctd_bond.accrual_config();

        let accrued_amount = accrued_interest_amount(&schedule, settlement_date, &accrual_config)?;

        // Convert accrued interest from absolute currency units to percentage of par
        // accrued_amount is in dollars (e.g., $7,000 on $100,000 notional)
        // We need to express it as dollars per $100 face (e.g., $7 per $100)
        let ctd_notional = ctd_bond.notional.amount();
        let accrued_pct = (accrued_amount / ctd_notional) * 100.0;

        // Calculate invoice price per contract
        // Invoice = (Futures_Price × CF) + Accrued
        // Note: Futures price is quoted per $100 face value, so we scale appropriately
        let futures_price_pct = self.quoted_price; // e.g., 125.50 for 125-16/32
        let invoice_pct = (futures_price_pct * conversion_factor) + accrued_pct;

        // Convert from percentage to actual money amount for contract size
        // Contract size is typically $100,000, so invoice_pct is per $100 face
        let invoice_per_contract = (invoice_pct / crate::constants::DECIMAL_TO_PERCENT)
            * self.contract_specs.contract_size;

        // Scale by number of contracts (notional / contract_size)
        let num_contracts = self.notional.amount() / self.contract_specs.contract_size;
        let total_invoice = invoice_per_contract * num_contracts;

        Ok(Money::new(total_invoice, self.notional.currency()))
    }

    /// Determine the Cheapest-to-Deliver (CTD) bond from the deliverable basket.
    ///
    /// This method calculates the **gross basis** (price-only, ignoring accrued
    /// interest and carry) for each deliverable bond and returns the bond with
    /// the lowest basis (i.e., the cheapest to deliver).
    ///
    /// # Gross Basis Calculation
    ///
    /// ```text
    /// Gross Basis = Clean Price - (Futures Price × Conversion Factor)
    /// ```
    ///
    /// This is the market-standard *gross* basis. The *net* basis additionally
    /// subtracts the carry to delivery (coupon income net of financing); for a
    /// carry-aware ranking use
    /// [`determine_ctd_by_implied_repo`](Self::determine_ctd_by_implied_repo),
    /// which is the market-preferred CTD criterion.
    ///
    /// The CTD is the bond that minimizes this value. A lower basis means the bond
    /// is cheaper relative to its invoice value, making it the optimal choice for
    /// delivery by the short position holder.
    ///
    /// # Arguments
    ///
    /// * `bond_clean_prices` - A slice of `(InstrumentId, f64)` tuples containing
    ///   the clean price (per 100 face) for each bond in the deliverable basket.
    ///   Bonds not included in this slice are skipped in the CTD calculation.
    ///
    /// # Returns
    ///
    /// Returns the `InstrumentId` of the CTD bond and its gross basis.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No valid bond prices are provided for any bond in the basket
    /// - All provided prices are non-positive
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use finstack_quant_valuations::instruments::fixed_income::bond_future::{
    ///     BondFuture, BondFutureSpecs, DeliverableBond, Position,
    /// };
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::money::Money;
    /// use finstack_quant_core::types::{CurveId, InstrumentId};
    /// use time::macros::date;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// // Create a bond future with multiple deliverables
    /// let bond1_id = InstrumentId::new("US912828XG33");
    /// let bond2_id = InstrumentId::new("US912828XG34");
    ///
    /// let future = BondFuture::builder()
    ///     .id(InstrumentId::new("TYH5"))
    ///     .notional(Money::new(1_000_000.0, Currency::USD))
    ///     .expiry(date!(2025-03-20))
    ///     .delivery_start(date!(2025-03-21))
    ///     .delivery_end(date!(2025-03-31))
    ///     .quoted_price(125.50)
    ///     .position(Position::Long)
    ///     .contract_specs(BondFutureSpecs::ust_10y())
    ///     .deliverable_basket(vec![
    ///         DeliverableBond { bond_id: bond1_id.clone(), conversion_factor: 0.8234 },
    ///         DeliverableBond { bond_id: bond2_id.clone(), conversion_factor: 0.8567 },
    ///     ])
    ///     .ctd_bond_id(bond1_id.clone())
    ///     .discount_curve_id(CurveId::new("USD-TREASURY"))
    ///     .build()
    ///     .expect("Valid bond future");
    ///
    /// // Determine CTD based on current market prices
    /// let bond_prices = vec![
    ///     (bond1_id.clone(), 103.25),  // Clean price per 100 face
    ///     (bond2_id.clone(), 107.50),
    /// ];
    ///
    /// let (ctd_id, gross_basis) = future.determine_ctd(&bond_prices)?;
    /// println!("CTD bond: {}, Gross basis: {:.4}", ctd_id.as_str(), gross_basis);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Production Considerations
    ///
    /// - **Accrued Interest**: For precise CTD determination including carry, use
    ///   [`determine_ctd_with_accrued`](Self::determine_ctd_with_accrued) which accounts
    ///   for accrued interest and settlement timing.
    /// - **Repo Rates**: In practice, different bonds may have different financing costs
    ///   (repo rates). The true CTD analysis should incorporate implied repo calculations.
    /// - **Timing**: CTD can change intraday as prices move; recalculate periodically.
    pub fn determine_ctd(
        &self,
        bond_clean_prices: &[(InstrumentId, f64)],
    ) -> finstack_quant_core::Result<(InstrumentId, f64)> {
        let mut best_ctd: Option<(InstrumentId, f64)> = None;

        for deliverable in &self.deliverable_basket {
            // Find the clean price for this bond
            if let Some((_, clean_price)) = bond_clean_prices
                .iter()
                .find(|(id, _)| *id == deliverable.bond_id)
            {
                if *clean_price <= 0.0 {
                    continue; // Skip invalid prices
                }

                // Calculate gross basis: Clean Price - (Futures Price × CF)
                let gross_basis = clean_price - (self.quoted_price * deliverable.conversion_factor);

                match &best_ctd {
                    None => {
                        best_ctd = Some((deliverable.bond_id.clone(), gross_basis));
                    }
                    Some((_, current_best_basis)) => {
                        if gross_basis < *current_best_basis {
                            best_ctd = Some((deliverable.bond_id.clone(), gross_basis));
                        }
                    }
                }
            }
        }

        best_ctd.ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "No valid bond prices provided for any bond in the deliverable basket".to_string(),
            )
        })
    }

    /// Determine CTD using gross basis including delivery accrued interest.
    ///
    /// Computes the gross basis for each deliverable bond:
    ///
    /// ```text
    /// Gross Basis = (Clean Price + Accrued Today) - (Futures Price × CF + Accrued at Delivery)
    /// ```
    ///
    /// The bond with the lowest gross basis is the CTD.
    ///
    /// # Arguments
    ///
    /// * `bond_prices_with_accrued` - A slice of `(InstrumentId, f64, f64, f64)` tuples:
    ///   - `InstrumentId`: Bond identifier
    ///   - `f64` (position 1): Clean price per 100 face
    ///   - `f64` (position 2): Accrued interest per 100 face as of today
    ///   - `f64` (position 3): Projected accrued interest per 100 face at delivery
    ///
    /// # Returns
    ///
    /// Returns the `InstrumentId` of the CTD bond and its gross basis.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use finstack_quant_valuations::instruments::fixed_income::bond_future::{
    ///     BondFuture, BondFutureSpecs, DeliverableBond, Position,
    /// };
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::money::Money;
    /// use finstack_quant_core::types::{CurveId, InstrumentId};
    /// use time::macros::date;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let bond1_id = InstrumentId::new("US912828XG33");
    /// let bond2_id = InstrumentId::new("US912828XG34");
    ///
    /// let future = BondFuture::builder()
    ///     .id(InstrumentId::new("TYH5"))
    ///     .notional(Money::new(1_000_000.0, Currency::USD))
    ///     .expiry(date!(2025-03-20))
    ///     .delivery_start(date!(2025-03-21))
    ///     .delivery_end(date!(2025-03-31))
    ///     .quoted_price(125.50)
    ///     .position(Position::Long)
    ///     .contract_specs(BondFutureSpecs::ust_10y())
    ///     .deliverable_basket(vec![
    ///         DeliverableBond { bond_id: bond1_id.clone(), conversion_factor: 0.8234 },
    ///         DeliverableBond { bond_id: bond2_id.clone(), conversion_factor: 0.8567 },
    ///     ])
    ///     .ctd_bond_id(bond1_id.clone())
    ///     .discount_curve_id(CurveId::new("USD-TREASURY"))
    ///     .build()
    ///     .expect("Valid bond future");
    ///
    /// let bond_data = vec![
    ///     (bond1_id.clone(), 103.25, 1.25, 1.75),  // (id, clean, accrued_today, accrued_at_delivery)
    ///     (bond2_id.clone(), 107.50, 1.50, 2.00),
    /// ];
    ///
    /// let (ctd_id, gross_basis) = future.determine_ctd_with_accrued(&bond_data)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn determine_ctd_with_accrued(
        &self,
        bond_prices_with_accrued: &[(InstrumentId, f64, f64, f64)],
    ) -> finstack_quant_core::Result<(InstrumentId, f64)> {
        let mut best_ctd: Option<(InstrumentId, f64)> = None;

        for deliverable in &self.deliverable_basket {
            if let Some((_, clean_price, accrued_today, accrued_at_delivery)) =
                bond_prices_with_accrued
                    .iter()
                    .find(|(id, _, _, _)| *id == deliverable.bond_id)
            {
                if *clean_price <= 0.0 {
                    continue;
                }

                let purchase_dirty = clean_price + accrued_today;
                let invoice_dirty =
                    self.quoted_price * deliverable.conversion_factor + accrued_at_delivery;
                let gross_basis = purchase_dirty - invoice_dirty;

                match &best_ctd {
                    None => {
                        best_ctd = Some((deliverable.bond_id.clone(), gross_basis));
                    }
                    Some((_, current_best)) => {
                        if gross_basis < *current_best {
                            best_ctd = Some((deliverable.bond_id.clone(), gross_basis));
                        }
                    }
                }
            }
        }

        best_ctd.ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "No valid bond prices provided for any bond in the deliverable basket".to_string(),
            )
        })
    }

    /// Determine the CTD bond by ranking deliverables on **implied repo rate**.
    ///
    /// The market-preferred CTD criterion: the cheapest-to-deliver bond is the
    /// one whose purchase-and-deliver round trip earns the **highest** implied
    /// repo rate (it offers the short the best financing arbitrage). Unlike
    /// gross-basis ranking this accounts for accrued interest, interim coupon
    /// income, and the time to delivery.
    ///
    /// # Arguments
    ///
    /// * `bond_data` - A slice of `(InstrumentId, f64, f64, f64, f64)` tuples:
    ///   - `InstrumentId`: bond identifier
    ///   - `f64` (position 1): clean price per 100 face
    ///   - `f64` (position 2): accrued interest per 100 face as of today
    ///   - `f64` (position 3): projected accrued interest per 100 face at delivery
    ///   - `f64` (position 4): coupon income per 100 face received before delivery
    /// * `days_to_delivery` - Number of days until delivery (must be positive)
    ///
    /// # Returns
    ///
    /// The `InstrumentId` of the CTD bond and its annualized implied repo rate
    /// (decimal, e.g. `0.05` for 5%).
    ///
    /// # Errors
    ///
    /// Returns an error if `days_to_delivery` is non-positive or if no valid
    /// bond data is provided for any bond in the deliverable basket.
    pub fn determine_ctd_by_implied_repo(
        &self,
        bond_data: &[(InstrumentId, f64, f64, f64, f64)],
        days_to_delivery: i32,
    ) -> finstack_quant_core::Result<(InstrumentId, f64)> {
        if days_to_delivery <= 0 {
            return Err(finstack_quant_core::Error::Validation(
                "Days to delivery must be positive".to_string(),
            ));
        }

        let mut best_ctd: Option<(InstrumentId, f64)> = None;

        for deliverable in &self.deliverable_basket {
            if let Some((id, clean_price, accrued_today, accrued_at_delivery, coupon_income)) =
                bond_data.iter().find(|(id, ..)| *id == deliverable.bond_id)
            {
                if *clean_price <= 0.0 {
                    continue;
                }

                let repo = self.implied_repo_rate(
                    id,
                    *clean_price,
                    *accrued_today,
                    *accrued_at_delivery,
                    *coupon_income,
                    days_to_delivery,
                )?;

                match &best_ctd {
                    None => best_ctd = Some((deliverable.bond_id.clone(), repo)),
                    Some((_, current_best)) => {
                        if repo > *current_best {
                            best_ctd = Some((deliverable.bond_id.clone(), repo));
                        }
                    }
                }
            }
        }

        best_ctd.ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "No valid bond data provided for any bond in the deliverable basket".to_string(),
            )
        })
    }

    /// Calculate implied repo rate for a specific deliverable bond.
    ///
    /// The implied repo rate represents the financing rate implied by the futures price
    /// and the bond's cash price. It's used to compare the attractiveness of different
    /// deliverable bonds and to identify arbitrage opportunities.
    ///
    /// # Formula
    ///
    /// ```text
    /// Implied Repo = [(Invoice Price + Coupon Income) / Purchase Price - 1] × (Annualization / days_to_delivery)
    /// ```
    ///
    /// Where:
    /// - Invoice Price = Futures Price × CF + Accrued at Delivery
    /// - Purchase Price = Clean Price + Accrued Today
    /// - Coupon Income = sum of coupon cashflows received between today and delivery
    ///
    /// # Arguments
    ///
    /// * `bond_id` - The identifier of the deliverable bond
    /// * `clean_price` - Current clean price per 100 face
    /// * `accrued_today` - Accrued interest per 100 face as of today
    /// * `accrued_at_delivery` - Accrued interest per 100 face at delivery date
    /// * `coupon_income` - Total coupon payments received between today and delivery (per 100 face)
    /// * `days_to_delivery` - Number of days until delivery
    ///
    /// # Returns
    ///
    /// The annualized implied repo rate as a decimal (e.g., 0.05 for 5%).
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The specified bond is not in the deliverable basket
    /// - The purchase price is non-positive
    /// - Days to delivery is zero or negative
    ///
    /// # Example
    ///
    /// ```ignore
    /// use finstack_quant_valuations::instruments::fixed_income::bond_future::{
    ///     BondFuture, BondFutureSpecs, DeliverableBond, Position,
    /// };
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::money::Money;
    /// use finstack_quant_core::types::{CurveId, InstrumentId};
    /// use time::macros::date;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let bond_id = InstrumentId::new("US912828XG33");
    /// let future = BondFuture::builder()
    ///     .id(InstrumentId::new("TYH5"))
    ///     .notional(Money::new(1_000_000.0, Currency::USD))
    ///     .expiry(date!(2025-03-20))
    ///     .delivery_start(date!(2025-03-21))
    ///     .delivery_end(date!(2025-03-31))
    ///     .quoted_price(125.50)
    ///     .position(Position::Long)
    ///     .contract_specs(BondFutureSpecs::ust_10y())
    ///     .deliverable_basket(vec![
    ///         DeliverableBond { bond_id: bond_id.clone(), conversion_factor: 0.8234 },
    ///     ])
    ///     .ctd_bond_id(bond_id.clone())
    ///     .discount_curve_id(CurveId::new("USD-TREASURY"))
    ///     .build()
    ///     .expect("Valid bond future");
    ///
    /// // Calculate implied repo for the CTD bond
    /// let implied_repo = future.implied_repo_rate(
    ///     &bond_id,
    ///     103.25,  // clean price
    ///     1.25,    // accrued today
    ///     1.75,    // accrued at delivery
    ///     0.0,     // no coupons between now and delivery
    ///     30,      // days to delivery
    /// )?;
    ///
    /// println!("Implied repo rate: {:.2}%", implied_repo * 100.0);
    /// # Ok(())
    /// # }
    /// ```
    pub fn implied_repo_rate(
        &self,
        bond_id: &InstrumentId,
        clean_price: f64,
        accrued_today: f64,
        accrued_at_delivery: f64,
        coupon_income: f64,
        days_to_delivery: i32,
    ) -> finstack_quant_core::Result<f64> {
        // Find the conversion factor for this bond
        let cf = self
            .deliverable_basket
            .iter()
            .find(|db| db.bond_id == *bond_id)
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "Bond {} not found in deliverable basket",
                    bond_id.as_str()
                ))
            })?
            .conversion_factor;

        if days_to_delivery <= 0 {
            return Err(finstack_quant_core::Error::Validation(
                "Days to delivery must be positive".to_string(),
            ));
        }

        // Purchase price (dirty price today)
        let purchase_price = clean_price + accrued_today;
        if purchase_price <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(
                "Purchase price must be positive".to_string(),
            ));
        }

        // Invoice price at delivery
        let invoice_price = (self.quoted_price * cf) + accrued_at_delivery;

        // Total proceeds include any coupon payments received during the holding period
        let total_proceeds = invoice_price + coupon_income;

        // Implied repo rate annualization uses contract-specific day-count basis.
        let annualization_basis = self
            .contract_specs
            .repo_day_count_basis
            .annualization_denominator();
        let holding_period_return = (total_proceeds / purchase_price) - 1.0;
        let annualized = holding_period_return * (annualization_basis / days_to_delivery as f64);

        Ok(annualized)
    }
}

// Manually implement a validated builder method
impl BondFutureBuilder {
    /// Build the BondFuture with validation.
    ///
    /// This is a wrapper around the generated `build()` method that adds
    /// validation checks after construction.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Any required field is missing (from the generated builder)
    /// - Validation fails (from `BondFuture::validate`)
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::money::Money;
    /// use finstack_quant_core::types::{CurveId, InstrumentId};
    /// use finstack_quant_valuations::instruments::fixed_income::bond_future::{
    ///     BondFuture, BondFutureSpecs, DeliverableBond, Position,
    /// };
    /// use time::macros::date;
    ///
    /// # fn main() -> finstack_quant_core::Result<()> {
    /// let ctd_bond_id = InstrumentId::new("US912828XG33");
    /// let future = BondFuture::builder()
    ///     .id(InstrumentId::new("TYH5"))
    ///     .notional(Money::new(1_000_000.0, Currency::USD))
    ///     .expiry(date!(2025-03-20))
    ///     .delivery_start(date!(2025-03-21))
    ///     .delivery_end(date!(2025-03-31))
    ///     .quoted_price(125.50)
    ///     .position(Position::Long)
    ///     .contract_specs(BondFutureSpecs::ust_10y())
    ///     .deliverable_basket(vec![DeliverableBond {
    ///         bond_id: ctd_bond_id.clone(),
    ///         conversion_factor: 0.8234,
    ///     }])
    ///     .ctd_bond_id(ctd_bond_id)
    ///     .discount_curve_id(CurveId::new("USD-TREASURY"))
    ///     .build_validated()?; // Validates after construction
    /// # let _ = future;
    /// # Ok(())
    /// # }
    /// ```
    pub fn build_validated(self) -> finstack_quant_core::Result<BondFuture> {
        let bond_future = self.build().map_err(|e| {
            finstack_quant_core::Error::Validation(format!("BondFuture construction failed: {}", e))
        })?;
        bond_future.validate()?;
        Ok(bond_future)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cashflow::CashflowProvider;
    use crate::instruments::fixed_income::bond::Bond;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::types::CurveId;
    use time::Month;

    #[test]
    fn test_deliverable_bond_construction() {
        let db = DeliverableBond {
            bond_id: InstrumentId::new("US912828XG33"),
            conversion_factor: 0.8234,
        };
        assert_eq!(db.conversion_factor, 0.8234);
        assert_eq!(db.bond_id.as_str(), "US912828XG33");
    }

    #[test]
    fn test_bond_future_specs_default() {
        let specs = BondFutureSpecs::default();
        assert_eq!(specs.contract_size, 100_000.0);
        assert_eq!(specs.tick_size, 1.0 / 64.0);
        assert_eq!(specs.tick_value, 15.625);
        assert_eq!(specs.standard_coupon, 0.06);
        assert_eq!(specs.standard_maturity_years, 10.0);
        assert_eq!(specs.settlement_days, 2);
    }

    #[test]
    fn test_ust_10y_specs() {
        let specs = BondFutureSpecs::ust_10y();
        assert_eq!(specs.contract_size, 100_000.0);
        assert_eq!(specs.tick_size, 1.0 / 64.0);
        assert_eq!(specs.tick_value, 15.625);
        assert_eq!(specs.standard_coupon, 0.06);
        assert_eq!(specs.standard_maturity_years, 10.0);
        assert_eq!(specs.settlement_days, 2);
        assert_eq!(specs.repo_day_count_basis, RepoDayCountBasis::Act360);
    }

    #[test]
    fn test_ust_5y_specs() {
        let specs = BondFutureSpecs::ust_5y();
        assert_eq!(specs.contract_size, 100_000.0);
        assert_eq!(specs.tick_size, 1.0 / 128.0);
        assert_eq!(specs.tick_value, 7.8125);
        assert_eq!(specs.standard_coupon, 0.06);
        assert_eq!(specs.standard_maturity_years, 5.0);
        assert_eq!(specs.settlement_days, 2);
    }

    #[test]
    fn test_ust_2y_specs() {
        let specs = BondFutureSpecs::ust_2y();
        assert_eq!(specs.contract_size, 200_000.0); // Note: 2Y is $200k
        assert_eq!(specs.tick_size, 1.0 / 256.0);
        assert_eq!(specs.tick_value, 7.8125);
        assert_eq!(specs.standard_coupon, 0.06);
        assert_eq!(specs.standard_maturity_years, 2.0);
        assert_eq!(specs.settlement_days, 2);
    }

    #[test]
    fn test_bund_specs() {
        let specs = BondFutureSpecs::bund();
        assert_eq!(specs.contract_size, 100_000.0);
        assert_eq!(specs.tick_size, 0.01);
        assert_eq!(specs.tick_value, 10.0);
        assert_eq!(specs.standard_coupon, 0.06);
        assert_eq!(specs.standard_maturity_years, 10.0);
        assert_eq!(specs.settlement_days, 2);
    }

    #[test]
    fn test_gilt_specs() {
        let specs = BondFutureSpecs::gilt();
        assert_eq!(specs.contract_size, 100_000.0);
        assert_eq!(specs.tick_size, 0.01);
        assert_eq!(specs.tick_value, 10.0);
        assert_eq!(specs.standard_coupon, 0.04); // Different from UST/Bund
        assert_eq!(specs.standard_maturity_years, 10.0);
        assert_eq!(specs.settlement_days, 2);
        assert_eq!(specs.repo_day_count_basis, RepoDayCountBasis::Act365);
    }

    #[test]
    fn test_bond_future_construction() {
        let deliverable = DeliverableBond {
            bond_id: InstrumentId::new("US912828XG33"),
            conversion_factor: 0.8234,
        };

        let future = BondFuture::builder()
            .id(InstrumentId::new("TYH5"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .expiry(Date::from_calendar_date(2025, Month::March, 20).expect("Valid date"))
            .delivery_start(Date::from_calendar_date(2025, Month::March, 21).expect("Valid date"))
            .delivery_end(Date::from_calendar_date(2025, Month::March, 31).expect("Valid date"))
            .quoted_price(125.50)
            .position(Position::Long)
            .contract_specs(BondFutureSpecs::default())
            .deliverable_basket(vec![deliverable])
            .ctd_bond_id(InstrumentId::new("US912828XG33"))
            .discount_curve_id(CurveId::new("USD-TREASURY"))
            .attributes(Attributes::new())
            .build()
            .expect("Valid bond future");

        assert_eq!(future.id.as_str(), "TYH5");
        assert_eq!(future.quoted_price, 125.50);
        assert_eq!(future.position, Position::Long);
        assert_eq!(future.deliverable_basket.len(), 1);
    }

    #[test]
    fn test_determine_ctd_by_implied_repo_ranks_highest_repo() {
        let bond1 = InstrumentId::new("BOND-1");
        let bond2 = InstrumentId::new("BOND-2");

        let future = BondFuture::builder()
            .id(InstrumentId::new("TYH5"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .expiry(Date::from_calendar_date(2025, Month::March, 20).expect("Valid date"))
            .delivery_start(Date::from_calendar_date(2025, Month::March, 21).expect("Valid date"))
            .delivery_end(Date::from_calendar_date(2025, Month::March, 31).expect("Valid date"))
            .quoted_price(125.50)
            .position(Position::Long)
            .contract_specs(BondFutureSpecs::default())
            .deliverable_basket(vec![
                DeliverableBond {
                    bond_id: bond1.clone(),
                    conversion_factor: 0.8234,
                },
                DeliverableBond {
                    bond_id: bond2.clone(),
                    conversion_factor: 0.8567,
                },
            ])
            .ctd_bond_id(bond1.clone())
            .discount_curve_id(CurveId::new("USD-TREASURY"))
            .attributes(Attributes::new())
            .build()
            .expect("Valid bond future");

        // Bond 2 is cheap relative to its invoice (high implied repo);
        // bond 1 is expensive (low / negative implied repo).
        let bond_data = vec![
            (bond1.clone(), 104.50, 1.25, 1.75, 0.0),
            (bond2.clone(), 106.50, 1.50, 2.00, 0.0),
        ];

        let (ctd_id, repo) = future
            .determine_ctd_by_implied_repo(&bond_data, 30)
            .expect("CTD by implied repo");
        assert_eq!(ctd_id, bond2);

        // The winner's repo must equal the single-bond calculation and
        // dominate the alternative.
        let repo1 = future
            .implied_repo_rate(&bond1, 104.50, 1.25, 1.75, 0.0, 30)
            .expect("repo bond1");
        let repo2 = future
            .implied_repo_rate(&bond2, 106.50, 1.50, 2.00, 0.0, 30)
            .expect("repo bond2");
        assert!((repo - repo2).abs() < 1e-12);
        assert!(repo2 > repo1);

        // Non-positive horizon is rejected.
        assert!(future.determine_ctd_by_implied_repo(&bond_data, 0).is_err());
    }

    #[test]
    fn test_position_long() {
        let pos = Position::Long;
        assert_eq!(pos, Position::Long);
        assert_eq!(format!("{}", pos), "long");
    }

    #[test]
    fn test_position_short() {
        let pos = Position::Short;
        assert_eq!(pos, Position::Short);
        assert_eq!(format!("{}", pos), "short");
    }

    #[test]
    fn test_validation_date_ordering_expiry_after_delivery() {
        let deliverable = DeliverableBond {
            bond_id: InstrumentId::new("US912828XG33"),
            conversion_factor: 0.8234,
        };

        // expiry_date >= delivery_start (invalid)
        let result = BondFuture::builder()
            .id(InstrumentId::new("TYH5"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .expiry(Date::from_calendar_date(2025, Month::March, 31).expect("Valid date")) // Wrong: same as delivery_end
            .delivery_start(Date::from_calendar_date(2025, Month::March, 21).expect("Valid date"))
            .delivery_end(Date::from_calendar_date(2025, Month::March, 31).expect("Valid date"))
            .quoted_price(125.50)
            .position(Position::Long)
            .contract_specs(BondFutureSpecs::default())
            .deliverable_basket(vec![deliverable])
            .ctd_bond_id(InstrumentId::new("US912828XG33"))
            .discount_curve_id(CurveId::new("USD-TREASURY"))
            .attributes(Attributes::new())
            .build_validated();

        assert!(result.is_err());
        let err_msg = format!("{}", result.expect_err("Should have validation error"));
        assert!(err_msg.contains("expiry") && err_msg.contains("delivery_start"));
    }

    #[test]
    fn test_validation_date_ordering_delivery_start_after_end() {
        let deliverable = DeliverableBond {
            bond_id: InstrumentId::new("US912828XG33"),
            conversion_factor: 0.8234,
        };

        // delivery_start >= delivery_end (invalid)
        let result = BondFuture::builder()
            .id(InstrumentId::new("TYH5"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .expiry(Date::from_calendar_date(2025, Month::March, 20).expect("Valid date"))
            .delivery_start(Date::from_calendar_date(2025, Month::March, 31).expect("Valid date")) // Wrong: after delivery_end
            .delivery_end(Date::from_calendar_date(2025, Month::March, 21).expect("Valid date"))
            .quoted_price(125.50)
            .position(Position::Long)
            .contract_specs(BondFutureSpecs::default())
            .deliverable_basket(vec![deliverable])
            .ctd_bond_id(InstrumentId::new("US912828XG33"))
            .discount_curve_id(CurveId::new("USD-TREASURY"))
            .attributes(Attributes::new())
            .build_validated();

        assert!(result.is_err());
        let err_msg = format!("{}", result.expect_err("Should have validation error"));
        assert!(err_msg.contains("delivery_start") && err_msg.contains("delivery_end"));
    }

    #[test]
    fn test_validation_empty_basket() {
        // Empty deliverable basket (invalid)
        let result = BondFuture::builder()
            .id(InstrumentId::new("TYH5"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .expiry(Date::from_calendar_date(2025, Month::March, 20).expect("Valid date"))
            .delivery_start(Date::from_calendar_date(2025, Month::March, 21).expect("Valid date"))
            .delivery_end(Date::from_calendar_date(2025, Month::March, 31).expect("Valid date"))
            .quoted_price(125.50)
            .position(Position::Long)
            .contract_specs(BondFutureSpecs::default())
            .deliverable_basket(vec![]) // Invalid: empty
            .ctd_bond_id(InstrumentId::new("US912828XG33"))
            .discount_curve_id(CurveId::new("USD-TREASURY"))
            .attributes(Attributes::new())
            .build_validated();

        assert!(result.is_err());
        let err_msg = format!("{}", result.expect_err("Should have validation error"));
        assert!(err_msg.contains("deliverable_basket") && err_msg.contains("empty"));
    }

    #[test]
    fn test_validation_ctd_not_in_basket() {
        let deliverable = DeliverableBond {
            bond_id: InstrumentId::new("US912828XG33"),
            conversion_factor: 0.8234,
        };

        // CTD bond not in basket (invalid)
        let result = BondFuture::builder()
            .id(InstrumentId::new("TYH5"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .expiry(Date::from_calendar_date(2025, Month::March, 20).expect("Valid date"))
            .delivery_start(Date::from_calendar_date(2025, Month::March, 21).expect("Valid date"))
            .delivery_end(Date::from_calendar_date(2025, Month::March, 31).expect("Valid date"))
            .quoted_price(125.50)
            .position(Position::Long)
            .contract_specs(BondFutureSpecs::default())
            .deliverable_basket(vec![deliverable])
            .ctd_bond_id(InstrumentId::new("UNKNOWN_BOND_ID")) // Invalid: not in basket
            .discount_curve_id(CurveId::new("USD-TREASURY"))
            .attributes(Attributes::new())
            .build_validated();

        assert!(result.is_err());
        let err_msg = format!("{}", result.expect_err("Should have validation error"));
        assert!(err_msg.contains("ctd_bond_id") && err_msg.contains("not found"));
    }

    #[test]
    fn test_validation_negative_conversion_factor() {
        let deliverable_valid = DeliverableBond {
            bond_id: InstrumentId::new("US912828XG33"),
            conversion_factor: 0.8234,
        };
        let deliverable_invalid = DeliverableBond {
            bond_id: InstrumentId::new("US912828XG34"),
            conversion_factor: -0.5, // Invalid: negative
        };

        // Negative conversion factor (invalid)
        let result = BondFuture::builder()
            .id(InstrumentId::new("TYH5"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .expiry(Date::from_calendar_date(2025, Month::March, 20).expect("Valid date"))
            .delivery_start(Date::from_calendar_date(2025, Month::March, 21).expect("Valid date"))
            .delivery_end(Date::from_calendar_date(2025, Month::March, 31).expect("Valid date"))
            .quoted_price(125.50)
            .position(Position::Long)
            .contract_specs(BondFutureSpecs::default())
            .deliverable_basket(vec![deliverable_valid, deliverable_invalid])
            .ctd_bond_id(InstrumentId::new("US912828XG33"))
            .discount_curve_id(CurveId::new("USD-TREASURY"))
            .attributes(Attributes::new())
            .build_validated();

        assert!(result.is_err());
        let err_msg = format!("{}", result.expect_err("Should have validation error"));
        assert!(err_msg.contains("conversion_factor") && err_msg.contains("positive"));
    }

    #[test]
    fn test_validation_zero_conversion_factor() {
        let deliverable_invalid = DeliverableBond {
            bond_id: InstrumentId::new("US912828XG33"),
            conversion_factor: 0.0, // Invalid: zero
        };

        // Zero conversion factor (invalid)
        let result = BondFuture::builder()
            .id(InstrumentId::new("TYH5"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .expiry(Date::from_calendar_date(2025, Month::March, 20).expect("Valid date"))
            .delivery_start(Date::from_calendar_date(2025, Month::March, 21).expect("Valid date"))
            .delivery_end(Date::from_calendar_date(2025, Month::March, 31).expect("Valid date"))
            .quoted_price(125.50)
            .position(Position::Long)
            .contract_specs(BondFutureSpecs::default())
            .deliverable_basket(vec![deliverable_invalid])
            .ctd_bond_id(InstrumentId::new("US912828XG33"))
            .discount_curve_id(CurveId::new("USD-TREASURY"))
            .attributes(Attributes::new())
            .build_validated();

        assert!(result.is_err());
        let err_msg = format!("{}", result.expect_err("Should have validation error"));
        assert!(err_msg.contains("conversion_factor") && err_msg.contains("positive"));
    }

    #[test]
    fn test_validation_success() {
        let deliverable = DeliverableBond {
            bond_id: InstrumentId::new("US912828XG33"),
            conversion_factor: 0.8234,
        };

        // All validations should pass
        let result = BondFuture::builder()
            .id(InstrumentId::new("TYH5"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .expiry(Date::from_calendar_date(2025, Month::March, 20).expect("Valid date"))
            .delivery_start(Date::from_calendar_date(2025, Month::March, 21).expect("Valid date"))
            .delivery_end(Date::from_calendar_date(2025, Month::March, 31).expect("Valid date"))
            .quoted_price(125.50)
            .position(Position::Long)
            .contract_specs(BondFutureSpecs::default())
            .deliverable_basket(vec![deliverable])
            .ctd_bond_id(InstrumentId::new("US912828XG33"))
            .discount_curve_id(CurveId::new("USD-TREASURY"))
            .attributes(Attributes::new())
            .build_validated();

        assert!(result.is_ok());
        let future = result.expect("Should build valid BondFuture");
        assert_eq!(future.id.as_str(), "TYH5");
        assert_eq!(future.deliverable_basket.len(), 1);
    }

    #[test]
    fn test_validation_allows_missing_ctd_with_single_deliverable() {
        let deliverable = DeliverableBond {
            bond_id: InstrumentId::new("US912828XG33"),
            conversion_factor: 0.8234,
        };

        let result = BondFuture::builder()
            .id(InstrumentId::new("TYH5"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .expiry(Date::from_calendar_date(2025, Month::March, 20).expect("Valid date"))
            .delivery_start(Date::from_calendar_date(2025, Month::March, 21).expect("Valid date"))
            .delivery_end(Date::from_calendar_date(2025, Month::March, 31).expect("Valid date"))
            .quoted_price(125.50)
            .position(Position::Long)
            .contract_specs(BondFutureSpecs::default())
            .deliverable_basket(vec![deliverable])
            .discount_curve_id(CurveId::new("USD-TREASURY"))
            .attributes(Attributes::new())
            .build_validated();

        assert!(result.is_ok());
    }

    // Builder-based constructor tests for each contract spec
    #[test]
    fn test_ust_10y_builder() {
        let deliverable = DeliverableBond {
            bond_id: InstrumentId::new("US912828XG33"),
            conversion_factor: 0.8234,
        };

        let future = BondFuture::builder()
            .id(InstrumentId::new("TYH5"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .expiry(Date::from_calendar_date(2025, Month::March, 20).expect("Valid date"))
            .delivery_start(Date::from_calendar_date(2025, Month::March, 21).expect("Valid date"))
            .delivery_end(Date::from_calendar_date(2025, Month::March, 31).expect("Valid date"))
            .quoted_price(125.50)
            .position(Position::Long)
            .contract_specs(BondFutureSpecs::ust_10y())
            .deliverable_basket(vec![deliverable])
            .ctd_bond_id(InstrumentId::new("US912828XG33"))
            .discount_curve_id(CurveId::new("USD-TREASURY"))
            .attributes(Attributes::new())
            .build_validated()
            .expect("Valid UST 10Y future");

        assert_eq!(future.id.as_str(), "TYH5");
        assert_eq!(future.quoted_price, 125.50);
        assert_eq!(future.position, Position::Long);
        assert_eq!(future.contract_specs.standard_coupon, 0.06);
        assert_eq!(future.contract_specs.standard_maturity_years, 10.0);
        assert_eq!(future.contract_specs.contract_size, 100_000.0);
        assert_eq!(future.contract_specs.tick_size, 1.0 / 64.0);
        assert_eq!(future.deliverable_basket.len(), 1);
    }

    #[test]
    fn test_ust_5y_builder() {
        let deliverable = DeliverableBond {
            bond_id: InstrumentId::new("US912828XG33"),
            conversion_factor: 0.7890,
        };

        let future = BondFuture::builder()
            .id(InstrumentId::new("FVH5"))
            .notional(Money::new(500_000.0, Currency::USD))
            .expiry(Date::from_calendar_date(2025, Month::March, 20).expect("Valid date"))
            .delivery_start(Date::from_calendar_date(2025, Month::March, 21).expect("Valid date"))
            .delivery_end(Date::from_calendar_date(2025, Month::March, 31).expect("Valid date"))
            .quoted_price(118.75)
            .position(Position::Long)
            .contract_specs(BondFutureSpecs::ust_5y())
            .deliverable_basket(vec![deliverable])
            .ctd_bond_id(InstrumentId::new("US912828XG33"))
            .discount_curve_id(CurveId::new("USD-TREASURY"))
            .attributes(Attributes::new())
            .build_validated()
            .expect("Valid UST 5Y future");

        assert_eq!(future.id.as_str(), "FVH5");
        assert_eq!(future.quoted_price, 118.75);
        assert_eq!(future.position, Position::Long);
        assert_eq!(future.contract_specs.standard_coupon, 0.06);
        assert_eq!(future.contract_specs.standard_maturity_years, 5.0);
        assert_eq!(future.contract_specs.contract_size, 100_000.0);
        assert_eq!(future.contract_specs.tick_size, 1.0 / 128.0);
        assert_eq!(future.deliverable_basket.len(), 1);
    }

    #[test]
    fn test_ust_2y_builder() {
        let deliverable = DeliverableBond {
            bond_id: InstrumentId::new("US912828XG33"),
            conversion_factor: 0.9123,
        };

        let future = BondFuture::builder()
            .id(InstrumentId::new("TUH5"))
            .notional(Money::new(400_000.0, Currency::USD))
            .expiry(Date::from_calendar_date(2025, Month::March, 20).expect("Valid date"))
            .delivery_start(Date::from_calendar_date(2025, Month::March, 21).expect("Valid date"))
            .delivery_end(Date::from_calendar_date(2025, Month::March, 31).expect("Valid date"))
            .quoted_price(105.25)
            .position(Position::Long)
            .contract_specs(BondFutureSpecs::ust_2y())
            .deliverable_basket(vec![deliverable])
            .ctd_bond_id(InstrumentId::new("US912828XG33"))
            .discount_curve_id(CurveId::new("USD-TREASURY"))
            .attributes(Attributes::new())
            .build_validated()
            .expect("Valid UST 2Y future");

        assert_eq!(future.id.as_str(), "TUH5");
        assert_eq!(future.quoted_price, 105.25);
        assert_eq!(future.position, Position::Long);
        assert_eq!(future.contract_specs.standard_coupon, 0.06);
        assert_eq!(future.contract_specs.standard_maturity_years, 2.0);
        assert_eq!(future.contract_specs.contract_size, 200_000.0); // 2Y is $200k
        assert_eq!(future.contract_specs.tick_size, 1.0 / 256.0);
        assert_eq!(future.deliverable_basket.len(), 1);
    }

    #[test]
    fn test_bund_builder() {
        let deliverable = DeliverableBond {
            bond_id: InstrumentId::new("DE0001102473"),
            conversion_factor: 0.8567,
        };

        let future = BondFuture::builder()
            .id(InstrumentId::new("FGBLH5"))
            .notional(Money::new(1_000_000.0, Currency::EUR))
            .expiry(Date::from_calendar_date(2025, Month::March, 20).expect("Valid date"))
            .delivery_start(Date::from_calendar_date(2025, Month::March, 21).expect("Valid date"))
            .delivery_end(Date::from_calendar_date(2025, Month::March, 31).expect("Valid date"))
            .quoted_price(132.15)
            .position(Position::Long)
            .contract_specs(BondFutureSpecs::bund())
            .deliverable_basket(vec![deliverable])
            .ctd_bond_id(InstrumentId::new("DE0001102473"))
            .discount_curve_id(CurveId::new("EUR-BUNDS"))
            .attributes(Attributes::new())
            .build_validated()
            .expect("Valid Bund future");

        assert_eq!(future.id.as_str(), "FGBLH5");
        assert_eq!(future.quoted_price, 132.15);
        assert_eq!(future.position, Position::Long);
        assert_eq!(future.contract_specs.standard_coupon, 0.06);
        assert_eq!(future.contract_specs.standard_maturity_years, 10.0);
        assert_eq!(future.contract_specs.contract_size, 100_000.0);
        assert_eq!(future.contract_specs.tick_size, 0.01); // Decimal, not 32nds
        assert_eq!(future.deliverable_basket.len(), 1);
    }

    #[test]
    fn test_gilt_builder() {
        let deliverable = DeliverableBond {
            bond_id: InstrumentId::new("GB00B128DH60"),
            conversion_factor: 0.7234,
        };

        let future = BondFuture::builder()
            .id(InstrumentId::new("GILTH5"))
            .notional(Money::new(500_000.0, Currency::GBP))
            .expiry(Date::from_calendar_date(2025, Month::March, 20).expect("Valid date"))
            .delivery_start(Date::from_calendar_date(2025, Month::March, 21).expect("Valid date"))
            .delivery_end(Date::from_calendar_date(2025, Month::March, 31).expect("Valid date"))
            .quoted_price(115.25)
            .position(Position::Long)
            .contract_specs(BondFutureSpecs::gilt())
            .deliverable_basket(vec![deliverable])
            .ctd_bond_id(InstrumentId::new("GB00B128DH60"))
            .discount_curve_id(CurveId::new("GBP-GILTS"))
            .attributes(Attributes::new())
            .build_validated()
            .expect("Valid Gilt future");

        assert_eq!(future.id.as_str(), "GILTH5");
        assert_eq!(future.quoted_price, 115.25);
        assert_eq!(future.position, Position::Long);
        assert_eq!(future.contract_specs.standard_coupon, 0.04); // 4%, not 6%
        assert_eq!(future.contract_specs.standard_maturity_years, 10.0);
        assert_eq!(future.contract_specs.contract_size, 100_000.0);
        assert_eq!(future.contract_specs.tick_size, 0.01);
        assert_eq!(future.deliverable_basket.len(), 1);
    }

    #[test]
    fn test_builder_short_position() {
        let deliverable = DeliverableBond {
            bond_id: InstrumentId::new("US912828XG33"),
            conversion_factor: 0.8234,
        };

        let future = BondFuture::builder()
            .id(InstrumentId::new("TYH5"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .expiry(Date::from_calendar_date(2025, Month::March, 20).expect("Valid date"))
            .delivery_start(Date::from_calendar_date(2025, Month::March, 21).expect("Valid date"))
            .delivery_end(Date::from_calendar_date(2025, Month::March, 31).expect("Valid date"))
            .quoted_price(125.50)
            .position(Position::Short)
            .contract_specs(BondFutureSpecs::ust_10y())
            .deliverable_basket(vec![deliverable])
            .ctd_bond_id(InstrumentId::new("US912828XG33"))
            .discount_curve_id(CurveId::new("USD-TREASURY"))
            .attributes(Attributes::new())
            .build_validated()
            .expect("Valid short future");

        assert_eq!(future.position, Position::Short);
    }

    #[test]
    fn test_builder_validation_error() {
        let deliverable = DeliverableBond {
            bond_id: InstrumentId::new("US912828XG33"),
            conversion_factor: 0.8234,
        };

        // Invalid: expiry after delivery start
        let result = BondFuture::builder()
            .id(InstrumentId::new("TYH5"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .expiry(Date::from_calendar_date(2025, Month::March, 25).expect("Valid date")) // After delivery_start
            .delivery_start(Date::from_calendar_date(2025, Month::March, 21).expect("Valid date"))
            .delivery_end(Date::from_calendar_date(2025, Month::March, 31).expect("Valid date"))
            .quoted_price(125.50)
            .position(Position::Long)
            .contract_specs(BondFutureSpecs::ust_10y())
            .deliverable_basket(vec![deliverable])
            .ctd_bond_id(InstrumentId::new("US912828XG33"))
            .discount_curve_id(CurveId::new("USD-TREASURY"))
            .attributes(Attributes::new())
            .build_validated();

        assert!(result.is_err());
    }

    #[test]
    fn test_cashflow_provider_rejects_physical_delivery_schedule() {
        let ctd_bond_id = InstrumentId::new("US912828XG33");
        let ctd_bond = Bond::fixed(
            ctd_bond_id.as_str(),
            Money::new(100_000.0, Currency::USD),
            0.05,
            Date::from_calendar_date(2020, Month::January, 15).expect("valid date"),
            Date::from_calendar_date(2030, Month::January, 15).expect("valid date"),
            "USD-OIS",
        )
        .expect("valid bond");
        let future = BondFuture::builder()
            .id(InstrumentId::new("TYH5"))
            .notional(Money::new(100_000.0, Currency::USD))
            .expiry(Date::from_calendar_date(2025, Month::March, 20).expect("valid date"))
            .delivery_start(Date::from_calendar_date(2025, Month::March, 21).expect("valid date"))
            .delivery_end(Date::from_calendar_date(2025, Month::March, 31).expect("valid date"))
            .quoted_price(125.50)
            .position(Position::Long)
            .contract_specs(BondFutureSpecs::default())
            .deliverable_basket(vec![DeliverableBond {
                bond_id: ctd_bond_id.clone(),
                conversion_factor: 0.8234,
            }])
            .ctd_bond_id(ctd_bond_id)
            .ctd_bond_opt(Some(ctd_bond))
            .discount_curve_id(CurveId::new("USD-TREASURY"))
            .attributes(Attributes::new())
            .build()
            .expect("valid future");

        let err = future
            .dated_cashflows(&MarketContext::new(), future.expiry)
            .expect_err("physical delivery requires the typed invoice result");
        assert!(
            err.to_string().contains(
                "physical delivery cannot be represented as a standalone cashflow schedule"
            ),
            "unexpected error: {err}"
        );
    }
}

// Implement Instrument trait for BondFuture
impl crate::instruments::common_impl::traits::Instrument for BondFuture {
    impl_instrument_base!(crate::pricer::InstrumentType::BondFuture);

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        BondFuture::validate(self)
    }

    fn default_model(&self) -> crate::pricer::ModelKey {
        crate::pricer::ModelKey::BondFutureCleanPriceProxy
    }

    fn market_dependencies(&self) -> finstack_quant_core::Result<MarketDependencies> {
        let mut deps = MarketDependencies::new();
        deps.add_discount_curve(self.discount_curve_id.clone());
        if let Some(repo_curve_id) = &self.repo_curve_id {
            deps.add_forward_curve(repo_curve_id.clone());
        }
        if let Some(ctd_bond) = &self.ctd_bond {
            deps.merge(
                crate::instruments::common_impl::traits::Instrument::market_dependencies(ctd_bond)?,
            );
        }
        Ok(deps)
    }

    fn base_value(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        let ctd_bond_id = self.resolve_ctd_bond_id()?;
        let ctd_bond = self.ctd_bond.as_ref().ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "BondFuture '{}' requires an embedded ctd_bond to price (resolved ctd_bond_id={}). \
Provide it at construction time via BondFutureBuilder::ctd_bond(...) or by using a constructor that embeds the CTD bond.",
                self.id.as_str(),
                ctd_bond_id.as_str()
            ))
        })?;

        // Use the exchange-provided conversion factor from the deliverable basket.
        let conversion_factor = self
            .deliverable_basket
            .iter()
            .find(|bond| bond.bond_id == ctd_bond_id)
            .ok_or_else(|| {
                finstack_quant_core::Error::Input(finstack_quant_core::InputError::NotFound {
                    id: format!(
                        "CTD bond {} not found in deliverable basket",
                        ctd_bond_id.as_str()
                    ),
                })
            })?
            .conversion_factor;

        // Calculate and return NPV
        super::pricer::BondFuturePricer::calculate_npv(
            self,
            ctd_bond,
            conversion_factor,
            market,
            as_of,
        )
    }

    fn effective_start_date(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.delivery_start)
    }

    fn expiry(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.delivery_end)
    }

    crate::impl_focused_pricing_overrides!();
}

// Declare canonical market dependencies for DV01 calculators.
impl finstack_quant_cashflows::CashflowScheduleSource for BondFuture {
    fn notional(&self) -> Option<Money> {
        Some(self.notional)
    }

    fn raw_cashflow_schedule(
        &self,
        _market: &finstack_quant_core::market_data::context::MarketContext,
        _as_of: Date,
    ) -> finstack_quant_core::Result<CashFlowSchedule> {
        Err(finstack_quant_core::Error::Validation(
            "BondFuture physical delivery cannot be represented as a standalone cashflow schedule; use the typed delivery result"
                .to_string(),
        ))
    }
}

#[cfg(test)]
mod instrument_trait_tests {
    use super::*;
    use crate::instruments::common_impl::traits::Instrument;
    use finstack_quant_core::currency::Currency;
    use time::Month;

    #[test]
    fn test_instrument_trait_key() {
        let deliverable = DeliverableBond {
            bond_id: InstrumentId::new("US912828XG33"),
            conversion_factor: 0.8234,
        };

        let future = BondFuture::builder()
            .id(InstrumentId::new("TYH5"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .expiry(Date::from_calendar_date(2025, Month::March, 20).expect("Valid date"))
            .delivery_start(Date::from_calendar_date(2025, Month::March, 21).expect("Valid date"))
            .delivery_end(Date::from_calendar_date(2025, Month::March, 31).expect("Valid date"))
            .quoted_price(125.50)
            .position(Position::Long)
            .contract_specs(BondFutureSpecs::default())
            .deliverable_basket(vec![deliverable])
            .ctd_bond_id(InstrumentId::new("US912828XG33"))
            .discount_curve_id(CurveId::new("USD-TREASURY"))
            .attributes(Attributes::new())
            .build()
            .expect("Valid bond future");

        use crate::instruments::common_impl::traits::Instrument;
        use crate::pricer::InstrumentType;

        assert_eq!(future.key(), InstrumentType::BondFuture);
        assert_eq!(future.id(), "TYH5");
    }

    #[test]
    fn test_instrument_trait_id() {
        let deliverable = DeliverableBond {
            bond_id: InstrumentId::new("US912828XG33"),
            conversion_factor: 0.8234,
        };

        let future = BondFuture::builder()
            .id(InstrumentId::new("TYH5"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .expiry(Date::from_calendar_date(2025, Month::March, 20).expect("Valid date"))
            .delivery_start(Date::from_calendar_date(2025, Month::March, 21).expect("Valid date"))
            .delivery_end(Date::from_calendar_date(2025, Month::March, 31).expect("Valid date"))
            .quoted_price(125.50)
            .position(Position::Long)
            .contract_specs(BondFutureSpecs::default())
            .deliverable_basket(vec![deliverable])
            .ctd_bond_id(InstrumentId::new("US912828XG33"))
            .discount_curve_id(CurveId::new("USD-TREASURY"))
            .attributes(Attributes::new())
            .build()
            .expect("Valid bond future");

        use crate::instruments::common_impl::traits::Instrument;

        assert_eq!(future.id(), "TYH5");
    }

    #[test]
    fn test_instrument_trait_attributes() {
        let deliverable = DeliverableBond {
            bond_id: InstrumentId::new("US912828XG33"),
            conversion_factor: 0.8234,
        };

        let attrs = Attributes::new()
            .with_tag("futures")
            .with_meta("exchange", "CBOT");

        let future = BondFuture::builder()
            .id(InstrumentId::new("TYH5"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .expiry(Date::from_calendar_date(2025, Month::March, 20).expect("Valid date"))
            .delivery_start(Date::from_calendar_date(2025, Month::March, 21).expect("Valid date"))
            .delivery_end(Date::from_calendar_date(2025, Month::March, 31).expect("Valid date"))
            .quoted_price(125.50)
            .position(Position::Long)
            .contract_specs(BondFutureSpecs::default())
            .deliverable_basket(vec![deliverable])
            .ctd_bond_id(InstrumentId::new("US912828XG33"))
            .discount_curve_id(CurveId::new("USD-TREASURY"))
            .attributes(attrs)
            .build()
            .expect("Valid bond future");

        use crate::instruments::common_impl::traits::Instrument;

        assert!(future.attributes().has_tag("futures"));
        assert_eq!(future.attributes().get_meta("exchange"), Some("CBOT"));
    }

    #[test]
    fn test_instrument_trait_clone_box() {
        let deliverable = DeliverableBond {
            bond_id: InstrumentId::new("US912828XG33"),
            conversion_factor: 0.8234,
        };

        let future = BondFuture::builder()
            .id(InstrumentId::new("TYH5"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .expiry(Date::from_calendar_date(2025, Month::March, 20).expect("Valid date"))
            .delivery_start(Date::from_calendar_date(2025, Month::March, 21).expect("Valid date"))
            .delivery_end(Date::from_calendar_date(2025, Month::March, 31).expect("Valid date"))
            .quoted_price(125.50)
            .position(Position::Long)
            .contract_specs(BondFutureSpecs::default())
            .deliverable_basket(vec![deliverable])
            .ctd_bond_id(InstrumentId::new("US912828XG33"))
            .discount_curve_id(CurveId::new("USD-TREASURY"))
            .attributes(Attributes::new())
            .build()
            .expect("Valid bond future");

        use crate::instruments::common_impl::traits::Instrument;

        let cloned = future.clone_box();
        assert_eq!(cloned.id(), "TYH5");
        assert_eq!(cloned.key(), crate::pricer::InstrumentType::BondFuture);
    }

    #[test]
    fn test_instrument_trait_as_any_downcast() {
        let deliverable = DeliverableBond {
            bond_id: InstrumentId::new("US912828XG33"),
            conversion_factor: 0.8234,
        };

        let future = BondFuture::builder()
            .id(InstrumentId::new("TYH5"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .expiry(Date::from_calendar_date(2025, Month::March, 20).expect("Valid date"))
            .delivery_start(Date::from_calendar_date(2025, Month::March, 21).expect("Valid date"))
            .delivery_end(Date::from_calendar_date(2025, Month::March, 31).expect("Valid date"))
            .quoted_price(125.50)
            .position(Position::Long)
            .contract_specs(BondFutureSpecs::default())
            .deliverable_basket(vec![deliverable])
            .ctd_bond_id(InstrumentId::new("US912828XG33"))
            .discount_curve_id(CurveId::new("USD-TREASURY"))
            .attributes(Attributes::new())
            .build()
            .expect("Valid bond future");

        use crate::instruments::common_impl::traits::Instrument;

        let instrument: &dyn Instrument = &future;
        let concrete_future: Option<&BondFuture> = instrument.as_any().downcast_ref::<BondFuture>();
        assert!(concrete_future.is_some());
        assert_eq!(
            concrete_future.expect("Should be BondFuture").id.as_str(),
            "TYH5"
        );
    }

    #[test]
    fn test_required_discount_curves() {
        let deliverable = DeliverableBond {
            bond_id: InstrumentId::new("US912828XG33"),
            conversion_factor: 0.8234,
        };

        let future = BondFuture::builder()
            .id(InstrumentId::new("TYH5"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .expiry(Date::from_calendar_date(2025, Month::March, 20).expect("Valid date"))
            .delivery_start(Date::from_calendar_date(2025, Month::March, 21).expect("Valid date"))
            .delivery_end(Date::from_calendar_date(2025, Month::March, 31).expect("Valid date"))
            .quoted_price(125.50)
            .position(Position::Long)
            .contract_specs(BondFutureSpecs::default())
            .deliverable_basket(vec![deliverable])
            .ctd_bond_id(InstrumentId::new("US912828XG33"))
            .discount_curve_id(CurveId::new("USD-TREASURY"))
            .attributes(Attributes::new())
            .build()
            .expect("Valid bond future");

        use crate::instruments::common_impl::traits::Instrument;

        let curves = future
            .market_dependencies()
            .expect("market_dependencies should succeed")
            .curves
            .discount_curves;
        assert_eq!(curves.len(), 1);
        assert_eq!(curves[0].as_str(), "USD-TREASURY");
    }

    #[test]
    fn test_market_dependencies() {
        let deliverable = DeliverableBond {
            bond_id: InstrumentId::new("US912828XG33"),
            conversion_factor: 0.8234,
        };

        let future = BondFuture::builder()
            .id(InstrumentId::new("TYH5"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .expiry(Date::from_calendar_date(2025, Month::March, 20).expect("Valid date"))
            .delivery_start(Date::from_calendar_date(2025, Month::March, 21).expect("Valid date"))
            .delivery_end(Date::from_calendar_date(2025, Month::March, 31).expect("Valid date"))
            .quoted_price(125.50)
            .position(Position::Long)
            .contract_specs(BondFutureSpecs::default())
            .deliverable_basket(vec![deliverable])
            .ctd_bond_id(InstrumentId::new("US912828XG33"))
            .discount_curve_id(CurveId::new("USD-TREASURY"))
            .attributes(Attributes::new())
            .build()
            .expect("Valid bond future");

        let curves = future
            .market_dependencies()
            .expect("market_dependencies")
            .curves;
        assert_eq!(curves.discount_curves.len(), 1);
        assert_eq!(curves.discount_curves[0].as_str(), "USD-TREASURY");
        assert_eq!(curves.forward_curves.len(), 0);
        assert_eq!(curves.credit_curves.len(), 0);
        assert!(!curves.is_empty());
        assert_eq!(curves.len(), 1);
    }
}
