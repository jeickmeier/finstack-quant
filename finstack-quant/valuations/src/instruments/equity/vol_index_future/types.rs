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
//! NPV = (Mark - Entry_Price) × Multiplier × Contracts × Position_Sign
//! ```
//! where:
//! - Entry_Price = `terms.entry_price`, the traded price of the position
//! - Mark = `terms.quoted_price` when supplied, otherwise the fair level
//!   ([`VolatilityIndexFuture::fair_price`]) interpolated from the vol index
//!   curve; the SOQ `terms.settlement_price` on the settlement date
//! - Multiplier = Contract multiplier (typically 1000 for VIX)
//! - Position_Sign = +1 for long, -1 for short
//!
//! This is standard futures mark-to-market: a long gains when the forward mark
//! rises above its entry price (matching [`EquityFuture`]). The MTM is
//! undiscounted because the position is daily margined.
//!
//! [`EquityFuture`]: crate::instruments::equity::EquityFuture
//!
//! No convexity adjustment is applied. This is exact only when the vol index
//! curve is built directly from quoted futures/forward vol levels (the curve
//! IS the futures strip). If the curve were instead derived from
//! variance-swap or option-implied *variance* levels, a futures-vs-forward
//! convexity (concavity in variance) adjustment would be required.
//!
//! # References
//!
//! - CBOE (2019). "VIX Futures Contract Specifications." `docs/REFERENCES.md#cboe-vix-white-paper`
//! - Whaley, R. E. (2009). "Understanding the VIX." *Journal of Portfolio Management*. `docs/REFERENCES.md#whaley-2009-vix`

use super::pricer;
use crate::impl_instrument_base;
use crate::instruments::common_impl::listed::ListedFutureTerms;
use crate::instruments::common_impl::traits::Attributes;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};

/// Volatility Index Future instrument.
///
/// Represents a futures contract on a volatility index such as VIX, VXN,
/// or VSTOXX. These contracts provide exposure to expected future volatility.
/// Position size, multiplier, entry price and lifecycle dates live in the
/// shared [`ListedFutureTerms`]; `terms.settlement_price` is the official
/// Special Opening Quotation (SOQ) in index points.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations::instruments::equity::vol_index_future::VolatilityIndexFuture;
/// use finstack_quant_valuations::instruments::{ListedFutureTerms, Position};
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::dates::Date;
/// use finstack_quant_core::types::{CurveId, InstrumentId};
/// use time::Month;
///
/// let settlement = Date::from_calendar_date(2025, Month::March, 19).unwrap();
/// let future = VolatilityIndexFuture::builder()
///     .id(InstrumentId::new("VIX-FUT-2025M03"))
///     .terms(
///         ListedFutureTerms::new(
///             5.0,
///             1_000.0,
///             Currency::USD,
///             21.50,
///             settlement,
///             settlement,
///             Position::Long,
///         )
///         .unwrap(),
///     )
///     .discount_curve_id(CurveId::new("USD-OIS"))
///     .vol_index_curve_id(CurveId::new("VIX"))
///     .build()
///     .expect("Valid future");
/// ```
#[derive(
    Clone,
    Debug,
    PartialEq,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VolatilityIndexFuture {
    /// Unique identifier.
    pub id: InstrumentId,
    /// Standard listed position and lifecycle terms. `terms.multiplier` is the
    /// settlement-currency value of one index point ($1,000 for CBOE VIX),
    /// `terms.entry_price` is the trade price in index points and
    /// `terms.settlement_date` is the SOQ date on which the final settlement
    /// price is fixed.
    pub terms: ListedFutureTerms,
    /// Discount curve identifier. **Unused in PV**: the future is daily
    /// margined so the mark-to-market is undiscounted, and no Dv01 is
    /// registered. Retained for market-data identification/scenario plumbing.
    pub discount_curve_id: CurveId,
    /// Volatility index forward curve identifier.
    pub vol_index_curve_id: CurveId,
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
    #[builder(default)]
    #[serde(default)]
    pub attributes: Attributes,
}

impl VolatilityIndexFuture {
    /// Create a canonical example VIX future for testing and documentation.
    pub fn example() -> finstack_quant_core::Result<Self> {
        use crate::instruments::Position;
        use time::macros::date;

        Self::builder()
            .id(InstrumentId::new("VIX-FUT-2025M03"))
            .terms(ListedFutureTerms::new(
                4.651162790697675,
                1_000.0,
                Currency::USD,
                21.50,
                date!(2025 - 03 - 19),
                date!(2025 - 03 - 19),
                Position::Long,
            )?)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .vol_index_curve_id(CurveId::new("VIX"))
            .attributes(Attributes::new())
            .build()
    }

