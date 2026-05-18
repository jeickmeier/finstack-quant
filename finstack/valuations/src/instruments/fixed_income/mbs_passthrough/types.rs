//! Agency MBS passthrough types and implementations.
//!
//! Defines the `AgencyMbsPassthrough` instrument for agency mortgage-backed
//! securities (FNMA, FHLMC, GNMA) with prepayment modeling, servicing fees,
//! and payment delay conventions.

use crate::cashflow::builder::specs::PrepaymentModelSpec;
use crate::cashflow::traits::CashflowProvider;
use crate::impl_instrument_base;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::PricingOverrides;
use finstack_core::currency::Currency;
use finstack_core::dates::{Date, DayCount};
use finstack_core::money::Money;
use finstack_core::types::{CurveId, InstrumentId, PoolId};
use finstack_core::Result;
use time::Month;

/// Agency program enumeration.
///
/// Identifies the government-sponsored enterprise (GSE) or government agency
/// that guarantees the mortgage-backed security.
///
/// # GNMA Programs
///
/// Ginnie Mae has two distinct programs with different payment delay conventions:
/// - **GNMA I**: Single-issuer pools with a 14-day stated delay. Payments on the 15th.
/// - **GNMA II**: Multi-issuer pools with a 50-day stated delay. Payments on the 20th.
///
/// Use `GnmaI` or `GnmaII` to select the appropriate convention. The legacy `Gnma`
/// variant maps to GNMA II (the larger and more actively traded program).
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AgencyProgram {
    /// Fannie Mae (Federal National Mortgage Association)
    Fnma,
    /// Freddie Mac (Federal Home Loan Mortgage Corporation)
    Fhlmc,
    /// Ginnie Mae II (Government National Mortgage Association) - multi-issuer pools.
    ///
    /// This is the legacy variant; equivalent to `GnmaII`. Use `GnmaI` or `GnmaII`
    /// for explicit program selection.
    Gnma,
    /// Ginnie Mae I - single-issuer pools with 14-day stated delay.
    ///
    /// GNMA I securities pay on the 15th of the month following the accrual
    /// period, resulting in a 14-day stated delay from month-end.
    GnmaI,
    /// Ginnie Mae II - multi-issuer pools with 50-day stated delay.
    ///
    /// GNMA II securities pay on the 20th of the month following the accrual
    /// period, resulting in a ~50-day stated delay from accrual start. This is
    /// the larger and more actively traded GNMA program.
    GnmaII,
}

impl AgencyProgram {
    /// Returns the approximate stated delay in days for this agency program.
    ///
    /// The stated delay is measured from the **first day of the accrual period**
    /// to the payment date. It is approximate because actual payment dates
    /// follow fixed calendar-day rules (e.g. 25th of M+1 for FNMA/UMBS),
    /// so the true day count varies by month length. Use
    /// [`payment_date_for_period`](Self::payment_date_for_period) for exact
    /// payment dates.
    ///
    /// Post-Single Security Initiative (June 2019), both FNMA and FHLMC issue
    /// UMBS with a 55-day delay. Legacy FHLMC Gold PCs (45-day) and ARM PCs
    /// (75-day) should use the `payment_lag_days` override on
    /// [`AgencyMbsPassthrough`].
    ///
    /// | Program | Stated Delay | Payment Day | Payment Month |
    /// |---------|-------------|-------------|---------------|
    /// | FNMA/FHLMC (UMBS) | ~55 days | 25th | M+1 |
    /// | GNMA I | ~14 days | 15th | M |
    /// | GNMA II | ~50 days | 20th | M+1 |
    pub fn payment_lag_days(&self) -> u32 {
        match self {
            AgencyProgram::Fnma | AgencyProgram::Fhlmc => 55,
            AgencyProgram::GnmaI => 14,
            AgencyProgram::Gnma | AgencyProgram::GnmaII => 50,
        }
    }

