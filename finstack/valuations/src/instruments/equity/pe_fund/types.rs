//! Private markets fund investment instrument type and implementations.

use super::pricer;
use crate::cashflow::traits::CashflowProvider;
use crate::impl_instrument_base;
use crate::instruments::common_impl::traits::{Attributes, Instrument};
use crate::instruments::equity::pe_fund::waterfall::{AllocationLedger, FundEvent, WaterfallSpec};
use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;
use finstack_core::types::{CurveId, InstrumentId};
use time::macros::date;

/// Private markets fund investment instrument.
///
/// Models a private equity, private credit, or alternative fund with a
/// cashflow waterfall that determines LP/GP allocation. Supports NAV
/// discounting when a `discount_curve_id` is provided, or falls back to
/// last-event date for IRR-only workflows.
#[derive(
    Clone,
    Debug,
    finstack_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct PrivateMarketsFund {
    /// Unique instrument identifier.
    pub id: InstrumentId,
    /// Functional currency of the fund.
    pub currency: Currency,
    /// Waterfall specification defining LP/GP allocation tiers.
    pub waterfall_spec: WaterfallSpec,
    /// Time-ordered list of fund events (contributions, proceeds, distributions).
    pub events: Vec<FundEvent>,
    /// Discount curve identifier for NAV present-value calculations.
    ///
    /// When `None`, the pricer falls back to the last event date as the
    /// valuation date and returns an undiscounted waterfall NAV.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discount_curve_id: Option<CurveId>,
    /// Unrealized net asset value attributable to the LP, stated as of the
    /// fund's valuation date.
    ///
    /// Holder-view residual value (review finding M13): the fund's present
    /// value is the PV of LP cashflows strictly after the valuation date plus
    /// this NAV. When `None`, the residual value is zero — a fully realized
    /// fund prices to ~0.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unrealized_nav: Option<Money>,
    /// Pricing overrides for scenario analysis and model configuration.
    #[serde(default)]
    #[builder(default)]
    pub pricing_overrides: crate::instruments::PricingOverrides,
    /// Attributes for scenario selection and tagging.
    #[serde(default)]
    #[builder(default)]
    pub attributes: Attributes,
}

impl PrivateMarketsFund {
    /// Create a new private markets fund instrument.
    pub fn new(
        id: impl Into<InstrumentId>,
        currency: Currency,
        waterfall_spec: WaterfallSpec,
        events: Vec<FundEvent>,
    ) -> Self {
        Self {
            id: id.into(),
            currency,
            waterfall_spec,
            events,
            discount_curve_id: None,
            unrealized_nav: None,
            pricing_overrides: crate::instruments::PricingOverrides::default(),
            attributes: Attributes::new(),
        }
    }

    /// Create a canonical example private markets fund with a simple waterfall and events.
    pub fn example() -> finstack_core::Result<Self> {
        use super::waterfall::{WaterfallSpec, WaterfallStyle};
        use finstack_core::currency::Currency;
        // Build a simple European-style waterfall: Return of capital -> 8% pref -> 50% catchup -> 80/20 promote
        let spec = WaterfallSpec::builder()
            .style(WaterfallStyle::European)
            .return_of_capital()
            .preferred_irr(0.08)
            .catchup(0.5)
            .promote_tier(0.12, 0.8, 0.2)
            .build()?;
        // Define a few cashflow events: contributions in year 1, proceeds in year 3, distribution in year 4
        let events = vec![
            super::waterfall::FundEvent::contribution(
                date!(2024 - 01 - 15),
                Money::new(5_000_000.0, Currency::USD),
            ),
            super::waterfall::FundEvent::contribution(
                date!(2024 - 06 - 15),
                Money::new(2_000_000.0, Currency::USD),
            ),
            super::waterfall::FundEvent::proceeds(
                date!(2026 - 03 - 01),
                Money::new(4_000_000.0, Currency::USD),
                "DEAL-1",
            ),
            super::waterfall::FundEvent::distribution(
                date!(2027 - 01 - 01),
                Money::new(4_000_000.0, Currency::USD),
            ),
        ];
        Ok(PrivateMarketsFund::new(
            InstrumentId::new("PMF-EXAMPLE"),
            Currency::USD,
            spec,
            events,
        )
        .with_discount_curve("USD-OIS"))
    }
    /// Set the discount curve for NAV present-value calculations.
    pub fn with_discount_curve(mut self, discount_curve_id: impl Into<CurveId>) -> Self {
        self.discount_curve_id = Some(discount_curve_id.into());
        self
    }

    /// Set the unrealized NAV attributable to the LP as of the valuation date.
    pub fn with_unrealized_nav(mut self, unrealized_nav: Money) -> Self {
        self.unrealized_nav = Some(unrealized_nav);
        self
    }

    /// Run the waterfall allocation engine on all fund events.
    pub fn run_waterfall(&self) -> finstack_core::Result<AllocationLedger> {
        pricer::run_waterfall(self)
    }

    /// Compute LP cashflows from running the waterfall.
    pub(crate) fn lp_cashflows(&self) -> finstack_core::Result<Vec<(Date, Money)>> {
        pricer::lp_cashflows(self)
    }
}

// Attributable is provided via blanket impl for all Instrument types

impl Instrument for PrivateMarketsFund {
    impl_instrument_base!(crate::pricer::InstrumentType::PrivateMarketsFund);

    // === Pricing Methods ===

    fn base_value(&self, curves: &MarketContext, as_of: Date) -> finstack_core::Result<Money> {
        pricer::compute_pv(self, curves, as_of)
    }

    fn resolve_pricing_as_of(&self, market: &MarketContext, requested: Date) -> Date {
        pricer::resolve_as_of(self, market, requested)
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

impl CashflowProvider for PrivateMarketsFund {
    fn cashflow_schedule(
        &self,
        _curves: &MarketContext,
        as_of: Date,
    ) -> finstack_core::Result<crate::cashflow::builder::CashFlowSchedule> {
        let flows = self.lp_cashflows()?;
        let schedule = crate::cashflow::traits::schedule_from_dated_flows(
            flows,
            finstack_core::dates::DayCount::Act365F,
            crate::cashflow::traits::ScheduleBuildOpts {
                kind: Some(crate::cashflow::primitives::CFKind::Fixed),
                ..Default::default()
            },
        );
        Ok(schedule.normalize_public(
            as_of,
            crate::cashflow::builder::CashflowRepresentation::Contractual,
        ))
    }
}
