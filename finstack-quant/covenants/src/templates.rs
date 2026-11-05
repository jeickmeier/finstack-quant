//! Covenant package templates for common deal structures.
//!
//! A [`CovenantTemplate`] names one standard covenant package used across
//! lending markets and carries its thresholds; [`covenant_template`] validates
//! them and returns the `Vec<CovenantSpec>` ready for insertion into a
//! [`crate::CovenantEngine`].

use super::engine::{
    Covenant, CovenantConsequence, CovenantScope, CovenantSpec, CovenantType, ThresholdTest,
};
use finstack_quant_core::dates::Tenor;
use finstack_quant_core::{Error, Result};
use serde::{Deserialize, Serialize};

/// One standard covenant package and its thresholds.
///
/// Ratios are in turns (`5.0` is 5.0x), yields and loan-to-value are decimal
/// fractions (`0.10` is 10%), and capex and liquidity are amounts in the
/// caller's reporting currency. Every threshold must be finite and
/// non-negative.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CovenantTemplate {
    /// Sponsor-backed leveraged loan: quarterly maintenance tests for maximum
    /// Debt / EBITDA, minimum interest and fixed-charge coverage, and an annual
    /// maximum capex. Leverage and interest coverage carry 30-day cure
    /// periods; a leverage breach steps the rate up 200bp and an
    /// interest-coverage breach blocks distributions.
    LboStandard {
        /// Maximum debt-to-EBITDA threshold in turns.
        initial_leverage: f64,
        /// Minimum interest-coverage threshold in turns.
        interest_coverage: f64,
        /// Minimum fixed-charge-coverage threshold in turns.
        fixed_charge_coverage: f64,
        /// Maximum annual capital expenditure in the reporting currency.
        max_capex: f64,
    },
    /// Covenant-lite leveraged loan: quarterly incurrence tests for maximum
    /// total and senior leverage, and an annual negative covenant on
    /// additional secured debt. No maintenance covenants.
    CovLite {
        /// Maximum total debt-to-EBITDA incurrence threshold in turns.
        max_leverage: f64,
        /// Maximum senior-debt-to-EBITDA incurrence threshold in turns.
        max_senior_leverage: f64,
    },
    /// Commercial real estate: quarterly maintenance tests for minimum DSCR
    /// (30-day cure, 100% cash sweep), minimum debt yield and maximum
    /// loan-to-value (50% cash sweep).
    RealEstate {
        /// Minimum debt-service-coverage ratio in turns.
        min_dscr: f64,
        /// Minimum debt yield as a decimal fraction.
        min_debt_yield: f64,
        /// Maximum loan-to-value ratio as a decimal fraction.
        max_ltv: f64,
    },
    /// Infrastructure / project finance: quarterly maintenance tests for a
    /// default DSCR (60-day cure, event of default), a higher distribution
    /// lock-up DSCR (blocks distributions only), minimum debt-service-reserve
    /// liquidity and maximum net Debt / EBITDA.
    ProjectFinance {
        /// Minimum default DSCR in turns.
        min_dscr: f64,
        /// Higher DSCR threshold in turns that blocks distributions.
        distribution_lockup_dscr: f64,
        /// Minimum debt-service-reserve amount in the reporting currency.
        min_liquidity: f64,
        /// Maximum net-debt-to-EBITDA threshold in turns.
        max_net_leverage: f64,
    },
}