    /// Validate listed terms and the positive-price domain of an index level.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        self.terms.validate()?;
        for (name, value) in [
            ("entry_price", Some(self.terms.entry_price)),
            ("quoted_price", self.terms.quoted_price),
            ("settlement_price", self.terms.settlement_price),
        ] {
            if value.is_some_and(|price| !price.is_finite() || price <= 0.0) {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "VolatilityIndexFuture '{}' terms.{name} must be positive and finite",
                    self.id
                )));
            }
        }
        Ok(())
    }

    /// Calculate the raw present value as f64.
    ///
    /// # Arguments
    ///
    /// * `market` - Market context containing the volatility index curve.
    /// * `as_of` - Valuation date controlling live versus final-settlement state.
    pub fn npv_raw(&self, market: &MarketContext, as_of: Date) -> finstack_quant_core::Result<f64> {
        pricer::compute_pv_raw(self, market, as_of)
    }

    /// Model futures level in index points: the volatility index curve read at
    /// `terms.settlement_date`.
    ///
    /// # Arguments
    ///
    /// * `market` - Market context containing the volatility index curve.
    /// * `as_of` - Valuation date. The curve's own base date anchors the time
    ///   axis, so the level does not depend on `as_of`; the argument keeps the
    ///   signature of the other listed futures' `fair_price`.
    pub fn fair_price(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        pricer::fair_price(self, market, as_of)
    }

    /// Resolve the live quote, model level, or official final settlement price.
    ///
    /// On and after `terms.settlement_date` the SOQ in `terms.settlement_price`
    /// is required; before it the listed-future lifecycle rules apply.
    ///
    /// # Arguments
    ///
    /// * `market` - Market context containing the volatility index curve.
    /// * `as_of` - Valuation date controlling live versus final-settlement state.
    pub fn mark_price(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        pricer::mark_price(self, market, as_of)
    }

    /// P&L change for a 1-point increase in the vol index level
    /// (`sign × contracts × multiplier`).
    pub fn delta_vol(&self) -> finstack_quant_core::Result<f64> {
        self.terms.point_delta()
    }
}

impl crate::instruments::common_impl::traits::Instrument for VolatilityIndexFuture {
    impl_instrument_base!(crate::pricer::InstrumentType::VolatilityIndexFuture);

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<
        crate::instruments::common_impl::dependencies::MarketDependencies,
    > {
        let mut deps = crate::instruments::common_impl::dependencies::MarketDependencies::new();
        deps.add_series_id(self.vol_index_curve_id.as_str());
        Ok(deps)
    }

    fn base_value(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Money> {
        pricer::compute_pv(self, curves, as_of)
    }

    fn base_value_raw(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        pricer::compute_pv_raw(self, curves, as_of)
    }

    fn base_value_raw_with_currency(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<(f64, Currency)> {
        Ok((
            pricer::compute_pv_raw(self, curves, as_of)?,
            self.terms.currency,
        ))
    }

    fn effective_start_date(&self) -> Option<Date> {
        None
    }

    fn expiry(&self) -> Option<Date> {
        Some(self.terms.settlement_date)
    }

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        self.validate()
    }

    crate::impl_focused_pricing_overrides!();
}

impl finstack_quant_cashflows::CashflowScheduleSource for VolatilityIndexFuture {
    /// Market-value exposure at the entry price:
    /// `contracts × multiplier × entry_price` in `terms.currency`.
    fn notional(&self) -> finstack_quant_core::Result<Option<Money>> {
        Ok(Some(Money::new(
            self.terms.contracts * self.terms.multiplier * self.terms.entry_price,
            self.terms.currency,
        )?))
    }

    fn raw_cashflow_schedule(
        &self,
        _curves: &MarketContext,
        _as_of: Date,
    ) -> finstack_quant_core::Result<crate::cashflow::builder::CashFlowSchedule> {
        Ok(crate::cashflow::traits::schedule_from_classified_flows(
            Vec::new(),
            finstack_quant_core::dates::DayCount::Act365F, // Standard for vol index futures
            crate::cashflow::traits::ScheduleBuildOpts {
                notional_hint: self.notional()?,
                meta: crate::cashflow::builder::CashFlowMeta {
                    representation: crate::cashflow::builder::CashflowRepresentation::NoResidual,
                    ..Default::default()
                },
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::traits::Instrument;
    use crate::instruments::Position;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::market_data::term_structures::{PriceCurve, PriceCurveKind};
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
        let vix = PriceCurve::builder("VIX")
            .kind(PriceCurveKind::VolIndex)
            .base_date(base_date)
            .spot_price(18.0)
            .knots([(0.0, 18.0), (0.25, 20.0), (0.5, 21.0), (1.0, 22.0)])
            .build()
            .expect("valid VIX curve");

        MarketContext::new().insert(disc).insert(vix)
    }

    /// One VIX contract ($1000/point) settling on 2025-04-01.
    fn listed_terms(entry_price: f64, position: Position) -> ListedFutureTerms {
        let settlement = Date::from_calendar_date(2025, Month::April, 1).expect("valid date");
        ListedFutureTerms::new(
            1.0,
            1_000.0,
            Currency::USD,
            entry_price,
            settlement,
            settlement,
            position,
        )
        .expect("terms")
    }

    #[test]
    fn test_at_market_future() {
        let market = setup_market();
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

        // Create a future at the forward price (should have zero NPV)
        let future = VolatilityIndexFuture::builder()
            .id(InstrumentId::new("VIX-ATM"))
            .terms(listed_terms(20.0, Position::Long))
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
            .terms(listed_terms(22.0, Position::Long))
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

        // Short position with entry price above forward
        let future = VolatilityIndexFuture::builder()
            .id(InstrumentId::new("VIX-SHORT"))
            .terms(listed_terms(22.0, Position::Short))
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
            .terms(listed_terms(20.0, Position::Long))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .vol_index_curve_id(CurveId::new("VIX"))
            .build()
            .expect("valid future");

        let delta = future.delta_vol().expect("delta");
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
        assert_eq!(future, recovered);
    }
}
