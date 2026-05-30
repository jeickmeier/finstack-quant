//! Volatility Index Future types and implementation.
//!
//! Defines the `VolatilityIndexFuture` instrument for VIX, VXN, VSTOXX, and
//! similar volatility index futures. These contracts allow market participants
//! to gain exposure to expected future volatility levels.
//!
//! # Contract Specifications
//!
//! VIX futures are traded on CBOE with the following standard specs:
//! - Multiplier: $1,000 per index point
//! - Tick size: 0.05 index points ($50 per tick)
//! - Settlement: Cash-settled to SOQ (Special Opening Quotation)
//!
//! # Pricing
//!
//! The present value of a volatility index future is:
//! ```text
//! NPV = (Forward_Vol - Quoted_Price) × Multiplier × Contracts × Position_Sign
//! ```
//! where:
//! - Quoted_Price = Entry/traded price of the future position
//! - Forward_Vol = Today's fair forward level (mark) interpolated from the vol index curve
//! - Multiplier = Contract multiplier (typically 1000 for VIX)
//! - Position_Sign = +1 for long, -1 for short
//!
//! This is standard futures mark-to-market: a long gains when the forward mark
//! rises above its entry price (matching [`EquityIndexFuture`]).
//!
//! [`EquityIndexFuture`]: crate::instruments::equity::EquityIndexFuture
//!
//! Unlike interest rate futures, VIX futures do not require convexity
//! adjustments because the underlying is already a volatility measure.
//!
//! # References
//!
//! - CBOE (2019). "VIX Futures Contract Specifications."
//! - Whaley, R. E. (2009). "Understanding the VIX." *Journal of Portfolio Management*.

use super::pricer;
use crate::cashflow::traits::CashflowProvider;
use crate::contract_specs::{embedded_registry, ContractSpecRegistry};
use crate::impl_instrument_base;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::rates::ir_future::Position;
use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;
use finstack_core::types::{CurveId, InstrumentId};
use time::macros::date;

/// Volatility Index Future instrument.
///
/// Represents a futures contract on a volatility index such as VIX, VXN,
/// or VSTOXX. These contracts provide exposure to expected future volatility.
///
/// # Examples
///
/// ```rust
/// use finstack_valuations::instruments::equity::vol_index_future::{
///     VolatilityIndexFuture, VolIndexContractSpecs,
/// };
/// use finstack_valuations::instruments::rates::ir_future::Position;
/// use finstack_core::currency::Currency;
/// use finstack_core::dates::Date;
/// use finstack_core::money::Money;
/// use finstack_core::types::{CurveId, InstrumentId};
/// use time::Month;
///
/// let future = VolatilityIndexFuture::builder()
///     .id(InstrumentId::new("VIX-FUT-2025M03"))
///     .notional(Money::new(100_000.0, Currency::USD))
///     .expiry(Date::from_calendar_date(2025, Month::March, 19).unwrap())
///     .settlement_date(Date::from_calendar_date(2025, Month::March, 19).unwrap())
///     .quoted_price(21.50)
///     .position(Position::Long)
///     .contract_specs(VolIndexContractSpecs::default())
///     .discount_curve_id(CurveId::new("USD-OIS"))
///     .vol_index_curve_id(CurveId::new("VIX"))
///     .build()
///     .expect("Valid future");
/// ```
#[derive(
    Clone,
    Debug,
    finstack_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct VolatilityIndexFuture {
    /// Unique identifier.
    pub id: InstrumentId,
    /// Notional exposure in currency units. PV is scaled by
    /// `notional.amount() / (multiplier × quoted_price)` to represent
    /// the number of contracts.
    pub notional: Money,
    /// Future expiry date (typically 30 days before VIX settlement).
    #[schemars(with = "String")]
    pub expiry: Date,
    /// Settlement date (SOQ calculation date).
    #[schemars(with = "String")]
    pub settlement_date: Date,
    /// Quoted future price (index points, e.g., 21.50).
    pub quoted_price: f64,
    /// Position side (Long or Short).
    pub position: Position,
    /// Contract specifications.
    #[builder(default)]
    #[serde(default)]
    pub contract_specs: VolIndexContractSpecs,
    /// Discount curve identifier for present value calculations.
    pub discount_curve_id: CurveId,
    /// Volatility index forward curve identifier.
    pub vol_index_curve_id: CurveId,
    /// Attributes for tagging and selection.
    #[builder(default)]
    #[serde(default)]
    pub pricing_overrides: crate::instruments::PricingOverrides,
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,
}

/// Contract specifications for volatility index futures.
///
/// VIX futures have standardized specifications set by CBOE:
/// - Standard multiplier: $1,000 per index point
/// - Minimum tick: 0.05 index points ($50)
/// - Weekly and monthly expiries available
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct VolIndexContractSpecs {
    /// Contract multiplier (USD per index point).
    /// VIX standard: 1000 (each point = $1,000)
    pub multiplier: f64,
    /// Tick size in index points.
    /// VIX standard: 0.05 points
    pub tick_size: f64,
    /// Tick value in currency units.
    /// VIX standard: $50 per tick (0.05 × 1000)
    pub tick_value: f64,
    /// Index identifier (e.g., "VIX", "VXN", "VSTOXX").
    pub index_id: String,
}

