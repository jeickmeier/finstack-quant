//! Equity Index Future types and implementation.
//!
//! Defines the `EquityIndexFuture` instrument for equity index futures such as
//! E-mini S&P 500 (ES), E-mini Nasdaq-100 (NQ), Euro Stoxx 50 (FESX), DAX (FDAX),
//! FTSE 100 (Z), and Nikkei 225 (NK).
//!
//! # Pricing
//!
//! Two pricing modes are supported:
//!
//! 1. **Mark-to-Market** (when `quoted_price` is provided):
//!    ```text
//!    NPV = (quoted_price - entry_price) × contracts × position_sign
//!    ```
//!
//! 2. **Fair Value** (cost-of-carry model):
//!    ```text
//!    F = S₀ × exp((r - q) × T)
//!    NPV = (F - entry_price) × contracts × position_sign
//!    ```
//!
//! where:
//! - S₀ = Current spot index level
//! - r = Risk-free rate (from discount curve)
//! - q = Continuous dividend yield
//! - T = Time to expiry in years
//!
//! # References
//!
//! - Hull, J. C. (2018). "Options, Futures, and Other Derivatives." Chapter 5.
//! - CME Group. "E-mini S&P 500 Futures Contract Specifications."

use super::pricer;
use crate::cashflow::traits::CashflowProvider;
use crate::contract_specs::{embedded_registry, ContractSpecRegistry};
use crate::impl_instrument_base;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::rates::ir_future::Position;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId, PriceId};
use time::macros::date;

/// Contract specifications for equity index futures.
///
/// Contains exchange-specific contract parameters such as multiplier,
/// tick size, and settlement method.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations::instruments::equity::equity_index_future::EquityFutureSpecs;
///
/// // E-mini S&P 500 specifications
/// let es_specs = EquityFutureSpecs::sp500_emini();
/// assert_eq!(es_specs.multiplier, 50.0);
/// assert_eq!(es_specs.tick_size, 0.25);
/// assert_eq!(es_specs.tick_value, 12.50);
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct EquityFutureSpecs {
    /// Contract multiplier (USD per index point).
    /// E-mini S&P 500: 50.0 (each point = $50)
    pub multiplier: f64,
    /// Tick size in index points.
    /// E-mini S&P 500: 0.25 points
    pub tick_size: f64,
    /// Tick value in currency units.
    /// E-mini S&P 500: $12.50 per tick (0.25 × 50)
    pub tick_value: f64,
    /// Settlement method description.
    pub settlement_method: String,
}

impl Default for EquityFutureSpecs {
    fn default() -> Self {
        Self::sp500_emini()
    }
}

#[allow(clippy::expect_used)]
fn contract_spec_registry() -> &'static ContractSpecRegistry {
    embedded_registry().expect("embedded contract-spec registry should load")
}

#[allow(clippy::expect_used)]
fn equity_future_specs_from_registry(id: &str) -> EquityFutureSpecs {
    contract_spec_registry()
        .equity_index_future_specs(id)
        .expect("embedded equity index future contract spec should exist")
}

impl EquityFutureSpecs {
    /// Create specs for E-mini S&P 500 futures (ES).
    ///
    /// CME contract specifications:
    /// - Multiplier: $50 per index point
    /// - Tick size: 0.25 index points
    /// - Tick value: $12.50 per tick
    /// - Settlement: Cash settled to Special Opening Quotation (SOQ)
    pub fn sp500_emini() -> Self {
        equity_future_specs_from_registry("cme.es")
    }

    /// Create specs for E-mini Nasdaq-100 futures (NQ).
    ///
    /// CME contract specifications:
    /// - Multiplier: $20 per index point
    /// - Tick size: 0.25 index points
    /// - Tick value: $5.00 per tick
    /// - Settlement: Cash settled to Special Opening Quotation (SOQ)
    pub fn nasdaq100_emini() -> Self {
        equity_future_specs_from_registry("cme.nq")
    }

    /// Create specs for Micro E-mini S&P 500 futures (MES).
    ///
    /// CME contract specifications:
    /// - Multiplier: $5 per index point (1/10 of E-mini)
    /// - Tick size: 0.25 index points
    /// - Tick value: $1.25 per tick
    pub fn sp500_micro_emini() -> Self {
        equity_future_specs_from_registry("cme.mes")
    }

    /// Create specs for Euro Stoxx 50 futures (FESX).
    ///
    /// Eurex contract specifications:
    /// - Multiplier: €10 per index point
    /// - Tick size: 1.0 index point
    /// - Tick value: €10.00 per tick
    pub fn euro_stoxx_50() -> Self {
        equity_future_specs_from_registry("eurex.fesx")
    }

