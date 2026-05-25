//! Commodity forward types and implementations.
//!
//! Defines the `CommodityForward` instrument for physical or cash-settled
//! commodity forward contracts. Pricing uses curve-based forward interpolation
//! with optional quoted price override.

use crate::cashflow::builder::{CashFlowSchedule, Notional};
use crate::cashflow::primitives::CFKind;
use crate::cashflow::CashflowProvider;
use crate::impl_instrument_base;
use crate::instruments::common_impl::parameters::{CommodityConvention, CommodityUnderlyingParams};
use crate::instruments::common_impl::traits::Attributes;
use finstack_core::currency::Currency;
use finstack_core::dates::{BusinessDayConvention, Date};
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;
use finstack_core::types::{CurveId, InstrumentId};
use finstack_core::Result;

/// Settlement type for commodity contracts.
pub use crate::instruments::common_impl::parameters::SettlementType;

/// Position direction (long/short) for commodity contracts.
pub use crate::instruments::common_impl::parameters::Position;

/// Commodity forward or futures contract.
///
/// Represents a commitment to buy or sell a commodity at a specified future
/// date at a predetermined price. Can be physically settled (delivery) or
/// cash settled (price difference).
///
/// # Pricing
///
/// Forward value is calculated as:
/// ```text
/// NPV = sign(position) × (F - K) × Q × M × DF(T)
/// ```
/// where:
/// - sign = +1.0 for Long, -1.0 for Short
/// - F = Forward price from price curve (or quoted_price if provided)
/// - K = Contract price (entry price). If None, treated as at-market (K = F)
/// - Q = Quantity
/// - M = Contract multiplier
/// - DF(T) = Discount factor to settlement date
///
/// # At-Market vs Off-Market
///
/// - **At-market**: `contract_price = None` → NPV ≈ 0 (like entering a new futures position)
/// - **Off-market**: `contract_price = Some(K)` → NPV reflects mark-to-market vs K
///
/// # Examples
///
/// ```rust
/// use finstack_valuations::instruments::commodity::commodity_forward::{CommodityForward, Position};
/// use finstack_valuations::instruments::CommodityUnderlyingParams;
/// use finstack_core::currency::Currency;
/// use finstack_core::dates::Date;
/// use finstack_core::types::{CurveId, InstrumentId};
/// use time::Month;
///
/// // At-market long forward (NPV ≈ 0)
/// let at_market = CommodityForward::builder()
///     .id(InstrumentId::new("WTI-FWD-2025M03"))
///     .underlying(CommodityUnderlyingParams::new("Energy", "CL", "BBL", Currency::USD))
///     .quantity(1000.0)
///     .multiplier(1.0)
///     .maturity(Date::from_calendar_date(2025, Month::March, 15).unwrap())
///     .position(Position::Long)
///     .forward_curve_id(CurveId::new("WTI-FORWARD"))
///     .discount_curve_id(CurveId::new("USD-OIS"))
///     .build()
///     .expect("Valid forward");
///
/// // Off-market forward with specific contract price
/// let off_market = CommodityForward::builder()
///     .id(InstrumentId::new("WTI-FWD-2025M03-TRADE"))
///     .underlying(CommodityUnderlyingParams::new("Energy", "CL", "BBL", Currency::USD))
///     .quantity(1000.0)
///     .multiplier(1.0)
///     .maturity(Date::from_calendar_date(2025, Month::March, 15).unwrap())
///     .position(Position::Long)
///     .contract_price_opt(Some(72.0)) // Entry price
///     .forward_curve_id(CurveId::new("WTI-FORWARD"))
///     .discount_curve_id(CurveId::new("USD-OIS"))
///     .build()
///     .expect("Valid forward");
/// ```
#[derive(
    Clone,
    Debug,
    finstack_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
