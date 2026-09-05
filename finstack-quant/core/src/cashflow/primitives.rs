//! Cashflow primitives and classification enums.
//!
//! Defines core types for representing individual cashflows.
//! These primitives are used throughout the valuations crate for
//! building instrument-specific payment schedules.
//!
//! # Types
//!
//! - [`CashFlow`]: Single dated payment with classification
//! - [`CFKind`]: Cashflow type enumeration (fixed, floating, principal, etc.)

use crate::dates::{Date, DayCount};
use crate::error::{InputError, NonFiniteKind};
use crate::money::Money;

/// Enumeration of cash-flow kinds for classification and ordering.
///
/// Used to distinguish between different types of cashflows for
/// proper sequencing, risk calculation, and accounting treatment.
///
/// # Sign Convention
///
/// The enum itself is **view agnostic**: individual instruments are
/// responsible for mapping these kinds into a holder or issuer view.
/// By convention in this crate:
///
/// | Kind | Holder View (Long) | Issuer View (Short) |
/// |------|-------------------|---------------------|
/// | Interest (Fixed/Float) | Positive (receive) | Negative (pay) |
/// | Notional (initial) | Negative (pay) | Positive (receive) |
/// | Notional (final) | Positive (receive) | Negative (pay) |
/// | Amortization | Positive (receive) | Negative (pay) |
/// | PIK | Increases notional | Increases liability |
/// | Fee | Negative (pay) | Positive (receive) |
///
/// When constructing cashflow schedules, instruments should apply the appropriate
/// sign based on the economic perspective being represented.
///
/// # Cashflow Categories
///
/// Variants are grouped by category:
/// - **Interest**: `Fixed`, `FloatReset`, `Stub`
/// - **Inflation**: `InflationCoupon`
/// - **Fees**: `Fee`, `CommitmentFee`, `UsageFee`, `FacilityFee`
/// - **Principal**: `Notional`, `PIK`, `Amortization`, `PrePayment`
/// - **Revolving**: `RevolvingDraw`, `RevolvingRepayment`
/// - **Credit Events**: `DefaultedNotional`, `Recovery`
/// - **Margin/Collateral**: `InitialMarginPost`, `VariationMarginPay`, etc.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CFKind {
    /// Fixed-rate coupon cash-flow.
    ///
    /// Periodic interest payment calculated as: `notional × rate × accrual_factor`.
    /// The `rate` field on [`CashFlow`] stores the coupon rate used.
    Fixed,

    /// Floating-rate coupon cash-flow (or index fixing event).
    ///
    /// Interest payment based on a reference rate (e.g., SOFR, EURIBOR) plus spread.
    /// The `reset_date` field on [`CashFlow`] indicates when the rate was fixed.
    /// The `rate` field stores the all-in rate (index + spread) if known.
    FloatReset,

    /// Inflation-linked coupon cash-flow.
    ///
    /// Periodic coupon payment on an inflation-linked bond or swap leg where the
    /// real coupon is multiplied by an index ratio (e.g., CPI / base CPI).
    /// The `rate` field stores the real coupon rate when known.
    InflationCoupon,

    /// Up-front fee or cost paid at inception.
    ///
    /// One-time fee paid at trade date or settlement date, such as origination fees,
    /// arrangement fees, or underwriting fees.
    Fee,

    /// Commitment fee on undrawn balance.
    ///
    /// Periodic fee charged on the undrawn portion of a revolving facility.
    /// Typically quoted in basis points per annum on the unused commitment.
    CommitmentFee,

    /// Usage fee on drawn balance.
    ///
    /// Additional fee charged when facility utilization exceeds a threshold,
    /// common in leveraged loan facilities.
    UsageFee,

    /// Facility fee on total commitment.
    ///
    /// Fee charged on the entire committed amount regardless of utilization,
    /// covering the lender's cost of keeping the facility available.
    FacilityFee,

    /// Principal exchange or notional flow.
    ///
    /// Used for initial notional payment (at inception), final notional repayment
    /// (at maturity), or intermediate notional exchanges in cross-currency swaps.
    /// For bonds: initial = purchase price, final = par redemption.
    Notional,

    /// Payment-in-kind interest capitalization.
    ///
    /// Interest that is added to the outstanding principal rather than paid in cash.
    /// Creates a new notional amount: `new_notional = old_notional + PIK_interest`.
    /// The amount field represents the interest capitalized.
    Pik,

    /// Scheduled amortization (principal repayment).
    ///
    /// Reduces the outstanding principal per the amortization schedule.
    /// Amount is positive from holder perspective (principal returned).
    /// After amortization: `remaining_notional = previous_notional - amortization`.
    Amortization,

    /// Prepayment of principal (unscheduled early repayment).
    ///
    /// Voluntary or mandatory early return of principal, common in:
    /// - Mortgage-backed securities (borrower refinancing)
    /// - CLO/CDO structures (collateral prepayments)
    /// - Callable bonds (issuer exercise)
    PrePayment,

    /// Revolving facility draw (borrowing).
    ///
    /// Increase in outstanding principal when borrower draws on a revolving facility.
    /// From holder (lender) view: negative (cash out to borrower).
    /// Increases the drawn balance used for interest calculations.
    RevolvingDraw,

    /// Revolving facility repayment.
    ///
    /// Decrease in outstanding principal when borrower repays revolving facility.
    /// From holder (lender) view: positive (cash in from borrower).
    /// Restores availability under the commitment.
    RevolvingRepayment,

    /// Defaulted notional (principal written down due to credit event).
    ///
    /// Represents the portion of principal that has experienced a credit event
    /// (failure to pay, bankruptcy, restructuring). Amount reflects the
    /// write-down from par.
    DefaultedNotional,

    /// Recovery cashflow from defaulted principal.
    ///
    /// Amount recovered through workout, liquidation, or settlement of defaulted
    /// debt. Typically expressed as a percentage of defaulted notional
    /// (recovery rate × defaulted amount).
    Recovery,

    /// Accrued-on-default interest (ISDA standard for CDS).
    ///
    /// Interest accrued but unpaid at the time of a credit event.
    /// Under ISDA standard conventions, the protection buyer pays
    /// the accrued premium from the last payment date to the default date.
    AccruedOnDefault,

    /// Irregular stub period interest.
    ///
    /// Interest payment for a non-standard accrual period at the beginning (front stub)
    /// or end (back stub) of a schedule. May be short stub (< regular period) or
    /// long stub (> regular period). Accrual factor reflects actual period length.
    Stub,

    // Margin and Collateral Cashflows (ISDA CSA / GMRA / BCBS-IOSCO)
    /// Initial margin posting (collateral transfer out to counterparty).
    ///
    /// Represents the posting of initial margin collateral under CSA or clearing
    /// house rules. The amount is typically calculated using SIMM, schedule-based,
    /// or haircut methodologies.
    InitialMarginPost,
    /// Initial margin return (collateral returned from counterparty).
    ///
    /// Represents the return of previously posted initial margin when exposure
    /// decreases or trade is terminated.
    InitialMarginReturn,
    /// Variation margin received (positive VM payment to us).
    ///
    /// Daily or periodic mark-to-market payment received when exposure moves
    /// in our favor. Governed by CSA threshold, MTA, and rounding rules.
    VariationMarginReceive,
    /// Variation margin paid (negative VM payment from us).
    ///
    /// Daily or periodic mark-to-market payment made when exposure moves
    /// against us. Governed by CSA threshold, MTA, and rounding rules.
    VariationMarginPay,
    /// Interest accrued on posted margin collateral.
    ///
    /// Represents interest paid or received on cash collateral posted as margin.
    /// Rate typically defined in CSA (e.g., Fed Funds, EONIA, SONIA).
    MarginInterest,
    /// Collateral substitution inflow (replacement collateral received).
    ///
    /// When a counterparty substitutes one form of eligible collateral for another,
    /// this represents the incoming replacement asset.
    CollateralSubstitutionIn,
    /// Collateral substitution outflow (original collateral returned).
    ///
    /// When a counterparty substitutes collateral, this represents the return
    /// of the original collateral being replaced.
    CollateralSubstitutionOut,
}