    /// Create specs for DAX futures (FDAX).
    ///
    /// Eurex contract specifications:
    /// - Multiplier: €25 per index point
    /// - Tick size: 0.5 index points
    /// - Tick value: €12.50 per tick
    pub fn dax() -> Self {
        equity_future_specs_from_registry("eurex.fdax")
    }

    /// Create specs for FTSE 100 futures (Z).
    ///
    /// ICE contract specifications:
    /// - Multiplier: £10 per index point
    /// - Tick size: 0.5 index points
    /// - Tick value: £5.00 per tick
    pub fn ftse_100() -> Self {
        equity_future_specs_from_registry("ice.ftse_100")
    }

    /// Create specs for Nikkei 225 futures (NK).
    ///
    /// CME/OSE contract specifications:
    /// - Multiplier: ¥500 per index point (CME dollar-denominated uses $5)
    /// - Tick size: 5.0 index points
    /// - Tick value: ¥2,500 per tick
    pub fn nikkei_225() -> Self {
        equity_future_specs_from_registry("ose.nikkei_225")
    }
}

/// Equity Index Future instrument.
///
/// Represents a futures contract on an equity index such as S&P 500, Nasdaq-100,
/// Euro Stoxx 50, DAX, FTSE 100, or Nikkei 225.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations::instruments::equity::equity_index_future::{
///     EquityIndexFuture, EquityFutureSpecs,
/// };
/// use finstack_quant_valuations::instruments::rates::ir_future::Position;
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::dates::Date;
/// use finstack_quant_core::money::Money;
/// use finstack_quant_core::types::{CurveId, InstrumentId};
/// use time::Month;
///
/// let future = EquityIndexFuture::builder()
///     .id(InstrumentId::new("ES-2025M03"))
///     .underlying_ticker("SPX".to_string())
///     .notional(Money::new(2_250_000.0, Currency::USD))
///     .expiry(Date::from_calendar_date(2025, Month::March, 21).expect("EquityIndexFuture example is valid"))
///     .last_trading_date(Date::from_calendar_date(2025, Month::March, 20).expect("EquityIndexFuture example is valid"))
///     .position(Position::Long)
///     .contract_specs(EquityFutureSpecs::sp500_emini())
///     .discount_curve_id(CurveId::new("USD-OIS"))
///     .spot_id("SPX-SPOT".into())
///     .build()
///     .expect("Valid future");
/// ```
#[derive(
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct EquityIndexFuture {
    /// Unique instrument identifier.
    pub id: InstrumentId,
    /// Index ticker symbol (e.g., "SPX", "NDX", "SX5E").
    pub underlying_ticker: String,
    /// Notional exposure in settlement currency.
    pub notional: Money,
    /// Future expiry/settlement date.
    #[schemars(with = "String")]
    pub expiry: Date,
    /// Last trading date (typically one day before expiry).
    #[schemars(with = "String")]
    pub last_trading_date: Date,
    /// Entry price at trade inception (optional for new trades).
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry_price: Option<f64>,
    /// Current quoted market price (if available, overrides fair value).
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quoted_price: Option<f64>,
    /// Position side (Long or Short).
    pub position: Position,
    /// Contract specifications.
    #[builder(default)]
    #[serde(default)]
    pub contract_specs: EquityFutureSpecs,
    /// Discount curve identifier for present value calculations.
    pub discount_curve_id: CurveId,
    /// Index spot price identifier for fair value calculation.
    pub spot_id: PriceId,
    /// Optional dividend yield identifier for fair value calculation.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub div_yield_id: Option<CurveId>,
    /// Optional discrete cash dividend schedule `(ex_date, amount)` for index carry.
    ///
    /// When non-empty, fair forward pricing uses PV spot adjustment and treats
    /// continuous dividend yield as zero to avoid double counting.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(with = "Vec<(String, f64)>")]
    pub discrete_dividends: Vec<(Date, f64)>,
    /// Attributes for tagging and selection.
    #[builder(default)]
    #[serde(default)]
    pub pricing_overrides: crate::instruments::PricingOverrides,
    /// Attributes for scenario selection and tagging
    #[serde(default)]
    #[builder(default)]
    pub attributes: Attributes,
}

impl EquityIndexFuture {
    /// Create a canonical example E-mini S&P 500 future for testing and documentation.
    pub fn example() -> finstack_quant_core::Result<Self> {
        Self::builder()
            .id(InstrumentId::new("ES-2025M03"))
            .underlying_ticker("SPX".to_string())
            .notional(Money::new(2_250_000.0, Currency::USD))
            .expiry(date!(2025 - 03 - 21))
            .last_trading_date(date!(2025 - 03 - 20))
            .entry_price_opt(Some(4500.0))
            .quoted_price_opt(Some(4550.0))
            .position(Position::Long)
            .contract_specs(EquityFutureSpecs::sp500_emini())
            .discount_curve_id(CurveId::new("USD-OIS"))
            .spot_id("SPX-SPOT".into())
            .attributes(Attributes::new())
            .build()
    }

