use crate::instruments::common_impl::models::SABRParameters as InternalSabrParameters;
use finstack_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_core::Result;

/// Volatility model for pricing
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Default,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum VolatilityModel {
    /// Black (Lognormal) model (1976)
    #[default]
    Black,
    /// Bachelier (Normal) model
    Normal,
}

/// Public SABR parameters for swaption volatility modeling.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct SABRParameters {
    /// Initial volatility (alpha)
    pub alpha: f64,
    /// CEV exponent (beta) - typically 0 to 1
    pub beta: f64,
    /// Volatility of volatility (nu/volvol)
    pub nu: f64,
    /// Correlation between asset and volatility (rho)
    pub rho: f64,
    /// Shift parameter for handling negative rates (optional)
    pub shift: Option<f64>,
}

impl SABRParameters {
    /// Create new SABR parameters with validation.
    pub fn new(alpha: f64, beta: f64, nu: f64, rho: f64) -> Result<Self> {
        InternalSabrParameters::new(alpha, beta, nu, rho)?;
        Ok(Self {
            alpha,
            beta,
            nu,
            rho,
            shift: None,
        })
    }

    /// Create new SABR parameters with a shift for negative rates.
    pub fn new_with_shift(alpha: f64, beta: f64, nu: f64, rho: f64, shift: f64) -> Result<Self> {
        InternalSabrParameters::new_with_shift(alpha, beta, nu, rho, shift)?;
        Ok(Self {
            alpha,
            beta,
            nu,
            rho,
            shift: Some(shift),
        })
    }

    /// Create SABR parameters with equity market standard (beta=1.0).
    pub fn equity_standard(alpha: f64, nu: f64, rho: f64) -> Result<Self> {
        InternalSabrParameters::equity_standard(alpha, nu, rho)?;
        Ok(Self {
            alpha,
            beta: 1.0,
            nu,
            rho,
            shift: None,
        })
    }

    /// Create SABR parameters with interest rate market standard (beta=0.5).
    pub fn rates_standard(alpha: f64, nu: f64, rho: f64) -> Result<Self> {
        InternalSabrParameters::rates_standard(alpha, nu, rho)?;
        Ok(Self {
            alpha,
            beta: 0.5,
            nu,
            rho,
            shift: None,
        })
    }

    /// Create SABR parameters with normal model convention (beta=0.0).
    pub fn normal(alpha: f64, nu: f64, rho: f64) -> Result<Self> {
        InternalSabrParameters::normal(alpha, nu, rho)?;
        Ok(Self {
            alpha,
            beta: 0.0,
            nu,
            rho,
            shift: None,
        })
    }

    /// Create SABR parameters with lognormal model convention (beta=1.0).
    pub fn lognormal(alpha: f64, nu: f64, rho: f64) -> Result<Self> {
        InternalSabrParameters::lognormal(alpha, nu, rho)?;
        Ok(Self {
            alpha,
            beta: 1.0,
            nu,
            rho,
            shift: None,
        })
    }

    pub(crate) fn to_internal(&self) -> Result<InternalSabrParameters> {
        match self.shift {
            Some(shift) => InternalSabrParameters::new_with_shift(
                self.alpha, self.beta, self.nu, self.rho, shift,
            ),
            None => InternalSabrParameters::new(self.alpha, self.beta, self.nu, self.rho),
        }
    }
}

impl std::fmt::Display for VolatilityModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VolatilityModel::Black => write!(f, "black"),
            VolatilityModel::Normal => write!(f, "normal"),
        }
    }
}

impl std::str::FromStr for VolatilityModel {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let normalized = s.to_ascii_lowercase().replace('-', "_");
        match normalized.as_str() {
            "black" | "lognormal" | "black76" => Ok(Self::Black),
            "normal" | "bachelier" => Ok(Self::Normal),
            other => Err(format!(
                "Unknown volatility model: '{}'. Valid: black, normal",
                other
            )),
        }
    }
}

/// Swaption settlement method
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SwaptionSettlement {
    /// Physical settlement (enter into underlying swap)
    Physical,
    /// Cash settlement (receive NPV of swap)
    Cash,
}

/// Cash settlement annuity method for cash-settled swaptions.
///
/// Different methods exist for calculating the annuity factor used in cash settlement:
///
/// # Market Background
///
/// When a swaption is cash-settled, the payoff is:
/// ```text
/// Payoff = Annuity × max(S - K, 0)  [for payer]
/// ```
///
/// The choice of annuity method affects the settlement amount and can result
/// in differences of several basis points on notional for steep curves.
///
/// # ⚠️ Production Recommendation
///
/// For production systems requiring ISDA compliance, use [`IsdaParPar`](Self::IsdaParPar):
///
/// ```ignore
/// let swaption = Swaption::example()
///     .with_cash_settlement_method(CashSettlementMethod::IsdaParPar);
/// ```
///
/// The default `ParYield` method is a fast approximation suitable for:
/// - Quick calculations and screening
/// - Flat yield curve environments
/// - Short-dated swaptions where precision is less critical
///
/// # References
///
/// - ISDA 2006 Definitions, Section 18.2
/// - "Interest Rate Models" by Brigo & Mercurio, Chapter 6
/// - Bloomberg VCUB/SWPM: Uses ISDA Par-Par for production
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Default,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CashSettlementMethod {
    /// Par yield approximation using flat forward rate.
    ///
    /// ```text
    /// A = (1 - (1 + S/m)^(-N)) / S
    /// ```
    ///
    /// This is a closed-form approximation that assumes the forward swap rate
    /// is a constant discount rate. Fast but less accurate for steep curves.
    ///
    /// **Note**: This was the legacy default. As of the market standards audit,
    /// [`IsdaParPar`](Self::IsdaParPar) is now the default for ISDA compliance.
    ParYield,

    /// ISDA Par-Par method using actual swap annuity from discount curve.
    ///
    /// ```text
    /// A = Σ τ_i × DF(t_i)
    /// ```
    ///
    /// Uses the actual market discount factors to compute the annuity,
    /// matching the PV01 of the underlying swap. This is the most accurate
    /// method and matches professional library implementations.
    ///
    /// # ✅ Default (ISDA Compliant)
    ///
    /// This is the default method, matching professional library implementations
    /// (Bloomberg VCUB/SWPM, QuantLib). Suitable for:
    /// - Production pricing requiring ISDA compliance
    /// - Steep yield curve environments
    /// - Long-dated swaptions (> 5Y into > 10Y swap)
    /// - Trade confirmation matching
    /// - Any situation where cash settlement valuation precision matters
    #[default]
    IsdaParPar,

    /// Zero coupon method discounting the single payment to swap maturity.
    ///
    /// ```text
    /// A = τ × DF(T_swap)
    /// ```
    ///
    /// Rarely used in modern markets; included for completeness.
    ZeroCoupon,
}

