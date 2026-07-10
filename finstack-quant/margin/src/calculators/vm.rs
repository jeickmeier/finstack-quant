//! Variation margin calculator.
//!
//! Implements ISDA CSA variation margin calculation logic including
//! threshold, MTA, and rounding rules.

use crate::types::{CsaSpec, MarginCall, MarginTenor};
use finstack_quant_core::dates::{adjust, BusinessDayConvention, CalendarRegistry, Date, DateExt};
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;
use tracing::{debug, warn};

/// Variation margin calculation result.
#[derive(Debug, Clone, PartialEq)]
pub struct VmResult {
    /// Calculation date
    pub date: Date,

    /// Gross mark-to-market exposure
    pub gross_exposure: Money,

    /// Net exposure after applying threshold and independent amount
    pub net_exposure: Money,

    /// Delivery amount (positive = we need to post margin)
    pub delivery_amount: Money,

    /// Return amount (positive = we receive margin back)
    pub return_amount: Money,

    /// Settlement date for the margin transfer
    pub settlement_date: Date,
}

impl VmResult {
    /// Get the net margin amount (delivery - return).
    #[must_use]
    pub fn net_margin(&self) -> Money {
        if self.delivery_amount.amount() > 0.0 {
            self.delivery_amount
        } else {
            Money::new(-self.return_amount.amount(), self.return_amount.currency())
        }
    }

    /// Check if a margin call is required.
    #[must_use]
    pub fn requires_call(&self) -> bool {
        self.delivery_amount.amount() > 0.0 || self.return_amount.amount() > 0.0
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
/// and return amounts split that signed amount by exposure sign.
///
/// Implementation delegates CSA/MTA/rounding logic to
/// `VmParameters::calculate_margin_call` to ensure consistent behavior
/// across margin utilities.
///
/// # Example
///
/// ```ignore
/// use finstack_quant_margin::{VmCalculator, CsaSpec};
/// use finstack_quant_core::money::Money;
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::dates::Date;
///
/// # fn main() -> finstack_quant_core::Result<()> {
/// let csa = CsaSpec::usd_regulatory()?;
/// let calc = VmCalculator::new(csa);
///
/// let exposure = Money::new(5_000_000.0, Currency::USD);
/// let posted = Money::new(3_000_000.0, Currency::USD);
/// let as_of = Date::from_calendar_date(2025, time::Month::January, 15).expect("valid");
///
/// let result = calc.calculate(exposure, posted, as_of)?;
/// println!("Delivery required: {}", result.delivery_amount);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct VmCalculator {
    csa: CsaSpec,
}

impl VmCalculator {
    fn calendar_for_csa(&self) -> Result<&'static dyn finstack_quant_core::dates::HolidayCalendar> {
        CalendarRegistry::global()
            .resolve_str(&self.csa.calendar_id)
            .ok_or_else(|| {
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
    /// * `posted_collateral` - Value of currently posted collateral
    /// * `as_of` - Calculation date
    ///
    /// # Returns
    ///
    /// [`VmResult`] with delivery and return amounts.
    pub fn calculate(
        &self,
        exposure: Money,
        posted_collateral: Money,
        as_of: Date,
    ) -> Result<VmResult> {
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

        // Single source of truth for the threshold + IA formula. Both
        // the reported net_exposure and the margin call are derived
        // from VmParameters so the two cannot silently drift.
        let net_exposure_money = vm_params.required_credit_support(exposure)?;
        let exp = exposure.amount();

        let net_call = vm_params.calculate_margin_call(exposure, posted_collateral)?;
        let (delivery, ret) = match net_call.amount().total_cmp(&0.0) {
            std::cmp::Ordering::Greater => {
                if exp >= 0.0 {
                    (net_call, Money::new(0.0, currency))
                } else {
                    (Money::new(0.0, currency), net_call)
                }
            }
            std::cmp::Ordering::Less => {
                let abs_amt = Money::new(net_call.amount().abs(), currency);
                // Negative credit support: return excess collateral when exposure ≥ 0;
                // post margin to counterparty when exposure < 0 (bilateral netting).
                if exp >= 0.0 {
                    (Money::new(0.0, currency), abs_amt)
                } else {
                    (abs_amt, Money::new(0.0, currency))
                }
            }
            std::cmp::Ordering::Equal => (Money::new(0.0, currency), Money::new(0.0, currency)),
        };

        // Calculate settlement date
        let settlement_date = self.calculate_settlement_date(as_of)?;

        Ok(VmResult {
            date: as_of,
            gross_exposure: exposure,
            net_exposure: net_exposure_money,
            delivery_amount: delivery,
            return_amount: ret,
            settlement_date,
        })
    }

    /// Generate a series of margin calls from an exposure time series.
    ///
    /// # Arguments
    ///
    /// * `exposures` - Time series of (date, exposure) pairs
    /// * `initial_collateral` - Initially posted collateral
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

                if result.delivery_amount.amount() > 0.0 {
                    debug!(date = %date, amount = result.delivery_amount.amount(), "VM delivery margin call");
                    calls.push(MarginCall::vm_delivery(
                        *date,
                        settlement_date,
                        result.delivery_amount,
                        *exposure,
                        self.csa.vm_params.threshold,
                        self.csa.vm_params.mta,
                    ));
                    current_collateral = if exposure.amount() < 0.0 {
                        current_collateral
                            .checked_sub(result.delivery_amount)
                            .map_err(|e| finstack_quant_core::Error::Validation(e.to_string()))?
                    } else {
                        current_collateral.checked_add(result.delivery_amount)?
                    };
                } else if result.return_amount.amount() > 0.0 {
                    debug!(date = %date, amount = result.return_amount.amount(), "VM return margin call");
                    calls.push(MarginCall::vm_return(
                        *date,
                        settlement_date,
                        result.return_amount,
                        *exposure,
                        self.csa.vm_params.threshold,
                        self.csa.vm_params.mta,
                    ));
                    current_collateral = if exposure.amount() < 0.0 {
                        current_collateral.checked_add(result.return_amount)?
                    } else {
                        current_collateral
                            .checked_sub(result.return_amount)
                            .map_err(|e| finstack_quant_core::Error::Validation(e.to_string()))?
                    };
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
                MarginTenor::Monthly => start.add_months(period),
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
    use crate::types::{EligibleCollateralSchedule, MarginCallTiming, VmParameters};
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
                Money::new(1_000_000.0, Currency::USD),
                Money::new(100_000.0, Currency::USD),
            ),
            im_params: None,
            eligible_collateral: EligibleCollateralSchedule::default(),
            call_timing: MarginCallTiming::default(),
            collateral_curve_id: CurveId::new("USD-OIS"),
        }
    }