    /// Compute the exact payment date for a given accrual period.
    ///
    /// Uses the agency's calendar-based rule rather than adding a fixed
    /// number of days, avoiding month-length distortions:
    ///
    /// | Program | Rule |
    /// |---------|------|
    /// | FNMA / FHLMC (UMBS) | 25th of the month following accrual |
    /// | GNMA I | 15th of the accrual month |
    /// | GNMA II | 20th of the month following accrual |
    ///
    /// If the resulting day falls on a weekend, the caller should adjust
    /// to the next business day separately.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_core::Error::Validation`] if `accrual_year` is
    /// outside the range supported by the calendar (the payment day-of-month
    /// — 15/20/25 — is always valid, so only an out-of-range year can fail).
    pub fn payment_date_for_period(
        &self,
        accrual_year: i32,
        accrual_month: Month,
    ) -> Result<Date> {
        let (pay_year, pay_month, pay_day) = match self {
            AgencyProgram::Fnma | AgencyProgram::Fhlmc => {
                let (y, m) = advance_month(accrual_year, accrual_month);
                (y, m, 25_u8)
            }
            AgencyProgram::GnmaI => {
                // GNMA I pays on the 15th of the accrual month (not M+1)
                (accrual_year, accrual_month, 15_u8)
            }
            AgencyProgram::Gnma | AgencyProgram::GnmaII => {
                let (y, m) = advance_month(accrual_year, accrual_month);
                (y, m, 20_u8)
            }
        };
        Date::from_calendar_date(pay_year, pay_month, pay_day).map_err(|e| {
            finstack_core::Error::Validation(format!(
                "invalid agency payment date {pay_year}-{:02}-{pay_day}: {e}",
                pay_month as u8
            ))
        })
    }

    /// Returns the canonical string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            AgencyProgram::Fnma => "FNMA",
            AgencyProgram::Fhlmc => "FHLMC",
            AgencyProgram::Gnma | AgencyProgram::GnmaII => "GNMA_II",
            AgencyProgram::GnmaI => "GNMA_I",
        }
    }

    /// Returns `true` if this is a Ginnie Mae program (any variant).
    pub fn is_gnma(&self) -> bool {
        matches!(
            self,
            AgencyProgram::Gnma | AgencyProgram::GnmaI | AgencyProgram::GnmaII
        )
    }
}

/// Advance a month by one, wrapping December -> January of the next year.
///
/// Uses [`time::Month::next`], which is total (no fallible conversion), so
/// this helper cannot panic.
fn advance_month(year: i32, month: Month) -> (i32, Month) {
    let next_year = if matches!(month, Month::December) {
        year + 1
    } else {
        year
    };
    (next_year, month.next())
}

impl std::fmt::Display for AgencyProgram {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl CashflowProvider for AgencyMbsPassthrough {
    fn notional(&self) -> Option<Money> {
        Some(self.current_face)
    }

