//! Term loan instrument type and core specifications.
//!
//! This module defines the [`TermLoan`] instrument type and its associated specifications
//! including rate types and trait implementations.
//!
//! # Overview
//!
//! The [`TermLoan`] type represents a fully-validated term loan instrument with:
//! - Fixed or floating rate specifications
//! - Optional DDTL (delayed-draw) features
//! - Covenant-driven events
//! - Amortization schedules
//! - Call schedules
//!
//! # Quick Example
//!
//! ```rust
//! use finstack_quant_valuations::instruments::fixed_income::term_loan::{TermLoan, RateSpec};
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_core::money::Money;
//! use finstack_quant_core::dates::*;
//! use finstack_quant_core::types::{InstrumentId, CurveId};
//! use time::Month;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a simple example term loan
//! let loan = TermLoan::example().unwrap();
//!
//! assert_eq!(loan.currency, Currency::USD);
//! assert_eq!(loan.notional_limit, Money::from((10_000_000_i64, Currency::USD)));
//! # Ok(())
//! # }
//! ```
//!
//! # See Also
//!
//! - [`RateSpec`] for rate type definitions
//! - [`super::spec`] module for all specification types

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{
    calendar::calendar_by_id, BusinessDayConvention, Date, DateExt, DayCount, StubKind, Tenor,
};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_core::InputError;
use rust_decimal::Decimal;

use super::spec::{
    AmortizationSpec, DdtlSpec, LoanCallSchedule, OidEirSpec, TermLoanCovenantEvents,
};
use crate::cashflow::builder::specs::CouponType;
use crate::cashflow::builder::FloatingRateSpec;
use crate::impl_instrument_base;
use crate::instruments::common_impl::traits::Attributes;
pub use crate::instruments::fixed_income::loan_terms::RateSpec;
use crate::instruments::fixed_income::loan_terms::UpfrontFee;
use crate::instruments::pricing_overrides::InstrumentPricingOverrides;

fn default_settlement_days() -> u32 {
    2
}

/// Term loan instrument with covenant and DDTL support.
///
/// Represents a fully-validated institutional term loan with support for:
/// - Fixed or floating interest rates
/// - Delayed-draw term loan (DDTL) features
/// - Payment-in-kind (PIK) interest
/// - Flexible amortization schedules
/// - Covenant-driven events (margin step-ups, cash sweeps, PIK toggles)
/// - Original issue discount (OID) handling
/// - Borrower call schedules
///
/// # Construction
///
/// Build with [`TermLoan::builder()`]; `build()` validates the complete
/// contract (dates, currencies, DDTL draws against the commitment in force,
/// covenant and call schedules):
///
/// ```
/// use finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoan;
///
/// let loan = TermLoan::example()?;
/// loan.validate()?;
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
///
/// # Cashflow Generation
///
/// Uses the [`CashflowProvider`](crate::cashflow::traits::CashflowProvider) trait:
/// - `dated_cashflows()` returns signed canonical schedule flows (coupons, amortization, redemptions)
/// - `cashflow_schedule()` returns the signed canonical schedule with `CFKind` metadata
///
/// # Pricing
///
/// Implements [`Instrument::value()`](crate::instruments::common_impl::traits::Instrument::value)
/// using deterministic cashflow discounting. PIK interest is capitalized and excluded from PV.
///
/// # Invariants
///
/// - `issue < maturity`
/// - `notional_limit.currency() == currency`
/// - All monetary amounts are in the same currency
/// - Amortization does not exceed outstanding principal
///
/// # Thread Safety
///
/// This type is `Send + Sync` as all fields are thread-safe.
#[derive(
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TermLoan {
    /// Unique instrument identifier
    pub id: InstrumentId,

    /// Currency for all cashflows
    pub currency: Currency,

    /// Maximum commitment / notional limit
    pub notional_limit: Money,

    /// Issue (effective) date
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub issue_date: Date,

    /// Maturity date
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub maturity: Date,

    /// Rate specification (fixed or floating)
    pub rate: RateSpec,

    /// Payment frequency for coupons/fees
    pub frequency: Tenor,

    /// Day count convention
    pub day_count: DayCount,

    /// Business day convention
    #[builder(default = BusinessDayConvention::ModifiedFollowing)]
    #[serde(default = "crate::serde_defaults::bdc_modified_following")]
    pub business_day_convention: BusinessDayConvention,

    /// Optional calendar id for adjustments
    pub calendar_id: Option<finstack_quant_core::types::CalendarId>,

    /// Stub rule
    #[builder(default = StubKind::ShortFront)]
    #[serde(default = "crate::serde_defaults::stub_short_front")]
    pub stub: StubKind,

    /// Discount curve identifier
    pub discount_curve_id: CurveId,

    /// Optional credit curve identifier (defaults to discount_curve_id if None)
    pub credit_curve_id: Option<CurveId>,

    /// Scheduled principal amortization (the shared cashflows
    /// `AmortizationSpec`). `LinearTo` and `StepRemaining` are rejected;
    /// `PercentOfOriginalPerPeriod` and `LinearBetween` apply per funded draw.
    pub amortization: AmortizationSpec,

    /// Coupon split type (Cash/PIK/Split)
    #[builder(default = CouponType::Cash)]
    #[serde(default)]
    pub coupon_type: CouponType,

    /// Upfront (arrangement or OID) fee paid on `issue_date`, as an amount or
    /// a fraction of the commitment (the DDTL `commitment`, else
    /// `notional_limit`).
    pub upfront_fee: Option<UpfrontFee>,

    /// Optional DDTL parameters; None => plain term loan
    pub ddtl: Option<DdtlSpec>,

    /// Optional covenant spec
    pub covenants: Option<TermLoanCovenantEvents>,

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

    /// Optional EIR amortization settings for reporting schedules
    pub oid_eir: Option<OidEirSpec>,

    /// Optional call schedule (borrower callability)
    pub call_schedule: Option<LoanCallSchedule>,

    /// Settlement days (T+n) used as the valuation/accrued anchor. Default is 2.
    ///
    /// Note: the LSTA target settlement for par/near-par secondary loan trades
    /// is T+7 (with delayed compensation beyond T+7, and T+20 for distressed).
    /// The T+2 default here is a pricing-anchor choice, not an LSTA
    /// convention; set `settlement_days: 7` to anchor at the LSTA par-trade
    /// target.
    #[builder(default = 2)]
    #[serde(default = "default_settlement_days")]
    pub settlement_days: u32,

    /// Attributes for tagging and scenarios
    pub attributes: Attributes,
}