impl std::fmt::Display for CFKind {
    #[allow(unreachable_patterns)] // non_exhaustive future-proofing
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            CFKind::Fixed => "fixed",
            CFKind::FloatReset => "float_reset",
            CFKind::InflationCoupon => "inflation_coupon",
            CFKind::Fee => "fee",
            CFKind::CommitmentFee => "commitment_fee",
            CFKind::UsageFee => "usage_fee",
            CFKind::FacilityFee => "facility_fee",
            CFKind::Notional => "notional",
            CFKind::Pik => "pik",
            CFKind::Amortization => "amortization",
            CFKind::PrePayment => "prepayment",
            CFKind::RevolvingDraw => "revolving_draw",
            CFKind::RevolvingRepayment => "revolving_repayment",
            CFKind::DefaultedNotional => "defaulted_notional",
            CFKind::Recovery => "recovery",
            CFKind::AccruedOnDefault => "accrued_on_default",
            CFKind::Stub => "stub",
            CFKind::InitialMarginPost => "initial_margin_post",
            CFKind::InitialMarginReturn => "initial_margin_return",
            CFKind::VariationMarginReceive => "variation_margin_receive",
            CFKind::VariationMarginPay => "variation_margin_pay",
            CFKind::MarginInterest => "margin_interest",
            CFKind::CollateralSubstitutionIn => "collateral_substitution_in",
            CFKind::CollateralSubstitutionOut => "collateral_substitution_out",
            _ => "unknown",
        };
        f.write_str(label)
    }
}