    #[test]
    fn vm_calculator_no_threshold() {
        let csa = CsaSpec::usd_regulatory().expect("registry should load");
        let calc = VmCalculator::new(csa);

        let exposure = Money::new(5_000_000.0, Currency::USD);
        let posted = Money::new(3_000_000.0, Currency::USD);
        let result = calc
            .calculate(exposure, posted, test_date(2025, 1, 15))
            .expect("calc ok");

        // With zero threshold, delivery = exposure - posted = 2M
        assert_eq!(result.delivery_amount.amount(), 2_000_000.0);
        assert_eq!(result.return_amount.amount(), 0.0);
    }

    #[test]
    fn vm_calculator_bilateral_negative_exposure_delivery() {
        let csa = CsaSpec::usd_regulatory().expect("registry should load");
        let calc = VmCalculator::new(csa);

        let exposure = Money::new(-2_000_000.0, Currency::USD);
        let posted = Money::new(0.0, Currency::USD);
        let result = calc
            .calculate(exposure, posted, test_date(2025, 1, 15))
            .expect("calc ok");

        assert_eq!(result.delivery_amount.amount(), 2_000_000.0);
        assert_eq!(result.return_amount.amount(), 0.0);
    }

    #[test]
    fn vm_calculator_with_threshold() {
        let csa = threshold_csa();
        let calc = VmCalculator::new(csa);

        // Exposure below threshold: no margin call
        let exposure = Money::new(500_000.0, Currency::USD);
        let posted = Money::new(0.0, Currency::USD);
        let result = calc
            .calculate(exposure, posted, test_date(2025, 1, 15))
            .expect("calc ok");

        assert_eq!(result.delivery_amount.amount(), 0.0);
        assert!(!result.requires_call());
    }

