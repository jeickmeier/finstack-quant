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

use crate::dates::{Date, DayCount, DayCountContext, DayCountContextState};
use crate::money::Money;
use crate::{InputError, NonFiniteKind};

/// Keep invalid rate observations from becoming absent rates in JSON.
fn serialize_optional_rate<S>(rate: &Option<f64>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    if let Some(rate) = rate
        && !rate.is_finite()
    {
        return Err(serde::ser::Error::custom(InputError::NonFiniteValue {
            kind: NonFiniteKind::classify(*rate),
        }));
    }
    serde::Serialize::serialize(rate, serializer)
}

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
/// - **Interest**: `Fixed`, `Floating`, `Stub`
/// - **Inflation**: `InflationCoupon`
/// - **Fees**: `Fee`, `CommitmentFee`, `UsageFee`, `FacilityFee`, `LcFee`, `FrontingFee`
/// - **Principal**: `Notional`, `PIK`, `Amortization`, `PrePayment`
/// - **Revolving**: `RevolvingDraw`, `RevolvingRepayment`
/// - **Credit Events**: `DefaultedNotional`, `Recovery`
/// - **Margin/Collateral**: `InitialMarginPost`, `VariationMarginPay`, etc.
///
/// # Labels
///
/// `Display`, `FromStr`, and serde share one snake_case label per kind
/// (`"fixed"`, `"floating"`, `"pre_payment"`). `FromStr` is ASCII
/// case-insensitive and ignores surrounding whitespace; serde accepts only
/// the canonical label.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
// Declaration order is the final same-date tie-break in cashflow ordering;
// reordering variants changes the order of otherwise identical rows.
pub enum CFKind {
    /// Fixed-rate coupon cash-flow.
    ///
    /// Periodic interest payment calculated as: `notional × rate × accrual_factor`.
    /// The `rate` field on [`CashFlow`] stores the coupon rate used.
    Fixed,

    /// Floating-rate coupon cash settlement.
    ///
    /// Interest payment based on a reference rate (e.g., SOFR, EURIBOR) plus
    /// spread; never a standalone fixing event. The `reset_date` field on
    /// [`CashFlow`] is the index observation date (metadata, not the payment
    /// date). The `rate` field stores the all-in rate (index + spread) if known.
    Floating,

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

    /// Letter-of-credit fee.
    ///
    /// Periodic fee on the face of letters of credit issued under a revolving
    /// facility's LC sublimit, typically at the drawn margin.
    LcFee,

    /// Fronting fee.
    ///
    /// Periodic fee paid to the issuing bank on the face of letters of credit
    /// outstanding under a revolving facility.
    FrontingFee,

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

    /// Credit protection payment.
    ///
    /// Actual cash paid by the protection seller on a credit event (for
    /// example a CDS default leg settlement). Distinct from asset recovery
    /// cash ([`Self::Recovery`]) and from a noncash principal write-down
    /// ([`Self::DefaultedNotional`]).
    ProtectionPayment,

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

    /// Net contractual derivative payoff settled in cash without exchanging principal.
    DerivativeSettlement,
}

/// Writes the snake_case serde label (`fixed`, `floating`, ...).
impl std::fmt::Display for CFKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&crate::wire::serde_label(self).map_err(|_| std::fmt::Error)?)
    }
}

impl std::str::FromStr for CFKind {
    type Err = crate::Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        crate::wire::serde_parse_with_context(
            "unknown cashflow kind",
            s,
            &s.trim().to_ascii_lowercase(),
        )
    }
}

