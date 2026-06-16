//! Covenant package templates for common deal structures.
//!
//! These presets encode standard covenant packages used across different
//! lending markets. Each function returns a `Vec<CovenantSpec>` ready for
//! insertion into a [`crate::CovenantEngine`].

use super::engine::{
    Covenant, CovenantConsequence, CovenantScope, CovenantSpec, CovenantType, ThresholdTest,
};
use finstack_quant_core::dates::Tenor;

fn maintenance(cov_type: CovenantType, freq: Tenor, metric: &str) -> CovenantSpec {
    CovenantSpec::with_metric(
        Covenant::new(cov_type, freq).with_scope(CovenantScope::Maintenance),
        metric,
    )
}

fn incurrence(cov_type: CovenantType, freq: Tenor, metric: &str) -> CovenantSpec {
    CovenantSpec::with_metric(
        Covenant::new(cov_type, freq).with_scope(CovenantScope::Incurrence),
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
pub fn lbo_standard(
    initial_leverage: f64,
    interest_coverage: f64,
    fixed_charge_coverage: f64,
    max_capex: f64,
) -> Vec<CovenantSpec> {
    vec![
        {
            let mut s = maintenance(
                CovenantType::MaxDebtToEBITDA {
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
    ]
}

/// "Covenant-lite" leveraged loan package (incurrence only).
///
/// Post-2015 leveraged loan standard with no maintenance covenants.
/// Only tested upon specific incurrence actions (new debt, acquisitions, dividends).
pub fn cov_lite(max_leverage: f64, max_senior_leverage: f64) -> Vec<CovenantSpec> {
    vec![
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
    ]
}

/// Commercial real estate (CRE) covenant package.
///
/// Standard for income-producing assets with:
/// - Min DSCR (primary maintenance covenant)
/// - Min Debt Yield (Net Operating Income / Loan Balance)
/// - Max Loan-to-Value (custom metric)
/// - Cash sweep triggered by DSCR breach
pub fn real_estate(min_dscr: f64, min_debt_yield: f64, max_ltv: f64) -> Vec<CovenantSpec> {
    vec![
        {
            let mut s = maintenance(
                CovenantType::MinDSCR {
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
            // Two Custom covenants share covenant_id "custom"; label them so
            // their reports/breaches don't collide.
            s.covenant = s.covenant.with_label("min_debt_yield");
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
            s.covenant = s.covenant.with_label("max_ltv");
            s.covenant
                .consequences
                .push(CovenantConsequence::CashSweep {
                    sweep_percentage: 0.5,
                });
            s
        },
    ]
}

/// Infrastructure / project finance covenant package.
///
/// Standard for long-dated project finance with:
/// - Min DSCR (primary maintenance) — labeled `min_dscr_default`
/// - Min DSCR for distribution lock-up (higher threshold) — labeled `min_dscr_lockup`
/// - Min Liquidity (debt service reserve)
/// - Max Net Debt / EBITDA
///
/// The two `MinDSCR` covenants share a type but carry distinct instance
/// labels so their reports, breaches, and consequences never collide: a
/// lock-up breach blocks distributions only and can never resolve to the
/// primary covenant's Event-of-Default consequence.
pub fn project_finance(
    min_dscr: f64,
    distribution_lockup_dscr: f64,
    min_liquidity: f64,
    max_net_leverage: f64,
) -> Vec<CovenantSpec> {
    vec![
        {
            let mut s = maintenance(
                CovenantType::MinDSCR {
                    threshold: min_dscr,
                },
                Tenor::quarterly(),
                "dscr",
            );
            s.covenant = s.covenant.with_label("min_dscr_default");
            s.covenant.cure_period_days = Some(60);
            s.covenant.consequences.push(CovenantConsequence::Default);
            s
        },
        {
            let mut s = maintenance(
                CovenantType::MinDSCR {
                    threshold: distribution_lockup_dscr,
                },
                Tenor::quarterly(),
                "dscr",
            );
            s.covenant = s.covenant.with_label("min_dscr_lockup");
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
            CovenantType::MaxNetDebtToEBITDA {
                threshold: max_net_leverage,
            },
            Tenor::quarterly(),
            "net_debt_to_ebitda",
        ),
    ]
}