impl TermLoan {
    /// Validate the complete loan contract, including DDTL and event schedules.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        let context = format!("Term loan '{}'", self.id.as_str());
        if self.issue_date >= self.maturity {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context} issue_date must precede maturity"
            )));
        }
        self.validate_money(self.notional_limit, "notional_limit", true)?;
        if self.discount_curve_id.as_str().trim().is_empty()
            || self
                .credit_curve_id
                .as_ref()
                .is_some_and(|id| id.as_str().trim().is_empty())
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context} curve identifiers cannot be empty"
            )));
        }
        if let Some(calendar_id) = self.calendar_id.as_deref() {
            calendar_by_id(calendar_id).ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "{context} unknown calendar_id '{calendar_id}'"
                ))
            })?;
        }
        if let RateSpec::Floating(rate) = &self.rate {
            rate.validate()?;
            if rate.forward_curve_id.as_str().trim().is_empty() {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "{context} floating-rate forward_curve_id cannot be empty"
                )));
            }
        }
        if let CouponType::Split {
            cash_fraction,
            pik_fraction,
        } = self.coupon_type
        {
            if cash_fraction < rust_decimal::Decimal::ZERO
                || pik_fraction < rust_decimal::Decimal::ZERO
                || cash_fraction > rust_decimal::Decimal::ONE
                || pik_fraction > rust_decimal::Decimal::ONE
                || (cash_fraction + pik_fraction - rust_decimal::Decimal::ONE).abs()
                    > rust_decimal::Decimal::new(1, 9)
            {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "{context} coupon split fractions must be in [0, 1] and sum to one"
                )));
            }
        }
        match &self.upfront_fee {
            Some(UpfrontFee::Amount(fee)) => self.validate_money(*fee, "upfront_fee", false)?,
            Some(UpfrontFee::FractionOfCommitment(fraction))
                if !(fraction.is_finite() && (0.0..=1.0).contains(fraction)) =>
            {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "{context} upfront_fee.fraction_of_commitment must be in [0, 1]"
                )));
            }
            _ => {}
        }
        self.validate_amortization(&context)?;
        if let Some(ddtl) = &self.ddtl {
            self.validate_ddtl(ddtl, &context)?;
        }
        if let Some(covenants) = &self.covenants {
            self.validate_covenants(covenants, &context)?;
        }
        if let Some(schedule) = &self.call_schedule {
            for call in &schedule.calls {
                if call.date < self.issue_date || call.date >= self.maturity {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "{context} call dates must lie in [issue, maturity)"
                    )));
                }
                if !call.price_pct_of_par.is_finite() || call.price_pct_of_par <= 0.0 {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "{context} call prices must be positive and finite"
                    )));
                }
                if let super::spec::LoanCallType::MakeWhole(spec) = &call.call_type {
                    if !(spec.spread_bp.is_finite() && spec.spread_bp >= 0.0)
                        || spec.reference_curve_id.as_str().trim().is_empty()
                    {
                        return Err(finstack_quant_core::Error::Validation(format!(
                            "{context} call_schedule make_whole spread_bp must be finite and \
                             non-negative with a non-empty reference_curve_id"
                        )));
                    }
                }
            }
            if schedule
                .calls
                .windows(2)
                .any(|calls| calls[0].date >= calls[1].date)
            {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "{context} call dates must be strictly increasing"
                )));
            }
        }
        let calendar_id = self
            .calendar_id
            .as_deref()
            .unwrap_or(crate::cashflow::builder::calendar::WEEKENDS_ONLY_ID);
        let periods = crate::cashflow::builder::periods::build_periods(
            crate::cashflow::builder::periods::BuildPeriodsParams {
                start: self.issue_date,
                end: self.maturity,
                frequency: self.frequency,
                stub: self.stub,
                business_day_convention: self.business_day_convention,
                calendar_id,
                end_of_month: false,
                day_count: self.day_count,
                payment_lag_days: 0,
                reset_lag_days: None,
                adjust_accrual_dates: false,
                roll_rule: crate::cashflow::builder::specs::RollRule::None,
            },
        )?;
        if periods.is_empty() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context} must generate at least one payment period"
            )));
        }
        Ok(())
    }

    fn validate_money(
        &self,
        money: Money,
        field: &str,
        strictly_positive: bool,
    ) -> finstack_quant_core::Result<()> {
        let amount = money.amount();
        if money.currency() != self.currency
            || !amount.is_finite()
            || (strictly_positive && amount <= 0.0)
            || (!strictly_positive && amount < 0.0)
        {
            let sign = if strictly_positive {
                "positive"
            } else {
                "non-negative"
            };
            return Err(finstack_quant_core::Error::Validation(format!(
                "Term loan '{}' {field} must be {sign}, finite, and denominated in {}",
                self.id.as_str(),
                self.currency
            )));
        }
        Ok(())
    }

    fn validate_amortization(&self, context: &str) -> finstack_quant_core::Result<()> {
        match &self.amortization {
            AmortizationSpec::None => {}
            AmortizationSpec::LinearTo { .. } | AmortizationSpec::StepRemaining { .. } => {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "{context} amortization supports none, linear_between, \
                     percent_of_original_per_period, percent_of_remaining_per_period and \
                     custom_principal"
                )));
            }
            AmortizationSpec::LinearBetween { start, end } => {
                if *start < self.issue_date || *start >= *end || *end > self.maturity {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "{context} linear amortization dates must lie in loan life and be ordered"
                    )));
                }
            }
            AmortizationSpec::PercentOfRemainingPerPeriod { pct }
            | AmortizationSpec::PercentOfOriginalPerPeriod { pct } => {
                if !(pct.is_finite() && (0.0..=1.0).contains(pct)) {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "{context} amortization pct must be a decimal in [0, 1]"
                    )));
                }
            }
            AmortizationSpec::CustomPrincipal { items } => {
                let mut total = 0.0;
                for (date, amount) in items {
                    if *date <= self.issue_date || *date > self.maturity {
                        return Err(finstack_quant_core::Error::Validation(format!(
                            "{context} custom amortization dates must lie in (issue, maturity]"
                        )));
                    }
                    self.validate_money(*amount, "custom amortization", true)?;
                    total += amount.amount();
                }
                if items.windows(2).any(|items| items[0].0 >= items[1].0)
                    || total > self.notional_limit.amount() + 1e-6
                {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "{context} custom amortization must be strictly ordered and cannot exceed notional"
                    )));
                }
            }
        }
        Ok(())
    }

    fn validate_ddtl(&self, ddtl: &DdtlSpec, context: &str) -> finstack_quant_core::Result<()> {
        self.validate_money(ddtl.commitment, "ddtl.commitment", true)?;
        if self.notional_limit.amount() > ddtl.commitment.amount() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context} notional_limit cannot exceed ddtl.commitment"
            )));
        }
        if ddtl.availability_start < self.issue_date
            || ddtl.availability_start > ddtl.availability_end
            || ddtl.availability_end > self.maturity
            || ddtl.usage_fee_bp < Decimal::ZERO
            || ddtl.commitment_fee_bp < Decimal::ZERO
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context} DDTL availability must lie inside the loan life and fees cannot be negative"
            )));
        }
        let mut cumulative_draws = 0.0;
        for draw in &ddtl.draws {
            if draw.date < ddtl.availability_start || draw.date > ddtl.availability_end {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "{context} DDTL draws must lie inside the availability window"
                )));
            }
            self.validate_money(draw.amount, "DDTL draw", true)?;
            cumulative_draws += draw.amount.amount();
            let effective_limit = ddtl.limit_in_force_at(draw.date).amount();
            if cumulative_draws > effective_limit + 1e-6 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "{context} cumulative DDTL draws exceed the effective commitment"
                )));
            }
        }
        if ddtl
            .draws
            .windows(2)
            .any(|draws| draws[0].date >= draws[1].date)
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context} DDTL draw dates must be strictly increasing"
            )));
        }
        let mut prior_limit = ddtl.commitment.amount();
        for step in &ddtl.commitment_steps {
            if step.date < ddtl.availability_start || step.date > ddtl.availability_end {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "{context} ddtl.commitment_steps dates must lie inside the availability window"
                )));
            }
            self.validate_money(step.amount, "ddtl.commitment_steps amount", false)?;
            if step.amount.amount() > prior_limit {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "{context} ddtl.commitment_steps amounts cannot increase"
                )));
            }
            if !step.reduction_fee_bp.is_zero() {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "{context} ddtl.commitment_steps carry no reduction fee; reduction_fee_bp must be 0"
                )));
            }
            prior_limit = step.amount.amount();
        }
        if ddtl
            .commitment_steps
            .windows(2)
            .any(|steps| steps[0].date >= steps[1].date)
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context} ddtl.commitment_steps dates must be strictly increasing"
            )));
        }
        match &ddtl.oid_policy {
            Some(
                super::spec::OidPolicy::WithheldBp(bp) | super::spec::OidPolicy::SeparateBp(bp),
            ) if *bp < Decimal::ZERO || *bp > Decimal::from(10_000) => {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "{context} ddtl.oid_policy basis points must be in [0, 10000]"
                )));
            }
            Some(
                super::spec::OidPolicy::WithheldAmount(amount)
                | super::spec::OidPolicy::SeparateAmount(amount),
            ) => {
                self.validate_money(*amount, "DDTL OID amount", false)?;
                if amount.amount() > ddtl.commitment.amount() {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "{context} DDTL OID amount cannot exceed ddtl.commitment"
                    )));
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn validate_covenants(
        &self,
        covenants: &TermLoanCovenantEvents,
        context: &str,
    ) -> finstack_quant_core::Result<()> {
        for step in &covenants.margin_steps {
            if step.date < self.issue_date
                || step.date > self.maturity
                || step.delta_bp < Decimal::ZERO
            {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "{context} covenants.margin_steps must be non-negative and inside the loan life"
                )));
            }
        }
        for toggle in &covenants.pik_toggles {
            if toggle.date < self.issue_date || toggle.date > self.maturity {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "{context} PIK toggles must lie inside the loan life"
                )));
            }
        }
        for sweep in &covenants.cash_sweeps {
            if sweep.date <= self.issue_date || sweep.date > self.maturity {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "{context} cash sweeps must lie in (issue, maturity]"
                )));
            }
            self.validate_money(sweep.amount, "cash sweep", true)?;
        }
        if covenants
            .draw_stop_dates
            .iter()
            .any(|date| *date < self.issue_date || *date > self.maturity)
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context} draw-stop dates must lie inside the loan life"
            )));
        }
        if covenants
            .margin_steps
            .windows(2)
            .any(|events| events[0].date >= events[1].date)
            || covenants
                .pik_toggles
                .windows(2)
                .any(|events| events[0].date >= events[1].date)
            || covenants
                .cash_sweeps
                .windows(2)
                .any(|events| events[0].date >= events[1].date)
            || covenants
                .draw_stop_dates
                .windows(2)
                .any(|dates| dates[0] >= dates[1])
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context} covenant event dates must be strictly increasing within each schedule"
            )));
        }
        Ok(())
    }

    /// Create a canonical example term loan for testing and documentation.
    ///
    /// Generates a 5-year USD term loan with:
    /// - $10M notional
    /// - 6% fixed rate
    /// - Quarterly payments
    /// - 2.5% per-period amortization
    /// - Act/360 day count
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoan;
    /// use finstack_quant_core::currency::Currency;
    ///
    /// let loan = TermLoan::example().unwrap();
    /// assert_eq!(loan.currency, Currency::USD);
    /// assert_eq!(loan.notional_limit.amount(), 10_000_000.0);
    /// ```
    pub fn example() -> finstack_quant_core::Result<Self> {
        use finstack_quant_core::dates::BusinessDayConvention;
        use finstack_quant_core::dates::StubKind;
        use time::macros::date;
        TermLoan::builder()
            .id(InstrumentId::new("TERM-LOAN-USD-5Y"))
            .currency(Currency::USD)
            .notional_limit(Money::from((10_000_000_i64, Currency::USD)))
            .issue_date(date!(2024 - 01 - 01))
            .maturity(date!(2029 - 01 - 01))
            .rate(RateSpec::Fixed { rate: 0.06 })
            .frequency(Tenor::quarterly())
            .day_count(DayCount::Act360)
            .business_day_convention(BusinessDayConvention::ModifiedFollowing)
            .calendar_id_opt(None)
            .stub(StubKind::None)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .credit_curve_id_opt(None)
            .amortization(AmortizationSpec::PercentOfRemainingPerPeriod { pct: 0.025 })
            .coupon_type(crate::cashflow::builder::specs::CouponType::Cash)
            .upfront_fee_opt(None)
            .ddtl_opt(None)
            .covenants_opt(None)
            .instrument_pricing_overrides(InstrumentPricingOverrides::default())
            .oid_eir_opt(None)
            .call_schedule_opt(None)
            .attributes(Attributes::new())
            .build()
    }

    /// Create an example floating-rate term loan with delayed-draw for testing and documentation.
    ///
    /// Returns a 7-year USD leveraged term loan with:
    /// - $20M DDTL commitment, SOFR + 400bps
    /// - Quarterly payments, Act/360
    /// - 12-month draw availability
    /// - 1% per-period amortization (of original notional)
    /// - 0% SOFR floor
    pub fn example_floating_with_ddtl() -> finstack_quant_core::Result<Self> {
        use finstack_quant_core::dates::BusinessDayConvention;
        use finstack_quant_core::dates::StubKind;
        use time::macros::date;

        let floating_rate = FloatingRateSpec {
            forward_curve_id: CurveId::new("USD-SOFR-3M"),
            spread_bp: Decimal::new(400, 0),
            gearing: Decimal::ONE,
            gearing_includes_spread: true,
            index_floor_bp: Some(Decimal::ZERO),
            all_in_floor_bp: None,
            all_in_cap_bp: None,
            index_cap_bp: None,
            overnight_index_constraints: Default::default(),
            reset_frequency: Tenor::quarterly(),
            index_tenor: None,
            reset_lag_days: 2,
            fixing_calendar_id: None,
            compounding: None,
            overnight_basis: None,
            fallback: Default::default(),
        };

        let ddtl = DdtlSpec {
            commitment: Money::from((20_000_000_i64, Currency::USD)),
            availability_start: date!(2024 - 01 - 15),
            availability_end: date!(2025 - 01 - 15),
            draws: vec![
                super::spec::DrawEvent {
                    date: date!(2024 - 04 - 15),
                    amount: Money::from((10_000_000_i64, Currency::USD)),
                },
                super::spec::DrawEvent {
                    date: date!(2024 - 07 - 15),
                    amount: Money::from((5_000_000_i64, Currency::USD)),
                },
            ],
            commitment_steps: vec![super::super::loan_terms::CommitmentStep {
                date: date!(2024 - 10 - 15),
                amount: Money::from((15_000_000_i64, Currency::USD)),
                reduction_fee_bp: Decimal::ZERO,
            }],
            usage_fee_bp: Decimal::from(25),
            commitment_fee_bp: Decimal::from(50),
            fee_base: super::spec::CommitmentFeeBase::Undrawn,
            oid_policy: None,
        };

        TermLoan::builder()
            .id(InstrumentId::new("TL-FLOAT-DDTL-7Y"))
            .currency(Currency::USD)
            .notional_limit(Money::from((20_000_000_i64, Currency::USD)))
            .issue_date(date!(2024 - 01 - 15))
            .maturity(date!(2031 - 01 - 15))
            .rate(RateSpec::Floating(floating_rate))
            .frequency(Tenor::quarterly())
            .day_count(DayCount::Act360)
            .business_day_convention(BusinessDayConvention::ModifiedFollowing)
            .calendar_id_opt(None)
            .stub(StubKind::ShortFront)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .credit_curve_id_opt(None)
            .amortization(AmortizationSpec::PercentOfOriginalPerPeriod { pct: 0.01 })
            .coupon_type(CouponType::Cash)
            .upfront_fee_opt(None)
            .ddtl_opt(Some(ddtl))
            .covenants_opt(None)
            .instrument_pricing_overrides(InstrumentPricingOverrides::default())
            .oid_eir_opt(None)
            .call_schedule_opt(None)
            .attributes(Attributes::new())
            .build()
    }

    /// Callable investment-grade term loan with make-whole and hard call schedule.
    ///
    /// $30M fixed 4.5%, 7Y, with:
    /// - Make-whole call at T+50bp for years 1-3
    /// - Soft call at 102% for years 3-5
    /// - Hard call at par from year 5
    pub fn example_callable() -> finstack_quant_core::Result<Self> {
        use finstack_quant_core::dates::BusinessDayConvention;
        use finstack_quant_core::dates::StubKind;
        use time::macros::date;

        let call_schedule = LoanCallSchedule {
            calls: vec![
                super::spec::LoanCall {
                    date: date!(2025 - 01 - 15),
                    price_pct_of_par: 100.0,
                    call_type: super::spec::LoanCallType::MakeWhole(super::spec::MakeWholeSpec {
                        reference_curve_id: CurveId::new("USD-OIS"),
                        spread_bp: 50.0,
                    }),
                },
                super::spec::LoanCall {
                    date: date!(2027 - 01 - 15),
                    price_pct_of_par: 102.0,
                    call_type: super::spec::LoanCallType::Soft,
                },
                super::spec::LoanCall {
                    date: date!(2029 - 01 - 15),
                    price_pct_of_par: 100.0,
                    call_type: super::spec::LoanCallType::Hard,
                },
            ],
        };

        TermLoan::builder()
            .id(InstrumentId::new("TL-CALLABLE-IG-7Y"))
            .currency(Currency::USD)
            .notional_limit(Money::from((30_000_000_i64, Currency::USD)))
            .issue_date(date!(2024 - 01 - 15))
            .maturity(date!(2031 - 01 - 15))
            .rate(RateSpec::Fixed { rate: 0.045 })
            .frequency(Tenor::semi_annual())
            .day_count(DayCount::Thirty360)
            .business_day_convention(BusinessDayConvention::ModifiedFollowing)
            .calendar_id_opt(None)
            .stub(StubKind::None)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .credit_curve_id_opt(Some(CurveId::new("USD-CREDIT-IG")))
            .amortization(AmortizationSpec::None)
            .coupon_type(CouponType::Cash)
            .upfront_fee_opt(None)
            .ddtl_opt(None)
            .covenants_opt(None)
            .instrument_pricing_overrides(InstrumentPricingOverrides::default())
            .oid_eir_opt(None)
            .call_schedule_opt(Some(call_schedule))
            .settlement_days(2)
            .attributes(Attributes::new())
            .build()
    }

    /// Resolve settlement date from `as_of` using business-day conventions when available.
    ///
    /// If `calendar_id` is set, settlement days are treated as business days on that calendar.
    /// Otherwise, a weekends-only weekday roll is used as default behavior.
    ///
    /// # Arguments
    ///
    /// * `as_of` - Valuation date from which the nonnegative settlement-day lag is counted.
    ///
    /// # Errors
    ///
    /// Returns an error if the settlement-day count exceeds the signed offset range,
    /// the shifted date exceeds the calendar, or the calendar cannot be resolved.
    pub fn settlement_date(&self, as_of: Date) -> finstack_quant_core::Result<Date> {
        if self.settlement_days == 0 {
            return Ok(as_of);
        }
        let settlement_days = i32::try_from(self.settlement_days).map_err(|_| {
            finstack_quant_core::Error::Validation("settlement days exceed supported range".into())
        })?;

        if let Some(calendar_id) = &self.calendar_id {
            let calendar = calendar_by_id(calendar_id).ok_or_else(|| {
                finstack_quant_core::Error::Input(InputError::NotFound {
                    id: format!("calendar:{}", calendar_id),
                })
            })?;
            return as_of.add_business_days(settlement_days, calendar);
        }

        as_of.add_weekdays(settlement_days)
    }

    /// Accrual configuration for settlement accrued-interest calculations.
    ///
    /// Mirrors `Bond::accrual_config`: linear accrual with the loan's coupon
    /// frequency (required for ACT/ACT ISMA day counts). PIK is excluded
    /// because capitalized interest accretes to the outstanding balance rather
    /// than being paid to the seller as cash accrued.
    pub fn accrual_config(&self) -> crate::cashflow::accrual::AccrualConfig {
        crate::cashflow::accrual::AccrualConfig {
            method: crate::cashflow::accrual::AccrualMethod::Linear,
            ex_coupon: None,
            include_pik: false,
            frequency: Some(self.frequency),
        }
    }
}

