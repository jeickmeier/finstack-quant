//! Variation margin calculator.
//!
//! Implements ISDA CSA variation margin calculation logic including
//! threshold, MTA, and rounding rules.

use crate::types::{CsaSpec, MarginCall, MarginTenor};
use finstack_quant_core::dates::{adjust, calendar_by_id, BusinessDayConvention, Date, DateExt};
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;
use tracing::{debug, warn};

/// Variation margin calculation result.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VmResult {
    /// Calculation date
    #[cfg_attr(feature = "json-schema", schemars(with = "String"))]
    pub date: Date,

    /// Gross mark-to-market exposure
    pub gross_exposure: Money,

    /// Net exposure after applying threshold and independent amount
    pub net_exposure: Money,

    /// Amount to post to the counterparty, including returned collateral
    pub post_amount: Money,

    /// Amount to collect from the counterparty, including returned collateral
    pub collect_amount: Money,

    /// Settlement date for the margin transfer
    #[cfg_attr(feature = "json-schema", schemars(with = "String"))]
    pub settlement_date: Date,

    /// CSA threshold applied symmetrically to `|gross_exposure|`, in the CSA
    /// base currency (non-negative).
    pub threshold: Money,

    /// CSA independent amount added to the threshold-adjusted exposure to
    /// give `net_exposure`, in the CSA base currency.
    pub independent_amount: Money,

    /// Signed collateral balance netted against `net_exposure`, exactly as
    /// passed to the calculator: positive when the desk holds collateral,
    /// negative when the desk has posted it.
    pub collateral_balance: Money,

    /// Signed credit support amount before the minimum-transfer test and
    /// rounding: `net_exposure − collateral_balance`. Positive means collect
    /// from the counterparty, negative means the desk pays (post or return).
    pub unrounded_call: Money,

    /// CSA minimum transfer amount in the CSA base currency. When
    /// `|unrounded_call| < mta`, no transfer is made and both `post_amount`
    /// and `collect_amount` are zero.
    pub mta: Money,

    /// CSA rounding increment in the CSA base currency. A transfer that
    /// passes the MTA test has its magnitude rounded to a multiple of this
    /// increment: up when `unrounded_call` has the same sign as
    /// `net_exposure` (a delivery), down otherwise (a return). Zero disables
    /// rounding.
    pub rounding_increment: Money,
}

impl VmResult {
    /// Get net cash outflow: post minus collect, positive when the desk pays.
    #[must_use]
    pub fn net_margin(&self) -> Money {
        if self.post_amount.amount() > 0.0 {
            self.post_amount
        } else {
            self.collect_amount.checked_neg()
        }
    }

    /// Check if a margin call is required.
    #[must_use]
    pub fn requires_call(&self) -> bool {
        self.post_amount.amount() > 0.0 || self.collect_amount.amount() > 0.0
    }
}

/// Variation margin calculator following ISDA CSA rules.
///
/// Calculates variation margin based on mark-to-market exposure,
/// applying threshold, MTA, independent amount, and rounding rules.
///
/// # ISDA CSA Formula
///
/// Credit support follows [`crate::VmParameters::calculate_margin_call`] (symmetric
/// threshold in `|Exposure|`, bilateral handling of signed exposure). Delivery
/// and collection amounts split that signed amount by cashflow direction.
///
/// Implementation delegates CSA/MTA/rounding logic to
/// `VmParameters::calculate_margin_call` to ensure consistent behavior
/// across margin utilities.
///
/// # Example
///
/// ```
/// use finstack_quant_margin::{VmCalculator, CsaSpec};
/// use finstack_quant_core::money::Money;
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::dates::Date;
///
/// # fn main() -> finstack_quant_core::Result<()> {
/// let csa = CsaSpec::usd_regulatory()?;
/// let calc = VmCalculator::new(csa);
///
/// let exposure = Money::from((5_000_000_i64, Currency::USD));
/// let posted = Money::from((3_000_000_i64, Currency::USD));
/// let as_of = Date::from_calendar_date(2025, time::Month::January, 15).expect("valid");
///
/// let result = calc.calculate(exposure, posted, as_of)?;
/// println!("Cash to post: {}", result.post_amount);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct VmCalculator {
    csa: CsaSpec,
}

