//! Covenant package templates for common deal structures.
//!
//! These presets encode standard covenant packages used across different
//! lending markets. Each function validates its threshold inputs and returns a
//! `Vec<CovenantSpec>` ready for insertion into a [`crate::CovenantEngine`].

use super::engine::{
    Covenant, CovenantConsequence, CovenantScope, CovenantSpec, CovenantType, ThresholdTest,
};
use finstack_quant_core::dates::Tenor;
use finstack_quant_core::{Error, Result};

/// Reject a template threshold that is not a finite, non-negative number.
///
/// Every template input is a ratio in turns, a decimal fraction, or a
/// reporting-currency amount; none of them is meaningful when negative, and a
/// `NaN` or infinite threshold would otherwise serialize as JSON `null` and
/// fail much later with an unrelated error.
fn check_threshold(name: &str, value: f64) -> Result<()> {
    if !value.is_finite() || value < 0.0 {
        return Err(Error::Validation(format!(
            "covenant template threshold `{name}` must be finite and non-negative, got {value}"
        )));
    }
    Ok(())
}

fn maintenance(cov_type: CovenantType, frequency: Tenor, metric: &str) -> CovenantSpec {
    let label = cov_type.covenant_id().to_string();
    CovenantSpec::with_metric(
        Covenant::new(cov_type, frequency, label).with_scope(CovenantScope::Maintenance),
        metric,
    )
}

fn incurrence(cov_type: CovenantType, frequency: Tenor, metric: &str) -> CovenantSpec {
    let label = cov_type.covenant_id().to_string();
    CovenantSpec::with_metric(
        Covenant::new(cov_type, frequency, label).with_scope(CovenantScope::Incurrence),
        metric,
    )
}

/// Standard leveraged buyout covenant package.
///
/// Typical for sponsor-backed leveraged loans with:
/// - Max Total Leverage (Debt/EBITDA) with step-down
/// - Min Interest Coverage
/// - Min Fixed Charge Coverage
/// - Max Capex
///
/// Consequences: rate increase on leverage breach, distribution block on coverage breach.
///
/// # Arguments
///
/// * `initial_leverage` - Maximum debt-to-EBITDA maintenance threshold in
///   turns, where `5.0` means 5.0x.
/// * `interest_coverage` - Minimum interest-coverage maintenance threshold in
///   turns.
/// * `fixed_charge_coverage` - Minimum fixed-charge-coverage maintenance
///   threshold in turns.
/// * `max_capex` - Maximum annual capital-expenditure amount in the caller's
///   reporting-currency convention.
///
/// # Errors
///
/// Returns [`Error::Validation`] when any input is `NaN`, infinite, or
/// negative.
pub fn lbo_standard(
    initial_leverage: f64,
    interest_coverage: f64,
    fixed_charge_coverage: f64,
    max_capex: f64,
) -> Result<Vec<CovenantSpec>> {
    check_threshold("initial_leverage", initial_leverage)?;
    check_threshold("interest_coverage", interest_coverage)?;
    check_threshold("fixed_charge_coverage", fixed_charge_coverage)?;
    check_threshold("max_capex", max_capex)?;
    Ok(vec![
        {
            let mut s = maintenance(
                CovenantType::MaxDebtToEbitda {
                    threshold: initial_leverage,
                },
                Tenor::quarterly(),
                "debt_to_ebitda",
            );
            s.covenant.cure_period_days = Some(30);
            s.covenant
                .consequences
                .push(CovenantConsequence::RateIncrease { bp_increase: 200.0 });
            s
        },
        {
            let mut s = maintenance(
                CovenantType::MinInterestCoverage {
                    threshold: interest_coverage,
                },
                Tenor::quarterly(),
                "interest_coverage",
            );
            s.covenant.cure_period_days = Some(30);
            s.covenant
                .consequences
                .push(CovenantConsequence::BlockDistributions);
            s
        },
        {
            let mut s = maintenance(
                CovenantType::MinFixedChargeCoverage {
                    threshold: fixed_charge_coverage,
                },
                Tenor::quarterly(),
                "fixed_charge_coverage",
            );
            s.covenant.cure_period_days = Some(30);
            s
        },
        maintenance(
            CovenantType::MaxCapex {
                threshold: max_capex,
            },
            Tenor::annual(),
            "capex",
        ),
    ])
}

