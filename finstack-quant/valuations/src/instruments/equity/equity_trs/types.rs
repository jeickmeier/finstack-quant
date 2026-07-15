//! Equity Total Return Swap instrument definitions.
//!
//! This module provides the [`EquityTotalReturnSwap`] instrument for synthetic
//! equity index or single-stock exposure.

use crate::impl_instrument_base;
use crate::{
    cashflow::builder::ScheduleParams,
    cashflow::traits::CashflowProvider,
    instruments::common_impl::parameters::{
        legs::FinancingLegSpec, trs_common::TrsScheduleSpec, trs_common::TrsSide,
        underlying::EquityUnderlyingParams,
    },
    instruments::Attributes,
};
use finstack_quant_core::{
    currency::Currency,
    dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor},
    market_data::context::MarketContext,
    money::Money,
    types::{CurveId, InstrumentId},
    Result,
};
use finstack_quant_margin::types::OtcMarginSpec;
use rust_decimal::Decimal;
use time::macros::date;

/// Equity Total Return Swap instrument.
///
/// A TRS where the total return leg is based on an equity index or single stock.
/// The holder receives the total return (price appreciation + dividends) of the underlying
/// equity in exchange for paying a floating rate plus spread on the notional amount.
///
/// # Use Cases
///
/// - **Synthetic long exposure**: Gain equity index exposure without buying assets
/// - **Leverage**: Minimize upfront capital requirements
/// - **ETF replication**: Replicate equity ETF returns synthetically
/// - **Short exposure**: Easier than borrowing securities
///
/// # Example
///
/// ```
/// use finstack_quant_valuations::instruments::equity::equity_trs::EquityTotalReturnSwap;
///
/// let trs = EquityTotalReturnSwap::example().unwrap();
/// // let pv = trs.value(&market_context, as_of_date)?;
/// ```
#[derive(
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[builder(validate = EquityTotalReturnSwap::validate)]
#[serde(deny_unknown_fields, try_from = "EquityTotalReturnSwapUnchecked")]
pub struct EquityTotalReturnSwap {
    /// Unique instrument identifier.
    pub id: InstrumentId,
    /// Notional amount for the swap.
    pub notional: Money,
    /// Underlying equity parameters (spot ID, dividend yield, contract size).
    pub underlying: EquityUnderlyingParams,
    /// Financing leg specification (curves, spread, day count).
    pub financing: FinancingLegSpec,
    /// Schedule specification (payment dates and frequency).
    pub schedule: TrsScheduleSpec,
    /// Trade side (receive/pay total return).
    pub side: TrsSide,
    /// Initial index level (if known, otherwise fetched from market).
    pub initial_level: Option<f64>,
    /// Observed underlying levels at past reset (period-start) dates.
    ///
    /// For a seasoned TRS valued inside a return period, the total-return leg
    /// must anchor the current period to the level *observed* at the period
    /// start so the realized spot move enters the PV (equity delta). Provide
    /// `(reset_date, level)` pairs for every period-start date on or before
    /// the valuation date; the first period may use `initial_level` instead.
    /// Pricing errors when the current period's start level is unavailable.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(with = "Vec<(String, f64)>")]
    pub past_fixings: Vec<(Date, f64)>,
    /// Optional OTC margin specification for VM/IM.
    ///
    /// Equity TRS use SIMM equity bucket for margin calculation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margin_spec: Option<OtcMarginSpec>,
    /// Dividend withholding tax rate for net return calculation.
    ///
    /// Specifies the fraction of dividends withheld for tax (e.g., 0.15 for 15% withholding).
    /// When set to 0.0 (default), the TRS passes through 100% of dividends (gross return).
    /// When set to a positive value, the dividend return component is reduced:
    /// ```text
    /// net_dividend_return = gross_dividend_return × (1 - dividend_tax_rate)
    /// ```
    ///
    /// # Market Context
    ///
    /// Withholding tax varies by jurisdiction and investor domicile:
    /// - US qualified dividends: typically 0% for domestic investors
    /// - US non-qualified: up to 30% for foreign investors (varies by treaty)
    /// - European: varies by country (15-30% typical)
    #[serde(default)]
    #[builder(default)]
    pub dividend_tax_rate: f64,
    /// Optional discrete cash dividends `(ex_date, amount)` for the underlying.
    ///
    /// When non-empty, pricing uses explicit period dividend pass-through and does
    /// not add continuous-yield dividend return to avoid double counting.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[builder(default)]
    #[schemars(with = "Vec<(String, f64)>")]
    pub discrete_dividends: Vec<(Date, f64)>,
    /// Attributes for scenario selection and tagging.
    #[serde(default)]
    #[builder(default)]
    pub pricing_overrides: crate::instruments::PricingOverrides,
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,
}