impl CFKind {
    /// Returns `true` for the four ordinary coupon kinds: fixed, floating,
    /// inflation-linked and stub coupons (record CORE-CONVENTIONS-015 #3).
    ///
    /// A classification, not a measure of all interest: PIK, accrued on
    /// default and margin interest need their owner's explicit selection.
    #[must_use]
    pub fn is_coupon(self) -> bool {
        matches!(
            self,
            CFKind::Fixed | CFKind::Floating | CFKind::InflationCoupon | CFKind::Stub
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

    /// Returns `true` for kinds that can carry a principal repayment:
    /// amortization, prepayment, revolver repayment, and notional.
    ///
    /// A classification, not proof that a repayment occurred (record
    /// CORE-CONVENTIONS-015 #3). Narrower than [`Self::is_principal_like`],
    /// which also counts PIK capitalization, revolver draws and defaulted
    /// notional. A notional row is a repayment only when it actually reduces
    /// principal; a funding row is not.
    #[must_use]
    pub fn is_repayment_like(self) -> bool {
        matches!(
            self,
            CFKind::Amortization
                | CFKind::Notional
                | CFKind::PrePayment
                | CFKind::RevolvingRepayment
        )
    }

    /// Returns `true` when this classified flow represents a cash settlement.
    ///
    /// [`CFKind::Pik`] is a capitalization event and
    /// [`CFKind::DefaultedNotional`] is a write-down; neither should be emitted
    /// by APIs whose contract is dated cash. Every kind is listed explicitly,
    /// so a new kind cannot silently become cash (record CORE-CONVENTIONS-015
    /// #2).
    #[must_use]
    pub const fn is_cash_settlement(self) -> bool {
        match self {
            CFKind::Pik | CFKind::DefaultedNotional => false,
            CFKind::Fixed
            | CFKind::Floating
            | CFKind::InflationCoupon
            | CFKind::Fee
            | CFKind::CommitmentFee
            | CFKind::UsageFee
            | CFKind::FacilityFee
            | CFKind::LcFee
            | CFKind::FrontingFee
            | CFKind::Notional
            | CFKind::Amortization
            | CFKind::PrePayment
            | CFKind::RevolvingDraw
            | CFKind::RevolvingRepayment
            | CFKind::Recovery
            | CFKind::AccruedOnDefault
            | CFKind::ProtectionPayment
            | CFKind::Stub
            | CFKind::InitialMarginPost
            | CFKind::InitialMarginReturn
            | CFKind::VariationMarginReceive
            | CFKind::VariationMarginPay
            | CFKind::MarginInterest
            | CFKind::CollateralSubstitutionIn
            | CFKind::CollateralSubstitutionOut
            | CFKind::DerivativeSettlement => true,
        }
    }
}

/// Contractual accrual metadata attached to one cashflow.
///
/// Deserialization validates the slice (record CORE-CONVENTIONS-015 #10).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields, try_from = "CashFlowAccrualWire")]
#[cfg_attr(feature = "json-schema", schemars(!try_from, deny_unknown_fields))]
pub struct CashFlowAccrual {
    /// Contractual accrual-period start date.
    #[cfg_attr(feature = "json-schema", schemars(with = "crate::wire::DateWire"))]
    pub(super) start: Date,
    /// Contractual accrual-period end date.
    #[cfg_attr(feature = "json-schema", schemars(with = "crate::wire::DateWire"))]
    pub(super) end: Date,
    /// Day-count convention used for the accrual factor.
    pub(super) day_count: DayCount,
    /// Projected index rate before spread, gearing, caps, or floors.
    /// Serialization rejects non-finite rates instead of encoding them as absent.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_optional_rate"
    )]
    pub(super) projected_index_rate: Option<f64>,
    /// The complete day-count context the accrual factor was calculated with
    /// (record CORE-CONVENTIONS-015 #7): the calendar for calendar-dependent
    /// conventions (BUS/252 requires it; joint calendars use `+`), the coupon
    /// frequency and the regular ICMA reference period or full ACT/365L coupon
    /// bounds (`coupon_period`, retained for every subinterval of that
    /// coupon), and whether `end` is the instrument termination date for the
    /// 30E/360 ISDA February exception (`None` is unspecified, which that
    /// convention rejects).
    pub(super) context: DayCountContextState,
    /// Consecutive nominal boundaries for a coupon spanning multiple ICMA reference periods.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "json-schema", schemars(with = "Vec<crate::wire::DateWire>"))]
    pub(super) nominal_dates: Option<std::sync::Arc<Vec<Date>>>,
}

/// Wire fields of [`CashFlowAccrual`]; deserialization validates them.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct CashFlowAccrualWire {
    start: Date,
    end: Date,
    day_count: DayCount,
    #[serde(default)]
    projected_index_rate: Option<f64>,
    context: DayCountContextState,
    #[serde(default)]
    nominal_dates: Vec<Date>,
}

impl TryFrom<CashFlowAccrualWire> for CashFlowAccrual {
    type Error = crate::Error;