impl std::str::FromStr for CFKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "fixed" => Ok(Self::Fixed),
            "float_reset" => Ok(Self::FloatReset),
            "inflation_coupon" => Ok(Self::InflationCoupon),
            "fee" => Ok(Self::Fee),
            "commitment_fee" => Ok(Self::CommitmentFee),
            "usage_fee" => Ok(Self::UsageFee),
            "facility_fee" => Ok(Self::FacilityFee),
            "notional" => Ok(Self::Notional),
            "pik" => Ok(Self::Pik),
            "amortization" => Ok(Self::Amortization),
            "prepayment" => Ok(Self::PrePayment),
            "revolving_draw" => Ok(Self::RevolvingDraw),
            "revolving_repayment" => Ok(Self::RevolvingRepayment),
            "defaulted_notional" => Ok(Self::DefaultedNotional),
            "recovery" => Ok(Self::Recovery),
            "accrued_on_default" => Ok(Self::AccruedOnDefault),
            "stub" => Ok(Self::Stub),
            "initial_margin_post" => Ok(Self::InitialMarginPost),
            "initial_margin_return" => Ok(Self::InitialMarginReturn),
            "variation_margin_receive" => Ok(Self::VariationMarginReceive),
            "variation_margin_pay" => Ok(Self::VariationMarginPay),
            "margin_interest" => Ok(Self::MarginInterest),
            "collateral_substitution_in" => Ok(Self::CollateralSubstitutionIn),
            "collateral_substitution_out" => Ok(Self::CollateralSubstitutionOut),
            _ => Err(format!("unknown cashflow kind {s:?}")),
        }
    }
}

impl CFKind {
    /// Returns `true` for interest-bearing cashflow kinds.
    ///
    /// Covers fixed coupons, floating resets, inflation-linked coupons,
    /// and irregular stub periods. Use this predicate wherever you need
    /// to distinguish interest flows from principal, fees, and credit-event flows.
    #[must_use]
    pub fn is_interest_like(self) -> bool {
        matches!(
            self,
            CFKind::Fixed | CFKind::FloatReset | CFKind::InflationCoupon | CFKind::Stub
        )
    }