    fn cashflow_schedule(
        &self,
        curves: &finstack_core::market_data::context::MarketContext,
        as_of: Date,
    ) -> Result<crate::cashflow::builder::CashFlowSchedule> {
        let _ = curves;
        let mut schedule =
            crate::instruments::fixed_income::mbs_passthrough::pricer::build_projected_schedule(
                self,
                as_of,
                Some(self.wam + 12),
            )?;
        schedule.meta.representation = crate::cashflow::builder::CashflowRepresentation::Projected;
        Ok(schedule)
    }
}

/// Pool type classification.
///
/// Distinguishes between generic (TBA-eligible) pools and specified pools
/// with known characteristics.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Hash,
    Default,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum PoolType {
    /// Generic pool (TBA-eligible, standard assumptions)
    #[default]
    Generic,
    /// Specified pool with known loan-level characteristics
    Specified,
}

/// Agency MBS passthrough instrument (pool or specified pool).
///
/// Represents an agency mortgage-backed security where principal and interest
/// payments from the underlying mortgage pool are passed through to investors,
/// net of servicing and guarantee fees.
///
/// # Cashflow Sign Convention
///
/// All cashflows are from the holder's (investor's) perspective:
/// - Principal and interest received are positive
/// - The initial purchase price is handled at trade level
///
/// # Payment Delay
///
/// Agency MBS have standardized payment delays measured from the **start**
/// of the accrual period (first day of the month) to the payment date:
/// - FNMA / FHLMC (UMBS): ~55 days → payment on the 25th of M+1
/// - GNMA I: ~14 days → payment on the 15th of M
/// - GNMA II: ~50 days → payment on the 20th of M+1
///
/// # Examples
///
/// ```rust
/// use finstack_valuations::instruments::fixed_income::mbs_passthrough::{
///     AgencyMbsPassthrough, AgencyProgram, PoolType,
/// };
/// use finstack_cashflows::builder::specs::PrepaymentModelSpec;
/// use finstack_core::currency::Currency;
/// use finstack_core::money::Money;
/// use finstack_core::dates::Date;
/// use finstack_core::types::{CurveId, InstrumentId};
/// use time::Month;
///
/// let mbs = AgencyMbsPassthrough::builder()
///     .id(InstrumentId::new("FN-MA1234"))
///     .pool_id("MA1234".into())
///     .agency(AgencyProgram::Fnma)
///     .pool_type(PoolType::Generic)
///     .original_face(Money::new(1_000_000.0, Currency::USD))
///     .current_face(Money::new(950_000.0, Currency::USD))
///     .current_factor(0.95)
///     .wac(0.045)
///     .pass_through_rate(0.04)
///     .servicing_fee_rate(0.0025)
///     .guarantee_fee_rate(0.0025)
///     .wam(348)
///     .issue_date(Date::from_calendar_date(2022, Month::January, 1).unwrap())
///     .maturity(Date::from_calendar_date(2052, Month::January, 1).unwrap())
///     .prepayment_model(PrepaymentModelSpec::psa(1.0))
///     .discount_curve_id(CurveId::new("USD-OIS"))
///     .day_count(finstack_core::dates::DayCount::Thirty360)
///     .build()
///     .expect("Valid MBS");
/// ```
#[derive(
    Clone,
    Debug,
    finstack_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct AgencyMbsPassthrough {
    /// Unique instrument identifier.
    pub id: InstrumentId,
    /// Pool identifier (CUSIP or internal pool ID).
    pub pool_id: PoolId,
    /// Agency program (FNMA, FHLMC, GNMA).
    pub agency: AgencyProgram,
    /// Pool type (generic or specified).
    #[builder(default)]
    #[serde(default)]
    pub pool_type: PoolType,
    /// Original face amount (initial principal balance).
    pub original_face: Money,
    /// Current face amount (remaining principal balance).
    pub current_face: Money,
    /// Current pool factor (current_face / original_face).
    pub current_factor: f64,
    /// Weighted average coupon (gross rate on underlying mortgages).
    pub wac: f64,
    /// Pass-through rate (net coupon to investor).
    pub pass_through_rate: f64,
    /// Servicing fee rate (annual, as decimal e.g., 0.0025 for 25 bps).
    ///
    /// Defaults to `0.0` when omitted.
    #[builder(default)]
    #[serde(default)]
    pub servicing_fee_rate: f64,
    /// Guarantee fee rate (annual, as decimal e.g., 0.0025 for 25 bps).
    ///
    /// Defaults to `0.0` when omitted.
    #[builder(default)]
    #[serde(default)]
    pub guarantee_fee_rate: f64,
    /// Weighted average maturity in months.
    pub wam: u32,
    /// Issue date of the pool.
    #[schemars(with = "String")]
    pub issue_date: Date,
    /// Legal maturity date.
    #[schemars(with = "String")]
    pub maturity: Date,
    /// Optional custom payment delay (overrides agency default).
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payment_lag_days: Option<u32>,
    /// Prepayment model specification.
    pub prepayment_model: PrepaymentModelSpec,
    /// Discount curve identifier for pricing.
    pub discount_curve_id: CurveId,
    /// Day count convention for accrual.
    pub day_count: DayCount,
    /// Pricing overrides (including quoted price for OAS).
    #[builder(default)]
    #[serde(default)]
    pub pricing_overrides: PricingOverrides,
    /// Attributes for scenario selection and tagging.
    #[builder(default)]
    #[serde(default)]
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,
}

impl AgencyMbsPassthrough {
    /// Create a canonical example MBS for testing and documentation.
    ///
    /// Returns a FNMA 30-year pool with realistic parameters.
    pub fn example() -> finstack_core::Result<Self> {
        use time::macros::date;
        Self::builder()
            .id(InstrumentId::new("FN-MA1234"))
            .pool_id("MA1234".into())
            .agency(AgencyProgram::Fnma)
            .pool_type(PoolType::Generic)
            .original_face(Money::new(1_000_000.0, Currency::USD))
            .current_face(Money::new(950_000.0, Currency::USD))
            .current_factor(0.95)
            .wac(0.045)
            .pass_through_rate(0.04)
            .servicing_fee_rate(0.0025)
            .guarantee_fee_rate(0.0025)
            .wam(348)
            .issue_date(date!(2022 - 01 - 01))
            .maturity(date!(2052 - 01 - 01))
            .prepayment_model(PrepaymentModelSpec::psa(1.0))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .day_count(DayCount::Thirty360)
            .pricing_overrides(PricingOverrides::default())
            .attributes(
                Attributes::new()
                    .with_tag("mbs")
                    .with_tag("agency")
                    .with_meta("program", "fnma"),
            )
            .build()
    }

