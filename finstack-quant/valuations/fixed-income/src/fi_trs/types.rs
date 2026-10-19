//! Fixed-income index total-return swap contract.

use finstack_quant_cashflows::builder::ScheduleParams;
use finstack_quant_core::{
    currency::Currency,
    dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor},
    market_data::context::MarketContext,
    money::Money,
    types::InstrumentId,
    Result,
};
use finstack_quant_margin::types::OtcMarginSpec;
use finstack_quant_valuations_core::impl_instrument_base;
use finstack_quant_valuations_core::instruments::Attributes;
use finstack_quant_valuations_core::instruments::{
    FinancingLegSpec, IndexUnderlyingParams, PayReceive, TrsScheduleSpec,
};
use rust_decimal::Decimal;

/// Fixed-income index total-return swap priced with a carry analytic.
///
/// For each wholly future scheduled period, the total-return leg uses
/// `exp(y × accrual_fraction) - 1`, where `y` is an annual continuously
/// compounded decimal yield and the accrual fraction uses the schedule day
/// count. Both legs are discounted with `financing.discount_curve_id`; the
/// financing leg projects `financing.forward_curve_id` plus `spread_bp`.
/// Receive-total-return value is `PV(total return) - PV(financing)`;
/// pay-total-return value reverses that sign.
///
/// # Model limitations
///
/// This deterministic carry analytic omits roll-down, underlying rate/spread
/// mark-to-market, stochastic credit, constituent decomposition, early
/// termination, and bespoke fees. The period in progress on the valuation
/// date is valued from the live index level (`underlying.index_id`) against
/// its historical reset level, then carried at the index yield to period end; a period
/// that has ended but is not yet paid uses the exact historical start and end
/// levels from `fixing_series_id`. Missing historical endpoints are errors.
/// Cashflow-schedule APIs
/// return projected or fixed net payments using the same calculations as
/// [`Self::pv_total_return_leg`] and [`Self::pv_financing_leg`] for projected
/// leg values.
///
/// # Construction
///
/// ```
/// use finstack_quant_core::{
///     currency::Currency, dates::DayCount, money::Money, types::InstrumentId,
/// };
/// use finstack_quant_cashflows::builder::ScheduleParams;
/// use finstack_quant_valuations_core::instruments::{Attributes, FinancingLegSpec, IndexUnderlyingParams, PayReceive};
/// use finstack_quant_valuations_fixed_income::fi_trs::FiIndexTotalReturnSwap;
/// use finstack_quant_valuations_fixed_income::fi_trs::{
///     TrsScheduleSpec,
/// };
/// use rust_decimal::Decimal;
/// use time::macros::date;
///
/// # fn main() -> finstack_quant_core::Result<()> {
/// let trs = FiIndexTotalReturnSwap::builder()
///     .id(InstrumentId::new("CORP-TRS"))
///     .notional(Money::from((10_000_000_i64, Currency::USD)))
///     .underlying(
///         IndexUnderlyingParams::new("US-CORP", Currency::USD)
///             .with_yield_id("US-CORP-YIELD")
///             .with_duration_id("US-CORP-DURATION"),
///     )
///     .financing_leg(FinancingLegSpec::new(
///         "USD-OIS", "USD-SOFR-3M", Decimal::from(35), DayCount::Act360,
///     ))
///     .schedule(TrsScheduleSpec::from_params(
///         date!(2026 - 01 - 02),
///         date!(2027 - 01 - 02),
///         ScheduleParams::quarterly_act360(),
///     ))
///     .side(PayReceive::Receive)
///     .initial_level_opt(None)
///     .attributes(Attributes::new())
///     .build()?;
/// assert_eq!(trs.notional.currency(), Currency::USD);
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
pub struct FiIndexTotalReturnSwap {
    /// Unique instrument identifier.
    pub id: InstrumentId,
    /// Notional amount for the swap.
    pub notional: Money,
    /// Underlying index parameters (index ID, yield, duration, base currency).
    pub underlying: IndexUnderlyingParams,
    /// Financing leg specification (curves, spread, day count).
    pub financing_leg: FinancingLegSpec,
    /// Schedule specification (payment dates and frequency).
    pub schedule: TrsScheduleSpec,
    /// Trade side: `receive` receives the total-return leg and pays financing;
    /// `pay` pays the total return and receives financing.
    pub side: PayReceive,
    /// Index level at the first contractual reset, in the
    /// index's own units.
    ///
    /// Used only at inception when no fixing series is supplied; the
    /// realized return to date is `index_level / initial_level`. Subsequent resets require `fixing_series_id`. Must be positive and finite when set.
    pub initial_level: Option<f64>,
    /// Market time-series identifier containing positive total-return index levels
    /// on contractual reset dates, including the final accrual boundary. Historical
    /// endpoints require exact observations; no yield-based history is inferred.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixing_series_id: Option<String>,
    /// Optional OTC margin specification for VM/IM.
    ///
    /// Fixed income index TRS use duration-based margin calculations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margin_spec: Option<OtcMarginSpec>,
    /// Instrument-owned pricing inputs.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "finstack_quant_valuations_core::instruments::InstrumentPricingOverrides::is_empty"
    )]
    pub instrument_pricing_overrides:
        finstack_quant_valuations_core::instruments::InstrumentPricingOverrides,
    /// Metric-time pricing configuration.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "finstack_quant_valuations_core::instruments::MetricPricingOverrides::is_empty"
    )]
    pub metric_pricing_overrides:
        finstack_quant_valuations_core::instruments::MetricPricingOverrides,
    /// Scenario-only pricing adjustments.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "finstack_quant_valuations_core::instruments::ScenarioPricingOverrides::is_empty"
    )]
    pub scenario_pricing_overrides:
        finstack_quant_valuations_core::instruments::ScenarioPricingOverrides,
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,
}

