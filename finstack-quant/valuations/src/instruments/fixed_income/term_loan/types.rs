//! Term loan instrument type and core specifications.
//!
//! This module defines the [`TermLoan`] instrument type and its associated specifications
//! including rate types, trait implementations, and conversion from [`TermLoanSpec`].
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
//! assert_eq!(loan.notional_limit, Money::new(10_000_000.0, Currency::USD));
//! # Ok(())
//! # }
//! ```
//!
//! # See Also
//!
//! - [`TermLoanSpec`] for the serializable specification type
//! - [`RateSpec`] for rate type definitions
//! - [`super::spec`] module for all specification types

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{
    calendar::calendar_by_id, BusinessDayConvention, Date, DateExt, DayCount, StubKind, Tenor,
};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{Bps, CurveId, InstrumentId, Rate};
use finstack_quant_core::InputError;

use super::spec::{
    AmortizationSpec, DdtlSpec, LoanCallSchedule, OidEirSpec, TermLoanCovenantEvents, TermLoanSpec,
};
use crate::cashflow::builder::specs::CouponType;
use crate::cashflow::builder::FloatingRateSpec;
use crate::impl_instrument_base;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::pricing_overrides::InstrumentPricingOverrides;

fn default_settlement_days() -> u32 {
    2
}

/// Rate specification for term loans.
///
///  Defines whether the loan uses fixed or floating rate interest, with full
/// support for floating rate features including floors, caps, and leverage.
///
/// # Variants
///
/// - [`Fixed`](RateSpec::Fixed): Constant rate specified in basis points
/// - [`Floating`](RateSpec::Floating): Index-based rate with spread and optional limits
///
/// # Examples
///
/// Fixed rate loan:
/// ```rust
/// use finstack_quant_valuations::instruments::fixed_income::term_loan::RateSpec;
///
/// let fixed_rate = RateSpec::Fixed { rate_bp: 600 };  // 6% fixed
/// ```
///
/// Floating rate with floor:
/// ```rust
/// use finstack_quant_valuations::instruments::fixed_income::term_loan::RateSpec;
/// use finstack_quant_cashflows::builder::FloatingRateSpec;
/// use finstack_quant_core::dates::Tenor;
/// use finstack_quant_core::types::CurveId;
/// use rust_decimal_macros::dec;
///
/// let floating = RateSpec::Floating(FloatingRateSpec {
///     index_id: CurveId::new("USD-SOFR-3M"),
///     spread_bp: dec!(300),     // +300 bps spread
///     gearing: dec!(1),
///     gearing_includes_spread: true,
///     index_floor_bp: Some(dec!(0)),  // 0% floor
///     all_in_floor_bp: None,
///     all_in_cap_bp: None,
///     index_cap_bp: None,
///     overnight_index_constraints: Default::default(),
///     reset_freq: Tenor::quarterly(),
///     index_tenor: None,
///     reset_lag_days: 2,
///     fixing_calendar_id: None,
///     overnight_compounding: None,
///     overnight_basis: None,
///     fallback: Default::default(),
/// });
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[allow(clippy::large_enum_variant)]
#[non_exhaustive]
pub enum RateSpec {
    /// Fixed annual rate in basis points
    Fixed {
        /// Fixed rate in basis points (e.g., 600 = 6%)
        rate_bp: i32,
    },

    /// Floating rate using canonical FloatingRateSpec.
    ///
    /// Uses the standard floating rate specification with full support
    /// for floors, caps, gearing, and reset conventions.
    ///
    /// **Note on calendars**: The `FloatingRateSpec.calendar_id` field is ignored
    /// for term loans. The loan-level `TermLoan::calendar_id` drives the payment
    /// schedule and business-day adjustments. Only `index_id`, `spread_bp`,
    /// `gearing`, `index_floor_bp`, `all_in_cap_bp`, and `reset_lag_days` are used from this
    /// specification.
    Floating(FloatingRateSpec),
}