    fn try_from(wire: CashFlowAccrualWire) -> crate::Result<Self> {
        Self::from_fields(
            wire.start,
            wire.end,
            wire.day_count,
            wire.projected_index_rate,
            wire.context,
            wire.nominal_dates,
        )
    }
}

impl CashFlowAccrual {
    /// The one constructor: publish an accrual slice only once it validates
    /// (record CORE-CONVENTIONS-015 #10).
    ///
    /// # Arguments
    ///
    /// * `start` - Actual accrual-slice start date (inclusive).
    /// * `end` - Actual accrual-slice end date (exclusive), independent of the
    ///   cash payment date; must follow `start`.
    /// * `day_count` - Day-count convention the accrual factor uses.
    /// * `projected_index_rate` - Decimal index rate before spread, gearing,
    ///   caps or floors (0.05 means 5%); `None` when not projected.
    /// * `context` - The complete day-count context the factor was
    ///   calculated with (calendar, frequency, business-day basis, reference
    ///   coupon period, termination status).
    ///
    /// # Errors
    ///
    /// Returns the [`Self::validate`] errors.
    pub fn new(
        start: Date,
        end: Date,
        day_count: DayCount,
        projected_index_rate: Option<f64>,
        context: DayCountContextState,
    ) -> crate::Result<Self> {
        Self::from_fields(
            start,
            end,
            day_count,
            projected_index_rate,
            context,
            Vec::new(),
        )
    }

    /// Construct a coupon on an explicit sequence of nominal ICMA periods.
    ///
    /// # Arguments
    ///
    /// * `start` - Inclusive accrual start in the first nominal period.
    /// * `end` - Exclusive accrual end in the last nominal period.
    /// * `projected_index_rate` - Optional decimal floating index rate before spread and gearing.
    /// * `context` - Contractual coupon frequency without a single reference period.
    /// * `nominal_dates` - Ordered unadjusted boundaries of all nominal periods touched by the coupon.
    pub fn from_nominal_dates(
        start: Date,
        end: Date,
        projected_index_rate: Option<f64>,
        context: DayCountContextState,
        nominal_dates: Vec<Date>,
    ) -> crate::Result<Self> {
        if nominal_dates.len() < 3 {
            return Err(crate::Error::Validation(
                "multi-period ICMA requires at least three nominal dates".into(),
            ));
        }
        Self::from_fields(
            start,
            end,
            DayCount::ActActIsma,
            projected_index_rate,
            context,
            nominal_dates,
        )
    }

    fn from_fields(
        start: Date,
        end: Date,
        day_count: DayCount,
        projected_index_rate: Option<f64>,
        context: DayCountContextState,
        nominal_dates: Vec<Date>,
    ) -> crate::Result<Self> {
        let accrual = Self {
            start,
            end,
            day_count,
            projected_index_rate,
            context,
            nominal_dates: (!nominal_dates.is_empty()).then(|| std::sync::Arc::new(nominal_dates)),
        };
        accrual.validate()?;
        Ok(accrual)
    }

    /// Actual accrual-slice start date.
    #[must_use]
    pub fn get_start(&self) -> Date {
        self.start
    }

    /// Actual accrual-slice end date.
    #[must_use]
    pub fn get_end(&self) -> Date {
        self.end
    }

    /// Day-count convention of the accrual factor.
    #[must_use]
    pub fn get_day_count(&self) -> DayCount {
        self.day_count
    }

    /// Projected decimal index rate before spread, gearing, caps or floors.
    #[must_use]
    pub fn get_projected_index_rate(&self) -> Option<f64> {
        self.projected_index_rate
    }

    /// Unadjusted ICMA boundaries when the accrual spans multiple nominal periods.
    pub fn get_nominal_dates(&self) -> &[Date] {
        self.nominal_dates.as_deref().map_or(&[], Vec::as_slice)
    }

    /// Day-count context the accrual factor was calculated with.
    #[must_use]
    pub fn get_context(&self) -> &DayCountContextState {
        &self.context
    }