/// Build the covenant specs of one standard package.
///
/// # Arguments
///
/// * `template` - The package and its thresholds: ratios in turns, yields and
///   loan-to-value as decimal fractions, capex and liquidity in the caller's
///   reporting currency.
///
/// # Returns
///
/// The package's specs in their documented order, ready for
/// [`crate::CovenantEngine`].
///
/// # Errors
///
/// Returns [`Error::Validation`] when any threshold is `NaN`, infinite, or
/// negative.
///
/// # Examples
///
/// ```
/// use finstack_quant_covenants::templates::{covenant_template, CovenantTemplate};
///
/// let specs = covenant_template(&CovenantTemplate::CovLite {
///     max_leverage: 7.0,
///     max_senior_leverage: 4.5,
/// })?;
/// assert_eq!(specs.len(), 3);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn covenant_template(template: &CovenantTemplate) -> Result<Vec<CovenantSpec>> {
    match *template {
        CovenantTemplate::LboStandard {
            initial_leverage,
            interest_coverage,
            fixed_charge_coverage,
            max_capex,
        } => lbo_standard(
            initial_leverage,
            interest_coverage,
            fixed_charge_coverage,
            max_capex,
        ),
        CovenantTemplate::CovLite {
            max_leverage,
            max_senior_leverage,
        } => cov_lite(max_leverage, max_senior_leverage),
        CovenantTemplate::RealEstate {
            min_dscr,
            min_debt_yield,
            max_ltv,
        } => real_estate(min_dscr, min_debt_yield, max_ltv),
        CovenantTemplate::ProjectFinance {
            min_dscr,
            distribution_lockup_dscr,
            min_liquidity,
            max_net_leverage,
        } => project_finance(
            min_dscr,
            distribution_lockup_dscr,
            min_liquidity,
            max_net_leverage,
        ),
    }
}

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
/// - Max Debt/EBITDA (constant threshold, tested quarterly)
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
fn lbo_standard(
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
fn cov_lite(max_leverage: f64, max_senior_leverage: f64) -> Result<Vec<CovenantSpec>> {
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
fn real_estate(min_dscr: f64, min_debt_yield: f64, max_ltv: f64) -> Result<Vec<CovenantSpec>> {
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
fn project_finance(
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

    fn lbo(initial_leverage: f64, interest_coverage: f64) -> CovenantTemplate {
        CovenantTemplate::LboStandard {
            initial_leverage,
            interest_coverage,
            fixed_charge_coverage: 1.1,
            max_capex: 50.0,
        }
    }

    #[test]
    fn templates_reject_non_finite_and_negative_thresholds() {
        assert!(covenant_template(&lbo(f64::NAN, 2.0)).is_err());
        assert!(covenant_template(&lbo(5.0, f64::INFINITY)).is_err());
        assert!(covenant_template(&lbo(5.0, -2.0)).is_err());
        assert!(
            covenant_template(&CovenantTemplate::CovLite {
                max_leverage: 7.0,
                max_senior_leverage: f64::NEG_INFINITY,
            })
            .is_err()
        );
        assert!(
            covenant_template(&CovenantTemplate::RealEstate {
                min_dscr: 1.25,
                min_debt_yield: f64::NAN,
                max_ltv: 0.75,
            })
            .is_err()
        );
        assert!(
            covenant_template(&CovenantTemplate::ProjectFinance {
                min_dscr: 1.2,
                distribution_lockup_dscr: 1.1,
                min_liquidity: -10.0,
                max_net_leverage: 7.0,
            })
            .is_err()
        );
    }

    #[test]
    fn templates_accept_valid_thresholds() {
        assert_eq!(covenant_template(&lbo(5.0, 2.0)).unwrap().len(), 4);
        let cov_lite = CovenantTemplate::CovLite {
            max_leverage: 7.0,
            max_senior_leverage: 4.5,
        };
        assert_eq!(covenant_template(&cov_lite).unwrap().len(), 3);
        let real_estate = CovenantTemplate::RealEstate {
            min_dscr: 1.25,
            min_debt_yield: 0.08,
            max_ltv: 0.75,
        };
        assert_eq!(covenant_template(&real_estate).unwrap().len(), 3);
        let project_finance = CovenantTemplate::ProjectFinance {
            min_dscr: 1.2,
            distribution_lockup_dscr: 1.1,
            min_liquidity: 10.0,
            max_net_leverage: 7.0,
        };
        assert_eq!(covenant_template(&project_finance).unwrap().len(), 4);
    }

    #[test]
    fn template_wire_form_is_kind_tagged_and_strict() {
        let json = serde_json::to_value(lbo(5.0, 2.0)).expect("serialize");
        assert_eq!(json["kind"], "lbo_standard");
        assert_eq!(json["initial_leverage"], 5.0);
        let back: CovenantTemplate = serde_json::from_value(json.clone()).expect("deserialize");
        assert_eq!(back, lbo(5.0, 2.0));
        let mut extra = json;
        extra["extra"] = serde_json::json!(1);
        assert!(serde_json::from_value::<CovenantTemplate>(extra).is_err());
        assert!(
            serde_json::from_value::<CovenantTemplate>(serde_json::json!({"kind": "unitranche"}))
                .is_err()
        );
    }
}