    /// Returns `true` for principal-balance cashflow kinds.
    ///
    /// Covers notional exchanges, PIK capitalization, scheduled amortization,
    /// prepayments, revolving draws/repayments, and defaulted notional.
    /// Interest, fees, recovery, and margin flows return `false`.
    #[must_use]
    pub fn is_principal_like(self) -> bool {
        matches!(
            self,
            CFKind::Notional
                | CFKind::Pik
                | CFKind::Amortization
                | CFKind::PrePayment
                | CFKind::RevolvingDraw
                | CFKind::RevolvingRepayment
                | CFKind::DefaultedNotional
        )
    }

    /// Parse a cashflow-kind label, accepting canonical `Display` names plus
    /// the reporting aliases `coupon` → [`CFKind::Fixed`] and `principal` →
    /// [`CFKind::Notional`]. Matching is ASCII case-insensitive.
    ///
    /// # Arguments
    ///
    /// * `s` - Kind label such as `"fixed"`, `"Notional"`, `"coupon"`, or
    ///   `"principal"`.
    ///
    /// # Errors
    ///
    /// Returns an error when `s` is not a known kind label or alias.
    pub fn parse_label(s: &str) -> std::result::Result<Self, String> {
        let normalized = s.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "coupon" => Ok(Self::Fixed),
            "principal" => Ok(Self::Notional),
            other => other.parse(),
        }
    }
}

/// Contractual accrual metadata attached to one cashflow.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CashFlowAccrual {
    /// Contractual accrual-period start date.
    #[serde(with = "crate::wire::date")]
    #[cfg_attr(feature = "json-schema", schemars(with = "crate::wire::DateWire"))]
    pub start: Date,
    /// Contractual accrual-period end date.
    #[serde(with = "crate::wire::date")]
    #[cfg_attr(feature = "json-schema", schemars(with = "crate::wire::DateWire"))]
    pub end: Date,
    /// Day-count convention used for the accrual factor.
    pub day_count: DayCount,
    /// Projected index rate before spread, gearing, caps, or floors.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub projected_index_rate: Option<f64>,
    /// Calendar identifier for calendar-dependent day counts, including BUS/252.
    /// Required for BUS/252; otherwise optional. Joint calendars use `+`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calendar_id: Option<String>,
}

/// A single dated cash-flow (payment or reset).
///
/// Represents a monetary flow at a specific date with metadata
/// for proper classification and risk calculation.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CashFlow {
    /// Payment date (or payment date for principal/fee, or reset date for `CFKind::FloatReset`).
    #[serde(with = "crate::wire::date")]
    #[cfg_attr(feature = "json-schema", schemars(with = "crate::wire::DateWire"))]
    pub date: Date,
    /// Optional index reset date (for floating coupons).
    #[serde(default, with = "crate::wire::optional_date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<crate::wire::DateWire>")
    )]
    pub reset_date: Option<Date>,
    /// Monetary amount including its currency.
    pub amount: Money,
    /// Category/kind of cash-flow.
    pub kind: CFKind,
    /// Accrual factor used for coupon amount and sensitivity.
    pub accrual_factor: f64,
    /// Effective rate used to calculate this cashflow (None if not rate-based or unknown).
    ///
    /// For interest/fees: the annual rate used in the calculation
    /// For notional/amortization/PIK: typically None
    ///
    /// This is stored at cashflow creation time when available.
    /// For instruments with intra-period events (e.g., revolving credit with draws/repays),
    /// this may represent a time-weighted average rate across sub-periods.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate: Option<f64>,
    /// Optional contractual accrual metadata owned by this flow.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accrual: Option<CashFlowAccrual>,
    /// Explicit change in outstanding principal, in the cashflow currency.
    /// Positive increases outstanding. When absent, the flow kind and amount
    /// determine the balance movement. Initial funding remains represented by
    /// the schedule's initial notional rather than being counted twice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub principal_delta: Option<Money>,
}