/// "Covenant-lite" leveraged loan package (incurrence only).
///
/// Post-2015 leveraged loan standard with no maintenance covenants.
/// Only tested upon specific incurrence actions (new debt, acquisitions, dividends).
///
/// # Arguments
///
/// * `max_leverage` - Maximum total debt-to-EBITDA incurrence threshold in
///   turns.
/// * `max_senior_leverage` - Maximum senior-debt-to-EBITDA incurrence
///   threshold in turns.
///
/// # Errors
///
/// Returns [`Error::Validation`] when any input is `NaN`, infinite, or
/// negative.
pub fn cov_lite(max_leverage: f64, max_senior_leverage: f64) -> Result<Vec<CovenantSpec>> {
    check_threshold("max_leverage", max_leverage)?;
    check_threshold("max_senior_leverage", max_senior_leverage)?;
    Ok(vec![
        incurrence(
            CovenantType::MaxTotalLeverage {
                threshold: max_leverage,
            },
            Tenor::quarterly(),
            "total_leverage",
        ),
        incurrence(
            CovenantType::MaxSeniorLeverage {
                threshold: max_senior_leverage,
            },
            Tenor::quarterly(),
            "senior_leverage",
        ),
        incurrence(
            CovenantType::Negative {
                restriction: "No additional secured debt without consent".to_string(),
            },
            Tenor::annual(),
            "negative_debt",
        ),
    ])
}

/// Commercial real estate (CRE) covenant package.
///
/// Standard for income-producing assets with:
/// - Min DSCR (primary maintenance covenant)
/// - Min Debt Yield (Net Operating Income / Loan Balance)
/// - Max Loan-to-Value (custom metric)
/// - Cash sweep triggered by DSCR breach
///
/// # Arguments
///
/// * `min_dscr` - Minimum debt-service-coverage maintenance ratio in turns.
/// * `min_debt_yield` - Minimum debt yield as a decimal fraction, such as
///   `0.10` for 10%.
/// * `max_ltv` - Maximum loan-to-value ratio as a decimal fraction, such as
///   `0.65` for 65%.
///
/// # Errors
///
/// Returns [`Error::Validation`] when any input is `NaN`, infinite, or
/// negative.
pub fn real_estate(min_dscr: f64, min_debt_yield: f64, max_ltv: f64) -> Result<Vec<CovenantSpec>> {
    check_threshold("min_dscr", min_dscr)?;
    check_threshold("min_debt_yield", min_debt_yield)?;
    check_threshold("max_ltv", max_ltv)?;
    Ok(vec![
        {
            let mut s = maintenance(
                CovenantType::MinDscr {
                    threshold: min_dscr,
                },
                Tenor::quarterly(),
                "dscr",
            );
            s.covenant.cure_period_days = Some(30);
            s.covenant
                .consequences
                .push(CovenantConsequence::CashSweep {
                    sweep_percentage: 1.0,
                });
            s
        },
        {
            let mut s = maintenance(
                CovenantType::Custom {
                    metric: "debt_yield".to_string(),
                    test: ThresholdTest::Minimum(min_debt_yield),
                },
                Tenor::quarterly(),
                "debt_yield",
            );
            s.covenant.label = "min_debt_yield".to_string();
            s
        },
        {
            let mut s = maintenance(
                CovenantType::Custom {
                    metric: "ltv".to_string(),
                    test: ThresholdTest::Maximum(max_ltv),
                },
                Tenor::quarterly(),
                "ltv",
            );
            s.covenant.label = "max_ltv".to_string();
            s.covenant
                .consequences
                .push(CovenantConsequence::CashSweep {
                    sweep_percentage: 0.5,
                });
            s
        },
    ])
}

