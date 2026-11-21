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
        } => {
            check_thresholds(&[
                ("initial_leverage", initial_leverage),
                ("interest_coverage", interest_coverage),
                ("fixed_charge_coverage", fixed_charge_coverage),
                ("max_capex", max_capex),
            ])?;
            Ok(vec![
                template_spec(
                    CovenantType::MaxDebtToEbitda {
                        threshold: initial_leverage,
                    },
                    Tenor::quarterly(),
                    CovenantScope::Maintenance,
                    "debt_to_ebitda",
                    None,
                    Some(CovenantConsequence::RateIncrease { bp_increase: 200.0 }),
                ),
                template_spec(
                    CovenantType::MinInterestCoverage {
                        threshold: interest_coverage,
                    },
                    Tenor::quarterly(),
                    CovenantScope::Maintenance,
                    "interest_coverage",
                    None,
                    Some(CovenantConsequence::BlockDistributions),
                ),
                template_spec(
                    CovenantType::MinFixedChargeCoverage {
                        threshold: fixed_charge_coverage,
                    },
                    Tenor::quarterly(),
                    CovenantScope::Maintenance,
                    "fixed_charge_coverage",
                    None,
                    None,
                ),
                template_spec(
                    CovenantType::MaxCapex {
                        threshold: max_capex,
                    },
                    Tenor::annual(),
                    CovenantScope::Maintenance,
                    "capex",
                    None,
                    None,
                ),
            ])
        }
        CovenantTemplate::CovLite {
            max_leverage,
            max_senior_leverage,
        } => {
            check_thresholds(&[
                ("max_leverage", max_leverage),
                ("max_senior_leverage", max_senior_leverage),
            ])?;
            Ok(vec![
                template_spec(
                    CovenantType::MaxTotalLeverage {
                        threshold: max_leverage,
                    },
                    Tenor::quarterly(),
                    CovenantScope::Incurrence,
                    "total_leverage",
                    None,
                    None,
                ),
                template_spec(
                    CovenantType::MaxSeniorLeverage {
                        threshold: max_senior_leverage,
                    },
                    Tenor::quarterly(),
                    CovenantScope::Incurrence,
                    "senior_leverage",
                    None,
                    None,
                ),
                template_spec(
                    CovenantType::Negative {
                        restriction: "No additional secured debt without consent".to_string(),
                    },
                    Tenor::annual(),
                    CovenantScope::Incurrence,
                    "negative_debt",
                    None,
                    None,
                ),
            ])
        }
        CovenantTemplate::RealEstate {
            min_dscr,
            min_debt_yield,
            max_ltv,
        } => {
            check_thresholds(&[
                ("min_dscr", min_dscr),
                ("min_debt_yield", min_debt_yield),
                ("max_ltv", max_ltv),
            ])?;
            Ok(vec![
                template_spec(
                    CovenantType::MinDscr {
                        threshold: min_dscr,
                    },
                    Tenor::quarterly(),
                    CovenantScope::Maintenance,
                    "dscr",
                    None,
                    Some(CovenantConsequence::CashSweep {
                        sweep_percentage: 1.0,
                    }),
                ),
                template_spec(
                    CovenantType::Custom {
                        metric: "debt_yield".to_string(),
                        test: ThresholdTest::Minimum(min_debt_yield),
                    },
                    Tenor::quarterly(),
                    CovenantScope::Maintenance,
                    "debt_yield",
                    Some("min_debt_yield"),
                    None,
                ),
                template_spec(
                    CovenantType::Custom {
                        metric: "ltv".to_string(),
                        test: ThresholdTest::Maximum(max_ltv),
                    },
                    Tenor::quarterly(),
                    CovenantScope::Maintenance,
                    "ltv",
                    Some("max_ltv"),
                    Some(CovenantConsequence::CashSweep {
                        sweep_percentage: 0.5,
                    }),
                ),
            ])
        }
        CovenantTemplate::ProjectFinance {
            min_dscr,
            distribution_lockup_dscr,
            min_liquidity,
            max_net_leverage,
        } => {
            check_thresholds(&[
                ("min_dscr", min_dscr),
                ("distribution_lockup_dscr", distribution_lockup_dscr),
                ("min_liquidity", min_liquidity),
                ("max_net_leverage", max_net_leverage),
            ])?;
            let mut default_dscr = template_spec(
                CovenantType::MinDscr {
                    threshold: min_dscr,
                },
                Tenor::quarterly(),
                CovenantScope::Maintenance,
                "dscr",
                Some("min_dscr_default"),
                Some(CovenantConsequence::Default),
            );
            default_dscr.covenant.cure_period_days = Some(60);
            Ok(vec![
                default_dscr,
                template_spec(
                    CovenantType::MinDscr {
                        threshold: distribution_lockup_dscr,
                    },
                    Tenor::quarterly(),
                    CovenantScope::Maintenance,
                    "dscr",
                    Some("min_dscr_lockup"),
                    Some(CovenantConsequence::BlockDistributions),
                ),
                template_spec(
                    CovenantType::MinLiquidity {
                        threshold: min_liquidity,
                    },
                    Tenor::quarterly(),
                    CovenantScope::Maintenance,
                    "liquidity",
                    None,
                    None,
                ),
                template_spec(
                    CovenantType::MaxNetDebtToEbitda {
                        threshold: max_net_leverage,
                    },
                    Tenor::quarterly(),
                    CovenantScope::Maintenance,
                    "net_debt_to_ebitda",
                    None,
                    None,
                ),
            ])
        }
    }
}

/// Reject any template threshold that is not finite and non-negative.
fn check_thresholds(entries: &[(&str, f64)]) -> Result<()> {
    for &(name, value) in entries {
        if !value.is_finite() || value < 0.0 {
            return Err(Error::Validation(format!(
                "covenant template threshold `{name}` must be finite and non-negative, got {value}"
            )));
        }
    }
    Ok(())
}

fn template_spec(
    cov_type: CovenantType,
    frequency: Tenor,
    scope: CovenantScope,
    metric: &str,
    label_override: Option<&str>,
    consequence: Option<CovenantConsequence>,
) -> CovenantSpec {
    let label = label_override
        .map(str::to_owned)
        .unwrap_or_else(|| cov_type.covenant_id().to_string());
    let mut cov = Covenant::new(cov_type, frequency, label).with_scope(scope);
    if let Some(c) = consequence {
        cov = cov.with_consequence(c);
    }
    CovenantSpec::with_metric(cov, metric)
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