impl CashFlow {
    /// Construct a cashflow without optional contractual accrual metadata.
    ///
    /// # Arguments
    ///
    /// * `date` - Settlement/payment date of the cashflow.
    /// * `reset_date` - Optional index observation date, no later than payment.
    /// * `amount` - Signed settlement cash including its currency.
    /// * `kind` - Classification controlling cash settlement and balance treatment.
    /// * `accrual_factor` - Nonnegative coupon year fraction under its day count.
    /// * `rate` - Optional decimal annual all-in coupon rate (0.05 means 5%).
    pub fn new(
        date: Date,
        reset_date: Option<Date>,
        amount: Money,
        kind: CFKind,
        accrual_factor: f64,
        rate: Option<f64>,
    ) -> Self {
        Self {
            date,
            reset_date,
            amount,
            kind,
            accrual_factor,
            rate,
            accrual: None,
            principal_delta: None,
        }
    }

    /// Attach a principal balance movement independently of settlement cash.
    ///
    /// # Arguments
    ///
    /// * `delta` - Signed change in outstanding, in the cashflow currency;
    ///   positive values increase principal and negative values repay it.
    #[must_use]
    pub fn with_principal_delta(mut self, delta: Money) -> Self {
        self.principal_delta = Some(delta);
        self
    }

    /// Attach contractual accrual metadata to this flow.
    ///
    /// # Arguments
    ///
    /// * `accrual` - Contractual coupon boundaries, day count, optional calendar,
    ///   and projected decimal index rate, independent of payment timing.
    #[must_use]
    pub fn with_accrual(mut self, accrual: CashFlowAccrual) -> Self {
        self.accrual = Some(accrual);
        self
    }