/// Infrastructure / project finance covenant package.
///
/// Standard for long-dated project finance with:
/// - Min DSCR (primary maintenance) — labeled `min_dscr_default`
/// - Min DSCR for distribution lock-up (higher threshold) — labeled `min_dscr_lockup`
/// - Min Liquidity (debt service reserve)
/// - Max Net Debt / EBITDA
///
/// The two `MinDscr` covenants share a type but carry distinct instance
/// labels so their reports, breaches, and consequences never collide: a
/// lock-up breach blocks distributions only and can never resolve to the
/// primary covenant's Event-of-Default consequence.
///
/// # Arguments
///
/// * `min_dscr` - Minimum DSCR in turns that causes a default after its cure
///   period when breached.
/// * `distribution_lockup_dscr` - Higher DSCR threshold in turns that blocks
///   distributions without an event of default.
/// * `min_liquidity` - Minimum debt-service-reserve liquidity in the caller's
///   reporting-currency convention.
/// * `max_net_leverage` - Maximum net-debt-to-EBITDA maintenance threshold in
///   turns.
///
/// # Errors
///
/// Returns [`Error::Validation`] when any input is `NaN`, infinite, or
/// negative.
pub fn project_finance(
    min_dscr: f64,
    distribution_lockup_dscr: f64,
    min_liquidity: f64,
    max_net_leverage: f64,
) -> Result<Vec<CovenantSpec>> {
    check_threshold("min_dscr", min_dscr)?;
    check_threshold("distribution_lockup_dscr", distribution_lockup_dscr)?;
    check_threshold("min_liquidity", min_liquidity)?;
    check_threshold("max_net_leverage", max_net_leverage)?;
    Ok(vec![
        {
            let mut s = maintenance(
                CovenantType::MinDscr {
                    threshold: min_dscr,
                },
                Tenor::quarterly(),
                "dscr",
            );
            s.covenant.label = "min_dscr_default".to_string();
            s.covenant.cure_period_days = Some(60);
            s.covenant.consequences.push(CovenantConsequence::Default);
            s
        },
        {
            let mut s = maintenance(
                CovenantType::MinDscr {
                    threshold: distribution_lockup_dscr,
                },
                Tenor::quarterly(),
                "dscr",
            );
            s.covenant.label = "min_dscr_lockup".to_string();
            s.covenant
                .consequences
                .push(CovenantConsequence::BlockDistributions);
            s
        },
        maintenance(
            CovenantType::MinLiquidity {
                threshold: min_liquidity,
            },
            Tenor::quarterly(),
            "liquidity",
        ),
        maintenance(
            CovenantType::MaxNetDebtToEbitda {
                threshold: max_net_leverage,
            },
            Tenor::quarterly(),
            "net_debt_to_ebitda",
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn templates_reject_non_finite_and_negative_thresholds() {
        assert!(lbo_standard(f64::NAN, 2.0, 1.1, 50.0).is_err());
        assert!(lbo_standard(5.0, f64::INFINITY, 1.1, 50.0).is_err());
        assert!(lbo_standard(5.0, 2.0, -1.1, 50.0).is_err());
        assert!(cov_lite(7.0, f64::NEG_INFINITY).is_err());
        assert!(real_estate(1.25, f64::NAN, 0.75).is_err());
        assert!(project_finance(1.2, 1.1, -10.0, 7.0).is_err());
    }

    #[test]
    fn templates_accept_valid_thresholds() {
        assert_eq!(lbo_standard(5.0, 2.0, 1.1, 50.0).unwrap().len(), 4);
        assert_eq!(cov_lite(7.0, 4.5).unwrap().len(), 3);
        assert_eq!(real_estate(1.25, 0.08, 0.75).unwrap().len(), 3);
        assert_eq!(project_finance(1.2, 1.1, 10.0, 7.0).unwrap().len(), 4);
    }
}
