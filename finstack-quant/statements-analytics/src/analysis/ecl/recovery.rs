//! Point-in-time credit-impaired exposure measured from dated recovery cash.

use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::math::Compounding;
use finstack_quant_core::money::Money;
use finstack_quant_core::wire::DecimalWire;
use finstack_quant_core::{Error, Result};
use rust_decimal::{Decimal, prelude::ToPrimitive};
use serde::{Deserialize, Serialize};

fn invalid(message: impl Into<String>) -> Error {
    Error::Validation(message.into())
}

/// Same-currency carrying exposure and annual effective interest policy at the reporting cut.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ImpairedExposure {
    /// Exposure identity from the approved reporting population.
    pub exposure_id: String,
    /// Approved source observation or subledger position.
    pub carrying_amount_source: String,
    /// Nonnegative carrying value in reporting currency.
    pub carrying_amount: Money,
    /// Effective reporting date for the carrying amount and discounting.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub reporting_date: Date,
    /// Elected annual effective original EIR as an exact decimal rate, above -100%.
    pub effective_interest_rate: DecimalWire,
    /// Year-fraction convention used to discount dated recovery cash.
    pub recovery_day_count: DayCount,
}

/// Dated expected recovery assessed on the same exposure and currency basis.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RecoveryEvidence {
    /// Approved recovery estimate or sourced cash-recovery record.
    pub source_id: String,
    /// Nonnegative expected cash recovery in the exposure currency.
    pub amount: Money,
    /// Date when the recovery cash is expected to be received.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub date: Date,
}

/// Credit-impaired allowance and its same-basis recovery bridge.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ImpairedEclResult {
    /// Admitted carrying value, date, rate and source.
    pub exposure: ImpairedExposure,
    /// Approved estimated cash recovery and its receipt date.
    pub recovery: RecoveryEvidence,
    /// Year fraction between the reporting and recovery dates on the elected clock.
    pub time_to_recovery_years: f64,
    /// Present value discount factor at the annual effective original EIR.
    pub discount_factor: f64,
    /// Dated recovery cash discounted once to the reporting date.
    pub present_value_recovery: Money,
    /// Carrying amount less discounted recovery before the nonnegative allowance floor.
    pub signed_shortfall: Money,
    /// Nonnegative reported allowance on the same currency basis.
    pub allowance: Money,
}

fn validate_impaired_exposure(exposure: &ImpairedExposure) -> Result<()> {
    if exposure.exposure_id.trim().is_empty()
        || exposure.carrying_amount_source.trim().is_empty()
        || exposure.carrying_amount.amount_decimal() < Decimal::ZERO
        || exposure.effective_interest_rate.0 <= Decimal::NEGATIVE_ONE
    {
        return Err(invalid(
            "Impaired ECL requires sourced nonnegative carrying cash and EIR above -100%",
        ));
    }
    // Validate the convention even when no scenario provides recovery evidence.
    exposure.recovery_day_count.year_fraction(
        exposure.reporting_date,
        exposure.reporting_date,
        DayCountContext::default(),
    )?;
    Ok(())
}