impl VmCalculator {
    /// Borrow the agreement applied to every calculation and generated margin call.
    #[must_use]
    pub fn get_csa(&self) -> &CsaSpec {
        &self.csa
    }

    fn calendar_for_csa(&self) -> Result<&'static dyn finstack_quant_core::dates::HolidayCalendar> {
        calendar_by_id(&self.csa.calendar_id).ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "CSA '{}' calendar '{}' is not registered",
                self.csa.id, self.csa.calendar_id
            ))
        })
    }

    fn add_business_days(&self, date: Date, days: i32) -> Result<Date> {
        if days == 0 {
            return Ok(date);
        }
        date.add_business_days(days, self.calendar_for_csa()?)
    }

    fn adjust_to_business_day(&self, date: Date) -> Result<Date> {
        adjust(
            date,
            BusinessDayConvention::Following,
            self.calendar_for_csa()?,
        )
    }

    /// Create a new VM calculator with the given CSA specification.
    #[must_use]
    pub fn new(csa: CsaSpec) -> Self {
        Self { csa }
    }

    /// Calculate variation margin given current exposure and posted collateral.
    ///
    /// # Arguments
    ///
    /// * `exposure` - Current mark-to-market exposure (positive = counterparty owes us)
    /// * `posted_collateral` - Signed collateral balance, positive held and negative posted, including pending agreed calls.
    /// * `as_of` - Calculation date
    ///
    /// # Returns
    ///
    /// [`VmResult`] with desk post and collect amounts.
    pub fn calculate(
        &self,
        exposure: Money,
        posted_collateral: Money,
        as_of: Date,
    ) -> Result<VmResult> {
        self.csa.validate()?;
        let currency = self.csa.base_currency;

        if exposure.currency() != currency {
            warn!(expected = %currency, got = %exposure.currency(), "VM exposure currency mismatch");
            return Err(finstack_quant_core::Error::Validation(format!(
                "VM exposure currency mismatch: expected {}, got {}",
                currency,
                exposure.currency()
            )));
        }
        if posted_collateral.currency() != currency {
            warn!(expected = %currency, got = %posted_collateral.currency(), "VM collateral currency mismatch");
            return Err(finstack_quant_core::Error::Validation(format!(
                "VM collateral currency mismatch: expected {}, got {}",
                currency,
                posted_collateral.currency()
            )));
        }

        let vm_params = &self.csa.vm_params;
        let steps = vm_params.margin_call_steps(exposure, posted_collateral)?;
        let net_call = steps.call;
        let post = Money::new((-net_call.amount()).max(0.0), currency)?;
        let collect = Money::new(net_call.amount().max(0.0), currency)?;

        let settlement_date = self.calculate_settlement_date(as_of)?;

        Ok(VmResult {
            date: as_of,
            gross_exposure: exposure,
            net_exposure: steps.required,
            post_amount: post,
            collect_amount: collect,
            settlement_date,
            threshold: vm_params.threshold,
            independent_amount: vm_params.independent_amount,
            collateral_balance: posted_collateral,
            unrounded_call: steps.unrounded,
            mta: vm_params.mta,
            rounding_increment: vm_params.rounding,
        })
    }

    /// Generate a series of margin calls from an exposure time series.
    ///
    /// # Arguments
    ///
    /// * `exposures` - Time series of (date, exposure) pairs
    /// * `initial_collateral` - Signed collateral balance: positive held, negative posted.
    ///
    /// # Returns
    ///
    /// Vector of [`MarginCall`] events.
    pub fn generate_margin_calls(
        &self,
        exposures: &[(Date, Money)],
        initial_collateral: Money,
    ) -> Result<Vec<MarginCall>> {
        let mut calls = Vec::new();
        let mut current_collateral = initial_collateral;

        for (date, exposure) in exposures {
            let result = self.calculate(*exposure, current_collateral, *date)?;

            if result.requires_call() {
                let settlement_date = result.settlement_date;

                if result.post_amount.amount() > 0.0 {
                    debug!(date = %date, amount = result.post_amount.amount(), "VM post margin call");
                    calls.push(MarginCall::vm_post(
                        *date,
                        settlement_date,
                        result.post_amount,
                        *exposure,
                        self.csa.vm_params.threshold,
                        self.csa.vm_params.mta,
                    ));
                    current_collateral = current_collateral.checked_sub(result.post_amount)?;
                } else if result.collect_amount.amount() > 0.0 {
                    debug!(date = %date, amount = result.collect_amount.amount(), "VM collect margin call");
                    calls.push(MarginCall::vm_collect(
                        *date,
                        settlement_date,
                        result.collect_amount,
                        *exposure,
                        self.csa.vm_params.threshold,
                        self.csa.vm_params.mta,
                    ));
                    current_collateral = current_collateral.checked_add(result.collect_amount)?;
                }
            }
        }

        Ok(calls)
    }

    /// Generate margin call dates based on frequency.
    pub fn margin_call_dates(&self, start: Date, end: Date) -> Result<Vec<Date>> {
        let mut dates = Vec::new();
        let adjusted_start = self.adjust_to_business_day(start)?;
        if matches!(self.csa.vm_params.frequency, MarginTenor::OnDemand) {
            let adjusted_end = self.adjust_to_business_day(end)?;
            if adjusted_start > adjusted_end {
                return Ok(dates);
            }
            dates.push(adjusted_start);
            if adjusted_end != adjusted_start {
                dates.push(adjusted_end);
            }
            return Ok(dates);
        }
        if matches!(self.csa.vm_params.frequency, MarginTenor::Daily) {
            let mut current = adjusted_start;
            while current <= end {
                dates.push(current);
                current = self.add_business_days(current, 1)?;
            }
            return Ok(dates);
        }

        // Weekly/monthly contracts retain their unadjusted roll anchor. If a
        // holiday moves one call date, the adjustment must not permanently
        // move every later contractual date.
        let mut period = 0_i32;
        loop {
            let contractual = match self.csa.vm_params.frequency {
                MarginTenor::Weekly => start + time::Duration::weeks(i64::from(period)),
                MarginTenor::Monthly => start.add_months(period)?,
                MarginTenor::Daily | MarginTenor::OnDemand => return Ok(dates),
            };
            if contractual > end {
                break;
            }
            let adjusted = self.adjust_to_business_day(contractual)?;
            if dates.last().copied() != Some(adjusted) {
                dates.push(adjusted);
            }
            period = period.checked_add(1).ok_or_else(|| {
                finstack_quant_core::Error::Validation(
                    "margin-call schedule period overflow".into(),
                )
            })?;
        }

        Ok(dates)
    }

    /// Calculate settlement date based on lag.
    fn calculate_settlement_date(&self, call_date: Date) -> Result<Date> {
        let lag = self.csa.vm_params.settlement_lag as i32;
        self.add_business_days(call_date, lag)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{EligibleCollateralSchedule, VmParameters};
    use crate::MarginCallType;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::types::CurveId;
    use time::Month;

    fn test_date(y: i32, m: u8, d: u8) -> Date {
        Date::from_calendar_date(y, Month::try_from(m).expect("valid month"), d)
            .expect("valid date")
    }

    fn threshold_csa() -> CsaSpec {
        CsaSpec {
            id: "TEST".to_string(),
            base_currency: Currency::USD,
            calendar_id: "usny".to_string(),
            vm_params: VmParameters::with_threshold(
                Money::from((1_000_000_i64, Currency::USD)),
                Money::from((100_000_i64, Currency::USD)),
            ),
            im_params: None,
            eligible_collateral: EligibleCollateralSchedule::default(),
            call_timing: crate::registry::embedded_registry()
                .expect("registry should load")
                .defaults
                .timing
                .standard
                .clone(),
            collateral_curve_id: CurveId::new("USD-OIS"),
        }
    }

    #[test]
    fn vm_calculator_no_threshold() {
        let csa = CsaSpec::usd_regulatory().expect("registry should load");
        let calc = VmCalculator::new(csa);

        let exposure = Money::from((5_000_000_i64, Currency::USD));
        let posted = Money::from((3_000_000_i64, Currency::USD));
        let result = calc
            .calculate(exposure, posted, test_date(2025, 1, 15))
            .expect("calc ok");

        // With zero threshold, delivery = exposure - posted = 2M
        assert_eq!(result.collect_amount.amount(), 2_000_000.0);
        assert_eq!(result.post_amount.amount(), 0.0);
    }

    #[test]
    fn vm_calculator_bilateral_negative_exposure_delivery() {
        let csa = CsaSpec::usd_regulatory().expect("registry should load");
        let calc = VmCalculator::new(csa);

        let exposure = Money::from((-2_000_000_i64, Currency::USD));
        let posted = Money::from((0_i64, Currency::USD));
        let result = calc
            .calculate(exposure, posted, test_date(2025, 1, 15))
            .expect("calc ok");

        assert_eq!(result.post_amount.amount(), 2_000_000.0);
        assert_eq!(result.collect_amount.amount(), 0.0);
    }

    #[test]
    fn vm_calculator_with_threshold() {
        let csa = threshold_csa();
        let calc = VmCalculator::new(csa);

        // Exposure below threshold: no margin call
        let exposure = Money::from((500_000_i64, Currency::USD));
        let posted = Money::from((0_i64, Currency::USD));
        let result = calc
            .calculate(exposure, posted, test_date(2025, 1, 15))
            .expect("calc ok");

        assert_eq!(result.post_amount.amount(), 0.0);
        assert!(!result.requires_call());
    }

    #[test]
    fn vm_calculator_return_excess() {
        let csa = CsaSpec::usd_regulatory().expect("registry should load");
        let calc = VmCalculator::new(csa);

        // Exposure dropped, have excess collateral
        let exposure = Money::from((1_000_000_i64, Currency::USD));
        let posted = Money::from((3_000_000_i64, Currency::USD));
        let result = calc
            .calculate(exposure, posted, test_date(2025, 1, 15))
            .expect("calc ok");

        // Return = posted - required = 3M - 1M = 2M
        assert_eq!(result.collect_amount.amount(), 0.0);
        assert_eq!(result.post_amount.amount(), 2_000_000.0);
    }

    #[test]
    fn vm_calculator_below_mta() {
        let csa = CsaSpec::usd_regulatory().expect("registry should load"); // MTA = 500K
        let calc = VmCalculator::new(csa);

        let exposure = Money::from((300_000_i64, Currency::USD));
        let posted = Money::from((0_i64, Currency::USD));
        let result = calc
            .calculate(exposure, posted, test_date(2025, 1, 15))
            .expect("calc ok");

        // 300K < 500K MTA, no call
        assert!(!result.requires_call());
    }

    #[test]
    fn vm_calculator_matches_vm_params() {
        let csa = CsaSpec::usd_regulatory().expect("registry should load");
        let calc = VmCalculator::new(csa.clone());
        let as_of = test_date(2025, 1, 15);

        let exposure = Money::from((2_000_000_i64, Currency::USD));
        let posted = Money::from((0_i64, Currency::USD));

        let params_call = csa
            .vm_params
            .calculate_margin_call(exposure, posted)
            .expect("matching currencies should succeed");
        let result = calc.calculate(exposure, posted, as_of).expect("calc ok");

        assert_eq!(result.collect_amount, params_call);
        assert_eq!(result.post_amount.amount(), 0.0);

        // Now flip to a return scenario
        let exposure = Money::from((500_000_i64, Currency::USD));
        let posted = Money::from((3_000_000_i64, Currency::USD));

        let params_call = csa
            .vm_params
            .calculate_margin_call(exposure, posted)
            .expect("matching currencies should succeed");
        let result = calc.calculate(exposure, posted, as_of).expect("calc ok");

        assert_eq!(result.collect_amount.amount(), 0.0);
        assert_eq!(
            result.post_amount,
            Money::new(params_call.amount().abs(), Currency::USD).expect("valid money fixture")
        );
    }

    #[test]
    fn vm_result_steps_replay_the_call() {
        let mut csa = CsaSpec::usd_regulatory().expect("registry should load");
        csa.vm_params.threshold = Money::from((1_000_000_i64, Currency::USD));
        csa.vm_params.independent_amount = Money::from((250_000_i64, Currency::USD));
        csa.vm_params.mta = Money::from((500_000_i64, Currency::USD));
        csa.vm_params.rounding = Money::from((100_000_i64, Currency::USD));
        let calc = VmCalculator::new(csa.clone());
        let as_of = test_date(2025, 1, 15);

        for (exposure, collateral) in [
            (5_234_567.0, 1_000_000.0),   // delivery, rounded up
            (1_500_000.0, 3_034_567.0),   // return, rounded down
            (1_600_000.0, 600_000.0),     // below MTA
            (-4_321_000.0, -1_000_000.0), // desk posts
        ] {
            let exposure = Money::new(exposure, Currency::USD).expect("money");
            let collateral = Money::new(collateral, Currency::USD).expect("money");
            let r = calc.calculate(exposure, collateral, as_of).expect("calc");

            assert_eq!(r.threshold, csa.vm_params.threshold);
            assert_eq!(r.independent_amount, csa.vm_params.independent_amount);
            assert_eq!(r.mta, csa.vm_params.mta);
            assert_eq!(r.rounding_increment, csa.vm_params.rounding);
            assert_eq!(r.collateral_balance, collateral);

            // gross -> threshold + IA -> net exposure
            let gross = r.gross_exposure.amount();
            let excess = (gross.abs() - r.threshold.amount()).max(0.0) * gross.signum();
            assert_eq!(
                r.net_exposure.amount(),
                excess + r.independent_amount.amount()
            );
            // net exposure -> collateral -> unrounded call
            let unrounded = r.net_exposure.amount() - r.collateral_balance.amount();
            assert_eq!(r.unrounded_call.amount(), unrounded);
            // MTA -> rounding -> call
            let signed_call = r.collect_amount.amount() - r.post_amount.amount();
            let expected = if unrounded.abs() < r.mta.amount() {
                0.0
            } else {
                let units = unrounded.abs() / r.rounding_increment.amount();
                let units = if unrounded * r.net_exposure.amount() > 0.0 {
                    units.ceil()
                } else {
                    units.floor()
                };
                unrounded.signum() * units * r.rounding_increment.amount()
            };
            assert_eq!(signed_call, expected);
        }
    }

    #[test]
    fn generate_margin_call_series() {
        let csa = CsaSpec::usd_regulatory().expect("registry should load");
        let calc = VmCalculator::new(csa);

        let exposures = vec![
            (
                test_date(2025, 1, 15),
                Money::from((1_000_000_i64, Currency::USD)),
            ),
            (
                test_date(2025, 1, 16),
                Money::from((2_000_000_i64, Currency::USD)),
            ),
            (
                test_date(2025, 1, 17),
                Money::from((1_500_000_i64, Currency::USD)),
            ),
        ];

        let calls = calc
            .generate_margin_calls(&exposures, Money::from((0_i64, Currency::USD)))
            .expect("margin calls ok");

        // Three calls: 2 deliveries (1M, then 1M more), then 1 return (0.5M excess)
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[0].call_type, MarginCallType::VariationMarginCollect);
        assert_eq!(calls[1].call_type, MarginCallType::VariationMarginCollect);
        assert_eq!(calls[2].call_type, MarginCallType::VariationMarginPost);
    }

    #[test]
    fn generate_margin_calls_tracks_negative_exposure_as_signed_collateral() {
        let csa = CsaSpec::usd_regulatory().expect("registry should load");
        let calc = VmCalculator::new(csa);

        let exposures = vec![
            (
                test_date(2025, 1, 15),
                Money::from((-2_000_000_i64, Currency::USD)),
            ),
            (
                test_date(2025, 1, 16),
                Money::from((-2_000_000_i64, Currency::USD)),
            ),
            (
                test_date(2025, 1, 17),
                Money::from((-2_000_000_i64, Currency::USD)),
            ),
        ];

        let calls = calc
            .generate_margin_calls(&exposures, Money::from((0_i64, Currency::USD)))
            .expect("margin calls ok");

        assert_eq!(
            calls.len(),
            1,
            "persistent deficit should not be called repeatedly"
        );
        assert_eq!(calls[0].call_type, MarginCallType::VariationMarginPost);
        assert_eq!(calls[0].amount, Money::from((2_000_000_i64, Currency::USD)));
    }

    #[test]
    fn on_demand_margin_call_dates_do_not_loop_when_start_equals_end() {
        let mut csa = CsaSpec::usd_regulatory().expect("registry should load");
        csa.vm_params.frequency = MarginTenor::OnDemand;
        let calc = VmCalculator::new(csa);

        let date = test_date(2025, 1, 15);
        let dates = calc.margin_call_dates(date, date).expect("call dates");

        assert_eq!(dates, vec![date]);
    }

    #[test]
    fn settlement_lag_uses_business_days() {
        let csa = CsaSpec::usd_regulatory().expect("registry should load"); // settlement_lag = 1
        let calc = VmCalculator::new(csa);
        let friday = test_date(2025, 1, 10);
        let exposure = Money::from((1_000_000_i64, Currency::USD));
        let posted = Money::from((0_i64, Currency::USD));

        let result = calc.calculate(exposure, posted, friday).expect("calc ok");
        // T+1 business day from Friday should be Monday.
        assert_eq!(result.settlement_date, test_date(2025, 1, 13));
    }

    #[test]
    fn daily_margin_call_dates_skip_weekends() {
        let csa = CsaSpec::usd_regulatory().expect("registry should load");
        let calc = VmCalculator::new(csa);
        let dates = calc
            .margin_call_dates(test_date(2025, 1, 10), test_date(2025, 1, 14))
            .expect("call dates");
        assert_eq!(
            dates,
            vec![
                test_date(2025, 1, 10),
                test_date(2025, 1, 13),
                test_date(2025, 1, 14)
            ]
        );
    }

    #[test]
    fn weekly_calls_preserve_calendar_cadence_across_holidays() {
        let mut csa = CsaSpec::usd_regulatory().expect("registry should load");
        csa.vm_params.frequency = MarginTenor::Weekly;
        let calc = VmCalculator::new(csa);
        let dates = calc
            .margin_call_dates(test_date(2025, 6, 27), test_date(2025, 7, 11))
            .expect("weekly dates");
        assert_eq!(
            dates,
            vec![
                test_date(2025, 6, 27),
                test_date(2025, 7, 7),  // Independence Day rolls following.
                test_date(2025, 7, 11), // Subsequent call keeps Friday anchor.
            ]
        );
    }

    #[test]
    fn monthly_calls_preserve_end_of_month_anchor() {
        let mut csa = CsaSpec::usd_regulatory().expect("registry should load");
        csa.vm_params.frequency = MarginTenor::Monthly;
        let calc = VmCalculator::new(csa);
        let dates = calc
            .margin_call_dates(test_date(2025, 1, 31), test_date(2025, 3, 31))
            .expect("monthly dates");
        assert_eq!(
            dates,
            vec![
                test_date(2025, 1, 31),
                test_date(2025, 2, 28),
                test_date(2025, 3, 31),
            ]
        );
    }

    #[test]
    fn invalid_contractual_calendar_is_an_error() {
        let mut csa = CsaSpec::usd_regulatory().expect("registry should load");
        csa.calendar_id = "missing-calendar".to_string();
        let calc = VmCalculator::new(csa);
        assert!(calc
            .margin_call_dates(test_date(2025, 1, 17), test_date(2025, 1, 24))
            .is_err());
    }
}