    /// Validate cashflow amount and fields.
    ///
    /// Zero amounts are valid: floored coupons (e.g. a floating coupon with
    /// a 0% floor in a negative-rate environment) legitimately produce
    /// zero-amount cashflows (
    /// zero amounts were previously rejected).
    ///
    /// # Errors
    /// Returns [`crate::Error::Input`] if the `amount`, accrual factor, or
    /// rate is non-finite, or [`crate::Error::Validation`] if the accrual
    /// factor is negative or the reset date is after the payment date.
    ///
    /// # Example
    /// ```rust
    /// use finstack_quant_core::cashflow::{CashFlow, CFKind};
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::dates::Date;
    /// use finstack_quant_core::money::Money;
    /// use time::Month;
    ///
    /// let date = Date::from_calendar_date(2025, Month::January, 15).expect("Valid date");
    /// let amount = Money::from((100_i64, Currency::USD));
    /// let cf = CashFlow::new(date, None, amount, CFKind::Fixed, 0.0, None);
    /// assert!(cf.validate().is_ok());
    ///
    /// // Zero amounts are valid (e.g. floored coupons).
    /// let zero_cf = CashFlow::new(
    ///     date,
    ///     None,
    ///     Money::from((0_i64, Currency::USD)),
    ///     CFKind::Fixed,
    ///     0.0,
    ///     None,
    /// );
    /// assert!(zero_cf.validate().is_ok());
    /// ```
    ///
    /// # Validation Rules
    ///
    /// - Amount must be finite (not NaN or Infinity); zero is allowed
    /// - Accrual factor must be finite (not NaN or Infinity)
    /// - Rate (if present) must be finite (not NaN or Infinity)
    /// - Reset date (if present) must not be after the payment date
    pub fn validate(&self) -> crate::Result<()> {
        if !self.amount.amount().is_finite() {
            let kind = NonFiniteKind::classify(self.amount.amount());
            return Err(InputError::NonFiniteValue { kind }.into());
        }

        if !self.accrual_factor.is_finite() {
            let kind = NonFiniteKind::classify(self.accrual_factor);
            return Err(InputError::NonFiniteValue { kind }.into());
        }

        if self.accrual_factor < 0.0 {
            return Err(crate::Error::Validation(
                "CashFlow: accrual_factor must be non-negative".into(),
            ));
        }

        if let Some(rate) = self.rate {
            if !rate.is_finite() {
                let kind = NonFiniteKind::classify(rate);
                return Err(InputError::NonFiniteValue { kind }.into());
            }
        }

        if let Some(reset) = self.reset_date {
            if reset > self.date {
                return Err(crate::Error::Validation(
                    "CashFlow: reset_date must not be after payment date".into(),
                ));
            }
        }

        if let Some(delta) = self.principal_delta {
            if delta.currency() != self.amount.currency() {
                return Err(crate::Error::CurrencyMismatch {
                    expected: self.amount.currency(),
                    actual: delta.currency(),
                });
            }
        }
        if let Some(accrual) = &self.accrual {
            if accrual.start >= accrual.end {
                return Err(crate::Error::Validation(
                    "CashFlow: accrual start must be before accrual end".into(),
                ));
            }
            if let Some(projected_index_rate) = accrual.projected_index_rate {
                if !projected_index_rate.is_finite() {
                    return Err(InputError::NonFiniteValue {
                        kind: NonFiniteKind::classify(projected_index_rate),
                    }
                    .into());
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::currency::Currency;
    use core::mem::size_of;
    use time::Month;

    fn assert_roundtrip<T>(value: T)
    where
        T: Clone + std::fmt::Display + std::str::FromStr + PartialEq,
    {
        let label = value.to_string();
        assert!(
            matches!(label.parse::<T>(), Ok(parsed) if parsed == value),
            "roundtrip failed for {label}"
        );
    }

    #[test]
    fn cashflow_size_is_reasonable() {
        let size = size_of::<CashFlow>();
        assert!(size <= 168, "CashFlow grew to {size} bytes");
    }

    #[test]
    fn cashflow_validation_works() {
        let date = Date::from_calendar_date(2025, Month::January, 15).expect("Valid test date");
        let amount = Money::from((100_i64, Currency::USD));

        let cf = CashFlow {
            date,
            reset_date: None,
            amount,
            kind: CFKind::Fixed,
            accrual_factor: 0.0,
            rate: None,
            accrual: None,
            principal_delta: None,
        };
        assert_eq!(cf.date, date);
        assert_eq!(cf.amount, amount);
        assert_eq!(cf.kind, CFKind::Fixed);
        assert!(cf.reset_date.is_none());
        assert_eq!(cf.accrual_factor, 0.0);
        assert!(cf.validate().is_ok());
    }

    #[test]
    fn cashflow_rejects_unknown_json_fields() {
        let json = serde_json::json!({
            "date": Date::from_calendar_date(2025, Month::January, 15).expect("valid date"),
            "reset_date": null,
            "amount": Money::from((100_i64, Currency::USD)),
            "kind": "fixed",
            "accrual_factor": 0.25,
            "rate": 0.05,
            "extra": true
        });

        let err = serde_json::from_value::<CashFlow>(json)
            .expect_err("unknown cashflow fields must be rejected");
        assert!(err.to_string().contains("extra"));
    }

    #[test]
    fn cashflow_kinds_construct_correctly() {
        let date = Date::from_calendar_date(2025, Month::March, 1).expect("Valid test date");
        let amt = Money::from((1_000_i64, Currency::EUR));

        let princ = CashFlow {
            date,
            reset_date: None,
            amount: amt,
            kind: CFKind::Notional,
            accrual_factor: 0.0,
            rate: None,
            accrual: None,
            principal_delta: None,
        };
        assert_eq!(princ.kind, CFKind::Notional);
        assert!(princ.validate().is_ok());

        let fee = CashFlow {
            date,
            reset_date: None,
            amount: amt,
            kind: CFKind::Fee,
            accrual_factor: 0.0,
            rate: None,
            accrual: None,
            principal_delta: None,
        };
        assert_eq!(fee.kind, CFKind::Fee);
        assert!(fee.validate().is_ok());

        let pik = CashFlow {
            date,
            reset_date: None,
            amount: amt,
            kind: CFKind::Pik,
            accrual_factor: 0.0,
            rate: None,
            accrual: None,
            principal_delta: None,
        };
        assert_eq!(pik.kind, CFKind::Pik);
        assert!(pik.validate().is_ok());

        let amort = CashFlow {
            date,
            reset_date: None,
            amount: amt,
            kind: CFKind::Amortization,
            accrual_factor: 0.0,
            rate: None,
            accrual: None,
            principal_delta: None,
        };
        assert_eq!(amort.kind, CFKind::Amortization);
        assert!(amort.validate().is_ok());

        // Zero amounts validate (floored coupons are legitimate).
        let zero = Money::from((0_i64, Currency::EUR));
        let zero_cf = CashFlow {
            date,
            reset_date: None,
            amount: zero,
            kind: CFKind::Fixed,
            accrual_factor: 0.0,
            rate: None,
            accrual: None,
            principal_delta: None,
        };
        assert!(zero_cf.validate().is_ok());
    }

    #[test]
    fn cfkind_is_interest_like_classifies_coupon_flows_only() {
        for kind in [
            CFKind::Fixed,
            CFKind::FloatReset,
            CFKind::InflationCoupon,
            CFKind::Stub,
        ] {
            assert!(kind.is_interest_like(), "{kind:?} should be interest-like");
        }

        for kind in [
            CFKind::Fee,
            CFKind::Notional,
            CFKind::Pik,
            CFKind::Amortization,
            CFKind::Recovery,
            CFKind::MarginInterest,
        ] {
            assert!(
                !kind.is_interest_like(),
                "{kind:?} should not be interest-like"
            );
        }
    }

    #[test]
    fn cfkind_is_principal_like_classifies_balance_changing_flows() {
        for kind in [
            CFKind::Notional,
            CFKind::Pik,
            CFKind::Amortization,
            CFKind::PrePayment,
            CFKind::RevolvingDraw,
            CFKind::RevolvingRepayment,
            CFKind::DefaultedNotional,
        ] {
            assert!(
                kind.is_principal_like(),
                "{kind:?} should be principal-like"
            );
        }

        for kind in [
            CFKind::Fixed,
            CFKind::Fee,
            CFKind::Recovery,
            CFKind::MarginInterest,
        ] {
            assert!(
                !kind.is_principal_like(),
                "{kind:?} should not be principal-like"
            );
        }
    }

    #[test]
    fn cfkind_parse_label_accepts_aliases_and_mixed_case() {
        assert_eq!(CFKind::parse_label("coupon").expect("alias"), CFKind::Fixed);
        assert_eq!(
            CFKind::parse_label("principal").expect("alias"),
            CFKind::Notional
        );
        assert_eq!(
            CFKind::parse_label("Notional").expect("mixed case"),
            CFKind::Notional
        );
        assert!(CFKind::parse_label("not-a-kind").is_err());
    }

    #[test]
    fn margin_cashflow_kinds_construct_correctly() {
        let date = Date::from_calendar_date(2025, Month::March, 1).expect("Valid test date");
        let amt = Money::from((1_000_000_i64, Currency::USD));

        // Initial margin posting
        let im_post = CashFlow {
            date,
            reset_date: None,
            amount: amt,
            kind: CFKind::InitialMarginPost,
            accrual_factor: 0.0,
            rate: None,
            accrual: None,
            principal_delta: None,
        };
        assert_eq!(im_post.kind, CFKind::InitialMarginPost);
        assert!(im_post.validate().is_ok());

        // Initial margin return
        let im_return = CashFlow {
            date,
            reset_date: None,
            amount: amt,
            kind: CFKind::InitialMarginReturn,
            accrual_factor: 0.0,
            rate: None,
            accrual: None,
            principal_delta: None,
        };
        assert_eq!(im_return.kind, CFKind::InitialMarginReturn);
        assert!(im_return.validate().is_ok());

        // Variation margin received
        let vm_receive = CashFlow {
            date,
            reset_date: None,
            amount: amt,
            kind: CFKind::VariationMarginReceive,
            accrual_factor: 0.0,
            rate: None,
            accrual: None,
            principal_delta: None,
        };
        assert_eq!(vm_receive.kind, CFKind::VariationMarginReceive);
        assert!(vm_receive.validate().is_ok());

        // Variation margin paid
        let vm_pay = CashFlow {
            date,
            reset_date: None,
            amount: amt,
            kind: CFKind::VariationMarginPay,
            accrual_factor: 0.0,
            rate: None,
            accrual: None,
            principal_delta: None,
        };
        assert_eq!(vm_pay.kind, CFKind::VariationMarginPay);
        assert!(vm_pay.validate().is_ok());

        // Margin interest
        let margin_int = CashFlow {
            date,
            reset_date: None,
            amount: Money::from((5_000_i64, Currency::USD)),
            kind: CFKind::MarginInterest,
            accrual_factor: 0.25,
            rate: Some(0.05),
            accrual: None,
            principal_delta: None,
        };
        assert_eq!(margin_int.kind, CFKind::MarginInterest);
        assert!(margin_int.validate().is_ok());

        // Collateral substitution in
        let sub_in = CashFlow {
            date,
            reset_date: None,
            amount: amt,
            kind: CFKind::CollateralSubstitutionIn,
            accrual_factor: 0.0,
            rate: None,
            accrual: None,
            principal_delta: None,
        };
        assert_eq!(sub_in.kind, CFKind::CollateralSubstitutionIn);
        assert!(sub_in.validate().is_ok());

        // Collateral substitution out
        let sub_out = CashFlow {
            date,
            reset_date: None,
            amount: amt,
            kind: CFKind::CollateralSubstitutionOut,
            accrual_factor: 0.0,
            rate: None,
            accrual: None,
            principal_delta: None,
        };
        assert_eq!(sub_out.kind, CFKind::CollateralSubstitutionOut);
        assert!(sub_out.validate().is_ok());
    }

    // CFKind FromStr / Display roundtrip tests

    #[test]
    fn cfkind_display_roundtrip() {
        let all = [
            CFKind::Fixed,
            CFKind::FloatReset,
            CFKind::InflationCoupon,
            CFKind::Fee,
            CFKind::CommitmentFee,
            CFKind::UsageFee,
            CFKind::FacilityFee,
            CFKind::Notional,
            CFKind::Pik,
            CFKind::Amortization,
            CFKind::PrePayment,
            CFKind::RevolvingDraw,
            CFKind::RevolvingRepayment,
            CFKind::DefaultedNotional,
            CFKind::Recovery,
            CFKind::AccruedOnDefault,
            CFKind::Stub,
            CFKind::InitialMarginPost,
            CFKind::InitialMarginReturn,
            CFKind::VariationMarginReceive,
            CFKind::VariationMarginPay,
            CFKind::MarginInterest,
            CFKind::CollateralSubstitutionIn,
            CFKind::CollateralSubstitutionOut,
        ];

        for kind in &all {
            assert_roundtrip(*kind);
        }
    }

    #[test]
    fn cfkind_from_str_rejects_noncanonical_spellings() {
        for rejected in ["amort", "pre_payment", "FIXED", "Float-Reset", " fixed"] {
            assert!(rejected.parse::<CFKind>().is_err());
        }
    }

    #[test]
    fn cfkind_from_str_unknown() {
        assert!("garbage".parse::<CFKind>().is_err());
    }
}