    /// Check this accrual slice: actual endpoints strictly increasing, a
    /// strictly increasing reference coupon period when present, a finite
    /// projected index rate when present, and all context inputs required by
    /// `day_count` (record CORE-CONVENTIONS-015 #7, #9, #10).
    ///
    /// For `DayCount::Bus252`, the holiday calendar ID must be present and
    /// resolve in the registry (date-range coverage stays at calculation time).
    /// Non-`Bus252` conventions do not resolve `calendar_id` at storage time.
    /// For `DayCount::Act365L`, enclosing `coupon_period` containment over
    /// `[start, end]` is verified here. For `DayCount::ActActIsma`, the
    /// nominal reference `coupon_period` must reproduce one month grid of
    /// `frequency` and overlap `[start, end]` (admitting regular periods,
    /// adjusted-boundary spillovers, and multi-period long stubs).
    ///
    /// # Errors
    ///
    /// Returns a validation or input error for an inverted or empty interval,
    /// a non-finite projected rate, missing or invalid convention context,
    /// an unknown `Bus252` calendar identifier, or an `Act365L`/`ActActIsma`
    /// slice incompatible with its reference coupon period.
    pub fn validate(&self) -> crate::Result<()> {
        if self
            .context
            .get_coupon_period()
            .is_some_and(|(start, end)| start >= end)
        {
            return Err(crate::Error::Validation(
                "CashFlow: reference coupon period start must be before end".into(),
            ));
        }
        if self.start >= self.end {
            return Err(crate::Error::Validation(
                "CashFlow: accrual start must be before accrual end".into(),
            ));
        }
        if let Some(projected_index_rate) = self.projected_index_rate
            && !projected_index_rate.is_finite()
        {
            return Err(InputError::NonFiniteValue {
                kind: NonFiniteKind::classify(projected_index_rate),
            }
            .into());
        }
        let calendar = match self.day_count {
            DayCount::Bus252 => {
                let id = self
                    .context
                    .get_calendar_id()
                    .ok_or(InputError::MissingCalendarForBus252)?;
                Some(crate::dates::calendar_by_id_strict(id)?)
            }
            _ => None,
        };
        let ctx = DayCountContext {
            calendar,
            frequency: self.context.get_frequency(),
            bus_basis: self.context.get_bus_basis(),
            coupon_period: self.context.get_coupon_period(),
            end_is_termination_date: self.context.get_end_is_termination_date(),
        };
        if let Some(dates) = &self.nominal_dates {
            if self.day_count != DayCount::ActActIsma
                || ctx.coupon_period.is_some()
                || dates.len() < 3
                || self.start < dates[0]
                || self.start >= dates[1]
                || self.end <= dates[dates.len() - 2]
                || self.end > dates[dates.len() - 1]
            {
                return Err(crate::Error::Validation(
                    "ICMA nominal periods must span the accrual exactly".into(),
                ));
            }
            for pair in dates.windows(2) {
                self.day_count.validate_context(DayCountContext {
                    coupon_period: Some((pair[0], pair[1])),
                    ..ctx
                })?;
            }
            return Ok(());
        }
        self.day_count.validate_context(ctx)?;
        match self.day_count {
            DayCount::Act365L => {
                self.day_count.year_fraction(self.start, self.end, ctx)?;
            }
            DayCount::ActActIsma => {
                if let Some((period_start, period_end)) = self.context.get_coupon_period()
                    && (self.start >= period_end || period_start >= self.end)
                {
                    return Err(crate::Error::Validation(format!(
                        "ACT/ACT ISMA accrual [{}, {}] must overlap reference coupon_period [{period_start}, {period_end}]",
                        self.start, self.end
                    )));
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Restrict contractual metadata to a balance segment, retaining its nominal denominators.
    ///
    /// # Arguments
    ///
    /// * `start` - Inclusive start within this accrual.
    /// * `end` - Exclusive end within this accrual, strictly after `start`.
    pub fn subperiod(&self, start: Date, end: Date) -> crate::Result<Self> {
        if start < self.start || end > self.end || start >= end {
            return Err(crate::Error::Validation("invalid accrual subperiod".into()));
        }
        let first = self
            .get_nominal_dates()
            .partition_point(|d| *d <= start)
            .saturating_sub(1);
        let last = self.get_nominal_dates().partition_point(|d| *d < end);
        let dates = self
            .get_nominal_dates()
            .get(first..=last)
            .unwrap_or_default();
        let reference = match dates {
            [lo, hi] => Some((*lo, *hi)),
            _ => self.context.get_coupon_period(),
        };
        let context = DayCountContextState::try_new(
            self.context.get_calendar_id().map(str::to_owned),
            self.context.get_frequency(),
            self.context.get_bus_basis(),
            reference,
            self.context
                .get_end_is_termination_date()
                .map(|terminal| terminal && end == self.end),
        )?;
        if dates.len() > 2 {
            Self::from_nominal_dates(
                start,
                end,
                self.projected_index_rate,
                context,
                dates.to_vec(),
            )
        } else {
            Self::new(
                start,
                end,
                self.day_count,
                self.projected_index_rate,
                context,
            )
        }
    }

    /// Calculate the fraction over this interval using each applicable nominal denominator.
    pub fn year_fraction(&self) -> crate::Result<f64> {
        let context = self.day_count_context()?;
        if self.nominal_dates.is_none() {
            return self.day_count.year_fraction(self.start, self.end, context);
        }
        let mut total = crate::math::summation::NeumaierAccumulator::new();
        for pair in self.get_nominal_dates().windows(2) {
            total.add(self.day_count.year_fraction(
                self.start.max(pair[0]),
                self.end.min(pair[1]),
                DayCountContext {
                    coupon_period: Some((pair[0], pair[1])),
                    ..context
                },
            )?)?;
        }
        total.get_total()
    }

    /// Resolve this slice's day-count context, looking up its calendar.
    ///
    /// # Errors
    ///
    /// Returns an error when the context fields are invalid or the calendar id
    /// names an unknown calendar.
    pub fn day_count_context(&self) -> crate::Result<DayCountContext<'static>> {
        self.context.to_ctx()
    }
}

/// A single dated cash-flow (payment or reset).
///
/// Represents a monetary flow at a specific date with metadata
/// for proper classification and risk calculation.
///
/// Deserialization validates the row (record CORE-CONVENTIONS-015 #10), so no
/// wire format can create an invalid cashflow.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields, try_from = "CashFlowWire")]
#[cfg_attr(feature = "json-schema", schemars(!try_from, deny_unknown_fields))]
pub struct CashFlow {
    /// Payment date (or payment date for principal/fee, or reset date for `CFKind::Floating`).
    #[cfg_attr(feature = "json-schema", schemars(with = "crate::wire::DateWire"))]
    pub(super) date: Date,
    /// Optional index reset date (for floating coupons).
    #[serde(default)]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<crate::wire::DateWire>")
    )]
    pub(super) reset_date: Option<Date>,
    /// Monetary amount including its currency.
    pub(super) amount: Money,
    /// Category/kind of cash-flow.
    pub(super) kind: CFKind,
    /// Accrual factor used for coupon amount and sensitivity.
    pub(super) accrual_factor: f64,
    /// Effective rate used to calculate this cashflow (None if not rate-based or unknown).
    ///
    /// For interest/fees: the annual rate used in the calculation
    /// For notional/amortization/PIK: typically None
    ///
    /// This is stored at cashflow creation time when available.
    /// For instruments with intra-period events (e.g., revolving credit with draws/repays),
    /// this may represent a time-weighted average rate across sub-periods.
    /// Serialization rejects non-finite rates instead of encoding them as absent.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_optional_rate"
    )]
    pub(super) rate: Option<f64>,
    /// Optional contractual accrual metadata owned by this flow.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) accrual: Option<CashFlowAccrual>,
    /// Explicit change in outstanding principal, in the cashflow currency.
    /// Positive increases outstanding. Required on every principal-like row
    /// (zero included) and absent on every other kind; nothing is inferred
    /// from the amount or kind (record CORE-CONVENTIONS-015 #4, #5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) principal_delta: Option<Money>,
    /// Economic date of the principal movement, independent of cash payment.
    /// Required with `principal_delta` on every principal-like row and absent
    /// otherwise; there is no payment-date fallback. PIK and write-down rows
    /// are dated at it (record CORE-CONVENTIONS-015 #6).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<crate::wire::DateWire>")
    )]
    pub(super) principal_date: Option<Date>,
}