pub struct CommodityForward {
    /// Unique instrument identifier.
    pub id: InstrumentId,
    /// Commodity underlying parameters (commodity_type, ticker, unit, currency).
    #[serde(flatten)]
    pub underlying: CommodityUnderlyingParams,
    /// Contract quantity in units.
    pub quantity: f64,
    /// Contract multiplier (typically 1.0 for OTC forwards, defaults to 1.0).
    #[serde(default = "crate::serde_defaults::multiplier_one")]
    #[builder(default = 1.0)]
    pub multiplier: f64,
    /// Settlement/delivery date.
    #[schemars(with = "String")]
    pub maturity: Date,
    /// Settlement type (physical or cash).
    ///
    /// Defaults to cash settlement when omitted in serialized payloads.
    #[serde(default = "crate::serde_defaults::settlement_cash")]
    #[builder(default = SettlementType::Cash)]
    pub settlement: SettlementType,
    /// Position direction (long or short).
    ///
    /// - Long: buyer of the commodity at settlement
    /// - Short: seller of the commodity at settlement
    #[builder(default)]
    #[serde(default)]
    pub position: Position,
    /// Contract price (entry/trade price K).
    ///
    /// If `None`, the forward is treated as **at-market** (K = F), meaning
    /// NPV ≈ 0 at inception (like a newly opened futures position).
    ///
    /// If `Some(K)`, the forward is **off-market** and NPV reflects the
    /// mark-to-market difference: sign × (F - K) × Q × M × DF.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract_price: Option<f64>,
    /// Optional quoted forward price (overrides curve lookup for F).
    ///
    /// This is a market price override, not the contract entry price.
    /// Use `contract_price` for the trade entry price K.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quoted_price: Option<f64>,
    /// Forward/futures price curve ID for price interpolation.
    ///
    /// Should reference a `PriceCurve` in the `MarketContext`.
    pub forward_curve_id: CurveId,
    /// Optional spot price ID (for delta calculations).
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spot_id: Option<String>,
    /// Discount curve ID.
    pub discount_curve_id: CurveId,
    /// Optional exchange identifier (e.g., "NYMEX", "ICE").
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exchange: Option<String>,
    /// Optional contract month (e.g., "2025M03" for March 2025).
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract_month: Option<String>,
    /// Optional market convention for this commodity.
    ///
    /// When set, provides default settlement days and calendar if not
    /// explicitly specified. See `CommodityConvention` for available options.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub convention: Option<CommodityConvention>,
    /// Settlement lag in business days (T+N).
    ///
    /// Defaults to 2 for most commodity markets (T+2). If `convention` is set,
    /// uses the convention's default unless explicitly overridden here.
    ///
    /// # Market Standards
    ///
    /// | Market | Settlement |
    /// |--------|------------|
    /// | Energy (WTI, Brent, NG) | T+2 |
    /// | Precious metals | T+2 |
    /// | Base metals (LME) | T+2 |
    /// | Power | T+1 |
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settlement_lag_days: Option<u32>,
    /// Calendar ID for settlement date adjustments.
    ///
    /// Used for business day adjustment of the settlement date. If `convention`
    /// is set, uses the convention's calendar unless explicitly overridden.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settlement_calendar_id: Option<String>,
    /// Business day convention for settlement date adjustment.
    ///
    /// Defaults to `Following` for energy commodities, `ModifiedFollowing`
    /// for precious metals.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settlement_bdc: Option<BusinessDayConvention>,
    /// Attributes for tagging and selection.
    #[serde(default)]
    #[builder(default)]
    pub pricing_overrides: crate::instruments::PricingOverrides,
    /// Attributes for scenario selection and tagging
    #[serde(default)]
    #[builder(default)]
    pub attributes: Attributes,
}