    /// Create an E-mini S&P 500 future with common defaults.
    ///
    /// # Arguments
    ///
    /// * `id` - Instrument identifier (e.g., "ESH5" for March 2025)
    /// * `notional` - Notional exposure in settlement currency
    /// * `expiry_date` - Contract expiry date
    /// * `last_trading_date` - Last trading date
    /// * `entry_price` - Entry price (None for new trades)
    /// * `position` - Long or Short
    /// * `discount_curve_id` - Discount curve for PV calculations
    pub fn sp500_emini(
        id: impl Into<InstrumentId>,
        notional: Money,
        expiry_date: Date,
        last_trading_date: Date,
        entry_price: Option<f64>,
        position: Position,
        discount_curve_id: impl Into<CurveId>,
    ) -> finstack_quant_core::Result<Self> {
        Self::builder()
            .id(id.into())
            .underlying_ticker("SPX".to_string())
            .notional(notional)
            .expiry(expiry_date)
            .last_trading_date(last_trading_date)
            .entry_price_opt(entry_price)
            .position(position)
            .contract_specs(EquityFutureSpecs::sp500_emini())
            .discount_curve_id(discount_curve_id.into())
            .spot_id("SPX-SPOT".into())
            .attributes(Attributes::new())
            .build()
    }

    /// Create an E-mini Nasdaq-100 future with common defaults.
    ///
    /// # Arguments
    ///
    /// * `id` - Instrument identifier (e.g., "NQH5" for March 2025)
    /// * `notional` - Notional exposure in settlement currency
    /// * `expiry_date` - Contract expiry date
    /// * `last_trading_date` - Last trading date
    /// * `entry_price` - Entry price (None for new trades)
    /// * `position` - Long or Short
    /// * `discount_curve_id` - Discount curve for PV calculations
    pub fn nasdaq100_emini(
        id: impl Into<InstrumentId>,
        notional: Money,
        expiry_date: Date,
        last_trading_date: Date,
        entry_price: Option<f64>,
        position: Position,
        discount_curve_id: impl Into<CurveId>,
    ) -> finstack_quant_core::Result<Self> {
        Self::builder()
            .id(id.into())
            .underlying_ticker("NDX".to_string())
            .notional(notional)
            .expiry(expiry_date)
            .last_trading_date(last_trading_date)
            .entry_price_opt(entry_price)
            .position(position)
            .contract_specs(EquityFutureSpecs::nasdaq100_emini())
            .discount_curve_id(discount_curve_id.into())
            .spot_id("NDX-SPOT".into())
            .attributes(Attributes::new())
            .build()
    }

    /// Get the position sign (+1 for Long, -1 for Short).
    pub fn position_sign(&self) -> f64 {
        match self.position {
            Position::Long => 1.0,
            Position::Short => -1.0,
        }
    }

    /// Calculate the current number of futures contracts implied by notional.
    pub fn num_contracts(&self, price: f64) -> f64 {
        let contract_value = price * self.contract_specs.multiplier;
        if contract_value > 0.0 {
            self.notional.amount() / contract_value
        } else {
            0.0
        }
    }

    /// Calculate delta exposure (index point sensitivity).
    ///
    /// # Formula
    /// ```text
    /// delta = contracts × multiplier × position_sign
    /// ```
    ///
    /// This represents the USD P&L change for a 1-point move in the index.
    pub fn delta(&self) -> f64 {
        let contracts = self.entry_contracts();
        self.contract_specs.multiplier * contracts * self.position_sign()
    }

    /// Number of contracts implied by notional at entry price.
    ///
    /// The contract count is fixed at trade inception and does not change
    /// when the market price moves.
    fn entry_contracts(&self) -> f64 {
        let px = self.entry_price.unwrap_or(1.0).max(1e-12);
        self.num_contracts(px)
    }

    /// Calculate the raw present value as f64.
    pub fn npv_raw(
        &self,
        context: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        pricer::compute_pv_raw(self, context, as_of)
    }

    /// Get the fair forward price using cost-of-carry model.
    ///
    /// # Formula
    /// ```text
    /// F = S₀ × exp((r - q) × T)
    /// ```
    pub fn fair_forward(
        &self,
        context: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        pricer::fair_forward(self, context, as_of)
    }
}