/// Wire fields of [`CashFlow`]; deserialization validates them.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct CashFlowWire {
    date: Date,
    #[serde(default)]
    reset_date: Option<Date>,
    amount: Money,
    kind: CFKind,
    accrual_factor: f64,
    #[serde(default)]
    rate: Option<f64>,
    #[serde(default)]
    accrual: Option<CashFlowAccrual>,
    #[serde(default)]
    principal_delta: Option<Money>,
    #[serde(default)]
    principal_date: Option<Date>,
}

impl TryFrom<CashFlowWire> for CashFlow {
    type Error = crate::Error;

    fn try_from(wire: CashFlowWire) -> crate::Result<Self> {
        let flow = Self {
            date: wire.date,
            reset_date: wire.reset_date,
            amount: wire.amount,
            kind: wire.kind,
            accrual_factor: wire.accrual_factor,
            rate: wire.rate,
            accrual: wire.accrual,
            principal_delta: wire.principal_delta,
            principal_date: wire.principal_date,
        };
        flow.validate()?;
        Ok(flow)
    }
}

/// Raw, possibly incomplete inputs of a [`CashFlow`].
///
/// A builder holds these and publishes them only through [`Self::build`] or
/// [`CashFlow::new`], which validate the complete row (record
/// CORE-CONVENTIONS-015 #10). `From<&CashFlow>` gives the inputs of an
/// existing row for an edit-and-rebuild update.
#[derive(Clone, Debug, PartialEq)]
pub struct CashFlowParams {
    /// Settlement/payment date, or the event date of a noncash balance row.
    pub date: Date,
    /// Single index observation date of a floating coupon.
    pub reset_date: Option<Date>,
    /// Signed settlement cash; zero for PIK and default write-down rows.
    pub amount: Money,
    /// Cashflow classification.
    pub kind: CFKind,
    /// Literal nonnegative accrual factor.
    pub accrual_factor: f64,
    /// Optional decimal effective rate.
    pub rate: Option<f64>,
    /// Optional single-slice contractual accrual metadata.
    pub accrual: Option<CashFlowAccrual>,
    /// Explicit principal change, required on principal-like rows.
    pub principal_delta: Option<Money>,
    /// Balance-effective date, required with `principal_delta`.
    pub principal_date: Option<Date>,
}