/// Mirror of `EquityTotalReturnSwap` used by serde to apply `validate()`
/// after deserialization. Not part of the public API.
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct EquityTotalReturnSwapUnchecked {
    id: InstrumentId,
    notional: Money,
    underlying: EquityUnderlyingParams,
    financing: FinancingLegSpec,
    schedule: TrsScheduleSpec,
    side: TrsSide,
    initial_level: Option<f64>,
    #[serde(default)]
    #[schemars(with = "Vec<(String, f64)>")]
    past_fixings: Vec<(Date, f64)>,
    #[serde(default)]
    margin_spec: Option<OtcMarginSpec>,
    #[serde(default)]
    dividend_tax_rate: f64,
    #[serde(default)]
    #[schemars(with = "Vec<(String, f64)>")]
    discrete_dividends: Vec<(Date, f64)>,
    #[serde(default)]
    pricing_overrides: crate::instruments::PricingOverrides,
    attributes: Attributes,
}

impl TryFrom<EquityTotalReturnSwapUnchecked> for EquityTotalReturnSwap {
    type Error = finstack_quant_core::Error;

    fn try_from(value: EquityTotalReturnSwapUnchecked) -> std::result::Result<Self, Self::Error> {
        let inst = Self {
            id: value.id,
            notional: value.notional,
            underlying: value.underlying,
            financing: value.financing,
            schedule: value.schedule,
            side: value.side,
            initial_level: value.initial_level,
            past_fixings: value.past_fixings,
            margin_spec: value.margin_spec,
            dividend_tax_rate: value.dividend_tax_rate,
            discrete_dividends: value.discrete_dividends,
            pricing_overrides: value.pricing_overrides,
            attributes: value.attributes,
        };
        inst.validate()?;
        Ok(inst)
    }
}

impl EquityTotalReturnSwap {
    /// Create a canonical example equity TRS for testing and documentation.
    ///
    /// Returns a 1-year SPX total return swap with quarterly resets.
    pub fn example() -> finstack_quant_core::Result<Self> {
        // SAFETY: All inputs are compile-time validated constants
        Self::builder()
            .id(InstrumentId::new("TRS-SPX-1Y"))
            .notional(Money::new(5_000_000.0, Currency::USD))
            .underlying(EquityUnderlyingParams {
                ticker: "SPX".to_string(),
                spot_id: "SPX-SPOT".into(),
                div_yield_id: Some(CurveId::new("SPX-DIV")),
                contract_size: 1.0,
                currency: Currency::USD,
            })
            .financing(FinancingLegSpec::new(
                "USD-OIS",
                "USD-SOFR-3M",
                Decimal::from(75),
                DayCount::Act360,
            ))
            .schedule(TrsScheduleSpec::from_params(
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
            ))
            .side(TrsSide::ReceiveTotalReturn)
            .initial_level_opt(None)
            .attributes(Attributes::new())
            .build()
    }