impl FiIndexTotalReturnSwap {
    /// Resolve the supplied index duration used by both risk metrics and margin.
    ///
    /// # Arguments
    ///
    /// * `market` - Market containing `underlying.duration_id` as a finite
    ///   unitless duration in years. No default duration is inferred.
    pub(crate) fn index_duration(&self, market: &MarketContext) -> Result<f64> {
        use finstack_quant_core::market_data::scalars::MarketScalar;
        let id = self.underlying.duration_id.as_ref().ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "FI TRS duration risk requires duration_id and a supplied or calculated index duration".into(),
            )
        })?;
        let duration = match market.get_price(id.as_str())? {
            MarketScalar::Unitless(value) => *value,
            MarketScalar::Price(_) => {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Index duration '{id}' must be a unitless scalar in years"
                )))
            }
        };
        if !duration.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Index duration '{id}' must be finite, got {duration}"
            )));
        }

        Ok(duration)
    }

    /// Validate the economic contract before pricing or host-language dispatch.
    pub fn validate(&self) -> Result<()> {
        let context = format!("FI index TRS '{}'", self.id.as_str());
        if !self.notional.amount().is_finite() || self.notional.amount() < 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context} notional must be non-negative and finite"
            )));
        }
        if self.notional.currency() != self.underlying.currency {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context} notional currency must match the underlying base currency"
            )));
        }
        self.underlying.validate(&context)?;
        self.financing_leg.validate(&context)?;
        self.schedule.validate(&context)?;
        if let Some(level) = self.initial_level {
            if !level.is_finite() || level <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "{context} initial_level must be positive and finite"
                )));
            }
        }
        Ok(())
    }

    /// Create a canonical example fixed income index TRS (USD Corporate Index, 1Y).
    pub fn example() -> finstack_quant_core::Result<Self> {
        use time::macros::date;
        let underlying = IndexUnderlyingParams::new("US-CORP-INDEX", Currency::USD)
            .with_yield_id("US-CORP-YIELD")
            .with_duration_id("US-CORP-DURATION");
        let financing = FinancingLegSpec::new(
            "USD-OIS",
            "USD-SOFR-3M",
            Decimal::from(100),
            DayCount::Act360,
        );
        let sched = TrsScheduleSpec::from_params(
            date!(2024 - 01 - 01),
            date!(2025 - 01 - 01),
            ScheduleParams {
                frequency: Tenor::quarterly(),
                day_count: DayCount::Act360,
                business_day_convention: BusinessDayConvention::Following,
                calendar_id: "weekends_only".into(),
                stub: StubKind::None,
                end_of_month: false,
                payment_lag_days: 0,
                adjust_accrual_dates: false,
                roll_rule: finstack_quant_cashflows::builder::RollRule::None,
            },
        );
        Self::builder()
            .id(InstrumentId::new("TRS-US-CORP-1Y"))
            .notional(Money::from((5_000_000_i64, Currency::USD)))
            .underlying(underlying)
            .financing_leg(financing)
            .schedule(sched)
            .side(PayReceive::Receive)
            .initial_level_opt(None)
            .attributes(Attributes::new())
            .build()
    }

    /// Calculates the present value of the total return leg.
    ///
    /// # Arguments
    /// * `market` — Market context containing curves and market data
    /// * `as_of` — Date that selects remaining return periods and anchors discounting
    ///
    /// # Returns
    /// Present value of the total return leg in the instrument's currency.
    pub fn pv_total_return_leg(&self, market: &MarketContext, as_of: Date) -> Result<Money> {
        self.validate()?;
        crate::fi_trs::pricer::pv_total_return_leg(self, market, as_of)
    }

    /// Calculates the present value of the financing leg.
    ///
    /// # Arguments
    /// * `market` — Market context containing curves and market data
    /// * `as_of` — Date that selects remaining financing periods and anchors discounting
    ///
    /// # Returns
    /// Present value of the financing leg in the instrument's currency.
    pub fn pv_financing_leg(&self, market: &MarketContext, as_of: Date) -> Result<Money> {
        self.validate()?;
        use finstack_quant_valuations_core::instruments::common_impl::pricing::TrsEngine;
        TrsEngine::pv_financing_leg(
            &self.financing_leg,
            &self.schedule,
            self.notional,
            market,
            as_of,
        )
    }

    /// Calculates the financing annuity for par spread calculation.
    ///
    /// # Arguments
    /// * `market` — Market context containing curves and market data
    /// * `as_of` — Date that selects remaining financing periods and anchors discounting
    ///
    /// # Returns
    /// Financing annuity (sum of discounted year fractions × notional).
    pub fn financing_annuity(&self, market: &MarketContext, as_of: Date) -> Result<f64> {
        self.validate()?;
        use finstack_quant_valuations_core::instruments::common_impl::pricing::TrsEngine;
        TrsEngine::financing_annuity(
            &self.financing_leg,
            &self.schedule,
            self.notional,
            market,
            as_of,
        )
    }
}