/// Measure a credit-impaired carrying exposure from its dated expected recovery.
///
/// # Arguments
/// * `exposure` - Same-basis carrying exposure, reporting date, source, original effective rate and clock.
/// * `recovery` - Approved nonnegative recovery cash, source and receipt date in exposure currency.
///
/// # Errors
/// Rejects missing scope or provenance, improper dates, mixed currency,
/// negative cash values, inadmissible EIR or day count, and discount arithmetic
/// outside the represented financial domain.
pub fn compute_impaired_ecl(
    exposure: &ImpairedExposure,
    recovery: &RecoveryEvidence,
) -> Result<ImpairedEclResult> {
    validate_impaired_exposure(exposure)?;
    if recovery.source_id.trim().is_empty() || recovery.amount.amount_decimal() < Decimal::ZERO {
        return Err(invalid(
            "Impaired recovery requires sourced nonnegative cash",
        ));
    }
    if recovery.amount.currency() != exposure.carrying_amount.currency() {
        return Err(Error::CurrencyMismatch {
            expected: exposure.carrying_amount.currency(),
            actual: recovery.amount.currency(),
        });
    }
    if recovery.date < exposure.reporting_date {
        return Err(invalid("Impaired recovery date precedes its reporting cut"));
    }
    let eir = exposure
        .effective_interest_rate
        .0
        .to_f64()
        .ok_or_else(|| invalid("Impaired ECL effective interest rate is out of range"))?;
    let time_to_recovery_years = exposure.recovery_day_count.year_fraction(
        exposure.reporting_date,
        recovery.date,
        DayCountContext::default(),
    )?;
    let discount_factor = Compounding::Periodic(std::num::NonZeroU32::MIN)
        .df_from_rate(eir, time_to_recovery_years)?;
    let factor_decimal = Decimal::from_f64_retain(discount_factor)
        .filter(|factor| *factor > Decimal::ZERO)
        .ok_or_else(|| invalid("Impaired recovery discount factor is not representable"))?;
    let present_value_recovery = recovery.amount.checked_mul(factor_decimal)?;
    let signed_shortfall = exposure
        .carrying_amount
        .checked_sub(present_value_recovery)?;
    let allowance = if signed_shortfall.amount_decimal() >= Decimal::ZERO {
        signed_shortfall
    } else {
        Money::zero(exposure.carrying_amount.currency())
    };
    Ok(ImpairedEclResult {
        exposure: exposure.clone(),
        recovery: recovery.clone(),
        time_to_recovery_years,
        discount_factor,
        present_value_recovery,
        signed_shortfall,
        allowance,
    })
}

/// Exact probability and the recovery evidence attributable to one scenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ImpairedEclScenario {
    /// Unique named economic scenario.
    pub id: String,
    /// Exact probability in [0, 1].
    pub weight: DecimalWire,
    /// Dated expected recovery for this scenario, or None if unavailable.
    pub recovery: Option<RecoveryEvidence>,
}

/// Measurement or reason a particular scenario has no measured allowance.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum ImpairedEclMeasurement {
    /// Positive probability with qualified recovery evidence.
    Measured {
        /// Sourced result before scenario weighting.
        result: Box<ImpairedEclResult>,
        /// Nonnegative same-currency contribution at the exact scenario weight.
        weighted_allowance: Money,
    },
    /// The recovery evidence required for measurement is absent.
    Unavailable {
        /// Explanation of the unavailable scenario.
        reason: String,
    },
    /// Zero probability, with no valuation of the supplied evidence.
    ZeroWeight,
}

/// Dated credit-impaired measurement for one elected scenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ImpairedEclOutcome {
    /// Scenario identity, preserved in caller order.
    pub id: String,
    /// Exact elected scenario probability.
    pub weight: DecimalWire,
    /// Measured allowance or explicit reason payment evidence was not priced.
    #[serde(flatten)]
    pub measurement: ImpairedEclMeasurement,
}

/// Weighted credit-impaired allowance and its separate scenario evidence.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct WeightedImpairedEclResult {
    /// Common carrying exposure, date, currency and original effective interest policy.
    pub exposure: ImpairedExposure,
    /// Sum of measured scenario contributions, or None for missing positive-weight evidence.
    pub allowance: Option<Money>,
    /// Measures and coverage in exact scenario input order.
    pub outcomes: Vec<ImpairedEclOutcome>,
}

