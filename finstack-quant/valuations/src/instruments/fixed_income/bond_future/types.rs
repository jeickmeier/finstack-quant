//! Bond future core types.
//!
//! This module defines the data structures for bond futures, including
//! the deliverable basket, contract specifications, and the main BondFuture type.

use crate::cashflow::builder::CashFlowSchedule;
use crate::contract_specs::{embedded_registry, ContractSpecRegistry};
use crate::instruments::common_impl::dependencies::MarketDependencies;
use crate::instruments::common_impl::listed::ListedFutureTerms;
use crate::instruments::common_impl::traits::impl_instrument_base;
use crate::instruments::common_impl::traits::Attributes;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
enum RepoDayCountWire {
    #[serde(rename = "act_360")]
    Act360,
    #[serde(rename = "act_365f")]
    Act365F,
}

impl From<RepoDayCountWire> for DayCount {
    fn from(value: RepoDayCountWire) -> Self {
        match value {
            RepoDayCountWire::Act360 => Self::Act360,
            RepoDayCountWire::Act365F => Self::Act365F,
        }
    }
}

impl TryFrom<DayCount> for RepoDayCountWire {
    type Error = finstack_quant_core::Error;

    fn try_from(value: DayCount) -> Result<Self, Self::Error> {
        match value {
            DayCount::Act360 => Ok(Self::Act360),
            DayCount::Act365F => Ok(Self::Act365F),
            unsupported => Err(finstack_quant_core::Error::Validation(format!(
                "bond-future repo_day_count must be act_360 or act_365f, got {unsupported:?}"
            ))),
        }
    }
}

mod repo_day_count_wire {
    use super::{DayCount, RepoDayCountWire};
    use serde::{Deserialize, Serialize};

    pub(super) fn serialize<S>(value: &DayCount, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        RepoDayCountWire::try_from(*value)
            .map_err(serde::ser::Error::custom)?
            .serialize(serializer)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<DayCount, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        RepoDayCountWire::deserialize(deserializer).map(Into::into)
    }
}

mod deliverable_basket_wire {
    use super::DeliverableBond;
    use serde::{Deserialize, Serialize};

    pub(super) fn serialize<S>(value: &[DeliverableBond], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if value.is_empty() {
            return Err(serde::ser::Error::custom(
                "deliverable_basket cannot be empty",
            ));
        }
        value.serialize(serializer)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Vec<DeliverableBond>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Vec::<DeliverableBond>::deserialize(deserializer)?;
        if value.is_empty() {
            return Err(serde::de::Error::custom(
                "deliverable_basket cannot be empty",
            ));
        }
        Ok(value)
    }
}

/// An eligible deliverable and the conversion factor used by pricing.
///
/// Pricing and CTD helpers consume the supplied positive factor; they do not
/// recalculate it. Prefer the exchange-published value for the security and
/// delivery month. For CME/CBOT contracts, callers can use
/// [`super::BondFuturePricer::calculate_conversion_factor`] to calculate a
/// factor and store the result here.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DeliverableBond {
    /// Identifier of the deliverable bond
    pub bond_id: InstrumentId,
    /// Positive conversion factor consumed by the model-price calculation.
    #[serde(
        serialize_with = "finstack_quant_core::wire::serialize_positive_f64",
        deserialize_with = "finstack_quant_core::wire::deserialize_positive_f64"
    )]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::PositiveF64Wire")
    )]
    pub conversion_factor: f64,
}

/// Contract specifications for bond futures.
///
/// Defines the notional bond parameters used for conversion factor
/// calculations and the implied-repo day count. Contract size lives on the
/// future's `terms`: `terms.multiplier` is the currency value of one full
/// price point, i.e. one hundredth of the per-contract face (1,000 for a
/// $100,000 UST 10Y contract).
///
/// Delivery timing is carried by the future's explicit `delivery_start` /
/// `terms.settlement_date`, so the spec holds no settlement lag or holiday
/// calendar.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations::instruments::fixed_income::bond_future::BondFutureSpecs;
///
/// // UST 10-year contract specs
/// let specs = BondFutureSpecs::default(); // UST 10Y defaults
/// assert_eq!(specs.standard_coupon, 0.06);
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BondFutureSpecs {
    /// Standard coupon rate for conversion factor calculation (e.g., 0.06 for 6%)
    pub standard_coupon: f64,
    /// Standard maturity in years for conversion factor calculation
    pub standard_maturity_years: f64,
    /// Day-count convention for implied repo rate annualization.
    ///
    /// Bond-future repo supports `act_360` and `act_365f`.
    #[serde(default = "default_repo_day_count", with = "repo_day_count_wire")]
    #[cfg_attr(feature = "json-schema", schemars(with = "RepoDayCountWire"))]
    pub repo_day_count: DayCount,
}

fn default_repo_day_count() -> DayCount {
    DayCount::Act360
}