impl std::fmt::Display for CashSettlementMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CashSettlementMethod::ParYield => write!(f, "par_yield"),
            CashSettlementMethod::IsdaParPar => write!(f, "isda_par_par"),
            CashSettlementMethod::ZeroCoupon => write!(f, "zero_coupon"),
        }
    }
}

impl std::str::FromStr for CashSettlementMethod {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let normalized = s.to_ascii_lowercase().replace('-', "_");
        match normalized.as_str() {
            "par_yield" | "paryield" => Ok(Self::ParYield),
            "isda_par_par" | "isdaparpar" | "par_par" => Ok(Self::IsdaParPar),
            "zero_coupon" | "zerocoupon" => Ok(Self::ZeroCoupon),
            other => Err(format!(
                "Unknown cash settlement method: '{}'. Valid: par_yield, isda_par_par, zero_coupon",
                other
            )),
        }
    }
}

impl std::fmt::Display for SwaptionSettlement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SwaptionSettlement::Physical => write!(f, "physical"),
            SwaptionSettlement::Cash => write!(f, "cash"),
        }
    }
}

impl std::str::FromStr for SwaptionSettlement {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let normalized = s.to_ascii_lowercase().replace('-', "_");
        match normalized.as_str() {
            "physical" => Ok(SwaptionSettlement::Physical),
            "cash" => Ok(SwaptionSettlement::Cash),
            other => Err(format!("Unknown swaption settlement: {}", other)),
        }
    }
}

/// Swaption exercise style
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Default,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SwaptionExercise {
    /// European exercise (only at expiry)
    #[default]
    European,
    /// Bermudan exercise (at discrete dates)
    Bermudan,
    /// American exercise (any time before expiry)
    American,
}

impl std::fmt::Display for SwaptionExercise {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SwaptionExercise::European => write!(f, "european"),
            SwaptionExercise::Bermudan => write!(f, "bermudan"),
            SwaptionExercise::American => write!(f, "american"),
        }
    }
}

impl std::str::FromStr for SwaptionExercise {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let normalized = s.to_ascii_lowercase().replace('-', "_");
        match normalized.as_str() {
            "european" => Ok(SwaptionExercise::European),
            "bermudan" => Ok(SwaptionExercise::Bermudan),
            "american" => Ok(SwaptionExercise::American),
            other => Err(format!("Unknown swaption exercise: {}", other)),
        }
    }
}

// ============================================================================
// Bermudan Swaption Types
// ============================================================================

/// Bermudan exercise schedule specification.
///
/// Defines the exercise dates and constraints for a Bermudan swaption.
/// Exercise dates are typically aligned with swap coupon dates.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct BermudanSchedule {
    /// Exercise dates (must be sorted, typically on swap coupon dates)
    #[schemars(with = "Vec<String>")]
    pub exercise_dates: Vec<Date>,
    /// Lockout period end (no exercise before this date)
    #[schemars(with = "Option<String>")]
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
    /// * `fixed_freq` - Fixed leg payment frequency
    pub fn co_terminal(
        first_exercise: Date,
        swap_end: Date,
        fixed_freq: Tenor,
    ) -> finstack_core::Result<Self> {
        let sched = crate::cashflow::builder::build_dates(
            first_exercise,
            swap_end,
            fixed_freq,
            StubKind::None,
            BusinessDayConvention::ModifiedFollowing, // Market standard per ISDA
            false,
            0,
            crate::cashflow::builder::calendar::WEEKENDS_ONLY_ID,
        )?;
        // Exercise dates are all coupon dates except the last one (maturity),
        // but always include the first_exercise date when it is before swap_end.
        let mut exercise_dates: Vec<Date> = Vec::new();
        if first_exercise < swap_end {
            exercise_dates.push(first_exercise);
        }
        exercise_dates.extend(
            sched
                .dates
                .into_iter()
                .filter(|&d| d > first_exercise && d < swap_end),
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
        let ctx = finstack_core::dates::DayCountContext::default();
        self.effective_dates()
            .iter()
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
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Default,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[non_exhaustive]
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
        let normalized = s.to_ascii_lowercase().replace('-', "_");
        match normalized.as_str() {
            "co_terminal" | "coterminal" => Ok(Self::CoTerminal),
            "non_co_terminal" | "noncoterminal" => Ok(Self::NonCoTerminal),
            other => Err(format!(
                "Unknown Bermudan type: '{}'. Valid: co_terminal, non_co_terminal",
                other
            )),
        }
    }
}