// =============================================================================
// Trait Implementations
// =============================================================================

impl crate::instruments::common_impl::traits::Instrument for EquityIndexFuture {
    impl_instrument_base!(crate::pricer::InstrumentType::EquityIndexFuture);

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<
        crate::instruments::common_impl::dependencies::MarketDependencies,
    > {
        let mut deps =
            crate::instruments::common_impl::dependencies::MarketDependencies::from_curve_dependencies(
                self,
            )?;
        deps.add_spot_id(self.spot_id.as_str());
        Ok(deps)
    }

    fn base_value(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Money> {
        pricer::compute_pv(self, curves, as_of)
    }

    fn value_raw(&self, curves: &MarketContext, as_of: Date) -> finstack_quant_core::Result<f64> {
        pricer::compute_pv_raw(self, curves, as_of)
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

impl CashflowProvider for EquityIndexFuture {
    fn notional(&self) -> Option<Money> {
        Some(self.notional)
    }

    fn cashflow_schedule(
        &self,
        _curves: &MarketContext,
        _as_of: Date,
    ) -> finstack_quant_core::Result<crate::cashflow::builder::CashFlowSchedule> {
        // Futures are daily settled (mark-to-market). There are no residual flows.
        Ok(crate::cashflow::traits::schedule_from_classified_flows(
            Vec::new(),
            finstack_quant_core::dates::DayCount::Act365F, // Standard for equity futures
            crate::cashflow::traits::ScheduleBuildOpts {
                notional_hint: self.notional(),
                representation: crate::cashflow::builder::CashflowRepresentation::NoResidual,
                ..Default::default()
            },
        ))
    }
}

impl crate::instruments::common_impl::traits::CurveDependencies for EquityIndexFuture {
    fn curve_dependencies(
        &self,
    ) -> finstack_quant_core::Result<crate::instruments::common_impl::traits::InstrumentCurves>
    {
        crate::instruments::common_impl::traits::InstrumentCurves::builder()
            .discount(self.discount_curve_id.clone())
            .build()
    }
}

impl crate::instruments::common_impl::traits::EquityDependencies for EquityIndexFuture {
    fn equity_dependencies(
        &self,
    ) -> finstack_quant_core::Result<crate::instruments::common_impl::traits::EquityInstrumentDeps>
    {
        crate::instruments::common_impl::traits::EquityInstrumentDeps::builder()
            .spot(self.spot_id.as_str())
            .build()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use time::Month;

    #[test]
    fn test_equity_index_future_construction() {
        let future = EquityIndexFuture::builder()
            .id(InstrumentId::new("ES-TEST"))
            .underlying_ticker("SPX".to_string())
            .notional(Money::new(2_250_000.0, Currency::USD))
            .expiry(Date::from_calendar_date(2025, Month::March, 21).expect("valid test date"))
            .last_trading_date(
                Date::from_calendar_date(2025, Month::March, 20).expect("valid test date"),
            )
            .position(Position::Long)
            .contract_specs(EquityFutureSpecs::sp500_emini())
            .discount_curve_id(CurveId::new("USD-OIS"))
            .spot_id("SPX-SPOT".into())
            .attributes(Attributes::new())
            .build()
            .expect("should build");

        assert_eq!(future.id.as_str(), "ES-TEST");
        assert_eq!(future.underlying_ticker, "SPX");
        assert_eq!(future.notional.amount(), 2_250_000.0);
        assert_eq!(future.notional.currency(), Currency::USD);
    }

    #[test]
    fn test_equity_index_future_example() {
        let future = EquityIndexFuture::example().expect("EquityIndexFuture example is valid");
        assert_eq!(future.id.as_str(), "ES-2025M03");
        assert_eq!(future.underlying_ticker, "SPX");
        assert_eq!(future.notional.amount(), 2_250_000.0);
        assert_eq!(future.contract_specs.multiplier, 50.0);
    }

    #[test]
    fn test_delta_calculation() {
        let future = EquityIndexFuture::example().expect("EquityIndexFuture example is valid");
        // Long 10 ES contracts: delta = 50 × 10 × 1 = 500
        assert_eq!(future.delta(), 500.0);

        let mut short_future = future;
        short_future.position = Position::Short;
        // Short 10 ES contracts: delta = 50 × 10 × (-1) = -500
        assert_eq!(short_future.delta(), -500.0);
    }

    #[test]
    fn test_num_contracts() {
        let future = EquityIndexFuture::example().expect("EquityIndexFuture example is valid");
        // At price 4500: contracts = 2,250,000 / (4500 × 50) = 10
        assert_eq!(future.num_contracts(4500.0), 10.0);
    }
}