impl crate::instruments::common_impl::traits::Instrument for TermLoan {
    impl_instrument_base!(crate::pricer::InstrumentType::TermLoan);

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        self.validate()
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<
        crate::instruments::common_impl::dependencies::MarketDependencies,
    > {
        let mut deps = crate::instruments::common_impl::dependencies::MarketDependencies::new();
        deps.add_discount_curve(self.discount_curve_id.clone());
        if let Some(credit_curve_id) = &self.credit_curve_id {
            deps.add_credit_curve(credit_curve_id.clone());
        }
        if let Some(schedule) = &self.call_schedule {
            for call in &schedule.calls {
                if let super::spec::LoanCallType::MakeWhole(spec) = &call.call_type {
                    deps.add_discount_curve(spec.reference_curve_id.clone());
                }
            }
        }
        if let RateSpec::Floating(spec) = &self.rate {
            deps.add_forward_curve(spec.forward_curve_id.clone());
            deps.add_series_id(finstack_quant_core::market_data::fixings::fixing_series_id(
                spec.forward_curve_id.as_str(),
            ));
        }
        Ok(deps)
    }

    fn default_model(&self) -> crate::pricer::ModelKey {
        // A supplied hazard curve is part of the economic contract even for
        // non-callable loans. Route those loans through the rates+credit tree
        // so survival and recovery affect PV and credit sensitivities.
        if self.credit_curve_id.is_some() {
            return crate::pricer::ModelKey::Tree;
        }
        if self
            .call_schedule
            .as_ref()
            .is_some_and(|cs| !cs.calls.is_empty())
        {
            return crate::pricer::ModelKey::Tree;
        }
        crate::pricer::ModelKey::Discounting
    }