    /// Get the approximate stated delay in days (from accrual start).
    ///
    /// Uses custom delay if set, otherwise uses agency-standard delay.
    /// For exact payment dates, prefer
    /// [`payment_date_for_accrual_period`](Self::payment_date_for_accrual_period).
    pub fn effective_payment_delay(&self) -> u32 {
        self.payment_lag_days
            .unwrap_or_else(|| self.agency.payment_lag_days())
    }

    /// Compute the exact payment date for an accrual period.
    ///
    /// When a custom `payment_lag_days` override is set, falls back to
    /// adding that many calendar days from `period_start`. Otherwise uses
    /// the agency's calendar-based rule via
    /// [`AgencyProgram::payment_date_for_period`].
    pub fn payment_date_for_accrual_period(&self, period_start: Date) -> Result<Date> {
        if let Some(custom_delay) = self.payment_lag_days {
            super::delay::actual_payment_date(period_start, custom_delay, false)
        } else {
            self.agency
                .payment_date_for_period(period_start.year(), period_start.month())
        }
    }

    /// Calculate seasoning in months from issue date to given date.
    pub fn seasoning_months(&self, as_of: Date) -> u32 {
        let days = (as_of - self.issue_date).whole_days();
        if days <= 0 {
            0
        } else {
            (days as f64 / 30.4375).floor() as u32
        }
    }

    /// Get SMM (single monthly mortality) for given date.
    pub fn smm(&self, as_of: Date) -> finstack_core::Result<f64> {
        let seasoning = self.seasoning_months(as_of);
        self.prepayment_model.smm(seasoning)
    }

    /// Calculate net coupon (pass-through rate) from WAC and fees.
    ///
    /// Should equal: WAC - servicing_fee_rate - guarantee_fee_rate
    pub fn calculated_net_coupon(&self) -> f64 {
        self.wac - self.servicing_fee_rate - self.guarantee_fee_rate
    }

    /// Validate that pass-through rate is consistent with WAC and fees.
    pub fn validate_coupon_consistency(&self) -> Result<()> {
        let calculated = self.calculated_net_coupon();
        let diff = (self.pass_through_rate - calculated).abs();
        if diff > 1e-6 {
            return Err(finstack_core::Error::Validation(format!(
                "Pass-through rate {} does not match WAC {} - servicing {} - g-fee {} = {}",
                self.pass_through_rate,
                self.wac,
                self.servicing_fee_rate,
                self.guarantee_fee_rate,
                calculated
            )));
        }
        Ok(())
    }
}

impl crate::instruments::common_impl::traits::CurveDependencies for AgencyMbsPassthrough {
    fn curve_dependencies(
        &self,
    ) -> finstack_core::Result<crate::instruments::common_impl::traits::InstrumentCurves> {
        crate::instruments::common_impl::traits::InstrumentCurves::builder()
            .discount(self.discount_curve_id.clone())
            .build()
    }
}

impl crate::instruments::common_impl::traits::Instrument for AgencyMbsPassthrough {
    impl_instrument_base!(crate::pricer::InstrumentType::AgencyMbsPassthrough);

