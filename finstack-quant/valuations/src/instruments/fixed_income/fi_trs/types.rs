//! Fixed-income index total-return swap contract.

use crate::impl_instrument_base;
use crate::{
    cashflow::builder::ScheduleParams,
    instruments::common_impl::parameters::{
        legs::FinancingLegSpec, trs_common::TrsScheduleSpec, trs_common::TrsSide,
        underlying::IndexUnderlyingParams,
    },
    instruments::Attributes,
};
use finstack_quant_core::{
    currency::Currency,
    dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor},
    market_data::context::MarketContext,
    money::Money,
    types::InstrumentId,
    Result,
};
use finstack_quant_margin::types::OtcMarginSpec;
use rust_decimal::Decimal;

/// Fixed-income index total-return swap priced with a carry analytic.
///
/// For each scheduled period, the total-return leg uses
/// `exp(y × accrual_fraction) - 1`, where `y` is an annual continuously
/// compounded decimal yield and the accrual fraction uses the schedule day
/// count. Both legs are discounted with `financing.discount_curve_id`; the
/// financing leg projects `financing.forward_curve_id` plus `spread_bp`.
/// Receive-total-return value is `PV(total return) - PV(financing)`;
/// pay-total-return value reverses that sign.
///
/// # Model limitations
///
/// This deterministic carry analytic omits realized index price returns,
/// roll-down, underlying rate/spread mark-to-market, stochastic credit,
/// constituent decomposition, early termination, and bespoke fees. Pricing
/// rejects an in-progress total-return accrual period. Cashflow-schedule APIs
/// return payment dates with zero amounts; use
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
/// use finstack_quant_valuations::instruments::{
///     Attributes, FinancingLegSpec, IndexUnderlyingParams,
/// };
/// use finstack_quant_valuations::instruments::fixed_income::fi_trs::FIIndexTotalReturnSwap;
/// use finstack_quant_valuations::instruments::fixed_income::fi_trs::{
///     TrsScheduleSpec, TrsSide,
/// };
/// use rust_decimal::Decimal;
/// use time::macros::date;
///
/// # fn main() -> finstack_quant_core::Result<()> {
/// let trs = FIIndexTotalReturnSwap::builder()
///     .id(InstrumentId::new("CORP-TRS"))
///     .notional(Money::from((10_000_000_i64, Currency::USD)))
///     .underlying(
///         IndexUnderlyingParams::new("US-CORP", Currency::USD)
///             .with_yield("US-CORP-YIELD")
///             .with_duration("US-CORP-DURATION"),
///     )
///     .financing(FinancingLegSpec::new(
///         "USD-OIS", "USD-SOFR-3M", Decimal::from(35), DayCount::Act360,
///     ))
///     .schedule(TrsScheduleSpec::from_params(
///         date!(2026 - 01 - 02),
///         date!(2027 - 01 - 02),
///         ScheduleParams::quarterly_act360(),
///     ))
///     .side(TrsSide::ReceiveTotalReturn)
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
pub struct FIIndexTotalReturnSwap {
    /// Unique instrument identifier.
    pub id: InstrumentId,
    /// Notional amount for the swap.
    pub notional: Money,
    /// Underlying index parameters (index ID, yield, duration, base currency).
    pub underlying: IndexUnderlyingParams,
    /// Financing leg specification (curves, spread, day count).
    pub financing: FinancingLegSpec,
    /// Schedule specification (payment dates and frequency).
    pub schedule: TrsScheduleSpec,
    /// Trade side (receive/pay total return).
    pub side: TrsSide,
    /// Optional reference level retained with the contract.
    ///
    /// The deterministic carry pricer does not consume or fetch this value.
    pub initial_level: Option<f64>,
    /// Optional OTC margin specification for VM/IM.
    ///
    /// Fixed income index TRS use duration-based margin calculations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margin_spec: Option<OtcMarginSpec>,
    /// Attributes for scenario selection and tagging.
    #[builder(default)]
    /// Instrument-owned pricing inputs.
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

impl FIIndexTotalReturnSwap {
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
        if self.notional.currency() != self.underlying.base_currency {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context} notional currency must match the underlying base currency"
            )));
        }
        self.underlying.validate(&context)?;
        self.financing.validate(&context)?;
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
            .with_yield("US-CORP-YIELD")
            .with_duration("US-CORP-DURATION")
            .with_contract_size(1.0);
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
                calendar_id: "weekends_only".to_string(),
                stub: StubKind::None,
                end_of_month: false,
                payment_lag_days: 0,
                adjust_accrual_dates: false,
                roll_rule: crate::cashflow::builder::specs::RollRule::None,
            },
        );
        Self::builder()
            .id(InstrumentId::new("TRS-US-CORP-1Y"))
            .notional(Money::from((5_000_000_i64, Currency::USD)))
            .underlying(underlying)
            .financing(financing)
            .schedule(sched)
            .side(TrsSide::ReceiveTotalReturn)
            .initial_level_opt(None)
            .attributes(Attributes::new())
            .build()
    }

    /// Creates an FI TRS that replicates a bond ETF.
    ///
    /// This is a convenience constructor for creating TRS positions that synthetically
    /// replicate bond ETF exposure.
    ///
    /// # Arguments
    /// * `etf_ticker` — ETF ticker symbol (e.g., "LQD", "AGG", "HYG")
    /// * `notional` — Notional amount in the ETF's currency
    /// * `financing` — Financing leg specification
    /// * `schedule` — Payment schedule specification
    /// * `yield_id` — Optional index yield market data identifier
    /// * `duration_id` — Optional index duration market data identifier
    ///
    /// # Errors
    ///
    /// Returns an error if the builder fails validation.
    pub fn replicate_etf(
        etf_ticker: &str,
        notional: Money,
        financing: FinancingLegSpec,
        schedule: TrsScheduleSpec,
        yield_id: Option<&str>,
        duration_id: Option<&str>,
    ) -> Result<Self> {
        let mut underlying = IndexUnderlyingParams::new(etf_ticker, notional.currency());
        if let Some(y) = yield_id {
            underlying = underlying.with_yield(y);
        }
        if let Some(d) = duration_id {
            underlying = underlying.with_duration(d);
        }

        Self::builder()
            .id(InstrumentId::new(format!("TRS-{}", etf_ticker)))
            .notional(notional)
            .underlying(underlying)
            .financing(financing)
            .schedule(schedule)
            .side(TrsSide::ReceiveTotalReturn)
            .initial_level_opt(None)
            .attributes(Attributes::new())
            .build()
    }

    /// Calculates the present value of the total return leg.
    ///
    /// # Arguments
    /// * `curves` — Market context containing curves and market data
    /// * `as_of` — Date that selects remaining return periods and anchors discounting
    ///
    /// # Returns
    /// Present value of the total return leg in the instrument's currency.
    pub fn pv_total_return_leg(&self, curves: &MarketContext, as_of: Date) -> Result<Money> {
        self.validate()?;
        crate::instruments::fixed_income::fi_trs::pricer::pv_total_return_leg(self, curves, as_of)
    }

    /// Calculates the present value of the financing leg.
    ///
    /// # Arguments
    /// * `curves` — Market context containing curves and market data
    /// * `as_of` — Date that selects remaining financing periods and anchors discounting
    ///
    /// # Returns
    /// Present value of the financing leg in the instrument's currency.
    pub fn pv_financing_leg(&self, curves: &MarketContext, as_of: Date) -> Result<Money> {
        self.validate()?;
        use crate::instruments::common_impl::pricing::TrsEngine;
        TrsEngine::pv_financing_leg(
            &self.financing,
            &self.schedule,
            self.notional,
            curves,
            as_of,
        )
    }

    /// Calculates the financing annuity for par spread calculation.
    ///
    /// # Arguments
    /// * `curves` — Market context containing curves and market data
    /// * `as_of` — Date that selects remaining financing periods and anchors discounting
    ///
    /// # Returns
    /// Financing annuity (sum of discounted year fractions × notional).
    pub fn financing_annuity(&self, curves: &MarketContext, as_of: Date) -> Result<f64> {
        self.validate()?;
        use crate::instruments::common_impl::pricing::TrsEngine;
        TrsEngine::financing_annuity(
            &self.financing,
            &self.schedule,
            self.notional,
            curves,
            as_of,
        )
    }
}