impl Default for VolIndexContractSpecs {
    fn default() -> Self {
        Self::vix()
    }
}

#[allow(clippy::expect_used)]
fn contract_spec_registry() -> &'static ContractSpecRegistry {
    embedded_registry().expect("embedded contract-spec registry should load")
}

#[allow(clippy::expect_used)]
fn vol_index_future_specs_from_registry(id: &str) -> VolIndexContractSpecs {
    contract_spec_registry()
        .vol_index_future_specs(id)
        .expect("embedded volatility index future contract spec should exist")
}

impl VolIndexContractSpecs {
    /// Create specs for standard VIX futures.
    pub fn vix() -> Self {
        vol_index_future_specs_from_registry("cboe.vix_future")
    }

    /// Create specs for Mini VIX futures.
    pub fn mini_vix() -> Self {
        vol_index_future_specs_from_registry("cboe.mini_vix_future")
    }

    /// Create specs for VSTOXX futures.
    pub fn vstoxx() -> Self {
        vol_index_future_specs_from_registry("eurex.vstoxx_future")
    }
}

impl VolatilityIndexFuture {
    /// Create a canonical example VIX future for testing and documentation.
    pub fn example() -> finstack_core::Result<Self> {
        // SAFETY: All inputs are compile-time validated constants
        Self::builder()
            .id(InstrumentId::new("VIX-FUT-2025M03"))
            .notional(Money::new(100_000.0, Currency::USD))
            .expiry(date!(2025 - 03 - 19))
            .settlement_date(date!(2025 - 03 - 19))
            .quoted_price(21.50)
            .position(Position::Long)
            .contract_specs(VolIndexContractSpecs::vix())
            .discount_curve_id(CurveId::new("USD-OIS"))
            .vol_index_curve_id(CurveId::new("VIX"))
            .attributes(Attributes::new())
            .build()
    }

    /// Calculate the number of contracts based on notional and quoted price.
    ///
    /// # Formula
    /// ```text
    /// contracts = notional / (multiplier × quoted_price)
    /// ```
    pub fn num_contracts(&self) -> f64 {
        let contract_value = self.contract_specs.multiplier * self.quoted_price;
        if contract_value > 0.0 {
            self.notional.amount() / contract_value
        } else {
            0.0
        }
    }

    /// Calculate the raw present value as f64.
    pub fn npv_raw(&self, context: &MarketContext) -> finstack_core::Result<f64> {
        pricer::compute_pv_raw(self, context)
    }

    /// Get the forward volatility level at settlement.
    pub fn forward_vol(&self, context: &MarketContext) -> finstack_core::Result<f64> {
        pricer::forward_vol(self, context)
    }

    /// Calculate DV01 (delta with respect to vol index level).
    ///
    /// Returns the P&L change for a 1-point increase in the vol index level.
    pub fn delta_vol(&self) -> f64 {
        pricer::delta_vol(self)
    }
}

// =============================================================================
// Trait Implementations
// =============================================================================

impl crate::instruments::common_impl::traits::Instrument for VolatilityIndexFuture {
    impl_instrument_base!(crate::pricer::InstrumentType::VolatilityIndexFuture);

    fn base_value(&self, curves: &MarketContext, _as_of: Date) -> finstack_core::Result<Money> {
        pricer::compute_pv(self, curves)
    }

