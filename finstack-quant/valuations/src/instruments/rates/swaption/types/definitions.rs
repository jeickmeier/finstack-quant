//! Pricing and metric helpers for interest-rate instruments.
//!
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::Result;

/// Cash settlement annuity method for cash-settled swaptions.
///
/// The trade confirmation or ISDA settlement matrix determines the method.
/// Modern EUR cash-settled swaptions use collateralized cash price; legacy
/// trades may retain par-yield or ISDA par-par terms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CashSettlementMethod {
    /// Collateralized cash price using the actual collateral-discounted swap
    /// annuity.
    ///
    /// This is the current EUR market standard under the ISDA settlement
    /// matrix. The exercise payoff is the collateralized NPV of the underlying
    /// swap, equivalently `A_collateral × max(±(S-K), 0)`.
    #[default]
    CollateralizedCashPrice,

    /// Legacy par-yield-curve cash settlement.
    ///
    /// The cash annuity is reconstructed from the forward swap rate as a flat
    /// discount yield:
    ///
    /// ```text
    /// A = (1 - (1 + S/m)^(-N)) / S
    /// ```
    ParYield,

    /// Legacy ISDA par-par settlement using the actual fixed-leg annuity.
    ///
    /// Retained for confirmations that explicitly name this settlement method;
    /// do not use it as a universal modern cash-settlement default.
    IsdaParPar,

    /// Zero-coupon settlement discounting one payment to swap maturity.
    ZeroCoupon,
}

impl std::fmt::Display for CashSettlementMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CashSettlementMethod::CollateralizedCashPrice => {
                write!(f, "collateralized_cash_price")
            }
            CashSettlementMethod::ParYield => write!(f, "par_yield"),
            CashSettlementMethod::IsdaParPar => write!(f, "isda_par_par"),
            CashSettlementMethod::ZeroCoupon => write!(f, "zero_coupon"),
        }
    }
}

impl std::str::FromStr for CashSettlementMethod {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "collateralized_cash_price" => Ok(Self::CollateralizedCashPrice),
            "par_yield" => Ok(Self::ParYield),
            "isda_par_par" => Ok(Self::IsdaParPar),
            "zero_coupon" => Ok(Self::ZeroCoupon),
            _ => Err(format!(
                "Unknown cash settlement method: '{s}'. Valid: \
                 collateralized_cash_price, par_yield, isda_par_par, zero_coupon"
            )),
        }
    }
}

// Bermudan Swaption Types

/// Bermudan exercise schedule specification.
///
/// Defines the exercise dates and constraints for a Bermudan swaption.
/// Exercise dates are typically aligned with swap coupon dates.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BermudanSchedule {
    /// Exercise dates (must be sorted, typically on swap coupon dates)
    #[serde(with = "finstack_quant_core::wire::dates")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Vec<finstack_quant_core::wire::DateWire>")
    )]
    pub exercise_dates: Vec<Date>,
    /// Lockout period end (no exercise before this date)
    #[serde(default, with = "finstack_quant_core::wire::optional_date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<finstack_quant_core::wire::DateWire>")
    )]
    pub lockout_end: Option<Date>,
    /// Notice period in business days before exercise
    pub notice_days: u32,
}

impl BermudanSchedule {
    /// Create a new Bermudan schedule with the given exercise dates.
    ///
    /// # Arguments
    /// * `exercise_dates` - Exercise dates (will be sorted)
    pub fn new(mut exercise_dates: Vec<Date>) -> Self {
        exercise_dates.sort();
        Self {
            exercise_dates,
            lockout_end: None,
            notice_days: 0,
        }
    }

    /// Create schedule with lockout period.
    pub fn with_lockout(mut self, lockout_end: Date) -> Self {
        self.lockout_end = Some(lockout_end);
        self
    }

    /// Create schedule with notice period.
    pub fn with_notice_days(mut self, days: u32) -> Self {
        self.notice_days = days;
        self
    }

    /// Generate co-terminal exercise dates from swap schedule.
    ///
    /// Creates exercise dates on each fixed leg payment date from `first_exercise`
    /// to `swap_end`, excluding the final payment date (swap maturity).
    ///
    /// # Arguments
    /// * `first_exercise` - First allowed exercise date
    /// * `swap_end` - Swap maturity date
    /// * `fixed_frequency` - Fixed leg payment frequency
    pub fn co_terminal(
        first_exercise: Date,
        swap_end: Date,
        fixed_frequency: Tenor,
    ) -> finstack_quant_core::Result<Self> {
        let periods = crate::cashflow::builder::periods::build_periods(
            crate::cashflow::builder::periods::BuildPeriodsParams {
                start: first_exercise,
                end: swap_end,
                frequency: fixed_frequency,
                stub: StubKind::None,
                business_day_convention: BusinessDayConvention::ModifiedFollowing,
                calendar_id: crate::cashflow::builder::calendar::WEEKENDS_ONLY_ID,
                end_of_month: false,
                day_count: finstack_quant_core::dates::DayCount::Act365F,
                payment_lag_days: 0,
                reset_lag_days: None,
                adjust_accrual_dates: false,
                roll_rule: crate::cashflow::builder::specs::RollRule::None,
            },
        )?;
        // Exercise dates are all coupon dates except the last one (maturity),
        // but always include the first_exercise date when it is before swap_end.
        let mut exercise_dates: Vec<Date> = Vec::new();
        if first_exercise < swap_end {
            exercise_dates.push(first_exercise);
        }
        exercise_dates.extend(
            periods
                .into_iter()
                .map(|period| period.payment_date)
                .filter(|&date| date > first_exercise && date < swap_end),
        );
        Ok(Self::new(exercise_dates))
    }

    /// Get effective exercise dates (filtered by lockout).
    pub fn effective_dates(&self) -> Vec<Date> {
        match self.lockout_end {
            Some(lockout) => self
                .exercise_dates
                .iter()
                .filter(|&&d| d > lockout)
                .copied()
                .collect(),
            None => self.exercise_dates.clone(),
        }
    }

    /// Convert exercise dates to year fractions from a given date.
    pub fn exercise_times(&self, as_of: Date, day_count: DayCount) -> Result<Vec<f64>> {
        let ctx = finstack_quant_core::dates::DayCountContext::default();
        self.effective_dates()
            .iter()
            .filter(|&&d| d > as_of)
            .map(|&d| day_count.year_fraction(as_of, d, ctx))
            .collect()
    }

    /// Number of exercise opportunities.
    pub fn num_exercises(&self) -> usize {
        self.effective_dates().len()
    }
}

/// Co-terminal vs non-co-terminal Bermudan exercise.
///
/// This distinction affects pricing methodology and calibration:
/// - Co-terminal: All exercise dates lead to the same swap end date
/// - Non-co-terminal: Each exercise date may have a different remaining swap tenor
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum BermudanType {
    /// All exercise dates lead to same swap end date (most common)
    #[default]
    CoTerminal,
    /// Exercise dates may have different swap end dates
    NonCoTerminal,
}

impl std::fmt::Display for BermudanType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BermudanType::CoTerminal => write!(f, "co_terminal"),
            BermudanType::NonCoTerminal => write!(f, "non_co_terminal"),
        }
    }
}

impl std::str::FromStr for BermudanType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "co_terminal" => Ok(Self::CoTerminal),
            "non_co_terminal" => Ok(Self::NonCoTerminal),
            _ => Err(format!(
                "Unknown Bermudan type: '{}'. Valid: co_terminal, non_co_terminal",
                s
            )),
        }
    }
}
