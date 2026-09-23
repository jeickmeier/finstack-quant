//! Agency MBS passthrough types and implementations.
//!
//! Defines the `AgencyMbsPassthrough` instrument for agency mortgage-backed
//! securities (FNMA, FHLMC, GNMA) with prepayment modeling, servicing fees,
//! and payment delay conventions.

use crate::cashflow::builder::specs::PrepaymentModelSpec;
use crate::impl_instrument_base;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::InstrumentPricingOverrides;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId, PoolId};
use finstack_quant_core::Result;
use time::Month;

/// Agency program enumeration.
///
/// Identifies the government-sponsored enterprise (GSE) or government agency
/// that guarantees the mortgage-backed security.
///
/// # GNMA Programs
///
/// Ginnie Mae has two distinct programs with different payment delay conventions:
/// - **GNMA I**: Single-issuer pools with a 45-day stated delay. Payments on the 15th.
/// - **GNMA II**: Multi-issuer pools with a 50-day stated delay. Payments on the 20th.
///
/// Use `GnmaI` or `GnmaII` to select the appropriate convention. Their
/// persisted values are exactly `GNMA_I` and `GNMA_II`, respectively.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AgencyProgram {
    /// Fannie Mae (Federal National Mortgage Association)
    Fnma,
    /// Freddie Mac (Federal Home Loan Mortgage Corporation)
    Fhlmc,
    /// Ginnie Mae I - single-issuer pools with 45-day stated delay.
    ///
    /// GNMA I securities pay on the 15th of the month following the accrual
    /// period, resulting in a 45-day stated delay from accrual start.
    GnmaI,
    /// Ginnie Mae II - multi-issuer pools with 50-day stated delay.
    ///
    /// GNMA II securities pay on the 20th of the month following the accrual
    /// period, resulting in a ~50-day stated delay from accrual start. This is
    /// the larger and more actively traded GNMA program.
    #[serde(rename = "GNMA_II")]
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
    /// | GNMA I | ~45 days | 15th | M+1 |
    /// | GNMA II | ~50 days | 20th | M+1 |
    pub fn payment_lag_days(&self) -> u32 {
        match self {
            AgencyProgram::Fnma | AgencyProgram::Fhlmc => 55,
            AgencyProgram::GnmaI => 45,
            AgencyProgram::GnmaII => 50,
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
    /// | GNMA I | 15th of the month following accrual |
    /// | GNMA II | 20th of the month following accrual |
    ///
    /// Payments roll to the next business day on the `usny` Federal Reserve
    /// calendar, including bank holidays.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::Error::Validation`] if `accrual_year` is
    /// outside the range supported by the calendar (the payment day-of-month
    /// — 15/20/25 — is always valid, so only an out-of-range year can fail).
    ///
    /// # Arguments
    ///
    /// * `accrual_year` - Calendar year of the MBS accrual month (must be a valid `time` calendar year).
    /// * `accrual_month` - Accrual month whose following-month payment date is
    ///   computed and adjusted on the Federal Reserve banking calendar.
    pub fn payment_date_for_period(&self, accrual_year: i32, accrual_month: Month) -> Result<Date> {
        let (pay_year, pay_month, pay_day) = match self {
            AgencyProgram::Fnma | AgencyProgram::Fhlmc => {
                let (y, m) = advance_month(accrual_year, accrual_month);
                (y, m, 25_u8)
            }
            AgencyProgram::GnmaI => {
                let (y, m) = advance_month(accrual_year, accrual_month);
                (y, m, 15_u8)
            }
            AgencyProgram::GnmaII => {
                let (y, m) = advance_month(accrual_year, accrual_month);
                (y, m, 20_u8)
            }
        };
        let payment = Date::from_calendar_date(pay_year, pay_month, pay_day).map_err(|e| {
            finstack_quant_core::Error::Validation(format!(
                "invalid agency payment date {pay_year}-{:02}-{pay_day}: {e}",
                pay_month as u8
            ))
        })?;
        finstack_quant_core::dates::adjust(
            payment,
            finstack_quant_core::dates::BusinessDayConvention::Following,
            finstack_quant_core::dates::calendar_by_id_strict("usny")?,
        )
    }

    /// Returns the canonical string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            AgencyProgram::Fnma => "FNMA",
            AgencyProgram::Fhlmc => "FHLMC",
            AgencyProgram::GnmaII => "GNMA_II",
            AgencyProgram::GnmaI => "GNMA_I",
        }
    }

    /// Returns `true` if this is a Ginnie Mae program (any variant).
    pub fn is_gnma(&self) -> bool {
        matches!(self, AgencyProgram::GnmaI | AgencyProgram::GnmaII)
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

impl finstack_quant_cashflows::CashflowScheduleSource for AgencyMbsPassthrough {
    fn notional(&self) -> finstack_quant_core::Result<Option<Money>> {
        Ok(Some(self.current_face))
    }

    fn raw_cashflow_schedule(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: Date,
    ) -> Result<crate::cashflow::builder::CashFlowSchedule> {
        let _ = curves;
        let schedule =
            crate::instruments::fixed_income::mbs_passthrough::pricer::build_projected_schedule(
                self,
                as_of,
                Some(self.wam + 12),
            )?;
        Ok(schedule
            .with_representation(crate::cashflow::builder::CashflowRepresentation::Projected))
    }
}

/// Pool type classification.
///
/// Distinguishes between generic (TBA-eligible) pools and specified pools
/// with known characteristics.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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
/// - GNMA I: ~45 days → payment on the 15th of M+1
/// - GNMA II: ~50 days → payment on the 20th of M+1
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations::instruments::fixed_income::mbs_passthrough::{
///     AgencyMbsPassthrough, AgencyProgram, PoolType,
/// };
/// use finstack_quant_cashflows::builder::specs::PrepaymentModelSpec;
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::money::Money;
/// use finstack_quant_core::dates::Date;
/// use finstack_quant_core::types::{CurveId, InstrumentId};
/// use time::Month;
///
/// let mbs = AgencyMbsPassthrough::builder()
///     .id(InstrumentId::new("FN-MA1234"))
///     .pool_id("MA1234".into())
///     .agency(AgencyProgram::Fnma)
///     .pool_type(PoolType::Generic)
///     .original_face(Money::from((1_000_000_i64, Currency::USD)))
///     .current_face(Money::from((950_000_i64, Currency::USD)))
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
///     .day_count(finstack_quant_core::dates::DayCount::Thirty360)
///     .build()
///     .expect("Valid MBS");
/// ```
#[derive(
    PartialEq,
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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
    /// Servicing fee rate (annual, as decimal e.g., 0.0025 for 25 bp).
    ///
    /// Defaults to `0.0` when omitted.
    #[builder(default)]
    #[serde(default)]
    pub servicing_fee_rate: f64,
    /// Guarantee fee rate (annual, as decimal e.g., 0.0025 for 25 bp).
    ///
    /// Defaults to `0.0` when omitted.
    #[builder(default)]
    #[serde(default)]
    pub guarantee_fee_rate: f64,
    /// Remaining weighted average maturity in months as of the valuation
    /// date (current WAM, not the original term). Pool age (WALA) for
    /// seasoning ramps is derived separately from `issue_date`.
    pub wam: u32,
    /// Issue date of the pool.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub issue_date: Date,
    /// End date of the latest accrual period whose delayed P&I payment has
    /// settled and is already reflected in `current_face`.
    ///
    /// When omitted, pricing infers the latest paid period from the agency
    /// payment-delay rule and `as_of`.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "finstack_quant_core::wire::optional_date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<finstack_quant_core::wire::DateWire>")
    )]
    pub last_paid_accrual_end: Option<Date>,
    /// Legal maturity date.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
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
    pub fn example() -> finstack_quant_core::Result<Self> {
        use time::macros::date;
        Self::builder()
            .id(InstrumentId::new("FN-MA1234"))
            .pool_id("MA1234".into())
            .agency(AgencyProgram::Fnma)
            .pool_type(PoolType::Generic)
            .original_face(Money::from((1_000_000_i64, Currency::USD)))
            .current_face(Money::from((950_000_i64, Currency::USD)))
            .current_factor(0.95)
            .wac(0.045)
            .pass_through_rate(0.04)
            .servicing_fee_rate(0.0025)
            .guarantee_fee_rate(0.0025)
            .wam(348)
            .issue_date(date!(2022 - 01 - 01))
            .last_paid_accrual_end_opt(None)
            .maturity(date!(2052 - 01 - 01))
            .prepayment_model(PrepaymentModelSpec::psa(1.0))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .day_count(DayCount::Thirty360)
            .instrument_pricing_overrides(InstrumentPricingOverrides::default())
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
    ///
    /// # Arguments
    ///
    /// * `period_start` - Accrual-period start date. When `payment_lag_days` is unset, only the
    ///   year and month feed the agency rule; when a custom lag is set, this date is the lag origin.
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
    pub fn smm(&self, as_of: Date) -> finstack_quant_core::Result<f64> {
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
            return Err(finstack_quant_core::Error::Validation(format!(
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

impl crate::instruments::common_impl::traits::Instrument for AgencyMbsPassthrough {
    impl_instrument_base!(crate::pricer::InstrumentType::AgencyMbsPassthrough);

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<
        crate::instruments::common_impl::dependencies::MarketDependencies,
    > {
        let mut deps = crate::instruments::common_impl::dependencies::MarketDependencies::new();
        deps.add_discount_curve(self.discount_curve_id.clone());
        Ok(deps)
    }

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        self.validate_coupon_consistency()?;
        crate::instruments::common_impl::validation::validate_date_range_strict(
            self.issue_date,
            self.maturity,
            "MBS issue-to-maturity",
        )?;
        if let Some(last_paid) = self.last_paid_accrual_end {
            if last_paid < self.issue_date || last_paid > self.maturity {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "MBS last_paid_accrual_end {last_paid} must fall between issue date {} and maturity {}",
                    self.issue_date, self.maturity
                )));
            }
        }
        Ok(())
    }

    fn base_value(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        crate::instruments::fixed_income::mbs_passthrough::pricer::price_mbs(self, market, as_of)
    }

    fn effective_start_date(&self) -> Option<Date> {
        Some(self.issue_date)
    }

    fn rate_risk_rebuild(
        &self,
        base: &finstack_quant_core::market_data::context::MarketContext,
        bumped: &finstack_quant_core::market_data::context::MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Option<Box<dyn crate::instruments::Instrument>>> {
        let pool = super::metrics::duration::rate_risk_pool(self, base, bumped, as_of)?;
        Ok(Some(Box::new(pool)))
    }

    crate::impl_focused_pricing_overrides!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agency_convention_vector_is_stable() {
        let cases = [
            (
                AgencyProgram::Fnma,
                "FNMA",
                55,
                false,
                Date::from_calendar_date(2024, Month::February, 26).expect("valid date"),
            ),
            (
                AgencyProgram::Fhlmc,
                "FHLMC",
                55,
                false,
                Date::from_calendar_date(2024, Month::February, 26).expect("valid date"),
            ),
            (
                AgencyProgram::GnmaI,
                "GNMA_I",
                45,
                true,
                Date::from_calendar_date(2024, Month::February, 15).expect("valid date"),
            ),
            (
                AgencyProgram::GnmaII,
                "GNMA_II",
                50,
                true,
                Date::from_calendar_date(2024, Month::February, 20).expect("valid date"),
            ),
        ];

        for (agency, canonical_name, payment_lag, is_gnma, expected_payment_date) in cases {
            assert_eq!(agency.as_str(), canonical_name);
            assert_eq!(agency.to_string(), canonical_name);
            assert_eq!(agency.payment_lag_days(), payment_lag);
            assert_eq!(agency.is_gnma(), is_gnma);
            assert_eq!(
                agency
                    .payment_date_for_period(2024, Month::January)
                    .expect("valid payment date"),
                expected_payment_date
            );
        }
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
    fn agency_program_accepts_only_canonical_gnma_ii() {
        let agency =
            serde_json::from_str::<AgencyProgram>("\"GNMA_II\"").expect("canonical GNMA spelling");
        assert_eq!(agency, AgencyProgram::GnmaII);
        for legacy in ["GNMA", "GNMA_I_I"] {
            serde_json::from_str::<AgencyProgram>(&format!("\"{legacy}\""))
                .expect_err("legacy GNMA spelling must be rejected");
        }
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

        let mut mbs_custom = mbs;
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

        let mut bad_mbs = mbs;
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

#[cfg(test)]
mod production_mortgage_audit {
    use super::*;
    use time::macros::date;

    #[test]
    fn agency_payments_follow_the_accrual_month_and_banking_calendar() {
        assert_eq!(AgencyProgram::GnmaI.payment_lag_days(), 45);
        assert_eq!(
            AgencyProgram::GnmaI
                .payment_date_for_period(2024, Month::January)
                .expect("payment"),
            date!(2024 - 02 - 15)
        );
        // February 15 is Sunday and February 16 is a Federal Reserve holiday.
        assert_eq!(
            AgencyProgram::GnmaI
                .payment_date_for_period(2026, Month::January)
                .expect("payment"),
            date!(2026 - 02 - 17)
        );
        assert_eq!(
            AgencyProgram::Fnma
                .payment_date_for_period(2026, Month::March)
                .expect("payment"),
            date!(2026 - 04 - 27)
        );
    }

    #[test]
    fn agency_schedule_uses_calendar_payment_dates() {
        let starts = [date!(2026 - 02 - 01), date!(2026 - 03 - 01)];
        let schedule =
            super::super::delay::payment_schedule(&starts, AgencyProgram::Fnma).expect("schedule");
        assert_eq!(schedule[0].1, date!(2026 - 03 - 25));
        assert_eq!(schedule[1].1, date!(2026 - 04 - 27));
    }
}