    #[test]
    fn vm_calculator_return_excess() {
        let csa = CsaSpec::usd_regulatory().expect("registry should load");
        let calc = VmCalculator::new(csa);

        // Exposure dropped, have excess collateral
        let exposure = Money::new(1_000_000.0, Currency::USD);
        let posted = Money::new(3_000_000.0, Currency::USD);
        let result = calc
            .calculate(exposure, posted, test_date(2025, 1, 15))
            .expect("calc ok");

        // Return = posted - required = 3M - 1M = 2M
        assert_eq!(result.delivery_amount.amount(), 0.0);
        assert_eq!(result.return_amount.amount(), 2_000_000.0);
    }

    #[test]
    fn vm_calculator_below_mta() {
        let csa = CsaSpec::usd_regulatory().expect("registry should load"); // MTA = 500K
        let calc = VmCalculator::new(csa);

        let exposure = Money::new(300_000.0, Currency::USD);
        let posted = Money::new(0.0, Currency::USD);
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

        let exposure = Money::new(2_000_000.0, Currency::USD);
        let posted = Money::new(0.0, Currency::USD);

        let params_call = csa
            .vm_params
            .calculate_margin_call(exposure, posted)
            .expect("matching currencies should succeed");
        let result = calc.calculate(exposure, posted, as_of).expect("calc ok");

        assert_eq!(result.delivery_amount, params_call);
        assert_eq!(result.return_amount.amount(), 0.0);

        // Now flip to a return scenario
        let exposure = Money::new(500_000.0, Currency::USD);
        let posted = Money::new(3_000_000.0, Currency::USD);

        let params_call = csa
            .vm_params
            .calculate_margin_call(exposure, posted)
            .expect("matching currencies should succeed");
        let result = calc.calculate(exposure, posted, as_of).expect("calc ok");

        assert_eq!(result.delivery_amount.amount(), 0.0);
        assert_eq!(
            result.return_amount,
            Money::new(params_call.amount().abs(), Currency::USD)
        );
    }

    #[test]
    fn generate_margin_call_series() {
        let csa = CsaSpec::usd_regulatory().expect("registry should load");
        let calc = VmCalculator::new(csa);

        let exposures = vec![
            (
                test_date(2025, 1, 15),
                Money::new(1_000_000.0, Currency::USD),
            ),
            (
                test_date(2025, 1, 16),
                Money::new(2_000_000.0, Currency::USD),
            ),
            (
                test_date(2025, 1, 17),
                Money::new(1_500_000.0, Currency::USD),
            ),
        ];

        let calls = calc
            .generate_margin_calls(&exposures, Money::new(0.0, Currency::USD))
            .expect("margin calls ok");

        // Three calls: 2 deliveries (1M, then 1M more), then 1 return (0.5M excess)
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[0].call_type, MarginCallType::VariationMarginDelivery);
        assert_eq!(calls[1].call_type, MarginCallType::VariationMarginDelivery);
        assert_eq!(calls[2].call_type, MarginCallType::VariationMarginReturn);
    }

    #[test]
    fn generate_margin_calls_tracks_negative_exposure_as_signed_collateral() {
        let csa = CsaSpec::usd_regulatory().expect("registry should load");
        let calc = VmCalculator::new(csa);

        let exposures = vec![
            (
                test_date(2025, 1, 15),
                Money::new(-2_000_000.0, Currency::USD),
            ),
            (
                test_date(2025, 1, 16),
                Money::new(-2_000_000.0, Currency::USD),
            ),
            (
                test_date(2025, 1, 17),
                Money::new(-2_000_000.0, Currency::USD),
            ),
        ];

        let calls = calc
            .generate_margin_calls(&exposures, Money::new(0.0, Currency::USD))
            .expect("margin calls ok");

        assert_eq!(
            calls.len(),
            1,
            "persistent deficit should not be called repeatedly"
        );
        assert_eq!(calls[0].call_type, MarginCallType::VariationMarginDelivery);
        assert_eq!(calls[0].amount, Money::new(2_000_000.0, Currency::USD));
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
        let exposure = Money::new(1_000_000.0, Currency::USD);
        let posted = Money::new(0.0, Currency::USD);

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