impl CashFlowParams {
    /// Start the inputs of a row without accrual metadata or principal effect.
    ///
    /// # Arguments
    ///
    /// * `date` - Settlement/payment date of the cashflow, or the event date of
    ///   a noncash balance row.
    /// * `reset_date` - Optional index observation date, no later than `date`.
    /// * `amount` - Signed settlement cash including its currency; zero for
    ///   PIK and default write-down rows.
    /// * `kind` - Classification controlling cash settlement and balance treatment.
    /// * `accrual_factor` - Literal nonnegative coupon year fraction.
    /// * `rate` - Optional decimal annual all-in coupon rate (0.05 means 5%).
    #[must_use]
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
            principal_date: None,
        }
    }

    /// Set the explicit principal change.
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

    /// Set the balance-effective date of the principal change.
    ///
    /// # Arguments
    ///
    /// * `date` - Contractual balance-effective date; cash still settles on
    ///   `date`, which may precede or follow it.
    #[must_use]
    pub fn with_principal_date(mut self, date: Date) -> Self {
        self.principal_date = Some(date);
        self
    }

    /// Set the contractual accrual metadata.
    ///
    /// # Arguments
    ///
    /// * `accrual` - One actual accrual calculation slice and its day-count
    ///   context, independent of payment timing.
    #[must_use]
    pub fn with_accrual(mut self, accrual: CashFlowAccrual) -> Self {
        self.accrual = Some(accrual);
        self
    }

    /// Publish the validated row.
    ///
    /// # Errors
    ///
    /// Returns the [`CashFlow::validate`] errors.
    pub fn build(self) -> crate::Result<CashFlow> {
        CashFlow::new(self)
    }
}

impl From<&CashFlow> for CashFlowParams {
    fn from(flow: &CashFlow) -> Self {
        Self {
            date: flow.date,
            reset_date: flow.reset_date,
            amount: flow.amount,
            kind: flow.kind,
            accrual_factor: flow.accrual_factor,
            rate: flow.rate,
            accrual: flow.accrual.clone(),
            principal_delta: flow.principal_delta,
            principal_date: flow.principal_date,
        }
    }
}