fn repo_annualization_denominator(day_count: DayCount) -> finstack_quant_core::Result<f64> {
    match day_count {
        DayCount::Act360 => Ok(360.0),
        DayCount::Act365F => Ok(365.0),
        unsupported => Err(finstack_quant_core::Error::Validation(format!(
            "bond-future repo_day_count must be act_360 or act_365f, got {unsupported:?}"
        ))),
    }
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
    /// - Contract face: $100,000 (`terms.multiplier` = 1,000 per point)
    /// - Standard coupon: 6% (0.06)
    /// - Standard maturity: 10 years
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
    /// - Contract face: $100,000 (`terms.multiplier` = 1,000 per point)
    /// - Standard coupon: 6% annual
    /// - Standard maturity: 10 years
    /// - Day count: Actual/Actual (ISDA)
    /// - Deliverable: U.S. Treasury notes with at least 6.5 years remaining maturity
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_valuations::instruments::fixed_income::bond_future::BondFutureSpecs;
    ///
    /// let specs = BondFutureSpecs::ust_10y();
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
    /// - Contract face: $100,000 (`terms.multiplier` = 1,000 per point)
    /// - Standard coupon: 6% annual
    /// - Standard maturity: 5 years
    /// - Day count: Actual/Actual (ISDA)
    /// - Deliverable: U.S. Treasury notes with at least 4 years, 2 months remaining maturity
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_valuations::instruments::fixed_income::bond_future::BondFutureSpecs;
    ///
    /// let specs = BondFutureSpecs::ust_5y();
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
    /// - Contract face: $200,000 (`terms.multiplier` = 2,000 per point; double the 5Y/10Y contracts)
    /// - Standard coupon: 6% annual
    /// - Standard maturity: 2 years
    /// - Day count: Actual/Actual (ISDA)
    /// - Deliverable: U.S. Treasury notes with at least 1 year, 9 months remaining maturity
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_valuations::instruments::fixed_income::bond_future::BondFutureSpecs;
    ///
    /// let specs = BondFutureSpecs::ust_2y();
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
    /// - Contract face: €100,000 (`terms.multiplier` = 1,000 per point)
    /// - Standard coupon: 6% annual
    /// - Standard maturity: 10 years
    /// - Day count: Actual/Actual (ISDA)
    /// - Deliverable: German Federal bonds with 8.5 to 10.5 years remaining maturity
    ///
    /// # Notes
    ///
    /// - Quoted in percentage points (e.g., 125.50 = 125.50%)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_valuations::instruments::fixed_income::bond_future::BondFutureSpecs;
    ///
    /// let specs = BondFutureSpecs::bund();
    /// assert_eq!(specs.standard_coupon, 0.06);
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
    /// - Contract face: £100,000 (`terms.multiplier` = 1,000 per point)
    /// - Standard coupon: 4% annual (note: different from UST/Bund 6%)
    /// - Standard maturity: 10 years
    /// - Day count: Actual/Actual (ISDA)
    /// - Deliverable: UK Gilts with 8.75 to 13 years remaining maturity
    ///
    /// # Notes
    ///
    /// - Quoted in percentage points (e.g., 125.50 = 125.50%)
    /// - Standard coupon is 4%, not 6% like other major markets
    /// - Long Gilt contract covers 8.75-13 year maturity range
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_valuations::instruments::fixed_income::bond_future::BondFutureSpecs;
    ///
    /// let specs = BondFutureSpecs::gilt();
    /// assert_eq!(specs.standard_coupon, 0.04);  // 4%, not 6%
    /// ```
    pub fn gilt() -> Self {
        bond_future_specs_from_registry("ice.gilt")
    }
}