/// Weight credit-impaired allowances under one dated carrying and discount policy.
///
/// # Arguments
/// * `exposure` - Verified carrying amount, currency, date, source, original annual rate and discount clock.
/// * `scenarios` - Unique named recovery scenarios with explicit decimal probabilities summing to exactly one.
///
/// # Errors
/// Rejects invalid probabilities or recovery evidence on positive-weight
/// scenarios. Missing positive-weight evidence leaves the aggregate unavailable.
pub fn compute_impaired_ecl_weighted(
    exposure: &ImpairedExposure,
    scenarios: &[ImpairedEclScenario],
) -> Result<WeightedImpairedEclResult> {
    let elections = scenarios
        .iter()
        .map(|scenario| super::MacroScenario {
            id: scenario.id.clone(),
            weight: scenario.weight,
            lgd_override: None,
        })
        .collect::<Vec<_>>();
    super::engine::validate_scenario_weights(&elections)?;
    validate_impaired_exposure(exposure)?;

    let mut all_measured = true;
    let mut allowance = Money::zero(exposure.carrying_amount.currency());
    let mut outcomes = Vec::with_capacity(scenarios.len());
    for scenario in scenarios {
        let measurement = if scenario.weight.0.is_zero() {
            ImpairedEclMeasurement::ZeroWeight
        } else if let Some(recovery) = &scenario.recovery {
            let result = compute_impaired_ecl(exposure, recovery)?;
            let weighted_allowance = result.allowance.checked_mul(scenario.weight.0)?;
            allowance = allowance.checked_add(weighted_allowance)?;
            ImpairedEclMeasurement::Measured {
                result: Box::new(result),
                weighted_allowance,
            }
        } else {
            all_measured = false;
            ImpairedEclMeasurement::Unavailable {
                reason: "missing approved dated recovery evidence".to_owned(),
            }
        };
        outcomes.push(ImpairedEclOutcome {
            id: scenario.id.clone(),
            weight: scenario.weight,
            measurement,
        });
    }
    Ok(WeightedImpairedEclResult {
        exposure: exposure.clone(),
        allowance: all_measured.then_some(allowance),
        outcomes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use time::macros::date;

    #[test]
    fn impaired_weights_are_exact_and_keep_available_scenario_contributions() {
        let day = date!(2025 - 01 - 01);
        let usd = |n| Money::from((n, Currency::USD));
        let exposure = ImpairedExposure {
            exposure_id: "loan-weighted".into(),
            carrying_amount_source: "approved-ledger".into(),
            carrying_amount: usd(100_i64),
            reporting_date: day,
            effective_interest_rate: DecimalWire(Decimal::ZERO),
            recovery_day_count: DayCount::Act365F,
        };
        let scenario = |id: &str, weight, recovery| ImpairedEclScenario {
            id: id.into(),
            weight: DecimalWire(weight),
            recovery,
        };
        let evidence = |amount| RecoveryEvidence {
            source_id: "workout".into(),
            amount: usd(amount),
            date: day,
        };
        let base = scenario("base", Decimal::new(7, 1), Some(evidence(80)));
        let bad_zero = scenario("ignored", Decimal::ZERO, Some(evidence(-10)));
        let missing = scenario("stress", Decimal::new(3, 1), None);
        let partial =
            compute_impaired_ecl_weighted(&exposure, &[base.clone(), missing, bad_zero.clone()])
                .unwrap();
        assert!(partial.allowance.is_none());
        let ImpairedEclMeasurement::Measured {
            weighted_allowance, ..
        } = partial.outcomes[0].measurement
        else {
            panic!("base measured")
        };
        assert_eq!(weighted_allowance, usd(14));
        assert!(matches!(
            partial.outcomes[2].measurement,
            ImpairedEclMeasurement::ZeroWeight
        ));
        let stress = scenario("stress", Decimal::new(3, 1), Some(evidence(20)));
        let measured = compute_impaired_ecl_weighted(&exposure, &[base, stress, bad_zero]).unwrap();
        assert_eq!(measured.allowance, Some(usd(38)));
    }

    #[test]
    fn discounts_dated_recovery_once_and_retains_signed_excess() {
        let exposure = ImpairedExposure {
            exposure_id: "loan-1".to_owned(),
            carrying_amount_source: "approved-ledger".to_owned(),
            carrying_amount: Money::from((100_i64, Currency::USD)),
            reporting_date: date!(2025 - 01 - 01),
            effective_interest_rate: DecimalWire(Decimal::new(10, 2)),
            recovery_day_count: DayCount::Act365F,
        };
        let mut recovery = RecoveryEvidence {
            source_id: "approved-workout".to_owned(),
            amount: Money::from((88_i64, Currency::USD)),
            date: date!(2026 - 01 - 01),
        };
        let result = compute_impaired_ecl(&exposure, &recovery).unwrap();
        assert!((result.present_value_recovery.amount() - 80.0).abs() < 1e-10);
        assert!((result.allowance.amount() - 20.0).abs() < 1e-10);
        recovery.amount = Money::from((120_i64, Currency::USD));
        recovery.date = exposure.reporting_date;
        let result = compute_impaired_ecl(&exposure, &recovery).unwrap();
        assert_eq!(result.signed_shortfall.amount_decimal(), Decimal::from(-20));
        assert_eq!(result.allowance.amount_decimal(), Decimal::ZERO);
        recovery.date = date!(2024 - 12 - 31);
        assert!(compute_impaired_ecl(&exposure, &recovery).is_err());
    }
}