impl RateSpec {
    /// Create a fixed-rate spec using typed basis points.
    pub fn fixed_bps(rate: Bps) -> Self {
        Self::Fixed {
            rate_bp: rate.as_bps(),
        }
    }

    /// Create a fixed-rate spec using a typed rate.
    pub fn fixed_rate(rate: Rate) -> Self {
        Self::Fixed {
            rate_bp: rate.as_bps(),
        }
    }
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
/// Create via [`TermLoanSpec`] conversion or use the builder pattern:
///
/// ```ignore
/// use finstack_quant_valuations::instruments::fixed_income::term_loan::spec::TermLoanSpec;
/// use finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoan;
///
/// # fn example(spec: TermLoanSpec) -> Result<(), Box<dyn std::error::Error>> {
/// let loan: TermLoan = spec.try_into()?;
/// # let _ = loan;
/// # Ok(())
/// # }
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
    finstack_quant_valuations_macros::FocusedPricingOverrides,
)]
#[serde(deny_unknown_fields)]
pub struct TermLoan {
    /// Unique instrument identifier
    pub id: InstrumentId,

    /// Currency for all cashflows
    pub currency: Currency,

    /// Maximum commitment / notional limit
    pub notional_limit: Money,

    /// Issue (effective) date
    #[schemars(with = "String")]
    pub issue_date: Date,

    /// Maturity date
    #[schemars(with = "String")]
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
    pub bdc: BusinessDayConvention,

    /// Optional calendar id for adjustments
    pub calendar_id: Option<String>,

    /// Stub rule
    #[builder(default = StubKind::ShortFront)]
    #[serde(default = "crate::serde_defaults::stub_short_front")]
    pub stub: StubKind,

    /// Discount curve identifier
    pub discount_curve_id: CurveId,

    /// Optional credit curve identifier (defaults to discount_curve_id if None)
    pub credit_curve_id: Option<CurveId>,

    /// Amortization specification
    pub amortization: AmortizationSpec,

    /// Coupon split type (Cash/PIK/Split)
    #[builder(default = CouponType::Cash)]
    #[serde(default)]
    pub coupon_type: CouponType,

    /// Upfront fee at issue (if any)
    pub upfront_fee: Option<Money>,

    /// Optional DDTL parameters; None => plain term loan
    pub ddtl: Option<DdtlSpec>,

    /// Optional covenant spec
    pub covenants: Option<TermLoanCovenantEvents>,

    /// Pricing overrides (quoted price, seed, etc.)
    #[builder(default)]
    #[serde(default)]
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

    /// Optional EIR amortization settings for reporting schedules
    pub oid_eir: Option<OidEirSpec>,

    /// Optional call schedule (borrower callability)
    pub call_schedule: Option<LoanCallSchedule>,

    /// Settlement days (T+n). Default is 2 for leveraged loans per LSTA conventions.
    ///
    /// LSTA standard for secondary market loan trades is T+2 (effective since 2023).
    /// Primary market trades may use different conventions.
    #[builder(default = 2)]
    #[serde(default = "default_settlement_days")]
    pub settlement_days: u32,

    /// Attributes for tagging and scenarios
    pub attributes: Attributes,
}