    fn base_value(
        &self,
        market: &finstack_core::market_data::context::MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> finstack_core::Result<finstack_core::money::Money> {
        crate::instruments::fixed_income::mbs_passthrough::pricer::price_mbs(self, market, as_of)
    }

    fn effective_start_date(&self) -> Option<Date> {
        Some(self.issue_date)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agency_program_payment_delays() {
        assert_eq!(AgencyProgram::Fnma.payment_lag_days(), 55);
        assert_eq!(AgencyProgram::Fhlmc.payment_lag_days(), 55);
        assert_eq!(AgencyProgram::Gnma.payment_lag_days(), 50);
        assert_eq!(AgencyProgram::GnmaI.payment_lag_days(), 14);
        assert_eq!(AgencyProgram::GnmaII.payment_lag_days(), 50);
    }

    #[test]
    fn test_payment_date_for_period_fnma() {
        let pay = AgencyProgram::Fnma
            .payment_date_for_period(2024, Month::January)
            .expect("valid payment date");
        assert_eq!(pay.month(), Month::February);
        assert_eq!(pay.day(), 25);

        let pay_dec = AgencyProgram::Fnma
            .payment_date_for_period(2024, Month::December)
            .expect("valid payment date");
        assert_eq!(pay_dec.year(), 2025);
        assert_eq!(pay_dec.month(), Month::January);
        assert_eq!(pay_dec.day(), 25);
    }

    #[test]
    fn test_payment_date_for_period_gnma_i() {
        let pay = AgencyProgram::GnmaI
            .payment_date_for_period(2024, Month::March)
            .expect("valid payment date");
        assert_eq!(pay.month(), Month::March);
        assert_eq!(pay.day(), 15);
    }

    #[test]
    fn test_payment_date_for_period_gnma_ii() {
        let pay = AgencyProgram::GnmaII
            .payment_date_for_period(2024, Month::January)
            .expect("valid payment date");
        assert_eq!(pay.month(), Month::February);
        assert_eq!(pay.day(), 20);
    }

    /// Item 15 regression: `payment_date_for_period` must return a `Result`
    /// rather than `unreachable!()`-panicking inside library code.
    ///
    /// The happy path (every agency, December wrap included) must yield `Ok`
    /// with the correct date; the function no longer contains a panicking
    /// `unreachable!` branch.
    #[test]
    fn payment_date_for_period_returns_result_not_panic() {
        // Every agency, every month — all must be Ok.
        for agency in [
            AgencyProgram::Fnma,
            AgencyProgram::Fhlmc,
            AgencyProgram::Gnma,
            AgencyProgram::GnmaI,
            AgencyProgram::GnmaII,
        ] {
            for month_num in 1u8..=12 {
                let month = Month::try_from(month_num).expect("valid month");
                let res = agency.payment_date_for_period(2024, month);
                assert!(
                    res.is_ok(),
                    "{agency:?} {month:?} should yield Ok, got {res:?}"
                );
            }
        }

        // December wrap rolls into the next calendar year.
        let dec = AgencyProgram::Fnma
            .payment_date_for_period(2024, Month::December)
            .expect("December wrap should be Ok");
        assert_eq!(dec.year(), 2025);
        assert_eq!(dec.month(), Month::January);
    }

    #[test]
    fn test_agency_program_display() {
        assert_eq!(AgencyProgram::Fnma.as_str(), "FNMA");
        assert_eq!(AgencyProgram::Fhlmc.as_str(), "FHLMC");
        assert_eq!(AgencyProgram::Gnma.as_str(), "GNMA_II");
        assert_eq!(AgencyProgram::GnmaI.as_str(), "GNMA_I");
        assert_eq!(AgencyProgram::GnmaII.as_str(), "GNMA_II");
    }

    #[test]
    fn test_agency_program_is_gnma() {
        assert!(!AgencyProgram::Fnma.is_gnma());
        assert!(!AgencyProgram::Fhlmc.is_gnma());
        assert!(AgencyProgram::Gnma.is_gnma());
        assert!(AgencyProgram::GnmaI.is_gnma());
        assert!(AgencyProgram::GnmaII.is_gnma());
    }

    #[test]
    fn test_mbs_example() {
        let mbs = AgencyMbsPassthrough::example().expect("AgencyMbsPassthrough example is valid");
        assert_eq!(mbs.id.as_str(), "FN-MA1234");
        assert_eq!(mbs.agency, AgencyProgram::Fnma);
        assert_eq!(mbs.pool_type, PoolType::Generic);
        assert!((mbs.current_factor - 0.95).abs() < 1e-10);
        assert!(mbs.attributes.has_tag("mbs"));
    }

    #[test]
    fn test_effective_payment_delay() {
        let mbs = AgencyMbsPassthrough::example().expect("AgencyMbsPassthrough example is valid");
        assert_eq!(mbs.effective_payment_delay(), 55);

        let mut mbs_custom = mbs.clone();
        mbs_custom.payment_lag_days = Some(45);
        assert_eq!(mbs_custom.effective_payment_delay(), 45);
    }

    #[test]
    fn test_payment_date_for_accrual_period_calendar() {
        let mbs = AgencyMbsPassthrough::example().expect("AgencyMbsPassthrough example is valid"); // FNMA
        let period_start = Date::from_calendar_date(2024, Month::February, 1).expect("valid date");
        let pay = mbs
            .payment_date_for_accrual_period(period_start)
            .expect("valid");
        assert_eq!(pay.month(), Month::March);
        assert_eq!(pay.day(), 25);
    }

    #[test]
    fn test_payment_date_for_accrual_period_custom_delay() {
        let mut mbs =
            AgencyMbsPassthrough::example().expect("AgencyMbsPassthrough example is valid");
        mbs.payment_lag_days = Some(45);
        let period_start = Date::from_calendar_date(2024, Month::January, 1).expect("valid date");
        let pay = mbs
            .payment_date_for_accrual_period(period_start)
            .expect("valid");
        assert_eq!(pay.month(), Month::February);
        assert_eq!(pay.day(), 15);
    }

    #[test]
    fn test_seasoning_months() {
        let mbs = AgencyMbsPassthrough::example().expect("AgencyMbsPassthrough example is valid");
        let as_of = Date::from_calendar_date(2024, Month::January, 1).expect("valid date");
        let seasoning = mbs.seasoning_months(as_of);
        assert!((23..=24).contains(&seasoning));
    }

    #[test]
    fn test_calculated_net_coupon() {
        let mbs = AgencyMbsPassthrough::example().expect("AgencyMbsPassthrough example is valid");
        let calculated = mbs.calculated_net_coupon();
        assert!((calculated - 0.04).abs() < 1e-10);
    }

    #[test]
    fn test_coupon_consistency_validation() {
        let mbs = AgencyMbsPassthrough::example().expect("AgencyMbsPassthrough example is valid");
        assert!(mbs.validate_coupon_consistency().is_ok());

        let mut bad_mbs = mbs.clone();
        bad_mbs.pass_through_rate = 0.05;
        assert!(bad_mbs.validate_coupon_consistency().is_err());
    }

    #[test]
    fn test_smm_calculation() {
        let mbs = AgencyMbsPassthrough::example().expect("AgencyMbsPassthrough example is valid");
        let as_of = Date::from_calendar_date(2024, Month::January, 1).expect("valid date");
        let smm = mbs.smm(as_of).expect("smm should succeed");
        assert!(smm > 0.0 && smm < 0.02);
    }

    #[test]
    fn test_mbs_serde_roundtrip() {
        let mbs = AgencyMbsPassthrough::example().expect("AgencyMbsPassthrough example is valid");
        let json = serde_json::to_string(&mbs).expect("serialize");
        let deserialized: AgencyMbsPassthrough = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(mbs.id.as_str(), deserialized.id.as_str());
        assert_eq!(mbs.agency, deserialized.agency);
    }

    #[test]
    fn test_serde_defaults_fee_rates_to_zero_when_omitted() {
        let mut value = serde_json::to_value(
            AgencyMbsPassthrough::example().expect("AgencyMbsPassthrough example is valid"),
        )
        .expect("serialize");
        let obj = value
            .as_object_mut()
            .expect("AgencyMbsPassthrough should serialize to an object");
        obj.remove("servicing_fee_rate");
        obj.remove("guarantee_fee_rate");

        let mbs: AgencyMbsPassthrough = serde_json::from_value(value).expect("deserialize");
        assert_eq!(mbs.servicing_fee_rate, 0.0);
        assert_eq!(mbs.guarantee_fee_rate, 0.0);
    }
}