impl finstack_quant_valuations_core::instruments::Instrument for FiIndexTotalReturnSwap {
    impl_instrument_base!(
        finstack_quant_valuations_core::pricer::InstrumentType::FiIndexTotalReturnSwap,
        crate::FIXED_INCOME
    );

    fn validate_invariants(&self) -> Result<()> {
        self.validate()
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<finstack_quant_valuations_core::instruments::MarketDependencies>
    {
        let mut deps = finstack_quant_valuations_core::instruments::MarketDependencies::new();
        deps.add_discount_curve(self.financing_leg.discount_curve_id.clone());
        deps.add_forward_curve(self.financing_leg.forward_curve_id.clone());
        if let Some(yield_id) = &self.underlying.yield_id {
            deps.add_market_scalar_id(yield_id);
        }
        if let Some(duration_id) = &self.underlying.duration_id {
            deps.add_market_scalar_id(duration_id);
        }
        if let Some(id) = &self.fixing_series_id {
            deps.add_series_id(id);
        }
        if self.initial_level.is_some() || self.fixing_series_id.is_some() {
            // Seasoned trades read the live index level for the period in progress.
            deps.add_market_scalar_id(self.underlying.index_id.as_str());
        }
        Ok(deps)
    }

    fn base_value(&self, curves: &MarketContext, as_of: Date) -> Result<Money> {
        self.validate()?;
        let total_return_pv = self.pv_total_return_leg(curves, as_of)?;

        let financing_pv = self.pv_financing_leg(curves, as_of)?;

        // Net PV depends on side
        let net_pv = match self.side {
            PayReceive::Receive => total_return_pv.checked_sub(financing_pv)?,
            PayReceive::Pay => financing_pv.checked_sub(total_return_pv)?,
        };

        Ok(net_pv)
    }

    fn as_marginable(&self) -> Option<&dyn finstack_quant_margin::Marginable> {
        Some(self)
    }

    fn expiry(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.schedule.end)
    }

    finstack_quant_valuations_core::impl_focused_pricing_overrides!();
}

impl finstack_quant_cashflows::CashflowScheduleSource for FiIndexTotalReturnSwap {
    fn notional(&self) -> finstack_quant_core::Result<Option<Money>> {
        Ok(Some(self.notional))
    }

    fn raw_cashflow_schedule(
        &self,
        context: &MarketContext,
        as_of: Date,
    ) -> Result<finstack_quant_cashflows::builder::CashFlowSchedule> {
        let (flows, projected_fixings) = super::pricer::net_cashflows(self, context, as_of)?;

        let schedule = finstack_quant_cashflows::schedule_from_dated_flows(
            flows,
            finstack_quant_cashflows::primitives::CFKind::Fixed,
            self.financing_leg.day_count,
            finstack_quant_cashflows::ScheduleBuildOpts {
                notional_hint: self.notional()?,
                meta: finstack_quant_cashflows::builder::CashFlowMeta {
                    representation:
                        finstack_quant_cashflows::builder::CashflowRepresentation::Projected,
                    projected_fixings,
                    ..Default::default()
                },
            },
        );
        Ok(schedule)
    }
}