    fn value_raw(&self, curves: &MarketContext, _as_of: Date) -> finstack_core::Result<f64> {
        pricer::compute_pv_raw(self, curves)
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

impl CashflowProvider for VolatilityIndexFuture {
    fn notional(&self) -> Option<Money> {
        Some(self.notional)
    }

    fn cashflow_schedule(
        &self,
        _curves: &MarketContext,
        _as_of: Date,
    ) -> finstack_core::Result<crate::cashflow::builder::CashFlowSchedule> {
        Ok(crate::cashflow::traits::schedule_from_classified_flows(
            Vec::new(),
            finstack_core::dates::DayCount::Act365F, // Standard for vol index futures
            crate::cashflow::traits::ScheduleBuildOpts {
                notional_hint: self.notional(),
                representation: crate::cashflow::builder::CashflowRepresentation::NoResidual,
                ..Default::default()
            },
        ))
    }
}

impl crate::instruments::common_impl::traits::CurveDependencies for VolatilityIndexFuture {
    fn curve_dependencies(
        &self,
    ) -> finstack_core::Result<crate::instruments::common_impl::traits::InstrumentCurves> {
        // Only include discount curve for DV01 calculations
        // Vol index curve sensitivity is handled separately via delta_vol
        crate::instruments::common_impl::traits::InstrumentCurves::builder()
            .discount(self.discount_curve_id.clone())
            .build()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::traits::Instrument;
    use finstack_core::market_data::term_structures::DiscountCurve;
    use finstack_core::market_data::term_structures::VolatilityIndexCurve;
    use time::Month;

    fn setup_market() -> MarketContext {
        let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

        // Create discount curve
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .knots([(0.0, 1.0), (1.0, 0.96)])
            .build()
            .expect("valid discount curve");

        // Create VIX forward curve - contango structure
        let vix = VolatilityIndexCurve::builder("VIX")
            .base_date(base_date)
            .spot_level(18.0)
            .knots([(0.0, 18.0), (0.25, 20.0), (0.5, 21.0), (1.0, 22.0)])
            .build()
            .expect("valid VIX curve");

        MarketContext::new().insert(disc).insert(vix)
    }

    #[test]
    fn test_at_market_future() {
        let market = setup_market();
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

        // Create a future at the forward price (should have zero NPV)
        let future = VolatilityIndexFuture::builder()
            .id(InstrumentId::new("VIX-ATM"))
            .notional(Money::new(20_000.0, Currency::USD)) // 1 contract at 20
            .expiry(Date::from_calendar_date(2025, Month::April, 1).expect("valid date"))
            .settlement_date(Date::from_calendar_date(2025, Month::April, 1).expect("valid date"))
            .quoted_price(20.0) // At the 3M forward level
            .position(Position::Long)
            .contract_specs(VolIndexContractSpecs::vix())
            .discount_curve_id(CurveId::new("USD-OIS"))
            .vol_index_curve_id(CurveId::new("VIX"))
            .build()
            .expect("valid future");

        let npv = future.value(&market, as_of).expect("value calculation");
        // At forward price, NPV should be approximately zero
        assert!(
            npv.amount().abs() < 100.0,
            "At-market future should have near-zero NPV, got {}",
            npv.amount()
        );
    }

    #[test]
    fn test_long_position_above_forward_has_loss() {
        let market = setup_market();
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

        // Long position entered above today's forward mark
        let future = VolatilityIndexFuture::builder()
            .id(InstrumentId::new("VIX-LONG"))
            .notional(Money::new(22_000.0, Currency::USD)) // ~1 contract
            .expiry(Date::from_calendar_date(2025, Month::April, 1).expect("valid date"))
            .settlement_date(Date::from_calendar_date(2025, Month::April, 1).expect("valid date"))
            .quoted_price(22.0) // Entry above the ~20 forward level
            .position(Position::Long)
            .contract_specs(VolIndexContractSpecs::vix())
            .discount_curve_id(CurveId::new("USD-OIS"))
            .vol_index_curve_id(CurveId::new("VIX"))
            .build()
            .expect("valid future");

        let npv = future.value(&market, as_of).expect("value calculation");
        // Long entered at 22, forward now ~20: mark-to-market loss (bought high).
        assert!(
            npv.amount() < 0.0,
            "Long future entered above the forward should have negative NPV"
        );
    }

    #[test]
    fn test_short_position_benefits_from_low_forward() {
        let market = setup_market();
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

        // Short position with quoted price above forward
        let future = VolatilityIndexFuture::builder()
            .id(InstrumentId::new("VIX-SHORT"))
            .notional(Money::new(22_000.0, Currency::USD))
            .expiry(Date::from_calendar_date(2025, Month::April, 1).expect("valid date"))
            .settlement_date(Date::from_calendar_date(2025, Month::April, 1).expect("valid date"))
            .quoted_price(22.0)
            .position(Position::Short)
            .contract_specs(VolIndexContractSpecs::vix())
            .discount_curve_id(CurveId::new("USD-OIS"))
            .vol_index_curve_id(CurveId::new("VIX"))
            .build()
            .expect("valid future");

        let npv = future.value(&market, as_of).expect("value calculation");
        // Short entered at 22, forward now ~20: mark-to-market gain (sold high).
        assert!(
            npv.amount() > 0.0,
            "Short future entered above the forward should have positive NPV"
        );
    }

    #[test]
    fn test_delta_vol() {
        let future = VolatilityIndexFuture::builder()
            .id(InstrumentId::new("VIX-DELTA"))
            .notional(Money::new(20_000.0, Currency::USD)) // 1 contract at 20
            .expiry(Date::from_calendar_date(2025, Month::April, 1).expect("valid date"))
            .settlement_date(Date::from_calendar_date(2025, Month::April, 1).expect("valid date"))
            .quoted_price(20.0)
            .position(Position::Long)
            .contract_specs(VolIndexContractSpecs::vix())
            .discount_curve_id(CurveId::new("USD-OIS"))
            .vol_index_curve_id(CurveId::new("VIX"))
            .build()
            .expect("valid future");

        let delta = future.delta_vol();
        // Long 1 contract: delta = +1 × 1000 = +1000
        // (NPV increases by $1000 for each 1-point increase in forward vol)
        assert!(
            (delta - 1000.0).abs() < 10.0,
            "Delta should be approximately +1000, got {}",
            delta
        );
    }

    #[test]
    fn test_serde_round_trip() {
        let future =
            VolatilityIndexFuture::example().expect("VolatilityIndexFuture example is valid");
        let json = serde_json::to_string(&future).expect("json serialization");
        let recovered: VolatilityIndexFuture =
            serde_json::from_str(&json).expect("json deserialization");
        assert_eq!(future.id, recovered.id);
        assert!((future.quoted_price - recovered.quoted_price).abs() < 1e-10);
    }
}