    fn base_value(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<Money> {
        match self.default_model() {
            crate::pricer::ModelKey::Tree => crate::instruments::fixed_income::term_loan::pricing::TermLoanTreePricer::new()
                .price_callable(self, curves, as_of),
            crate::pricer::ModelKey::Discounting => {
                crate::instruments::fixed_income::term_loan::pricing::TermLoanDiscountingPricer::price(
                    self, curves, as_of,
                )
            }
            model => Err(finstack_quant_core::Error::Validation(format!(
                "TermLoan '{}' has no pricer for model {model:?}",
                self.id
            ))),
        }
    }

    fn effective_start_date(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.issue_date)
    }

    crate::impl_focused_pricing_overrides!();
}

impl crate::cashflow::traits::CashflowScheduleSource for TermLoan {
    fn notional(&self) -> finstack_quant_core::Result<Option<finstack_quant_core::money::Money>> {
        Ok(Some(self.notional_limit))
    }

    fn raw_cashflow_schedule(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        _as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<crate::cashflow::builder::CashFlowSchedule> {
        let schedule = crate::instruments::fixed_income::term_loan::cashflows::generate_cashflows(
            self, curves,
        )?;

        Ok(schedule
            .with_representation(crate::cashflow::builder::CashflowRepresentation::Projected))
    }
}

impl finstack_quant_covenants::InstrumentMutator for TermLoan {
    fn set_default_status(
        &mut self,
        is_default: bool,
        _as_of: Date,
    ) -> finstack_quant_core::Result<()> {
        if is_default {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Term loan '{}' cannot apply a realized default consequence without explicit recovery amount and settlement date",
                self.id.as_str()
            )));
        }
        self.attributes.meta.remove("defaulted");
        self.attributes.meta.remove("default_date");
        Ok(())
    }

    fn increase_rate(&mut self, increase: f64) -> finstack_quant_core::Result<()> {
        // Covenant margin increases apply in whole basis points.
        let bp_increase = Decimal::from((increase * 10_000.0).round() as i64);
        match &mut self.rate {
            RateSpec::Fixed { rate } => {
                let stepped = finstack_quant_core::decimal::f64_to_decimal(*rate)?
                    + bp_increase / Decimal::from(10_000);
                *rate = finstack_quant_core::decimal::decimal_to_f64(stepped)?;
            }
            RateSpec::Floating(spec) => {
                spec.spread_bp += bp_increase;
            }
        }
        self.validate()
    }

    fn set_cash_sweep(&mut self, percentage: f64) -> finstack_quant_core::Result<()> {
        if percentage != 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Term loan '{}' cannot apply a percentage cash sweep without a dated excess-cash-flow amount; use a typed CashSweepEvent",
                self.id.as_str()
            )));
        }
        self.attributes.meta.remove("cash_sweep_pct");
        Ok(())
    }

    fn set_distribution_block(&mut self, blocked: bool) -> finstack_quant_core::Result<()> {
        self.attributes
            .meta
            .insert("distributions_blocked".to_string(), blocked.to_string());
        Ok(())
    }

    fn require_collateral(
        &mut self,
        _description: &str,
        _as_of: Date,
    ) -> finstack_quant_core::Result<()> {
        Err(finstack_quant_core::Error::Validation(format!(
            "Term loan '{}' cannot execute a collateral requirement without a typed collateral agreement", self.id.as_str()
        )))
    }

    fn set_maturity(&mut self, new_maturity: Date) -> finstack_quant_core::Result<()> {
        // Re-validate: shortening or extending maturity can strand
        // amortization, call, covenant, or DDTL dates outside the loan life.
        self.maturity = new_maturity;
        self.validate()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::dates::Date;
    use time::Month;

    #[test]
    fn date_offset_rejects_wrapping_settlement_days() {
        let mut loan = TermLoan::example().expect("example loan");
        loan.settlement_days = u32::MAX;
        for calendar in [None, Some("weekends_only".into())] {
            loan.calendar_id = calendar;
            assert!(loan.settlement_date(loan.issue_date).is_err());
        }
    }

    #[test]
    fn unsupported_realized_covenant_consequences_fail_explicitly() {
        use finstack_quant_covenants::InstrumentMutator;

        let mut loan = TermLoan::example().expect("example loan");
        let as_of = Date::from_calendar_date(2025, Month::January, 2).expect("date");

        let default_error = loan
            .set_default_status(true, as_of)
            .expect_err("default without settlement economics must fail");
        assert!(default_error.to_string().contains("recovery amount"));
        assert!(!loan.attributes.meta.contains_key("defaulted"));

        let sweep_error = loan
            .set_cash_sweep(1.0)
            .expect_err("percentage sweep without a cash-flow base must fail");
        assert!(sweep_error.to_string().contains("CashSweepEvent"));
        assert!(!loan.attributes.meta.contains_key("cash_sweep_pct"));
    }

    #[test]
    fn floating_term_loan_uses_the_canonical_fixing_series_id() {
        let loan = TermLoan::example_floating_with_ddtl().expect("floating example");
        let RateSpec::Floating(spec) = &loan.rate else {
            unreachable!("floating example must use a floating rate");
        };
        let expected = finstack_quant_core::market_data::fixings::fixing_series_id(
            spec.forward_curve_id.as_str(),
        );

        let deps =
            crate::instruments::Instrument::market_dependencies(&loan).expect("dependencies");
        assert_eq!(deps.series_ids, vec![expected]);
    }
}