/// Bond future instrument.
///
/// A standardized contract with a basket of eligible deliverables. The short
/// chooses the bond to deliver, commonly the cheapest-to-deliver (CTD) bond.
///
/// # Contract Mechanics
///
/// - **Conversion factors**: Supplied per deliverable, preferably from the exchange.
/// - **CTD resolution**: Explicit `ctd_bond_id`, then embedded `ctd_bond.id`,
///   then the sole basket member. A larger basket requires an explicit or embedded CTD.
/// - **CTD analysis**: [`crate::instruments::Instrument::value`] marks the
///   caller-supplied CTD only; it does not rank the basket. Refresh the CTD
///   selection daily when the basket can switch.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::types::{CurveId, InstrumentId};
/// use finstack_quant_valuations::instruments::fixed_income::bond_future::{
///     BondFuture, BondFutureSpecs, DeliverableBond,
/// };
/// use finstack_quant_valuations::instruments::{Attributes, ListedFutureTerms, Position};
/// use time::macros::date;
///
/// # fn main() -> finstack_quant_core::Result<()> {
/// // 10 UST 10Y contracts ($100,000 face each, $1,000 per price point).
/// let future = BondFuture::builder()
///     .id(InstrumentId::new("TYH5"))
///     .terms(ListedFutureTerms::new(
///         10.0,
///         1_000.0,
///         Currency::USD,
///         125.50,
///         date!(2025 - 03 - 20),
///         date!(2025 - 03 - 31),
///         Position::Long,
///     )?)
///     .delivery_start(date!(2025 - 03 - 21))
///     .contract_specs(BondFutureSpecs::ust_10y())
///     .deliverable_basket(vec![DeliverableBond {
///         bond_id: InstrumentId::new("US912828XG33"),
///         conversion_factor: 0.8234,
///     }])
///     .discount_curve_id(CurveId::new("USD-TREASURY"))
///     .attributes(Attributes::new())
///     .build()?;
/// assert_eq!(future.deliverable_basket.len(), 1);
/// # Ok(())
/// # }
/// ```
#[derive(
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BondFuture {
    /// Unique identifier for the contract
    pub id: InstrumentId,

    /// Standard listed position and lifecycle terms. `terms.multiplier` is the
    /// currency value of one full price point (per-contract face / 100),
    /// `terms.entry_price` the trade price per 100 face, `terms.last_trading_date`
    /// the last trading day and `terms.settlement_date` the last delivery date.
    pub terms: ListedFutureTerms,

    /// First delivery date
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub delivery_start: Date,

    /// Contract specifications (standard coupon, standard maturity, repo day count)
    pub contract_specs: BondFutureSpecs,

    /// Basket of deliverable bonds with conversion factors
    #[serde(with = "deliverable_basket_wire")]
    #[cfg_attr(feature = "json-schema", schemars(with = "Vec<DeliverableBond>"))]
    #[cfg_attr(feature = "json-schema", schemars(length(min = 1)))]
    pub deliverable_basket: Vec<DeliverableBond>,

    /// Selected cheapest-to-deliver (CTD) bond identifier.
    ///
    /// Resolution is deterministic: this explicit identifier takes precedence,
    /// followed by `ctd_bond.id`, then the sole basket member. A larger basket
    /// with neither form of selection fails validation. The resolved identifier
    /// must be in `deliverable_basket`; an embedded bond must have the same ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ctd_bond_id: Option<InstrumentId>,

    /// Optional embedded CTD bond definition.
    ///
    /// Model-price and NPV paths require this definition because
    /// `MarketContext` contains market data, not an instrument registry.
    /// Identifier-only CTD analysis and conversion-factor lookup do not require it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    pub ctd_bond: Option<crate::instruments::fixed_income::bond::Bond>,

    /// Financing/discount curve identifier.
    ///
    /// Used to carry the CTD bond forward to the delivery date (the cost of
    /// financing the position) whenever no explicit [`repo_curve_id`](Self::repo_curve_id)
    /// is set. Must be provisionable as a discount curve in the market context.
    pub discount_curve_id: CurveId,

    /// Optional repo/financing curve identifier.
    ///
    /// When set, this curve is used for financing/carry calculations instead
    /// of `discount_curve_id`. This allows capturing repo specials, where
    /// specific collateral (e.g., on-the-run Treasuries) trades at rates
    /// different from the general funding curve.
    ///
    /// If `None`, the `discount_curve_id` is used for financing calculations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo_curve_id: Option<CurveId>,

    /// Instrument-owned pricing inputs.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::InstrumentPricingOverrides::is_empty"
    )]
    pub instrument_pricing_overrides: crate::instruments::InstrumentPricingOverrides,
    /// Metric-time pricing configuration.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::MetricPricingOverrides::is_empty"
    )]
    pub metric_pricing_overrides: crate::instruments::MetricPricingOverrides,
    /// Scenario-only pricing adjustments.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::ScenarioPricingOverrides::is_empty"
    )]
    pub scenario_pricing_overrides: crate::instruments::ScenarioPricingOverrides,
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,
}