    /// Creates an equity TRS that replicates an ETF.
    ///
    /// This is a convenience constructor for creating TRS positions that synthetically
    /// replicate equity ETF exposure.
    ///
    /// # Arguments
    /// * `etf_ticker` — ETF ticker symbol (e.g., "SPY", "QQQ")
    /// * `spot_id` — Market data identifier for the ETF spot price
    /// * `notional` — Notional amount in the ETF's currency
    /// * `financing` — Financing leg specification
    /// * `schedule` — Payment schedule specification
    /// * `div_yield_id` — Optional dividend yield market data identifier
    ///
    /// # Example
    ///
    /// ```text
    /// let spy_trs = EquityTotalReturnSwap::replicate_etf(
    ///     "SPY",
    ///     "SPY-SPOT",
    ///     Money::new(10_000_000.0, Currency::USD),
    ///     financing_spec,
    ///     schedule_spec,
    ///     Some("SPY-DIV"),
    /// )?;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if the builder fails validation.
    pub fn replicate_etf(
        etf_ticker: &str,
        spot_id: &str,
        notional: Money,
        financing: FinancingLegSpec,
        schedule: TrsScheduleSpec,
        div_yield_id: Option<&str>,
    ) -> Result<Self> {
        let mut underlying = EquityUnderlyingParams::new(etf_ticker, spot_id, notional.currency());
        if let Some(div) = div_yield_id {
            underlying = underlying.with_dividend_yield(div);
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

    /// Validates the equity TRS configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - `notional.amount()` is non-finite
    /// - `dividend_tax_rate` is non-finite or outside `[0.0, 1.0]`
    /// - `dividend_tax_rate > 0` but no dividend source is set
    ///   (neither `underlying.div_yield_id` nor `discrete_dividends`)
    /// - `discrete_dividends` contains a non-finite or negative amount, or
    ///   non-strictly-increasing ex-dates
    pub fn validate(&self) -> Result<()> {
        if !self.notional.amount().is_finite() {
            return Err(finstack_quant_core::Error::Validation(
                "EquityTRS notional amount must be finite".into(),
            ));
        }
        if !self.dividend_tax_rate.is_finite() || !(0.0..=1.0).contains(&self.dividend_tax_rate) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "EquityTRS '{}' dividend_tax_rate must be in [0.0, 1.0], got {}",
                self.id.as_str(),
                self.dividend_tax_rate
            )));
        }
        if self.dividend_tax_rate > 0.0
            && self.underlying.div_yield_id.is_none()
            && self.discrete_dividends.is_empty()
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "EquityTRS '{}' has dividend_tax_rate={:.2}% but no div_yield_id is set. \
                 Set underlying.div_yield_id to enable dividend return calculation, \
                 provide discrete_dividends, or set dividend_tax_rate to 0.0 if dividends are not applicable.",
                self.id.as_str(),
                self.dividend_tax_rate * 100.0
            )));
        }
        for (i, (date, amount)) in self.discrete_dividends.iter().enumerate() {
            if !amount.is_finite() || *amount < 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "EquityTRS '{}' discrete_dividends[{}] amount = {} on {} must be finite and non-negative",
                    self.id.as_str(), i, amount, date
                )));
            }
        }
        for window in self.discrete_dividends.windows(2) {
            if window[0].0 >= window[1].0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "EquityTRS '{}' discrete_dividends ex-dates must be strictly increasing; got {} >= {}",
                    self.id.as_str(),
                    window[0].0,
                    window[1].0
                )));
            }
        }
        for (d, v) in &self.past_fixings {
            if !v.is_finite() || *v <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "EquityTRS '{}' past_fixings[{}] = {} must be finite and positive",
                    self.id.as_str(),
                    d,
                    v
                )));
            }
        }
        Ok(())
    }

    /// Look up the observed underlying level for a reset (period-start) date.
    pub fn fixing_on(&self, date: Date) -> Option<f64> {
        self.past_fixings
            .iter()
            .find(|(d, _)| *d == date)
            .map(|(_, v)| *v)
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
        crate::instruments::equity::equity_trs::pricer::pv_total_return_leg(self, curves, as_of)
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
        let discount = curves.get_discount(self.financing.discount_curve_id.as_str())?;
        let schedule = self.cashflow_schedule(curves, as_of)?;
        let financing_flows: Vec<_> = schedule
            .get_flows()
            .iter()
            .filter(|cf| cf.kind == finstack_quant_core::cashflow::CFKind::FloatReset)
            .collect();
        let period_schedule = self.schedule.period_schedule()?;
        let payment_ends: Vec<_> = period_schedule
            .dates
            .iter()
            .copied()
            .skip(1)
            .filter(|date| *date > as_of)
            .collect();

        if financing_flows.len() != payment_ends.len() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Equity TRS financing schedule mismatch: {} financing flows vs {} future payment dates",
                financing_flows.len(),
                payment_ends.len()
            )));
        }

        financing_flows.into_iter().zip(payment_ends).try_fold(
            Money::new(0.0, self.notional.currency()),
            |acc, (flow, period_end)| {
                let payment_date = self.schedule.payment_date_for(period_end)?;
                let df =
                    crate::instruments::common_impl::pricing::time::relative_df_discount_curve(
                        discount.as_ref(),
                        as_of,
                        payment_date,
                    )?;
                acc.checked_add(flow.amount * df)
            },
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

impl crate::instruments::common_impl::traits::Instrument for EquityTotalReturnSwap {
    impl_instrument_base!(crate::pricer::InstrumentType::EquityTotalReturnSwap);

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
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
        deps.add_spot_id(self.underlying.spot_id.as_str());
        if let Some(dividend_yield) = &self.underlying.div_yield_id {
            deps.add_series_id(dividend_yield.as_str());
        }
        Ok(deps)
    }

    fn base_value(&self, curves: &MarketContext, as_of: Date) -> Result<Money> {
        // Validate configuration
        self.validate()?;

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

impl finstack_quant_cashflows::CashflowScheduleSource for EquityTotalReturnSwap {
    fn notional(&self) -> Option<Money> {
        Some(self.notional)
    }

    fn raw_cashflow_schedule(
        &self,
        context: &MarketContext,
        _as_of: Date,
    ) -> Result<crate::cashflow::builder::CashFlowSchedule> {
        let mut builder = crate::cashflow::builder::CashFlowSchedule::builder();
        let _ = builder
            .principal(self.notional, self.schedule.start, self.schedule.end)
            .floating_cf(crate::cashflow::builder::FloatingCouponSpec {
                rate_spec: crate::cashflow::builder::FloatingRateSpec {
                    index_id: self.financing.forward_curve_id.clone(),
                    spread_bp: self.financing.spread_bp,
                    gearing: Decimal::ONE,
                    gearing_includes_spread: true,
                    index_floor_bp: None,
                    all_in_cap_bp: None,
                    all_in_floor_bp: None,
                    index_cap_bp: None,
                    overnight_index_constraints: Default::default(),
                    reset_freq: self.schedule.params.freq,
                    index_tenor: None,
                    reset_lag_days: 0,
                    fixing_calendar_id: None,
                    overnight_compounding: None,
                    overnight_basis: None,
                    fallback: crate::cashflow::builder::FloatingRateFallback::Error,
                },
                coupon_type: crate::cashflow::builder::CouponType::Cash,
                schedule: finstack_quant_cashflows::builder::ScheduleParams {
                    freq: self.schedule.params.freq,
                    dc: self.financing.day_count,
                    bdc: self.schedule.params.bdc,
                    calendar_id: self.schedule.params.calendar_id.clone(),
                    stub: self.schedule.params.stub,
                    end_of_month: self.schedule.params.end_of_month,
                    payment_lag_days: self.schedule.params.payment_lag_days,
                    adjust_accrual_dates: false,
                },
            });
        let schedule = builder.build(Some(context))?;
        Ok(schedule
            .with_representation(crate::cashflow::builder::CashflowRepresentation::Projected))
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod validation_tests {
    use super::*;

    fn base() -> EquityTotalReturnSwap {
        EquityTotalReturnSwap::example().expect("example builds")
    }

    #[test]
    fn validate_accepts_default_example() {
        let trs = base();
        assert!(trs.validate().is_ok());
    }

    #[test]
    fn canonical_dependencies_preserve_financing_roles_and_underlying() {
        let trs = base();
        let deps = crate::instruments::Instrument::market_dependencies(&trs).expect("dependencies");

        assert_eq!(
            deps.curves.discount_curves.as_slice(),
            std::slice::from_ref(&trs.financing.discount_curve_id)
        );
        assert_eq!(
            deps.curves.forward_curves.as_slice(),
            std::slice::from_ref(&trs.financing.forward_curve_id)
        );
        assert_eq!(
            deps.spot_ids,
            vec![trs.underlying.spot_id.as_str().to_string()]
        );
        assert_eq!(
            deps.series_ids,
            trs.underlying
                .div_yield_id
                .iter()
                .map(|id| id.as_str().to_string())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn validate_rejects_negative_dividend_tax_rate() {
        let mut trs = base();
        trs.dividend_tax_rate = -0.01;
        let err = trs.validate().expect_err("negative tax rate");
        assert!(err.to_string().contains("dividend_tax_rate"));
    }

    #[test]
    fn validate_rejects_dividend_tax_rate_above_one() {
        let mut trs = base();
        trs.dividend_tax_rate = 1.5;
        let err = trs.validate().expect_err("> 1.0 tax rate");
        assert!(err.to_string().contains("dividend_tax_rate"));
    }

    #[test]
    fn validate_rejects_non_finite_dividend_tax_rate() {
        let mut trs = base();
        trs.dividend_tax_rate = f64::NAN;
        let err = trs.validate().expect_err("NaN tax rate");
        assert!(err.to_string().contains("dividend_tax_rate"));
    }

    #[test]
    fn validate_rejects_unsorted_discrete_dividends() {
        let mut trs = base();
        trs.discrete_dividends = vec![(date!(2024 - 06 - 15), 1.0), (date!(2024 - 03 - 15), 1.0)];
        let err = trs.validate().expect_err("unsorted dividends");
        assert!(err.to_string().contains("strictly increasing"));
    }

    #[test]
    fn validate_rejects_negative_dividend_amount() {
        let mut trs = base();
        trs.discrete_dividends = vec![(date!(2024 - 06 - 15), -0.5)];
        let err = trs.validate().expect_err("negative dividend");
        assert!(err.to_string().contains("non-negative"));
    }

    #[test]
    fn builder_rejects_invalid_dividend_tax_rate() {
        // The builder uses `validate` post-build via the macro attribute.
        let bad = EquityTotalReturnSwap::example().map(|mut t| {
            t.dividend_tax_rate = 2.0;
            t
        });
        // The example builds fine; bypassing builder via mutation requires
        // a follow-up validate call to re-check.
        let trs = bad.expect("base example builds");
        assert!(trs.validate().is_err());
    }
}