impl CommodityForward {
    /// Create a canonical example commodity forward for testing and documentation.
    ///
    /// Returns a WTI crude oil forward with realistic parameters.
    #[allow(clippy::expect_used)] // Example uses hardcoded valid values
    pub fn example() -> Self {
        Self::builder()
            .id(InstrumentId::new("WTI-FWD-2025M03"))
            .underlying(CommodityUnderlyingParams::new(
                "Energy",
                "CL",
                "BBL",
                Currency::USD,
            ))
            .quantity(1000.0)
            .multiplier(1.0)
            .maturity(
                Date::from_calendar_date(2025, time::Month::March, 15).expect("Valid example date"),
            )
            .settlement(SettlementType::Cash)
            .position(Position::Long)
            .forward_curve_id(CurveId::new("WTI-FORWARD"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .exchange_opt(Some("NYMEX".to_string()))
            .contract_month_opt(Some("2025M03".to_string()))
            .convention_opt(Some(CommodityConvention::WTICrude)) // Use WTI convention
            .attributes(
                Attributes::new()
                    .with_tag("energy")
                    .with_meta("sector", "crude"),
            )
            .build()
            .expect("Example commodity forward construction should not fail")
    }

    /// Calculate the net present value of this commodity forward.
    ///
    /// # Formula
    ///
    /// ```text
    /// NPV = sign(position) × (F - K) × Q × M × DF(T)
    /// ```
    ///
    /// where:
    /// - sign = +1.0 for Long, -1.0 for Short
    /// - F = Market forward price from `quoted_price` or `PriceCurve`
    /// - K = Contract price (`contract_price`). If `None`, K = F (at-market)
    /// - Q = Quantity
    /// - M = Contract multiplier
    /// - DF(T) = Discount factor to settlement date
    ///
    /// # Arguments
    ///
    /// * `market` - Market context with curves and prices
    /// * `as_of` - Valuation date
    ///
    /// # Returns
    ///
    /// Present value in the instrument's currency.
    ///
    /// Get the market forward price for this contract.
    ///
    /// Uses `quoted_price` if provided, otherwise interpolates from the
    /// `PriceCurve` referenced by `forward_curve_id`.
    ///
    /// # Curve Lookup Order
    ///
    /// 1. If `quoted_price` is set, return it directly
    /// 2. Look up `PriceCurve` by `forward_curve_id`
    /// 3. If `spot_id` is set and PriceCurve not found, use cost-of-carry model
    ///
    /// # Errors
    ///
    /// Returns an error if neither `quoted_price` nor `PriceCurve` is available.
    ///
    /// # Note on PriceCurve Evaluation
    ///
    /// When using a `PriceCurve`, this method uses `price_on_date(maturity)`
    /// which respects the curve's own day count convention. This avoids hard-coding
    /// Act365F and ensures consistent time calculation across different curves.
    pub fn forward_price(&self, market: &MarketContext, as_of: Date) -> Result<f64> {
        // Use quoted price if available
        if let Some(price) = self.quoted_price {
            return Ok(price);
        }

        // At or past settlement, return spot price from curve
        if self.maturity <= as_of {
            if let Ok(price_curve) = market.get_price_curve(self.forward_curve_id.as_str()) {
                return Ok(price_curve.spot_price());
            }
        }

        // Primary path: use PriceCurve with date-based evaluation
        if let Ok(price_curve) = market.get_price_curve(self.forward_curve_id.as_str()) {
            // Use price_on_date to respect the curve's day count convention
            return price_curve.price_on_date(self.maturity);
        }

        // Fallback: if we have a spot price and discount curve, use cost-of-carry model
        // F = S × exp(r × T) where r is the implied carry rate
        if let Some(spot_id) = &self.spot_id {
            if let Ok(spot_scalar) = market.get_price(spot_id) {
                let spot = match spot_scalar {
                    finstack_core::market_data::scalars::MarketScalar::Price(m) => m.amount(),
                    finstack_core::market_data::scalars::MarketScalar::Unitless(v) => *v,
                };

                // Try to get discount curve for carry rate
                // Use the curve's own day count for consistency with zero rate lookup
                if let Ok(disc) = market.get_discount(self.discount_curve_id.as_str()) {
                    use finstack_core::dates::DayCountContext;
                    let curve_dc = disc.day_count();
                    let t = curve_dc
                        .year_fraction(as_of, self.maturity, DayCountContext::default())
                        .unwrap_or(0.0)
                        .max(0.0);
                    let rate = disc.zero(t);
                    return Ok(spot * (rate * t).exp());
                }

                // If no discount curve, return spot as approximation
                return Ok(spot);
            }
        }

        // If no PriceCurve and no spot, fail with a clear error
        Err(finstack_core::Error::Input(
            finstack_core::error::InputError::NotFound {
                id: format!(
                    "PriceCurve '{}' not found. \
                     Use MarketContext::insert() to add a commodity forward price curve.",
                    self.forward_curve_id
                ),
            },
        ))
    }

    /// Get the effective notional value at settlement.
    pub fn notional_value(&self, forward_price: f64) -> f64 {
        forward_price * self.quantity * self.multiplier
    }

    /// Check if this forward is at-market (no contract price set).
    ///
    /// At-market forwards have NPV ≈ 0 at inception.
    pub fn is_at_market(&self) -> bool {
        self.contract_price.is_none()
    }

    /// Get the effective settlement lag in business days.
    ///
    /// Resolution order:
    /// 1. `settlement_lag_days` if explicitly set
    /// 2. `convention.settlement_days()` if convention is set
    /// 3. Default: 2 (T+2, standard for most commodities)
    pub fn effective_settlement_lag(&self) -> u32 {
        self.settlement_lag_days
            .or_else(|| self.convention.map(|c| c.settlement_days()))
            .unwrap_or(2)
    }

    /// Get the effective settlement calendar ID.
    ///
    /// Resolution order:
    /// 1. `settlement_calendar_id` if explicitly set
    /// 2. `convention.calendar_id()` if convention is set
    /// 3. `None` (no calendar adjustment)
    pub fn effective_settlement_calendar(&self) -> Option<&str> {
        self.settlement_calendar_id
            .as_deref()
            .or_else(|| self.convention.map(|c| c.calendar_id()))
    }

    /// Get the effective business day convention for settlement.
    ///
    /// Resolution order:
    /// 1. `settlement_bdc` if explicitly set
    /// 2. `convention.business_day_convention()` if convention is set
    /// 3. Default: `Following`
    pub fn effective_settlement_bdc(&self) -> BusinessDayConvention {
        self.settlement_bdc
            .or_else(|| self.convention.map(|c| c.business_day_convention()))
            .unwrap_or(BusinessDayConvention::Following)
    }

    fn contractual_invoice_amount(&self, market: &MarketContext, as_of: Date) -> Result<Money> {
        let contract_price = self
            .contract_price
            .unwrap_or(self.forward_price(market, as_of)?);
        let amount = -self.position.sign() * contract_price * self.quantity * self.multiplier;
        Ok(Money::new(amount, self.underlying.currency))
    }
}

impl crate::instruments::common_impl::traits::CurveDependencies for CommodityForward {
    fn curve_dependencies(
        &self,
    ) -> finstack_core::Result<crate::instruments::common_impl::traits::InstrumentCurves> {
        crate::instruments::common_impl::traits::InstrumentCurves::builder()
            .discount(self.discount_curve_id.clone())
            .forward(self.forward_curve_id.clone())
            .build()
    }
}

impl crate::instruments::common_impl::traits::EquityDependencies for CommodityForward {
    fn equity_dependencies(
        &self,
    ) -> finstack_core::Result<crate::instruments::common_impl::traits::EquityInstrumentDeps> {
        Ok(
            crate::instruments::common_impl::traits::EquityInstrumentDeps {
                spot_id: self.spot_id.clone(),
                vol_surface_id: None,
                // Forward (linear in spot, no vol surface) — strike-aware
                // vega clamp detection is irrelevant.
                reference_strike: None,
            },
        )
    }
}

impl crate::instruments::common_impl::traits::Instrument for CommodityForward {
    impl_instrument_base!(crate::pricer::InstrumentType::CommodityForward);

    fn market_dependencies(
        &self,
    ) -> finstack_core::Result<crate::instruments::common_impl::dependencies::MarketDependencies>
    {
        let mut deps =
            crate::instruments::common_impl::dependencies::MarketDependencies::from_curve_dependencies(
                self,
            )?;
        if let Some(spot_id) = self.spot_id.as_deref() {
            deps.add_spot_id(spot_id);
        }
        Ok(deps)
    }

    fn base_value(
        &self,
        market: &finstack_core::market_data::context::MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> finstack_core::Result<finstack_core::money::Money> {
        // If settlement has passed, value is zero
        if self.maturity < as_of {
            return Ok(finstack_core::money::Money::new(
                0.0,
                self.underlying.currency,
            ));
        }

        // Get market forward price F from quoted price or curve
        let forward_price = self.forward_price(market, as_of)?;

        // Get contract price K (entry price). If None, treat as at-market (K = F)
        let contract_price = self.contract_price.unwrap_or(forward_price);

        // Get discount factor
        let disc = market.get_discount(self.discount_curve_id.as_str())?;
        let df = disc.df_between_dates(as_of, self.maturity)?;

        // NPV = sign(position) × (F - K) × Q × M × DF
        let price_diff = forward_price - contract_price;
        let notional_qty = self.quantity * self.multiplier;
        let pv = self.position.sign() * price_diff * notional_qty * df;

        Ok(finstack_core::money::Money::new(
            pv,
            self.underlying.currency,
        ))
    }

    fn effective_start_date(&self) -> Option<Date> {
        None
    }

    fn expiry(&self) -> Option<Date> {
        Some(self.maturity)
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

impl CashflowProvider for CommodityForward {
    fn cashflow_schedule(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_core::Result<CashFlowSchedule> {
        let invoice = self.contractual_invoice_amount(market, as_of)?;
        let anchor = if as_of < self.maturity {
            as_of
        } else {
            self.maturity - time::Duration::days(1)
        };
        let mut builder = CashFlowSchedule::builder();
        let _ = builder.principal(Money::new(0.0, invoice.currency()), anchor, self.maturity);
        let _ = builder.add_principal_event(
            self.maturity,
            Money::new(0.0, invoice.currency()),
            Some(Money::new(-invoice.amount(), invoice.currency())),
            CFKind::Notional,
        );
        let mut schedule = builder.build_with_curves(None)?;
        schedule.notional = Notional::par(invoice.amount().abs(), invoice.currency());
        Ok(schedule.normalize_public(
            as_of,
            crate::cashflow::builder::CashflowRepresentation::Projected,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cashflow::CashflowProvider;
    use crate::instruments::common_impl::parameters::CommodityUnderlyingParams;
    use crate::instruments::common_impl::traits::Instrument;
    use finstack_core::market_data::term_structures::{DiscountCurve, PriceCurve};
    use time::Month;

    fn test_market(as_of: Date) -> MarketContext {
        // Create a PriceCurve for WTI forward prices
        let price_curve = PriceCurve::builder("WTI-FORWARD")
            .base_date(as_of)
            .spot_price(75.0)
            .knots([(0.0, 75.0), (0.25, 76.0), (0.5, 77.0), (1.0, 78.0)])
            .build()
            .expect("Valid price curve");

        // Create a discount curve
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (0.25, 0.99), (0.5, 0.98), (1.0, 0.96)])
            .build()
            .expect("Valid discount curve");

        MarketContext::new().insert(price_curve).insert(disc)
    }

    #[test]
    fn test_commodity_forward_creation() {
        let forward = CommodityForward::builder()
            .id(InstrumentId::new("TEST-FWD"))
            .underlying(CommodityUnderlyingParams::new(
                "Energy",
                "CL",
                "BBL",
                Currency::USD,
            ))
            .quantity(1000.0)
            .multiplier(1.0)
            .maturity(Date::from_calendar_date(2025, Month::June, 15).expect("valid date"))
            .position(Position::Long)
            .forward_curve_id(CurveId::new("CL-FWD"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .attributes(Attributes::new())
            .build()
            .expect("should build");

        assert_eq!(forward.id.as_str(), "TEST-FWD");
        assert_eq!(forward.underlying.ticker, "CL");
        assert_eq!(forward.quantity, 1000.0);
        assert_eq!(forward.underlying.currency, Currency::USD);
        assert_eq!(forward.position, Position::Long);
        assert!(forward.is_at_market());
    }

    #[test]
    fn test_commodity_forward_example() {
        let forward = CommodityForward::example();
        assert_eq!(forward.id.as_str(), "WTI-FWD-2025M03");
        assert_eq!(forward.underlying.commodity_type, "Energy");
        assert_eq!(forward.underlying.ticker, "CL");
        assert_eq!(forward.position, Position::Long);
        assert!(forward.attributes.has_tag("energy"));
    }

    #[test]
    fn test_commodity_forward_with_quoted_price() {
        let forward = CommodityForward::builder()
            .id(InstrumentId::new("GC-FWD"))
            .underlying(CommodityUnderlyingParams::new(
                "Metal",
                "GC",
                "OZ",
                Currency::USD,
            ))
            .quantity(100.0)
            .multiplier(1.0)
            .maturity(Date::from_calendar_date(2025, Month::April, 15).expect("valid date"))
            .quoted_price_opt(Some(2000.0))
            .forward_curve_id(CurveId::new("GC-FWD"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .attributes(Attributes::new())
            .build()
            .expect("should build");

        assert_eq!(forward.quoted_price, Some(2000.0));

        // When quoted price is set, it should be used directly
        let market = MarketContext::new();
        let as_of = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
        let price = forward
            .forward_price(&market, as_of)
            .expect("should get price");
        assert_eq!(price, 2000.0);
    }

    #[test]
    fn test_commodity_forward_at_market_npv_near_zero() {
        let as_of = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
        let market = test_market(as_of);

        // At-market forward (no contract_price)
        let forward = CommodityForward::builder()
            .id(InstrumentId::new("WTI-AT-MARKET"))
            .underlying(CommodityUnderlyingParams::new(
                "Energy",
                "CL",
                "BBL",
                Currency::USD,
            ))
            .quantity(1000.0)
            .multiplier(1.0)
            .maturity(Date::from_calendar_date(2025, Month::April, 15).expect("valid date"))
            .position(Position::Long)
            .forward_curve_id(CurveId::new("WTI-FORWARD"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .build()
            .expect("should build");

        let npv = forward.value(&market, as_of).expect("should price");
        // At-market: K = F, so NPV = sign × (F - F) × Q × M × DF = 0
        assert!(
            npv.amount().abs() < 1e-10,
            "At-market forward NPV should be ~0, got {}",
            npv.amount()
        );
    }

    #[test]
    fn test_commodity_forward_off_market_long() {
        let as_of = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
        let market = test_market(as_of);

        // Off-market long forward with contract price below current forward
        let forward = CommodityForward::builder()
            .id(InstrumentId::new("WTI-OFF-MARKET-LONG"))
            .underlying(CommodityUnderlyingParams::new(
                "Energy",
                "CL",
                "BBL",
                Currency::USD,
            ))
            .quantity(1000.0)
            .multiplier(1.0)
            .maturity(Date::from_calendar_date(2025, Month::April, 15).expect("valid date"))
            .position(Position::Long)
            .contract_price_opt(Some(72.0)) // Bought at $72, market is ~$76
            .forward_curve_id(CurveId::new("WTI-FORWARD"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .build()
            .expect("should build");

        let npv = forward.value(&market, as_of).expect("should price");
        // Long position, F > K: NPV should be positive
        assert!(
            npv.amount() > 0.0,
            "Long position with F > K should have positive NPV, got {}",
            npv.amount()
        );
    }

    #[test]
    fn test_commodity_forward_off_market_short() {
        let as_of = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
        let market = test_market(as_of);

        // Off-market short forward with contract price below current forward
        let forward = CommodityForward::builder()
            .id(InstrumentId::new("WTI-OFF-MARKET-SHORT"))
            .underlying(CommodityUnderlyingParams::new(
                "Energy",
                "CL",
                "BBL",
                Currency::USD,
            ))
            .quantity(1000.0)
            .multiplier(1.0)
            .maturity(Date::from_calendar_date(2025, Month::April, 15).expect("valid date"))
            .position(Position::Short)
            .contract_price_opt(Some(72.0)) // Sold at $72, market is ~$76
            .forward_curve_id(CurveId::new("WTI-FORWARD"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .build()
            .expect("should build");

        let npv = forward.value(&market, as_of).expect("should price");
        // Short position, F > K: NPV should be negative (loss on short)
        assert!(
            npv.amount() < 0.0,
            "Short position with F > K should have negative NPV, got {}",
            npv.amount()
        );
    }

    #[test]
    fn test_commodity_forward_position_sign_symmetry() {
        let as_of = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
        let market = test_market(as_of);
        let settlement = Date::from_calendar_date(2025, Month::April, 15).expect("valid date");

        let wti = CommodityUnderlyingParams::new("Energy", "CL", "BBL", Currency::USD);
        let long = CommodityForward::builder()
            .id(InstrumentId::new("LONG"))
            .underlying(wti.clone())
            .quantity(1000.0)
            .multiplier(1.0)
            .maturity(settlement)
            .position(Position::Long)
            .contract_price_opt(Some(72.0))
            .forward_curve_id(CurveId::new("WTI-FORWARD"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .build()
            .expect("should build");

        let short = CommodityForward::builder()
            .id(InstrumentId::new("SHORT"))
            .underlying(wti)
            .quantity(1000.0)
            .multiplier(1.0)
            .maturity(settlement)
            .position(Position::Short)
            .contract_price_opt(Some(72.0))
            .forward_curve_id(CurveId::new("WTI-FORWARD"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .build()
            .expect("should build");

        let long_npv = long.value(&market, as_of).expect("should price");
        let short_npv = short.value(&market, as_of).expect("should price");

        // Long + Short should net to zero (opposing positions cancel)
        let net = long_npv.amount() + short_npv.amount();
        assert!(
            net.abs() < 1e-10,
            "Long + Short should sum to zero, got {}",
            net
        );
    }

    #[test]
    fn test_commodity_forward_instrument_trait() {
        use crate::instruments::common_impl::traits::Instrument;

        let forward = CommodityForward::example();

        assert_eq!(forward.id(), "WTI-FWD-2025M03");
        assert_eq!(
            forward.key(),
            crate::pricer::InstrumentType::CommodityForward
        );
        assert!(forward.attributes().has_tag("energy"));
    }

    #[test]
    fn test_commodity_forward_curve_dependencies() {
        use crate::instruments::common_impl::traits::CurveDependencies;

        let forward = CommodityForward::example();
        let deps = forward.curve_dependencies().expect("curve_dependencies");

        assert_eq!(deps.discount_curves.len(), 1);
        assert_eq!(deps.forward_curves.len(), 1);
    }

    #[test]
    fn test_commodity_forward_convention_defaults() {
        use finstack_core::dates::BusinessDayConvention;

        // Forward with WTI convention
        let forward = CommodityForward::builder()
            .id(InstrumentId::new("WTI-CONV-TEST"))
            .underlying(CommodityUnderlyingParams::new(
                "Energy",
                "CL",
                "BBL",
                Currency::USD,
            ))
            .quantity(1000.0)
            .multiplier(1.0)
            .maturity(Date::from_calendar_date(2025, Month::June, 15).expect("valid date"))
            .position(Position::Long)
            .forward_curve_id(CurveId::new("WTI-FORWARD"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .convention_opt(Some(CommodityConvention::WTICrude))
            .build()
            .expect("should build");

        // WTI convention: T+2, Following, NYMEX calendar
        assert_eq!(forward.effective_settlement_lag(), 2);
        assert_eq!(forward.effective_settlement_calendar(), Some("nymex"));
        assert_eq!(
            forward.effective_settlement_bdc(),
            BusinessDayConvention::Following
        );

        // Gold convention: T+2, Modified Following, COMEX calendar
        let gold_forward = CommodityForward::builder()
            .id(InstrumentId::new("GOLD-CONV-TEST"))
            .underlying(CommodityUnderlyingParams::new(
                "Metal",
                "GC",
                "OZ",
                Currency::USD,
            ))
            .quantity(100.0)
            .multiplier(1.0)
            .maturity(Date::from_calendar_date(2025, Month::June, 15).expect("valid date"))
            .position(Position::Long)
            .forward_curve_id(CurveId::new("GC-FORWARD"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .convention_opt(Some(CommodityConvention::Gold))
            .build()
            .expect("should build");

        assert_eq!(gold_forward.effective_settlement_lag(), 2);
        assert_eq!(gold_forward.effective_settlement_calendar(), Some("comex"));
        assert_eq!(
            gold_forward.effective_settlement_bdc(),
            BusinessDayConvention::ModifiedFollowing
        );
    }

    #[test]
    fn test_commodity_forward_explicit_override_convention() {
        use finstack_core::dates::BusinessDayConvention;

        // Explicit settlement lag overrides convention
        let forward = CommodityForward::builder()
            .id(InstrumentId::new("OVERRIDE-TEST"))
            .underlying(CommodityUnderlyingParams::new(
                "Energy",
                "CL",
                "BBL",
                Currency::USD,
            ))
            .quantity(1000.0)
            .multiplier(1.0)
            .maturity(Date::from_calendar_date(2025, Month::June, 15).expect("valid date"))
            .position(Position::Long)
            .forward_curve_id(CurveId::new("WTI-FORWARD"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .convention_opt(Some(CommodityConvention::WTICrude)) // T+2, Following
            .settlement_lag_days_opt(Some(1)) // Override to T+1
            .settlement_bdc_opt(Some(BusinessDayConvention::ModifiedFollowing)) // Override BDC
            .build()
            .expect("should build");

        // Explicit values take precedence over convention
        assert_eq!(forward.effective_settlement_lag(), 1);
        assert_eq!(
            forward.effective_settlement_bdc(),
            BusinessDayConvention::ModifiedFollowing
        );
        // Calendar still comes from convention
        assert_eq!(forward.effective_settlement_calendar(), Some("nymex"));
    }

    #[test]
    fn test_commodity_forward_no_convention_defaults() {
        use finstack_core::dates::BusinessDayConvention;

        // No convention set - should use hardcoded defaults
        let forward = CommodityForward::builder()
            .id(InstrumentId::new("NO-CONV-TEST"))
            .underlying(CommodityUnderlyingParams::new(
                "Energy",
                "CL",
                "BBL",
                Currency::USD,
            ))
            .quantity(1000.0)
            .multiplier(1.0)
            .maturity(Date::from_calendar_date(2025, Month::June, 15).expect("valid date"))
            .position(Position::Long)
            .forward_curve_id(CurveId::new("WTI-FORWARD"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .build()
            .expect("should build");

        // Defaults: T+2, Following, no calendar
        assert_eq!(forward.effective_settlement_lag(), 2);
        assert_eq!(forward.effective_settlement_calendar(), None);
        assert_eq!(
            forward.effective_settlement_bdc(),
            BusinessDayConvention::Following
        );
    }

    #[test]
    fn test_commodity_forward_serde_roundtrip() {
        let forward = CommodityForward::example();
        let json = serde_json::to_string(&forward).expect("serialize");
        let deserialized: CommodityForward = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(forward.id.as_str(), deserialized.id.as_str());
        assert_eq!(forward.underlying.ticker, deserialized.underlying.ticker);
        assert_eq!(forward.quantity, deserialized.quantity);
        assert_eq!(forward.position, deserialized.position);
    }

    #[test]
    fn test_cashflow_provider_emits_contractual_invoice_flow() {
        let as_of = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
        let market = test_market(as_of);

        let forward = CommodityForward::builder()
            .id(InstrumentId::new("WTI-INVOICE"))
            .underlying(CommodityUnderlyingParams::new(
                "Energy",
                "CL",
                "BBL",
                Currency::USD,
            ))
            .quantity(1_000.0)
            .multiplier(1.0)
            .maturity(Date::from_calendar_date(2025, Month::April, 15).expect("valid date"))
            .position(Position::Long)
            .contract_price_opt(Some(72.0))
            .forward_curve_id(CurveId::new("WTI-FORWARD"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .build()
            .expect("should build");

        let flows = forward
            .dated_cashflows(&market, as_of)
            .expect("contractual schedule should build");

        assert_eq!(
            flows.len(),
            1,
            "commodity forward should emit one invoice flow"
        );
        assert_eq!(flows[0].0, forward.maturity);
        assert_eq!(flows[0].1.currency(), Currency::USD);
        assert!(
            (flows[0].1.amount() + 72_000.0).abs() < 1e-10,
            "expected invoice flow of -72,000 USD, got {}",
            flows[0].1.amount()
        );
    }
}