impl TermLoan {
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
            .notional_limit(Money::new(10_000_000.0, Currency::USD))
            .issue_date(date!(2024 - 01 - 01))
            .maturity(date!(2029 - 01 - 01))
            .rate(RateSpec::Fixed { rate_bp: 600 }) // 6%
            .frequency(Tenor::quarterly())
            .day_count(DayCount::Act360)
            .bdc(BusinessDayConvention::ModifiedFollowing)
            .calendar_id_opt(None)
            .stub(StubKind::None)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .credit_curve_id_opt(None)
            .amortization(super::spec::AmortizationSpec::PercentPerPeriod { bp: 250 }) // 2.5% per period
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
    /// - $50M notional, SOFR + 400bps
    /// - Quarterly payments, Act/360
    /// - $20M DDTL commitment with 12-month availability
    /// - 1% per-period amortization (of original notional)
    /// - 0% SOFR floor
    pub fn example_floating_with_ddtl() -> finstack_quant_core::Result<Self> {
        use finstack_quant_core::dates::BusinessDayConvention;
        use finstack_quant_core::dates::StubKind;
        use rust_decimal::Decimal;
        use time::macros::date;

        let floating_rate = FloatingRateSpec {
            index_id: CurveId::new("USD-SOFR-3M"),
            spread_bp: Decimal::new(400, 0),
            gearing: Decimal::ONE,
            gearing_includes_spread: true,
            index_floor_bp: Some(Decimal::ZERO),
            all_in_floor_bp: None,
            all_in_cap_bp: None,
            index_cap_bp: None,
            overnight_index_constraints: Default::default(),
            reset_freq: Tenor::quarterly(),
            index_tenor: None,
            reset_lag_days: 2,
            fixing_calendar_id: None,
            overnight_compounding: None,
            overnight_basis: None,
            fallback: Default::default(),
        };

        let ddtl = DdtlSpec {
            commitment_limit: Money::new(20_000_000.0, Currency::USD),
            availability_start: date!(2024 - 01 - 15),
            availability_end: date!(2025 - 01 - 15),
            draws: vec![
                super::spec::DrawEvent {
                    date: date!(2024 - 04 - 15),
                    amount: Money::new(10_000_000.0, Currency::USD),
                },
                super::spec::DrawEvent {
                    date: date!(2024 - 07 - 15),
                    amount: Money::new(5_000_000.0, Currency::USD),
                },
            ],
            commitment_step_downs: vec![super::spec::CommitmentStepDown {
                date: date!(2024 - 10 - 15),
                new_limit: Money::new(15_000_000.0, Currency::USD),
            }],
            usage_fee_bp: 25,
            commitment_fee_bp: 50,
            fee_base: super::spec::CommitmentFeeBase::Undrawn,
            oid_policy: None,
        };

        TermLoan::builder()
            .id(InstrumentId::new("TL-FLOAT-DDTL-7Y"))
            .currency(Currency::USD)
            .notional_limit(Money::new(50_000_000.0, Currency::USD))
            .issue_date(date!(2024 - 01 - 15))
            .maturity(date!(2031 - 01 - 15))
            .rate(RateSpec::Floating(floating_rate))
            .frequency(Tenor::quarterly())
            .day_count(DayCount::Act360)
            .bdc(BusinessDayConvention::ModifiedFollowing)
            .calendar_id_opt(None)
            .stub(StubKind::ShortFront)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .credit_curve_id_opt(None)
            .amortization(AmortizationSpec::PercentOfOriginalNotional { bp: 100 })
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

    /// Create an example term loan with covenants and call schedule for testing and documentation.
    ///
    /// Returns a 5-year USD fixed-rate term loan with:
    /// - $25M notional, 6% fixed rate
    /// - Quarterly payments, Act/360
    /// - Margin step-ups at years 2 and 3 (+25bp each)
    /// - Cash sweep at year 3
    /// - Soft call at 102, hard call at 101
    pub fn example_with_covenants() -> finstack_quant_core::Result<Self> {
        use finstack_quant_core::dates::BusinessDayConvention;
        use finstack_quant_core::dates::StubKind;
        use time::macros::date;

        let covenants = TermLoanCovenantEvents {
            margin_stepups: vec![
                super::spec::MarginStepUp {
                    date: date!(2026 - 01 - 15),
                    delta_bp: 25,
                },
                super::spec::MarginStepUp {
                    date: date!(2027 - 01 - 15),
                    delta_bp: 25,
                },
            ],
            pik_toggles: vec![],
            cash_sweeps: vec![super::spec::CashSweepEvent {
                date: date!(2027 - 01 - 15),
                amount: Money::new(2_500_000.0, Currency::USD),
            }],
            draw_stop_dates: vec![],
        };

        let call_schedule = LoanCallSchedule {
            calls: vec![
                super::spec::LoanCall {
                    date: date!(2025 - 07 - 15),
                    price_pct_of_par: 102.0,
                    call_type: super::spec::LoanCallType::Soft,
                },
                super::spec::LoanCall {
                    date: date!(2026 - 07 - 15),
                    price_pct_of_par: 101.0,
                    call_type: super::spec::LoanCallType::Hard,
                },
            ],
        };

        TermLoan::builder()
            .id(InstrumentId::new("TL-COVENANT-5Y"))
            .currency(Currency::USD)
            .notional_limit(Money::new(25_000_000.0, Currency::USD))
            .issue_date(date!(2024 - 01 - 15))
            .maturity(date!(2029 - 01 - 15))
            .rate(RateSpec::Fixed { rate_bp: 600 })
            .frequency(Tenor::quarterly())
            .day_count(DayCount::Act360)
            .bdc(BusinessDayConvention::ModifiedFollowing)
            .calendar_id_opt(None)
            .stub(StubKind::None)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .credit_curve_id_opt(None)
            .amortization(AmortizationSpec::None)
            .coupon_type(CouponType::Cash)
            .upfront_fee_opt(None)
            .ddtl_opt(None)
            .covenants_opt(Some(covenants))
            .instrument_pricing_overrides(InstrumentPricingOverrides::default())
            .oid_eir_opt(None)
            .call_schedule_opt(Some(call_schedule))
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
                    call_type: super::spec::LoanCallType::MakeWhole {
                        treasury_spread_bp: 50,
                    },
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
            .notional_limit(Money::new(30_000_000.0, Currency::USD))
            .issue_date(date!(2024 - 01 - 15))
            .maturity(date!(2031 - 01 - 15))
            .rate(RateSpec::Fixed { rate_bp: 450 })
            .frequency(Tenor::semi_annual())
            .day_count(DayCount::Thirty360)
            .bdc(BusinessDayConvention::ModifiedFollowing)
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
    pub fn settlement_date(&self, as_of: Date) -> finstack_quant_core::Result<Date> {
        if self.settlement_days == 0 {
            return Ok(as_of);
        }

        if let Some(calendar_id) = &self.calendar_id {
            let calendar = calendar_by_id(calendar_id).ok_or_else(|| {
                finstack_quant_core::Error::Input(InputError::NotFound {
                    id: format!("calendar:{}", calendar_id),
                })
            })?;
            return as_of.add_business_days(self.settlement_days as i32, calendar);
        }

        Ok(as_of.add_weekdays(self.settlement_days as i32))
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

impl TryFrom<TermLoanSpec> for TermLoan {
    type Error = finstack_quant_core::Error;

    fn try_from(spec: TermLoanSpec) -> Result<Self, Self::Error> {
        if spec.issue >= spec.maturity {
            return Err(InputError::InvalidDateRange.into());
        }

        let TermLoanSpec {
            id,
            discount_curve_id,
            credit_curve_id,
            currency,
            notional_limit,
            issue,
            maturity,
            rate,
            frequency,
            day_count,
            bdc,
            calendar_id,
            stub,
            amortization,
            coupon_type,
            upfront_fee,
            ddtl,
            covenants,
            instrument_pricing_overrides,
            metric_pricing_overrides,
            scenario_pricing_overrides,
            oid_eir,
            call_schedule,
            settlement_days,
        } = spec;

        let resolved_notional = match (notional_limit, ddtl.as_ref()) {
            (Some(limit), _) => limit,
            (None, Some(ddtl_spec)) => ddtl_spec.commitment_limit,
            (None, None) => {
                return Err(InputError::NotFound {
                    id: "notional_limit".to_string(),
                }
                .into())
            }
        };

        validate_currency(currency, resolved_notional)?;
        if let Some(fee) = upfront_fee.as_ref() {
            validate_currency(currency, *fee)?;
        }

        if let AmortizationSpec::Custom(items) = &amortization {
            for (_, amt) in items {
                validate_currency(currency, *amt)?;
            }
        }

        if let Some(cov) = &covenants {
            for sweep in &cov.cash_sweeps {
                validate_currency(currency, sweep.amount)?;
            }
        }

        if let Some(ddtl_spec) = &ddtl {
            validate_currency(currency, ddtl_spec.commitment_limit)?;
            if resolved_notional.amount() > ddtl_spec.commitment_limit.amount() {
                return Err(InputError::Invalid.into());
            }
            for draw in &ddtl_spec.draws {
                validate_currency(currency, draw.amount)?;
            }
            // Validate cumulative draws do not exceed commitment limit (accounting
            // for step-downs).  Only valid draws within the availability window
            // are considered, mirroring the cashflow generator's filtering logic.
            {
                let mut sorted_draws: Vec<_> = ddtl_spec
                    .draws
                    .iter()
                    .filter(|d| {
                        d.date >= ddtl_spec.availability_start
                            && d.date <= ddtl_spec.availability_end
                    })
                    .collect();
                sorted_draws.sort_by_key(|d| d.date);

                let mut cumulative = 0.0_f64;
                for draw in &sorted_draws {
                    // Determine effective limit at draw date (after step-downs)
                    let mut limit = ddtl_spec.commitment_limit.amount();
                    for sd in &ddtl_spec.commitment_step_downs {
                        if sd.date <= draw.date {
                            limit = sd.new_limit.amount();
                        }
                    }
                    cumulative += draw.amount.amount();
                    if cumulative > limit + 1e-6 {
                        return Err(InputError::Invalid.into());
                    }
                }
            }
            for step in &ddtl_spec.commitment_step_downs {
                validate_currency(currency, step.new_limit)?;
            }
            match &ddtl_spec.oid_policy {
                Some(
                    super::spec::OidPolicy::WithheldAmount(m)
                    | super::spec::OidPolicy::SeparateAmount(m),
                ) => {
                    validate_currency(currency, *m)?;
                }
                Some(
                    super::spec::OidPolicy::WithheldPct(bp)
                    | super::spec::OidPolicy::SeparatePct(bp),
                ) => {
                    if *bp < 0 {
                        return Err(InputError::Invalid.into());
                    }
                }
                None => {}
            }
        }

        TermLoan::builder()
            .id(id)
            .currency(currency)
            .notional_limit(resolved_notional)
            .issue_date(issue)
            .maturity(maturity)
            .rate(rate)
            .frequency(frequency)
            .day_count(day_count)
            .bdc(bdc)
            .calendar_id_opt(calendar_id)
            .stub(stub)
            .discount_curve_id(discount_curve_id)
            .credit_curve_id_opt(credit_curve_id)
            .amortization(amortization)
            .coupon_type(coupon_type)
            .upfront_fee_opt(upfront_fee)
            .ddtl_opt(ddtl)
            .covenants_opt(covenants)
            .instrument_pricing_overrides(instrument_pricing_overrides)
            .metric_pricing_overrides(metric_pricing_overrides)
            .scenario_pricing_overrides(scenario_pricing_overrides)
            .oid_eir_opt(oid_eir)
            .call_schedule_opt(call_schedule)
            .settlement_days(settlement_days)
            .attributes(Attributes::new())
            .build()
    }
}

fn validate_currency(expected: Currency, money: Money) -> Result<(), finstack_quant_core::Error> {
    if money.currency() != expected {
        return Err(InputError::Invalid.into());
    }
    Ok(())
}

impl crate::instruments::common_impl::traits::Instrument for TermLoan {
    impl_instrument_base!(crate::pricer::InstrumentType::TermLoan);

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
        if let RateSpec::Floating(spec) = &self.rate {
            deps.add_forward_curve(spec.index_id.clone());
            deps.add_series_id(finstack_quant_core::market_data::fixings::fixing_series_id(
                spec.index_id.as_str(),
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
        if let Some(ref cs) = self.call_schedule {
            let has_exercisable = cs
                .calls
                .iter()
                .any(|c| !matches!(c.call_type, super::spec::LoanCallType::MakeWhole { .. }));
            if has_exercisable {
                return crate::pricer::ModelKey::Tree;
            }
        }
        crate::pricer::ModelKey::Discounting
    }

    fn base_value(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<Money> {
        // If the loan has exercisable call options (Hard/Soft), use tree-based
        // pricing to capture optionality with frictional exercise.
        // MakeWhole calls are non-economic by design and do not require a tree.
        if self.credit_curve_id.is_some() {
            return crate::instruments::fixed_income::term_loan::pricing::TermLoanTreePricer::new()
                .price_callable(self, curves, as_of);
        }
        if let Some(ref cs) = self.call_schedule {
            let has_exercisable = cs
                .calls
                .iter()
                .any(|c| !matches!(c.call_type, super::spec::LoanCallType::MakeWhole { .. }));
            if has_exercisable {
                return crate::instruments::fixed_income::term_loan::pricing::TermLoanTreePricer::new(
                )
                .price_callable(self, curves, as_of);
            }
        }

        // Otherwise delegate to deterministic discounting pricer.
        crate::instruments::fixed_income::term_loan::pricing::TermLoanDiscountingPricer::price(
            self, curves, as_of,
        )
    }

    fn effective_start_date(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.issue_date)
    }

    crate::impl_focused_pricing_overrides!();
}

impl crate::cashflow::traits::CashflowScheduleSource for TermLoan {
    fn notional(&self) -> Option<finstack_quant_core::money::Money> {
        Some(self.notional_limit)
    }

    fn raw_cashflow_schedule(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<crate::cashflow::builder::CashFlowSchedule> {
        let schedule = crate::instruments::fixed_income::term_loan::cashflows::generate_cashflows(
            self, curves, as_of,
        )?;

        Ok(schedule
            .with_representation(crate::cashflow::builder::CashflowRepresentation::Projected))
    }
}

impl finstack_quant_covenants::InstrumentMutator for TermLoan {
    fn set_default_status(
        &mut self,
        is_default: bool,
        as_of: Date,
    ) -> finstack_quant_core::Result<()> {
        self.attributes
            .meta
            .insert("defaulted".to_string(), is_default.to_string());
        if is_default {
            self.attributes
                .meta
                .insert("default_date".to_string(), as_of.to_string());
        }
        Ok(())
    }

    fn increase_rate(&mut self, increase: f64) -> finstack_quant_core::Result<()> {
        let bps_increase = (increase * 10_000.0).round() as i32;
        match &mut self.rate {
            RateSpec::Fixed { rate_bp } => {
                *rate_bp += bps_increase;
            }
            RateSpec::Floating(spec) => {
                spec.spread_bp += rust_decimal::Decimal::new(bps_increase as i64, 0);
            }
        }
        Ok(())
    }

    fn set_cash_sweep(&mut self, percentage: f64) -> finstack_quant_core::Result<()> {
        self.attributes
            .meta
            .insert("cash_sweep_pct".to_string(), percentage.to_string());
        Ok(())
    }

    fn set_distribution_block(&mut self, blocked: bool) -> finstack_quant_core::Result<()> {
        self.attributes
            .meta
            .insert("distributions_blocked".to_string(), blocked.to_string());
        Ok(())
    }

    fn set_maturity(&mut self, new_maturity: Date) -> finstack_quant_core::Result<()> {
        self.maturity = new_maturity;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cashflow::builder::specs::CouponType;
    use crate::instruments::fixed_income::term_loan::spec::CommitmentFeeBase;
    use finstack_quant_core::dates::Date;
    use time::Month;

    #[test]
    fn floating_term_loan_uses_the_canonical_fixing_series_id() {
        let loan = TermLoan::example_floating_with_ddtl().expect("floating example");
        let RateSpec::Floating(spec) = &loan.rate else {
            unreachable!("floating example must use a floating rate");
        };
        let expected =
            finstack_quant_core::market_data::fixings::fixing_series_id(spec.index_id.as_str());

        let deps =
            crate::instruments::Instrument::market_dependencies(&loan).expect("dependencies");
        assert_eq!(deps.series_ids, vec![expected]);
    }

    #[test]
    fn test_term_loan_spec_conversion_plain() {
        let issue = Date::from_calendar_date(2024, Month::January, 2).expect("valid date");
        let maturity = Date::from_calendar_date(2029, Month::January, 2).expect("valid date");

        let spec = TermLoanSpec {
            id: InstrumentId::new("TL-PLAIN"),
            discount_curve_id: CurveId::new("USD-CREDIT"),
            credit_curve_id: None,
            currency: Currency::USD,
            notional_limit: Some(Money::new(5_000_000.0, Currency::USD)),
            issue,
            maturity,
            rate: RateSpec::Fixed { rate_bp: 550 },
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: StubKind::None,
            amortization: AmortizationSpec::None,
            coupon_type: CouponType::Cash,
            upfront_fee: None,
            ddtl: None,
            covenants: None,
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            oid_eir: None,
            call_schedule: None,
            settlement_days: 2,
        };

        let loan: TermLoan = spec.try_into().expect("conversion should succeed");
        assert_eq!(loan.notional_limit.amount(), 5_000_000.0);
        assert_eq!(loan.currency, Currency::USD);
    }

    #[test]
    fn test_term_loan_spec_conversion_ddtl_defaults_notional() {
        let issue = Date::from_calendar_date(2025, Month::March, 1).expect("valid date");
        let maturity = Date::from_calendar_date(2030, Month::March, 1).expect("valid date");
        let commitment = Money::new(12_000_000.0, Currency::USD);

        let ddtl = DdtlSpec {
            commitment_limit: commitment,
            availability_start: issue,
            availability_end: issue,
            draws: Vec::new(),
            commitment_step_downs: Vec::new(),
            usage_fee_bp: 0,
            commitment_fee_bp: 0,
            fee_base: CommitmentFeeBase::Undrawn,
            oid_policy: None,
        };

        let spec = TermLoanSpec {
            id: InstrumentId::new("TL-DDTL"),
            discount_curve_id: CurveId::new("USD-CREDIT"),
            credit_curve_id: None,
            currency: Currency::USD,
            notional_limit: None,
            issue,
            maturity,
            rate: RateSpec::Fixed { rate_bp: 450 },
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: StubKind::None,
            amortization: AmortizationSpec::None,
            coupon_type: CouponType::Cash,
            upfront_fee: None,
            ddtl: Some(ddtl),
            covenants: None,
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            oid_eir: None,
            call_schedule: None,
            settlement_days: 2,
        };

        let loan: TermLoan = spec.try_into().expect("conversion should succeed");
        assert_eq!(loan.notional_limit, commitment);
    }

    #[test]
    fn test_term_loan_spec_conversion_missing_notional() {
        let issue = Date::from_calendar_date(2024, Month::January, 2).expect("valid date");
        let maturity = Date::from_calendar_date(2026, Month::January, 2).expect("valid date");

        let spec = TermLoanSpec {
            id: InstrumentId::new("TL-MISSING"),
            discount_curve_id: CurveId::new("USD-CREDIT"),
            credit_curve_id: None,
            currency: Currency::USD,
            notional_limit: None,
            issue,
            maturity,
            rate: RateSpec::Fixed { rate_bp: 500 },
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: StubKind::None,
            amortization: AmortizationSpec::None,
            coupon_type: CouponType::Cash,
            upfront_fee: None,
            ddtl: None,
            covenants: None,
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            oid_eir: None,
            call_schedule: None,
            settlement_days: 2,
        };

        let err = TermLoan::try_from(spec).expect_err("missing notional should fail");
        match err {
            finstack_quant_core::Error::Input(InputError::NotFound { .. }) => {}
            _ => panic!("unexpected error: {err:?}"),
        }
    }
}