impl crate::instruments::common_impl::traits::Instrument for FIIndexTotalReturnSwap {
    impl_instrument_base!(crate::pricer::InstrumentType::FiIndexTotalReturnSwap);

    fn validate_invariants(&self) -> Result<()> {
        self.validate()
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<
        crate::instruments::common_impl::dependencies::MarketDependencies,
    > {
        let mut deps = crate::instruments::common_impl::dependencies::MarketDependencies::new();
        deps.add_discount_curve(self.financing.discount_curve_id.clone());
        deps.add_forward_curve(self.financing.forward_curve_id.clone());
        if let Some(yield_id) = &self.underlying.yield_id {
            deps.add_market_scalar_id(yield_id);
        }
        if let Some(duration_id) = &self.underlying.duration_id {
            deps.add_market_scalar_id(duration_id);
        }
        Ok(deps)
    }

    fn base_value(&self, curves: &MarketContext, as_of: Date) -> Result<Money> {
        self.validate()?;
        let total_return_pv = self.pv_total_return_leg(curves, as_of)?;

        let financing_pv = self.pv_financing_leg(curves, as_of)?;

        // Net PV depends on side
        let net_pv = match self.side {
            TrsSide::ReceiveTotalReturn => total_return_pv.checked_sub(financing_pv)?,
            TrsSide::PayTotalReturn => financing_pv.checked_sub(total_return_pv)?,
        };

        Ok(net_pv)
    }

    fn as_marginable(&self) -> Option<&dyn finstack_quant_margin::Marginable> {
        Some(self)
    }

    fn expiry(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.schedule.end)
    }

    fn effective_start_date(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.schedule.start)
    }

    crate::impl_focused_pricing_overrides!();
}

impl finstack_quant_cashflows::CashflowScheduleSource for FIIndexTotalReturnSwap {
    fn notional(&self) -> finstack_quant_core::Result<Option<Money>> {
        Ok(Some(self.notional))
    }

    fn raw_cashflow_schedule(
        &self,
        _context: &MarketContext,
        _as_of: Date,
    ) -> Result<crate::cashflow::builder::CashFlowSchedule> {
        // TRS cashflow amounts depend on realized returns and are therefore
        // unknown at trade time. We return the payment-date skeleton with
        // zero amounts so that schedule-based queries (e.g., next payment
        // date, period count) work correctly. Consumers that need projected
        // amounts should use `pv_total_return_leg` / `pv_financing_leg` instead.
        let period_schedule = self.schedule.period_schedule()?;

        let mut flows = Vec::new();
        for date in period_schedule.dates.iter().skip(1) {
            flows.push((*date, Money::from((0_i64, self.notional.currency()))));
        }

        let schedule = crate::cashflow::traits::schedule_from_dated_flows(
            flows,
            crate::cashflow::primitives::CFKind::Fixed,
            self.financing.day_count,
            crate::cashflow::traits::ScheduleBuildOpts {
                notional_hint: self.notional()?,
                meta: crate::cashflow::builder::CashFlowMeta {
                    representation: crate::cashflow::builder::CashflowRepresentation::Projected,
                    ..Default::default()
                },
            },
        );
        Ok(schedule)
    }
}