impl BondFuture {
    /// Create a representative UST 10Y bond future example.
    ///
    /// Long position, 10 contracts ($1M face), 2 deliverable bonds
    /// with published conversion factors.
    pub fn example() -> finstack_quant_core::Result<Self> {
        use crate::instruments::Position;
        use finstack_quant_core::currency::Currency;

        let last_trading_date = time::macros::date!(2025 - 09 - 19);
        let delivery_start = time::macros::date!(2025 - 09 - 22);
        let last_delivery_date = time::macros::date!(2025 - 09 - 30);

        let bond1_id = InstrumentId::new("US91282CJL54");
        let bond2_id = InstrumentId::new("US91282CHT18");

        Self::builder()
            .id(InstrumentId::new("TYU5"))
            .terms(ListedFutureTerms::new(
                10.0,
                1_000.0,
                Currency::USD,
                112.25,
                last_trading_date,
                last_delivery_date,
                Position::Long,
            )?)
            .delivery_start(delivery_start)
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
            .build()
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

    /// Conversion factor of a basket member, looked up by bond identifier.
    fn conversion_factor_for(&self, bond_id: &InstrumentId) -> finstack_quant_core::Result<f64> {
        self.deliverable_basket
            .iter()
            .find(|deliverable| deliverable.bond_id == *bond_id)
            .map(|deliverable| deliverable.conversion_factor)
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "BondFuture '{}': bond {} is not in deliverable_basket",
                    self.id.as_str(),
                    bond_id.as_str()
                ))
            })
    }

    /// Embedded CTD bond and its basket conversion factor.
    fn embedded_ctd(
        &self,
    ) -> finstack_quant_core::Result<(&crate::instruments::fixed_income::bond::Bond, f64)> {
        let ctd_bond_id = self.resolve_ctd_bond_id()?;
        let ctd_bond = self.ctd_bond.as_ref().ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "BondFuture '{}' requires an embedded ctd_bond to price (resolved ctd_bond_id={}). \
Provide it at construction time via BondFutureBuilder::ctd_bond(...) or by using a constructor that embeds the CTD bond.",
                self.id.as_str(),
                ctd_bond_id.as_str()
            ))
        })?;
        Ok((ctd_bond, self.conversion_factor_for(&ctd_bond_id)?))
    }

    /// Model futures price in points per 100 face: the carry-adjusted forward
    /// clean price of the embedded CTD bond at delivery divided by its
    /// conversion factor.
    ///
    /// # Arguments
    ///
    /// * `market` - Market context containing the CTD pricing inputs and the
    ///   financing curve (`repo_curve_id`, else `discount_curve_id`).
    /// * `as_of` - Valuation date for the spot CTD value; the CTD is carried to
    ///   `max(delivery_start, as_of)`.
    ///
    /// # Errors
    ///
    /// Returns an error when no CTD bond is embedded or CTD pricing fails.
    pub fn fair_price(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        let (ctd_bond, conversion_factor) = self.embedded_ctd()?;
        super::pricer::BondFuturePricer::fair_price_for_future(
            self,
            ctd_bond,
            conversion_factor,
            market,
            as_of,
        )
    }

    /// Resolve the live quote, model price, or official final settlement price.
    ///
    /// Live contracts use `terms.terms.entry_price` when supplied and otherwise
    /// [`Self::fair_price`]; after `terms.last_trading_date` the official
    /// `terms.settlement_price` is required.
    ///
    /// # Arguments
    ///
    /// * `market` - Market context containing the CTD pricing inputs when a
    ///   live model price is needed.
    /// * `as_of` - Valuation date controlling live versus post-trading state.
    pub fn mark_price(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        self.terms
            .resolve_mark(self.id.as_str(), as_of, || self.fair_price(market, as_of))
    }

    /// Conversion factor of the resolved cheapest-to-deliver bond.
    ///
    /// Resolves the CTD as documented on [`Self::ctd_bond_id`] and returns its
    /// basket conversion factor.
    ///
    /// # Errors
    ///
    /// Returns a validation error when the CTD cannot be resolved or is not a
    /// member of `deliverable_basket`.
    pub(crate) fn ctd_conversion_factor(&self) -> finstack_quant_core::Result<f64> {
        self.conversion_factor_for(&self.resolve_ctd_bond_id()?)
    }

    /// Validate the BondFuture parameters.
    ///
    /// This method checks the following invariants:
    /// - Date ordering: terms.last_trading_date < delivery_start < terms.settlement_date
    /// - Deliverable basket is non-empty
    /// - CTD bond exists in deliverable basket
    /// - All conversion factors are positive
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`](finstack_quant_core::Error::Validation) if any validation fails.
    fn validate(&self) -> finstack_quant_core::Result<()> {
        // Date ordering validation
        self.terms.validate()?;
        if self.terms.last_trading_date >= self.delivery_start {
            return Err(finstack_quant_core::Error::Validation(format!(
                "terms.last_trading_date ({}) must be before delivery_start ({})",
                self.terms.last_trading_date, self.delivery_start
            )));
        }
        if self.delivery_start >= self.terms.settlement_date {
            return Err(finstack_quant_core::Error::Validation(format!(
                "delivery_start ({}) must be before terms.settlement_date ({})",
                self.delivery_start, self.terms.settlement_date
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

        if self.terms.entry_price < 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "terms.entry_price must be finite and non-negative, got {}",
                self.terms.entry_price
            )));
        }
        repo_annualization_denominator(self.contract_specs.repo_day_count)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cashflow::CashflowProvider;
    use crate::instruments::fixed_income::bond::Bond;
    use crate::instruments::Position;
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
        assert_eq!(specs.standard_coupon, 0.06);
        assert_eq!(specs.standard_maturity_years, 10.0);
    }

    #[test]
    fn test_ust_10y_specs() {
        let specs = BondFutureSpecs::ust_10y();
        assert_eq!(specs.standard_coupon, 0.06);
        assert_eq!(specs.standard_maturity_years, 10.0);
        assert_eq!(specs.repo_day_count, DayCount::Act360);
    }

    #[test]
    fn test_ust_5y_specs() {
        let specs = BondFutureSpecs::ust_5y();
        assert_eq!(specs.standard_coupon, 0.06);
        assert_eq!(specs.standard_maturity_years, 5.0);
    }

    #[test]
    fn test_ust_2y_specs() {
        let specs = BondFutureSpecs::ust_2y();
        assert_eq!(specs.standard_coupon, 0.06);
        assert_eq!(specs.standard_maturity_years, 2.0);
    }

    #[test]
    fn test_bund_specs() {
        let specs = BondFutureSpecs::bund();
        assert_eq!(specs.standard_coupon, 0.06);
        assert_eq!(specs.standard_maturity_years, 10.0);
    }

    #[test]
    fn test_gilt_specs() {
        let specs = BondFutureSpecs::gilt();
        assert_eq!(specs.standard_coupon, 0.04); // Different from UST/Bund
        assert_eq!(specs.standard_maturity_years, 10.0);
        assert_eq!(specs.repo_day_count, DayCount::Act365F);
    }

    #[test]
    fn repo_day_count_wire_accepts_only_supported_canonical_values() {
        for value in ["act_360", "act_365f"] {
            let mut json = serde_json::to_value(BondFutureSpecs::default())
                .expect("serialize bond future specs");
            json["repo_day_count"] = serde_json::json!(value);
            assert!(
                serde_json::from_value::<BondFutureSpecs>(json).is_ok(),
                "{value} must be accepted"
            );
        }

        for value in ["act360", "act365", "30_360", "act_act"] {
            let mut json = serde_json::to_value(BondFutureSpecs::default())
                .expect("serialize bond future specs");
            json["repo_day_count"] = serde_json::json!(value);
            assert!(
                serde_json::from_value::<BondFutureSpecs>(json).is_err(),
                "{value} must be rejected"
            );
        }
    }

    #[test]
    fn test_bond_future_construction() {
        let deliverable = DeliverableBond {
            bond_id: InstrumentId::new("US912828XG33"),
            conversion_factor: 0.8234,
        };

        let future = BondFuture::builder()
            .id(InstrumentId::new("TYH5"))
            .terms(
                ListedFutureTerms::new(
                    10.0,
                    1_000.0,
                    Currency::USD,
                    125.50,
                    Date::from_calendar_date(2025, Month::March, 20).expect("Valid date"),
                    Date::from_calendar_date(2025, Month::March, 31).expect("Valid date"),
                    Position::Long,
                )
                .expect("terms"),
            )
            .delivery_start(Date::from_calendar_date(2025, Month::March, 21).expect("Valid date"))
            .contract_specs(BondFutureSpecs::default())
            .deliverable_basket(vec![deliverable])
            .ctd_bond_id(InstrumentId::new("US912828XG33"))
            .discount_curve_id(CurveId::new("USD-TREASURY"))
            .attributes(Attributes::new())
            .build()
            .expect("Valid bond future");

        assert_eq!(future.id.as_str(), "TYH5");
        assert_eq!(future.terms.entry_price, 125.50);
        assert_eq!(future.terms.position, Position::Long);
        assert_eq!(future.deliverable_basket.len(), 1);
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
            .terms(
                ListedFutureTerms::new(
                    10.0,
                    1_000.0,
                    Currency::USD,
                    125.50,
                    Date::from_calendar_date(2025, Month::March, 31).expect("Valid date"),
                    Date::from_calendar_date(2025, Month::March, 31).expect("Valid date"),
                    Position::Long,
                )
                .expect("terms"),
            )
            .delivery_start(Date::from_calendar_date(2025, Month::March, 21).expect("Valid date"))
            .contract_specs(BondFutureSpecs::default())
            .deliverable_basket(vec![deliverable])
            .ctd_bond_id(InstrumentId::new("US912828XG33"))
            .discount_curve_id(CurveId::new("USD-TREASURY"))
            .attributes(Attributes::new())
            .build();

        assert!(result.is_err());
        let err_msg = format!("{}", result.expect_err("Should have validation error"));
        assert!(err_msg.contains("terms.last_trading_date") && err_msg.contains("delivery_start"));
    }

    #[test]
    fn test_validation_date_ordering_delivery_start_after_end() {
        let deliverable = DeliverableBond {
            bond_id: InstrumentId::new("US912828XG33"),
            conversion_factor: 0.8234,
        };

        // delivery_start >= terms.settlement_date (invalid)
        let result = BondFuture::builder()
            .id(InstrumentId::new("TYH5"))
            .terms(
                ListedFutureTerms::new(
                    10.0,
                    1_000.0,
                    Currency::USD,
                    125.50,
                    Date::from_calendar_date(2025, Month::March, 20).expect("Valid date"),
                    Date::from_calendar_date(2025, Month::March, 21).expect("Valid date"),
                    Position::Long,
                )
                .expect("terms"),
            )
            .delivery_start(Date::from_calendar_date(2025, Month::March, 31).expect("Valid date")) // Wrong: after terms.settlement_date
            .contract_specs(BondFutureSpecs::default())
            .deliverable_basket(vec![deliverable])
            .ctd_bond_id(InstrumentId::new("US912828XG33"))
            .discount_curve_id(CurveId::new("USD-TREASURY"))
            .attributes(Attributes::new())
            .build();

        assert!(result.is_err());
        let err_msg = format!("{}", result.expect_err("Should have validation error"));
        assert!(err_msg.contains("delivery_start") && err_msg.contains("terms.settlement_date"));
    }

    #[test]
    fn test_validation_empty_basket() {
        // Empty deliverable basket (invalid)
        let result = BondFuture::builder()
            .id(InstrumentId::new("TYH5"))
            .terms(
                ListedFutureTerms::new(
                    10.0,
                    1_000.0,
                    Currency::USD,
                    125.50,
                    Date::from_calendar_date(2025, Month::March, 20).expect("Valid date"),
                    Date::from_calendar_date(2025, Month::March, 31).expect("Valid date"),
                    Position::Long,
                )
                .expect("terms"),
            )
            .delivery_start(Date::from_calendar_date(2025, Month::March, 21).expect("Valid date"))
            .contract_specs(BondFutureSpecs::default())
            .deliverable_basket(vec![]) // Invalid: empty
            .ctd_bond_id(InstrumentId::new("US912828XG33"))
            .discount_curve_id(CurveId::new("USD-TREASURY"))
            .attributes(Attributes::new())
            .build();

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
            .terms(
                ListedFutureTerms::new(
                    10.0,
                    1_000.0,
                    Currency::USD,
                    125.50,
                    Date::from_calendar_date(2025, Month::March, 20).expect("Valid date"),
                    Date::from_calendar_date(2025, Month::March, 31).expect("Valid date"),
                    Position::Long,
                )
                .expect("terms"),
            )
            .delivery_start(Date::from_calendar_date(2025, Month::March, 21).expect("Valid date"))
            .contract_specs(BondFutureSpecs::default())
            .deliverable_basket(vec![deliverable])
            .ctd_bond_id(InstrumentId::new("UNKNOWN_BOND_ID")) // Invalid: not in basket
            .discount_curve_id(CurveId::new("USD-TREASURY"))
            .attributes(Attributes::new())
            .build();

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
            .terms(
                ListedFutureTerms::new(
                    10.0,
                    1_000.0,
                    Currency::USD,
                    125.50,
                    Date::from_calendar_date(2025, Month::March, 20).expect("Valid date"),
                    Date::from_calendar_date(2025, Month::March, 31).expect("Valid date"),
                    Position::Long,
                )
                .expect("terms"),
            )
            .delivery_start(Date::from_calendar_date(2025, Month::March, 21).expect("Valid date"))
            .contract_specs(BondFutureSpecs::default())
            .deliverable_basket(vec![deliverable_valid, deliverable_invalid])
            .ctd_bond_id(InstrumentId::new("US912828XG33"))
            .discount_curve_id(CurveId::new("USD-TREASURY"))
            .attributes(Attributes::new())
            .build();

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
            .terms(
                ListedFutureTerms::new(
                    10.0,
                    1_000.0,
                    Currency::USD,
                    125.50,
                    Date::from_calendar_date(2025, Month::March, 20).expect("Valid date"),
                    Date::from_calendar_date(2025, Month::March, 31).expect("Valid date"),
                    Position::Long,
                )
                .expect("terms"),
            )
            .delivery_start(Date::from_calendar_date(2025, Month::March, 21).expect("Valid date"))
            .contract_specs(BondFutureSpecs::default())
            .deliverable_basket(vec![deliverable_invalid])
            .ctd_bond_id(InstrumentId::new("US912828XG33"))
            .discount_curve_id(CurveId::new("USD-TREASURY"))
            .attributes(Attributes::new())
            .build();

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
            .terms(
                ListedFutureTerms::new(
                    10.0,
                    1_000.0,
                    Currency::USD,
                    125.50,
                    Date::from_calendar_date(2025, Month::March, 20).expect("Valid date"),
                    Date::from_calendar_date(2025, Month::March, 31).expect("Valid date"),
                    Position::Long,
                )
                .expect("terms"),
            )
            .delivery_start(Date::from_calendar_date(2025, Month::March, 21).expect("Valid date"))
            .contract_specs(BondFutureSpecs::default())
            .deliverable_basket(vec![deliverable])
            .ctd_bond_id(InstrumentId::new("US912828XG33"))
            .discount_curve_id(CurveId::new("USD-TREASURY"))
            .attributes(Attributes::new())
            .build();

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
            .terms(
                ListedFutureTerms::new(
                    10.0,
                    1_000.0,
                    Currency::USD,
                    125.50,
                    Date::from_calendar_date(2025, Month::March, 20).expect("Valid date"),
                    Date::from_calendar_date(2025, Month::March, 31).expect("Valid date"),
                    Position::Long,
                )
                .expect("terms"),
            )
            .delivery_start(Date::from_calendar_date(2025, Month::March, 21).expect("Valid date"))
            .contract_specs(BondFutureSpecs::default())
            .deliverable_basket(vec![deliverable])
            .discount_curve_id(CurveId::new("USD-TREASURY"))
            .attributes(Attributes::new())
            .build();

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
            .terms(
                ListedFutureTerms::new(
                    10.0,
                    1_000.0,
                    Currency::USD,
                    125.50,
                    Date::from_calendar_date(2025, Month::March, 20).expect("Valid date"),
                    Date::from_calendar_date(2025, Month::March, 31).expect("Valid date"),
                    Position::Long,
                )
                .expect("terms"),
            )
            .delivery_start(Date::from_calendar_date(2025, Month::March, 21).expect("Valid date"))
            .contract_specs(BondFutureSpecs::ust_10y())
            .deliverable_basket(vec![deliverable])
            .ctd_bond_id(InstrumentId::new("US912828XG33"))
            .discount_curve_id(CurveId::new("USD-TREASURY"))
            .attributes(Attributes::new())
            .build()
            .expect("Valid UST 10Y future");

        assert_eq!(future.id.as_str(), "TYH5");
        assert_eq!(future.terms.entry_price, 125.50);
        assert_eq!(future.terms.position, Position::Long);
        assert_eq!(future.contract_specs.standard_coupon, 0.06);
        assert_eq!(future.contract_specs.standard_maturity_years, 10.0);
        assert_eq!(future.terms.multiplier, 1_000.0);
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
            .terms(
                ListedFutureTerms::new(
                    5.0,
                    1_000.0,
                    Currency::USD,
                    118.75,
                    Date::from_calendar_date(2025, Month::March, 20).expect("Valid date"),
                    Date::from_calendar_date(2025, Month::March, 31).expect("Valid date"),
                    Position::Long,
                )
                .expect("terms"),
            )
            .delivery_start(Date::from_calendar_date(2025, Month::March, 21).expect("Valid date"))
            .contract_specs(BondFutureSpecs::ust_5y())
            .deliverable_basket(vec![deliverable])
            .ctd_bond_id(InstrumentId::new("US912828XG33"))
            .discount_curve_id(CurveId::new("USD-TREASURY"))
            .attributes(Attributes::new())
            .build()
            .expect("Valid UST 5Y future");

        assert_eq!(future.id.as_str(), "FVH5");
        assert_eq!(future.terms.entry_price, 118.75);
        assert_eq!(future.terms.position, Position::Long);
        assert_eq!(future.contract_specs.standard_coupon, 0.06);
        assert_eq!(future.contract_specs.standard_maturity_years, 5.0);
        assert_eq!(future.terms.multiplier, 1_000.0);
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
            .terms(
                ListedFutureTerms::new(
                    2.0,
                    2_000.0,
                    Currency::USD,
                    105.25,
                    Date::from_calendar_date(2025, Month::March, 20).expect("Valid date"),
                    Date::from_calendar_date(2025, Month::March, 31).expect("Valid date"),
                    Position::Long,
                )
                .expect("terms"),
            )
            .delivery_start(Date::from_calendar_date(2025, Month::March, 21).expect("Valid date"))
            .contract_specs(BondFutureSpecs::ust_2y())
            .deliverable_basket(vec![deliverable])
            .ctd_bond_id(InstrumentId::new("US912828XG33"))
            .discount_curve_id(CurveId::new("USD-TREASURY"))
            .attributes(Attributes::new())
            .build()
            .expect("Valid UST 2Y future");

        assert_eq!(future.id.as_str(), "TUH5");
        assert_eq!(future.terms.entry_price, 105.25);
        assert_eq!(future.terms.position, Position::Long);
        assert_eq!(future.contract_specs.standard_coupon, 0.06);
        assert_eq!(future.contract_specs.standard_maturity_years, 2.0);
        assert_eq!(future.terms.multiplier, 2_000.0);
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
            .terms(
                ListedFutureTerms::new(
                    10.0,
                    1_000.0,
                    Currency::EUR,
                    132.15,
                    Date::from_calendar_date(2025, Month::March, 20).expect("Valid date"),
                    Date::from_calendar_date(2025, Month::March, 31).expect("Valid date"),
                    Position::Long,
                )
                .expect("terms"),
            )
            .delivery_start(Date::from_calendar_date(2025, Month::March, 21).expect("Valid date"))
            .contract_specs(BondFutureSpecs::bund())
            .deliverable_basket(vec![deliverable])
            .ctd_bond_id(InstrumentId::new("DE0001102473"))
            .discount_curve_id(CurveId::new("EUR-BUNDS"))
            .attributes(Attributes::new())
            .build()
            .expect("Valid Bund future");

        assert_eq!(future.id.as_str(), "FGBLH5");
        assert_eq!(future.terms.entry_price, 132.15);
        assert_eq!(future.terms.position, Position::Long);
        assert_eq!(future.contract_specs.standard_coupon, 0.06);
        assert_eq!(future.contract_specs.standard_maturity_years, 10.0);
        assert_eq!(future.terms.multiplier, 1_000.0);
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
            .terms(
                ListedFutureTerms::new(
                    5.0,
                    1_000.0,
                    Currency::GBP,
                    115.25,
                    Date::from_calendar_date(2025, Month::March, 20).expect("Valid date"),
                    Date::from_calendar_date(2025, Month::March, 31).expect("Valid date"),
                    Position::Long,
                )
                .expect("terms"),
            )
            .delivery_start(Date::from_calendar_date(2025, Month::March, 21).expect("Valid date"))
            .contract_specs(BondFutureSpecs::gilt())
            .deliverable_basket(vec![deliverable])
            .ctd_bond_id(InstrumentId::new("GB00B128DH60"))
            .discount_curve_id(CurveId::new("GBP-GILTS"))
            .attributes(Attributes::new())
            .build()
            .expect("Valid Gilt future");

        assert_eq!(future.id.as_str(), "GILTH5");
        assert_eq!(future.terms.entry_price, 115.25);
        assert_eq!(future.terms.position, Position::Long);
        assert_eq!(future.contract_specs.standard_coupon, 0.04); // 4%, not 6%
        assert_eq!(future.contract_specs.standard_maturity_years, 10.0);
        assert_eq!(future.terms.multiplier, 1_000.0);
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
            .terms(
                ListedFutureTerms::new(
                    10.0,
                    1_000.0,
                    Currency::USD,
                    125.50,
                    Date::from_calendar_date(2025, Month::March, 20).expect("Valid date"),
                    Date::from_calendar_date(2025, Month::March, 31).expect("Valid date"),
                    Position::Short,
                )
                .expect("terms"),
            )
            .delivery_start(Date::from_calendar_date(2025, Month::March, 21).expect("Valid date"))
            .contract_specs(BondFutureSpecs::ust_10y())
            .deliverable_basket(vec![deliverable])
            .ctd_bond_id(InstrumentId::new("US912828XG33"))
            .discount_curve_id(CurveId::new("USD-TREASURY"))
            .attributes(Attributes::new())
            .build()
            .expect("Valid short future");

        assert_eq!(future.terms.position, Position::Short);
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
            .terms(
                ListedFutureTerms::new(
                    10.0,
                    1_000.0,
                    Currency::USD,
                    125.50,
                    Date::from_calendar_date(2025, Month::March, 25).expect("Valid date"),
                    Date::from_calendar_date(2025, Month::March, 31).expect("Valid date"),
                    Position::Long,
                )
                .expect("terms"),
            )
            .delivery_start(Date::from_calendar_date(2025, Month::March, 21).expect("Valid date"))
            .contract_specs(BondFutureSpecs::ust_10y())
            .deliverable_basket(vec![deliverable])
            .ctd_bond_id(InstrumentId::new("US912828XG33"))
            .discount_curve_id(CurveId::new("USD-TREASURY"))
            .attributes(Attributes::new())
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_cashflow_provider_rejects_physical_delivery_schedule() {
        let ctd_bond_id = InstrumentId::new("US912828XG33");
        let ctd_bond = Bond::fixed(
            ctd_bond_id.as_str(),
            Money::from((100_000_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
            Date::from_calendar_date(2020, Month::January, 15).expect("valid date"),
            Date::from_calendar_date(2030, Month::January, 15).expect("valid date"),
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("valid bond");
        let future = BondFuture::builder()
            .id(InstrumentId::new("TYH5"))
            .terms(
                ListedFutureTerms::new(
                    1.0,
                    1_000.0,
                    Currency::USD,
                    125.50,
                    Date::from_calendar_date(2025, Month::March, 20).expect("valid date"),
                    Date::from_calendar_date(2025, Month::March, 31).expect("valid date"),
                    Position::Long,
                )
                .expect("terms"),
            )
            .delivery_start(Date::from_calendar_date(2025, Month::March, 21).expect("valid date"))
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
            .dated_cashflows(&MarketContext::new(), future.terms.last_trading_date)
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
            // The pricer resolves the financing curve via
            // `MarketContext::get_discount`, so the repo curve is a
            // discount-curve dependency (not a forward curve).
            deps.add_discount_curve(repo_curve_id.clone());
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
        let (ctd_bond, conversion_factor) = self.embedded_ctd()?;
        super::pricer::BondFuturePricer::calculate_npv(
            self,
            ctd_bond,
            conversion_factor,
            market,
            as_of,
        )
    }

    fn expiry(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.terms.settlement_date)
    }

    crate::instruments::common_impl::traits::impl_focused_pricing_overrides!();
}

// Declare canonical market dependencies for DV01 calculators.
impl finstack_quant_cashflows::CashflowScheduleSource for BondFuture {
    /// Face exposure `terms.contracts × terms.multiplier × 100` in `terms.currency`.
    fn notional(&self) -> finstack_quant_core::Result<Option<Money>> {
        Ok(Some(Money::new(
            self.terms.contracts * self.terms.multiplier * 100.0,
            self.terms.currency,
        )?))
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
    use crate::instruments::Position;
    use finstack_quant_core::currency::Currency;
    use time::Month;

    #[test]
    fn test_instrument_trait_as_any_downcast() {
        let deliverable = DeliverableBond {
            bond_id: InstrumentId::new("US912828XG33"),
            conversion_factor: 0.8234,
        };

        let future = BondFuture::builder()
            .id(InstrumentId::new("TYH5"))
            .terms(
                ListedFutureTerms::new(
                    10.0,
                    1_000.0,
                    Currency::USD,
                    125.50,
                    Date::from_calendar_date(2025, Month::March, 20).expect("Valid date"),
                    Date::from_calendar_date(2025, Month::March, 31).expect("Valid date"),
                    Position::Long,
                )
                .expect("terms"),
            )
            .delivery_start(Date::from_calendar_date(2025, Month::March, 21).expect("Valid date"))
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
            .terms(
                ListedFutureTerms::new(
                    10.0,
                    1_000.0,
                    Currency::USD,
                    125.50,
                    Date::from_calendar_date(2025, Month::March, 20).expect("Valid date"),
                    Date::from_calendar_date(2025, Month::March, 31).expect("Valid date"),
                    Position::Long,
                )
                .expect("terms"),
            )
            .delivery_start(Date::from_calendar_date(2025, Month::March, 21).expect("Valid date"))
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

        let mut future = BondFuture::builder()
            .id(InstrumentId::new("TYH5"))
            .terms(
                ListedFutureTerms::new(
                    10.0,
                    1_000.0,
                    Currency::USD,
                    125.50,
                    Date::from_calendar_date(2025, Month::March, 20).expect("Valid date"),
                    Date::from_calendar_date(2025, Month::March, 31).expect("Valid date"),
                    Position::Long,
                )
                .expect("terms"),
            )
            .delivery_start(Date::from_calendar_date(2025, Month::March, 21).expect("Valid date"))
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

        // A configured repo curve is consumed via `get_discount`, so it must
        // be declared as a discount-curve dependency.
        future.repo_curve_id = Some(CurveId::new("USD-SPECIAL-REPO"));
        let curves = future
            .market_dependencies()
            .expect("market_dependencies with repo curve")
            .curves;
        assert_eq!(curves.discount_curves.len(), 2);
        assert_eq!(curves.discount_curves[1].as_str(), "USD-SPECIAL-REPO");
        assert_eq!(curves.forward_curves.len(), 0);
    }
}