impl CashFlow {
    /// The one canonical constructor: publish complete inputs only once they
    /// validate (record CORE-CONVENTIONS-015 #10).
    ///
    /// # Arguments
    ///
    /// * `params` - The complete row inputs; see [`CashFlowParams`].
    ///
    /// # Errors
    ///
    /// Returns the [`Self::validate`] errors: nonfinite scalars, invalid
    /// chronology, a missing or extra principal effect, nonzero cash on a
    /// noncash kind, or an invalid accrual slice.
    pub fn new(params: CashFlowParams) -> crate::Result<Self> {
        let flow = Self {
            date: params.date,
            reset_date: params.reset_date,
            amount: params.amount,
            kind: params.kind,
            accrual_factor: params.accrual_factor,
            rate: params.rate,
            accrual: params.accrual,
            principal_delta: params.principal_delta,
            principal_date: params.principal_date,
        };
        flow.validate()?;
        Ok(flow)
    }

    /// Return a copy with a new principal change and balance date, updated
    /// together; on error the original row is unchanged.
    ///
    /// # Arguments
    ///
    /// * `delta` - Signed change in outstanding, in the cashflow currency.
    /// * `date` - Balance-effective date of the change.
    ///
    /// # Errors
    ///
    /// Returns the [`Self::validate`] errors for the updated row.
    pub fn with_principal(&self, delta: Money, date: Date) -> crate::Result<Self> {
        CashFlowParams::from(self)
            .with_principal_delta(delta)
            .with_principal_date(date)
            .build()
    }

    /// Return the payment (cash settlement) date, or the event date of a
    /// noncash balance row.
    #[must_use]
    pub fn get_date(&self) -> Date {
        self.date
    }

    /// Return the single index observation date of a floating coupon.
    #[must_use]
    pub fn get_reset_date(&self) -> Option<Date> {
        self.reset_date
    }

    /// Return the signed settlement cash, zero for noncash balance events.
    #[must_use]
    pub fn get_amount(&self) -> Money {
        self.amount
    }

    /// Return the cashflow classification.
    #[must_use]
    pub fn get_kind(&self) -> CFKind {
        self.kind
    }

    /// Return the literal accrual factor (zero is a zero-length accrual).
    #[must_use]
    pub fn get_accrual_factor(&self) -> f64 {
        self.accrual_factor
    }

    /// Return the effective rate metadata, if known.
    #[must_use]
    pub fn get_rate(&self) -> Option<f64> {
        self.rate
    }

    /// Return the optional single-slice contractual accrual metadata.
    #[must_use]
    pub fn get_accrual(&self) -> &Option<CashFlowAccrual> {
        &self.accrual
    }

    /// Return the explicit principal change of a principal row.
    #[must_use]
    pub fn get_principal_delta(&self) -> Option<Money> {
        self.principal_delta
    }

    /// Return the explicit balance-effective date of a principal row.
    ///
    /// `None` for a row that moves no principal; there is no payment-date
    /// fallback (record CORE-CONVENTIONS-015 #6).
    #[must_use]
    pub fn get_balance_date(&self) -> Option<Date> {
        self.principal_date
    }

    /// Return a copy with new contractual accrual metadata; on error the
    /// original row is unchanged.
    ///
    /// # Arguments
    ///
    /// * `accrual` - One actual accrual calculation slice and its day-count
    ///   context, independent of payment timing.
    ///
    /// # Errors
    ///
    /// Returns the [`Self::validate`] errors for the updated row.
    pub fn with_accrual(&self, accrual: CashFlowAccrual) -> crate::Result<Self> {
        CashFlowParams::from(self).with_accrual(accrual).build()
    }

