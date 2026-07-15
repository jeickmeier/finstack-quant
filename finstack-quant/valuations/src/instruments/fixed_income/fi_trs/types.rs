//! Fixed Income Index Total Return Swap instrument definitions.
//!
//! This module provides the [`FIIndexTotalReturnSwap`] instrument for synthetic
//! fixed income index exposure.

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

/// Fixed Income Index Total Return Swap instrument.
///
/// A TRS where the total return leg is based on a fixed income index (e.g., corporate bond index).
/// The holder receives the total return (carry + roll) of the underlying index in exchange
/// for paying a floating rate plus spread on the notional amount.
///
/// # Use Cases
///
/// - **Synthetic bond exposure**: Gain bond index exposure without buying bonds
/// - **Duration management**: Adjust portfolio duration synthetically
/// - **ETF replication**: Replicate bond ETF returns synthetically
/// - **Credit exposure**: Access corporate bond index returns
///
/// # Example
///
/// ```
/// use finstack_quant_valuations::instruments::fixed_income::fi_trs::FIIndexTotalReturnSwap;
///
/// let trs = FIIndexTotalReturnSwap::example().unwrap();
/// // let pv = trs.value(&market_context, as_of_date)?;
/// ```
#[derive(
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    finstack_quant_valuations_macros::FocusedPricingOverrides,
)]
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
    /// Initial index level (if known, otherwise fetched from market).
    pub initial_level: Option<f64>,
    /// Optional OTC margin specification for VM/IM.
    ///
    /// Fixed income index TRS use duration-based margin calculations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margin_spec: Option<OtcMarginSpec>,
    /// Attributes for scenario selection and tagging.
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

impl FIIndexTotalReturnSwap {
    /// Create a canonical example fixed income index TRS (USD Corporate Index, 1Y).
    pub fn example() -> finstack_quant_core::Result<Self> {
        use time::macros::date;
        let underlying = IndexUnderlyingParams::new("US-CORP-INDEX", Currency::USD)
            .with_yield("US-CORP-YIELD")
            .with_duration("US-CORP-DURATION")
            .with_convexity("US-CORP-CONVEXITY")
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
                freq: Tenor::quarterly(),
                dc: DayCount::Act360,
                bdc: BusinessDayConvention::Following,
                calendar_id: "weekends_only".to_string(),
                stub: StubKind::None,
                end_of_month: false,
                payment_lag_days: 0,
                adjust_accrual_dates: false,
            },
        );
        Self::builder()
            .id(InstrumentId::new("TRS-US-CORP-1Y"))
            .notional(Money::new(5_000_000.0, Currency::USD))
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
    /// # Example
    ///
    /// ```ignore
    /// let lqd_trs = FIIndexTotalReturnSwap::replicate_etf(
    ///     "LQD",
    ///     Money::new(10_000_000.0, Currency::USD),
    ///     financing_spec,
    ///     schedule_spec,
    ///     Some("LQD-YIELD"),
    ///     Some("LQD-DURATION"),
    /// );
    /// ```
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
    /// * `as_of` — Valuation date
    ///
    /// # Returns
    /// Present value of the total return leg in the instrument's currency.
    pub fn pv_total_return_leg(&self, curves: &MarketContext, as_of: Date) -> Result<Money> {
        crate::instruments::fixed_income::fi_trs::pricer::pv_total_return_leg(self, curves, as_of)
    }

    /// Calculates the present value of the financing leg.
    ///
    /// # Arguments
    /// * `curves` — Market context containing curves and market data
    /// * `as_of` — Valuation date
    ///
    /// # Returns
    /// Present value of the financing leg in the instrument's currency.
    pub fn pv_financing_leg(&self, curves: &MarketContext, as_of: Date) -> Result<Money> {
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
    /// * `as_of` — Valuation date
    ///
    /// # Returns
    /// Financing annuity (sum of discounted year fractions × notional).
    pub fn financing_annuity(&self, curves: &MarketContext, as_of: Date) -> Result<f64> {
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

// ============================================================================
// Trait Implementations
// ============================================================================

impl crate::instruments::common_impl::traits::Instrument for FIIndexTotalReturnSwap {
    impl_instrument_base!(crate::pricer::InstrumentType::FIIndexTotalReturnSwap);

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<
        crate::instruments::common_impl::dependencies::MarketDependencies,
    > {
        let mut deps = crate::instruments::common_impl::dependencies::MarketDependencies::new();
        deps.add_discount_curve(self.financing.discount_curve_id.clone());
        deps.add_forward_curve(self.financing.forward_curve_id.clone());
        if let Some(yield_id) = &self.underlying.yield_id {
            deps.add_spot_id(yield_id);
        }
        if let Some(duration_id) = &self.underlying.duration_id {
            deps.add_spot_id(duration_id);
        }
        Ok(deps)
    }

    fn base_value(&self, curves: &MarketContext, as_of: Date) -> Result<Money> {
        // Calculate total return leg PV
        let total_return_pv = self.pv_total_return_leg(curves, as_of)?;

        // Calculate financing leg PV
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
    fn notional(&self) -> Option<Money> {
        Some(self.notional)
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
            flows.push((*date, Money::new(0.0, self.notional.currency())));
        }

        let schedule = crate::cashflow::traits::schedule_from_dated_flows(
            flows,
            crate::cashflow::primitives::CFKind::Fixed,
            self.financing.day_count,
            crate::cashflow::traits::ScheduleBuildOpts {
                notional_hint: self.notional(),
                meta: crate::cashflow::builder::CashFlowMeta {
                    representation: crate::cashflow::builder::CashflowRepresentation::Projected,
                    ..Default::default()
                },
            },
        );
        Ok(schedule)
    }
}