    /// Validate cashflow amount and fields.
    ///
    /// Zero amounts are valid: floored coupons (e.g. a floating coupon with
    /// a 0% floor in a negative-rate environment) legitimately produce
    /// zero-amount cashflows.
    ///
    /// # Errors
    /// Returns [`crate::Error::Input`] if the `amount`, accrual factor, rate,
    /// or projected index rate is non-finite, [`crate::Error::CurrencyMismatch`]
    /// if the principal delta is in another currency, or
    /// [`crate::Error::Validation`] if the accrual factor is negative, the
    /// reset date is after the payment date, or an accrual period is empty.
    ///
    /// # Example
    /// ```rust
    /// use finstack_quant_core::cashflow::{CashFlowParams, CFKind};
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::dates::Date;
    /// use finstack_quant_core::money::Money;
    /// use time::Month;
    ///
    /// let date = Date::from_calendar_date(2025, Month::January, 15).expect("Valid date");
    /// let amount = Money::from((100_i64, Currency::USD));
    /// let cf = CashFlowParams::new(date, None, amount, CFKind::Fixed, 0.0, None).build()?;
    /// assert!(cf.validate().is_ok());
    ///
    /// // Zero amounts are valid (e.g. floored coupons).
    /// let zero = Money::from((0_i64, Currency::USD));
    /// assert!(CashFlowParams::new(date, None, zero, CFKind::Fixed, 0.0, None).build().is_ok());
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    ///
    /// # Validation Rules
    ///
    /// - Zero monetary amounts are allowed; `Money` guarantees representability
    /// - Accrual factor must be finite (not NaN or Infinity)
    /// - Rate (if present) must be finite (not NaN or Infinity)
    /// - Reset date (if present) must not be after the payment date
    /// - A net derivative settlement has a zero accrual factor and no coupon accrual metadata
    /// - Principal delta (if present) must share the amount's currency
    /// - Accrual metadata (if present) must have start before end, a coupon
    ///   period with start before end, and a finite projected index rate
    pub fn validate(&self) -> crate::Result<()> {
        if !self.accrual_factor.is_finite() {
            let kind = NonFiniteKind::classify(self.accrual_factor);
            return Err(InputError::NonFiniteValue { kind }.into());
        }

        if self.accrual_factor < 0.0 {
            return Err(crate::Error::Validation(
                "CashFlow: accrual_factor must be non-negative".into(),
            ));
        }

        if let Some(rate) = self.rate
            && !rate.is_finite()
        {
            let kind = NonFiniteKind::classify(rate);
            return Err(InputError::NonFiniteValue { kind }.into());
        }

        if let Some(reset) = self.reset_date
            && reset > self.date
        {
            return Err(crate::Error::Validation(
                "CashFlow: reset_date must not be after payment date".into(),
            ));
        }

        if let Some(delta) = self.principal_delta
            && delta.currency() != self.amount.currency()
        {
            return Err(crate::Error::CurrencyMismatch {
                expected: self.amount.currency(),
                actual: delta.currency(),
            });
        }
        if self.kind == CFKind::DerivativeSettlement
            && (self.accrual.is_some() || self.accrual_factor != 0.0)
        {
            return Err(crate::Error::Validation(
                "net derivative settlement cannot carry coupon accrual".into(),
            ));
        }
        // Every principal-like row states its complete principal effect, an
        // explicit signed delta (zero included) and its balance date; other
        // kinds carry neither (record CORE-CONVENTIONS-015 #4).
        if self.kind.is_principal_like() {
            if self.principal_delta.is_none() || self.principal_date.is_none() {
                return Err(crate::Error::Validation(format!(
                    "CashFlow: {} row on {} requires an explicit principal_delta and \
                     principal_date; neither is inferred from its cash",
                    self.kind, self.date
                )));
            }
        } else if self.principal_delta.is_some() || self.principal_date.is_some() {
            return Err(crate::Error::Validation(format!(
                "CashFlow: {} row on {} is not a principal kind and cannot carry \
                 principal_delta or principal_date",
                self.kind, self.date
            )));
        }
        // PIK and DefaultedNotional are noncash balance events: exactly zero
        // cash, dated at their balance event (record CORE-CONVENTIONS-015 #1, #6).
        if !self.kind.is_cash_settlement() {
            if !self.amount.amount_decimal().is_zero() {
                return Err(crate::Error::Validation(format!(
                    "CashFlow: {} row on {} is a noncash balance event and must carry zero \
                     cash; its value belongs in principal_delta",
                    self.kind, self.date
                )));
            }
            if self.principal_date != Some(self.date) {
                return Err(crate::Error::Validation(format!(
                    "CashFlow: {} row on {} is a noncash balance event; its date must \
                     equal its principal_date",
                    self.kind, self.date
                )));
            }
        }
        if let Some(accrual) = &self.accrual {
            accrual.validate()?;
        }

        Ok(())
    }
}
