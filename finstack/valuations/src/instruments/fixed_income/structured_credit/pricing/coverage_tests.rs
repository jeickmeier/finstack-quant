//! Coverage test calculations for structured credit instruments.
//!
//! This module provides OC and IC test calculations for waterfall diversion.

use crate::instruments::fixed_income::structured_credit::types::{Pool, TrancheStructure};
use crate::instruments::fixed_income::structured_credit::utils::frequency_periods_per_year;
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;
use finstack_core::types::CreditRating;
use finstack_core::HashMap;
use finstack_core::Result;
use finstack_core::{Error as CoreError, InputError};

use serde::{Deserialize, Serialize};

/// Coverage test type (OC/IC).
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[non_exhaustive]
pub enum CoverageTest {
    /// Overcollateralization test.
    OC {
        /// Unique test identifier.
        id: String,
        /// Required OC ratio (e.g., 1.25 = 125%).
        required_ratio: f64,
        /// Include cash in numerator.
        include_cash: bool,
        /// Include only performing assets.
        performing_only: bool,
    },
    /// Interest coverage test.
    IC {
        /// Unique test identifier.
        id: String,
        /// Required IC ratio (e.g., 1.20 = 120%).
        required_ratio: f64,
    },
}

impl CoverageTest {
    /// Create new OC test with standard settings.
    pub fn new_oc(required_ratio: f64) -> Self {
        Self::OC {
            id: format!("oc_test_{}", (required_ratio * 100.0) as u32),
            required_ratio,
            include_cash: true,
            performing_only: true,
        }
    }

    /// Create new OC test with explicit ID.
    pub fn new_oc_with_id(id: impl Into<String>, required_ratio: f64) -> Self {
        Self::OC {
            id: id.into(),
            required_ratio,
            include_cash: true,
            performing_only: true,
        }
    }

    /// Create new IC test.
    pub fn new_ic(required_ratio: f64) -> Self {
        Self::IC {
            id: format!("ic_test_{}", (required_ratio * 100.0) as u32),
            required_ratio,
        }
    }

    /// Create new IC test with explicit ID.
    pub fn new_ic_with_id(id: impl Into<String>, required_ratio: f64) -> Self {
        Self::IC {
            id: id.into(),
            required_ratio,
        }
    }

    /// Get the test ID.
    pub fn id(&self) -> &str {
        match self {
            Self::OC { id, .. } => id.as_str(),
            Self::IC { id, .. } => id.as_str(),
        }
    }

    /// Get the required ratio for this test.
    pub fn required_level(&self) -> f64 {
        match self {
            Self::OC { required_ratio, .. } => *required_ratio,
            Self::IC { required_ratio, .. } => *required_ratio,
        }
    }

    /// Calculate the test result.
    pub fn calculate(&self, context: &TestContext) -> Result<TestResult> {
        match self {
            Self::OC {
                id,
                required_ratio,
                include_cash,
                performing_only,
            } => self.calculate_oc(
                context,
                id.clone(),
                *required_ratio,
                *include_cash,
                *performing_only,
            ),
            Self::IC { id, required_ratio } => {
                self.calculate_ic(context, id.clone(), *required_ratio)
            }
        }
    }

    fn calculate_oc(
        &self,
        context: &TestContext,
        test_id: String,
        required_ratio: f64,
        include_cash: bool,
        performing_only: bool,
    ) -> Result<TestResult> {
        let tranche = context
            .tranches
            .tranches
            .iter()
            .find(|t| t.id.as_str() == context.tranche_id)
            .ok_or_else(|| {
                CoreError::from(InputError::NotFound {
                    id: format!("tranche:{}", context.tranche_id),
                })
            })?;

        // Use current tranche balances when available (M4 fix: avoid stale balances)
        let tranche_balance = context
            .tranche_balances
            .and_then(|b| b.get(tranche.id.as_str()))
            .copied()
            .unwrap_or(tranche.current_balance);

        let senior_balance = if let Some(tb) = context.tranche_balances {
            context
                .tranches
                .senior_to(context.tranche_id)
                .iter()
                .try_fold(Money::new(0.0, tranche_balance.currency()), |acc, t| {
                    let bal = tb.get(t.id.as_str()).copied().unwrap_or(t.current_balance);
                    acc.checked_add(bal)
                })
                .unwrap_or_else(|_| Money::new(0.0, tranche_balance.currency()))
        } else {
            context.tranches.senior_balance(context.tranche_id)
        };

        // Use current pool balance override when available and no haircuts
        let mut numerator = if let (Some(pool_bal), true) = (
            context.current_pool_balance,
            context.haircuts.map(|h| h.is_empty()).unwrap_or(true),
        ) {
            pool_bal
        } else {
            collateral_balance_with_haircuts(context.pool, performing_only, context.haircuts)?
        };

        if include_cash {
            numerator = numerator.checked_add(context.cash_balance)?;
        }

        // OC denominator = test tranche balance + all senior tranche balances
        // i.e., Sum(all tranche balances at this seniority level and above)
        let denominator = tranche_balance
            .checked_add(senior_balance)
            .unwrap_or(tranche_balance);

        let ratio = if denominator.amount() > 0.0 {
            numerator.amount() / denominator.amount()
        } else {
            f64::INFINITY
        };

        let mut is_passing = ratio >= required_ratio;
        if let Some(threshold) = context.par_value_threshold {
            if ratio < threshold {
                is_passing = false;
            }
        }

        // Cure amount = note paydown needed to restore OC ratio.
        //
        // W-22: the numerator includes `cash_balance`. Diverting cash to pay
        // down notes removes that cash from the numerator at the same time it
        // pays down the denominator, so the cure solves
        //   (numerator - X) / (denominator - X) >= required_ratio
        //   => numerator - X >= required_ratio * (denominator - X)
        //   => X * (required_ratio - 1) >= required_ratio * denominator - numerator
        //   => X >= (numerator - required_ratio * denominator) / (1 - required_ratio)
        // Valid for any required_ratio != 1 (when breaching, both numerator and
        // denominator of this expression carry the same sign, so X is positive).
        // At required_ratio == 1 the diversion never changes the ratio, so the
        // breach is uncurable by self-funding paydown; report a zero cure.
        let cure_amount = if !is_passing && required_ratio > 0.0 {
            let denom = 1.0 - required_ratio;
            let paydown_needed = if denom.abs() > f64::EPSILON {
                (numerator.amount() - required_ratio * denominator.amount()) / denom
            } else {
                0.0
            };
            Some(Money::new(paydown_needed.max(0.0), denominator.currency()))
        } else {
            None
        };

        Ok(TestResult {
            test_id,
            tranche_id: context.tranche_id.to_string(),
            current_ratio: ratio,
            is_passing,
            cure_amount,
        })
    }

    fn calculate_ic(
        &self,
        context: &TestContext,
        test_id: String,
        required_ratio: f64,
    ) -> Result<TestResult> {
        let tranche = context
            .tranches
            .tranches
            .iter()
            .find(|t| t.id.as_str() == context.tranche_id)
            .ok_or_else(|| {
                CoreError::from(InputError::NotFound {
                    id: format!("tranche:{}", context.tranche_id),
                })
            })?;

        // Use full all-in rate (index + spread) when market context is available,
        // falling back to spread-only when market data is absent.
        let tranche_rate = if let Some(market) = context.market {
            tranche
                .coupon
                .try_current_rate_with_index(context.as_of, market)
                .unwrap_or_else(|_| tranche.coupon.current_rate(context.as_of))
        } else {
            tranche.coupon.current_rate(context.as_of)
        };

        // Use current tranche balances when available (M4 fix)
        let tranche_bal = context
            .tranche_balances
            .and_then(|b| b.get(tranche.id.as_str()))
            .copied()
            .unwrap_or(tranche.current_balance);

        // Use actual day-count accrual when period_start is available (m3 fix);
        // fall back to periods-per-year approximation as default behavior.
        let accrual_factor = if let Some(period_start) = context.period_start {
            tranche
                .day_count
                .year_fraction(
                    period_start,
                    context.as_of,
                    finstack_core::dates::DayCountContext::default(),
                )
                .unwrap_or_else(|_| 1.0 / frequency_periods_per_year(tranche.frequency))
        } else {
            1.0 / frequency_periods_per_year(tranche.frequency)
        };

        let interest_due = Money::new(
            tranche_bal.amount() * tranche_rate * accrual_factor,
            tranche_bal.currency(),
        );

        let senior_tranches = context.tranches.senior_to(context.tranche_id);
        let senior_interest_due = senior_tranches
            .iter()
            .try_fold(Money::new(0.0, interest_due.currency()), |acc, t| {
                let rate = if let Some(market) = context.market {
                    t.coupon
                        .try_current_rate_with_index(context.as_of, market)
                        .unwrap_or_else(|_| t.coupon.current_rate(context.as_of))
                } else {
                    t.coupon.current_rate(context.as_of)
                };
                let t_bal = context
                    .tranche_balances
                    .and_then(|b| b.get(t.id.as_str()))
                    .copied()
                    .unwrap_or(t.current_balance);
                let t_accrual = if let Some(period_start) = context.period_start {
                    t.day_count
                        .year_fraction(
                            period_start,
                            context.as_of,
                            finstack_core::dates::DayCountContext::default(),
                        )
                        .unwrap_or_else(|_| 1.0 / frequency_periods_per_year(t.frequency))
                } else {
                    1.0 / frequency_periods_per_year(t.frequency)
                };
                let interest = Money::new(t_bal.amount() * rate * t_accrual, t_bal.currency());
                acc.checked_add(interest)
            })
            .unwrap_or_else(|_| Money::new(0.0, interest_due.currency()));

        let total_interest_due = interest_due
            .checked_add(senior_interest_due)
            .unwrap_or(interest_due);

        let ratio = if total_interest_due.amount() > 0.0 {
            context.interest_collections.amount() / total_interest_due.amount()
        } else {
            f64::INFINITY
        };

        let is_passing = ratio >= required_ratio;

        // W-21: IC cure = cash that must be diverted to senior interest so the
        // test clears. The IC test passes when
        //   (interest_collections + X) / total_interest_due >= required_ratio
        //   => X >= required_ratio * total_interest_due - interest_collections
        // This is the senior interest shortfall against the required coverage
        // level; without it an IC-only breach diverts zero cash.
        let cure_amount = if !is_passing {
            let shortfall = required_ratio * total_interest_due.amount()
                - context.interest_collections.amount();
            Some(Money::new(
                shortfall.max(0.0),
                context.interest_collections.currency(),
            ))
        } else {
            None
        };

        Ok(TestResult {
            test_id,
            tranche_id: context.tranche_id.to_string(),
            current_ratio: ratio,
            is_passing,
            cure_amount,
        })
    }
}

/// Context needed to calculate coverage tests.
pub struct TestContext<'a> {
    /// Pool reference.
    pub pool: &'a Pool,
    /// Tranche structure reference.
    pub tranches: &'a TrancheStructure,
    /// Target tranche ID.
    pub tranche_id: &'a str,
    /// As-of date.
    pub as_of: finstack_core::dates::Date,
    /// Period start date for day-count accrual.
    pub period_start: Option<finstack_core::dates::Date>,
    /// Cash balance.
    pub cash_balance: Money,
    /// Interest collections.
    pub interest_collections: Money,
    /// Optional rating haircuts for collateral.
    pub haircuts: Option<&'a HashMap<CreditRating, f64>>,
    /// Optional par value threshold (ratio).
    pub par_value_threshold: Option<f64>,
    /// Optional market context for floating rate index lookups in IC tests.
    pub market: Option<&'a MarketContext>,
    /// Current tranche balances (overrides `tranche.current_balance` when present).
    pub tranche_balances: Option<&'a HashMap<String, Money>>,
    /// Current pool balance override (used when asset-level balances are stale).
    pub current_pool_balance: Option<Money>,
}

/// Result of a coverage test calculation.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TestResult {
    /// Test identifier.
    pub test_id: String,
    /// Tranche identifier associated with the test.
    #[serde(default)]
    pub tranche_id: String,
    /// Current calculated ratio.
    pub current_ratio: f64,
    /// Whether test is currently passing.
    pub is_passing: bool,
    /// Cure amount if failing. For OC tests this is the note paydown needed to
    /// restore the OC ratio; for IC tests it is the cash to divert to senior
    /// interest so the test clears.
    pub cure_amount: Option<Money>,
}

fn collateral_balance_with_haircuts(
    pool: &Pool,
    performing_only: bool,
    haircuts: Option<&HashMap<CreditRating, f64>>,
) -> Result<Money> {
    if haircuts.map(|h| h.is_empty()).unwrap_or(true) {
        return Ok(if performing_only {
            pool.performing_balance()?
        } else {
            pool.total_balance()?
        });
    }

    let mut total = Money::new(0.0, pool.base_currency());
    for asset in &pool.assets {
        if performing_only && asset.is_defaulted {
            continue;
        }

        let mut amount = asset.balance.amount();
        if let Some(map) = haircuts {
            let haircut = asset
                .credit_quality
                .and_then(|rating| map.get(&rating).copied())
                .or_else(|| map.get(&CreditRating::NR).copied())
                .unwrap_or(0.0)
                .clamp(0.0, 1.0);
            amount *= 1.0 - haircut;
        }

        total = total.checked_add(Money::new(amount, total.currency()))?;
    }

    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::fixed_income::structured_credit::types::{
        DealType, Pool, Seniority, Tranche, TrancheCoupon, TrancheStructure,
    };
    use finstack_core::currency::Currency;
    use finstack_core::dates::Date;
    use time::Month;

    #[test]
    fn test_oc_test_creation() {
        let test = CoverageTest::new_oc(1.15);
        assert_eq!(test.required_level(), 1.15);
    }

    #[test]
    fn test_oc_test_calculation() {
        let pool = Pool::new("TEST", DealType::CLO, Currency::USD);
        let test = CoverageTest::new_oc(1.25);

        let tranche = Tranche::new(
            "TEST_TRANCHE",
            0.0,
            100.0,
            Seniority::Senior,
            Money::new(100_000.0, Currency::USD),
            TrancheCoupon::Fixed { rate: 0.05 },
            Date::from_calendar_date(2030, Month::January, 1).expect("Valid date"),
        )
        .expect("Valid tranche");

        let tranches = TrancheStructure::new(vec![tranche]).expect("Valid tranche structure");

        let context = TestContext {
            pool: &pool,
            tranches: &tranches,
            tranche_id: "TEST_TRANCHE",
            as_of: Date::from_calendar_date(2025, Month::January, 1).expect("Valid date"),
            period_start: None,
            cash_balance: Money::new(0.0, Currency::USD),
            interest_collections: Money::new(0.0, Currency::USD),
            haircuts: None,
            par_value_threshold: None,
            market: None,
            tranche_balances: None,
            current_pool_balance: None,
        };

        let result = test
            .calculate(&context)
            .expect("calculation should succeed");

        assert_eq!(result.current_ratio, 0.0);
        assert!(!result.is_passing);
    }

    #[test]
    fn test_ic_test_calculation() {
        let pool = Pool::new("TEST", DealType::CLO, Currency::USD);
        let test = CoverageTest::new_ic(1.20);

        let tranche = Tranche::new(
            "TEST_TRANCHE",
            0.0,
            100.0,
            Seniority::Senior,
            Money::new(100_000.0, Currency::USD),
            TrancheCoupon::Fixed { rate: 0.05 },
            Date::from_calendar_date(2030, Month::January, 1).expect("Valid date"),
        )
        .expect("Valid tranche");

        let tranches = TrancheStructure::new(vec![tranche]).expect("Valid tranche structure");

        let context = TestContext {
            pool: &pool,
            tranches: &tranches,
            tranche_id: "TEST_TRANCHE",
            as_of: Date::from_calendar_date(2025, Month::January, 1).expect("Valid date"),
            period_start: None,
            cash_balance: Money::new(0.0, Currency::USD),
            interest_collections: Money::new(1_500.0, Currency::USD),
            haircuts: None,
            par_value_threshold: None,
            market: None,
            tranche_balances: None,
            current_pool_balance: None,
        };

        let result = test
            .calculate(&context)
            .expect("calculation should succeed");

        assert!((result.current_ratio - 1.2).abs() < 0.01);
        assert!(result.is_passing);
    }

    /// W-22: the OC cure amount must account for the cash term leaving the
    /// numerator when it is diverted to pay down notes. Diverting the computed
    /// cure must bring the OC ratio to exactly the required ratio.
    #[test]
    fn test_oc_cure_with_cash_in_numerator_restores_exact_ratio() {
        // Numerator = collateral (90k, stays) + cash (30k, leaves on diversion).
        // Denominator = 100k. Ratio = 120k / 100k = 1.20, breaches a 1.25 trigger.
        let pool = Pool::new("TEST", DealType::CLO, Currency::USD);
        let required_ratio = 1.25_f64;
        let test = CoverageTest::new_oc(required_ratio);

        let tranche = Tranche::new(
            "SENIOR",
            0.0,
            100.0,
            Seniority::Senior,
            Money::new(100_000.0, Currency::USD),
            TrancheCoupon::Fixed { rate: 0.05 },
            Date::from_calendar_date(2030, Month::January, 1).expect("Valid date"),
        )
        .expect("Valid tranche");
        let tranches = TrancheStructure::new(vec![tranche]).expect("Valid tranche structure");

        let collateral = 90_000.0_f64;
        let cash = 30_000.0_f64;
        let context = TestContext {
            pool: &pool,
            tranches: &tranches,
            tranche_id: "SENIOR",
            as_of: Date::from_calendar_date(2025, Month::January, 1).expect("Valid date"),
            period_start: None,
            cash_balance: Money::new(cash, Currency::USD),
            interest_collections: Money::new(0.0, Currency::USD),
            haircuts: None,
            par_value_threshold: None,
            market: None,
            tranche_balances: None,
            current_pool_balance: Some(Money::new(collateral, Currency::USD)),
        };

        let result = test.calculate(&context).expect("calculation should succeed");
        assert!(!result.is_passing, "OC test should breach (ratio 1.20 < 1.25)");

        let cure = result.cure_amount.expect("breach must yield a cure amount");
        let x = cure.amount();
        assert!(
            x <= cash + 1e-6,
            "cure {x} should be fundable from available cash {cash}"
        );

        // numerator = collateral + cash, denominator = 100k.
        let numerator = collateral + cash;
        let denominator = 100_000.0_f64;
        // Diverting X removes cash from the numerator AND pays down the
        // denominator. The cured ratio must equal the required ratio exactly.
        let cured_ratio = (numerator - x) / (denominator - x);
        assert!(
            (cured_ratio - required_ratio).abs() < 1e-6,
            "cured ratio {cured_ratio} should equal required {required_ratio}; cure X={x}"
        );
    }

    /// W-21: an IC-test breach must produce a non-`None` cure amount equal to
    /// the senior interest shortfall, so IC-only breaches actually divert cash.
    #[test]
    fn test_ic_breach_yields_senior_interest_shortfall_cure() {
        let pool = Pool::new("TEST", DealType::CLO, Currency::USD);
        let required_ratio = 1.20_f64;
        let test = CoverageTest::new_ic(required_ratio);

        let tranche = Tranche::new(
            "TEST_TRANCHE",
            0.0,
            100.0,
            Seniority::Senior,
            Money::new(100_000.0, Currency::USD),
            TrancheCoupon::Fixed { rate: 0.05 },
            Date::from_calendar_date(2030, Month::January, 1).expect("Valid date"),
        )
        .expect("Valid tranche");
        let tranches = TrancheStructure::new(vec![tranche]).expect("Valid tranche structure");

        // Interest collections far below interest due => IC test breaches.
        let context = TestContext {
            pool: &pool,
            tranches: &tranches,
            tranche_id: "TEST_TRANCHE",
            as_of: Date::from_calendar_date(2025, Month::January, 1).expect("Valid date"),
            period_start: None,
            cash_balance: Money::new(0.0, Currency::USD),
            interest_collections: Money::new(100.0, Currency::USD),
            haircuts: None,
            par_value_threshold: None,
            market: None,
            tranche_balances: None,
            current_pool_balance: None,
        };

        let result = test.calculate(&context).expect("calculation should succeed");
        assert!(!result.is_passing, "IC test should breach");
        let cure = result
            .cure_amount
            .expect("an IC breach must yield a non-None cure amount");
        assert!(
            cure.amount() > 0.0,
            "IC cure must be positive (the interest shortfall), got {}",
            cure.amount()
        );
    }
}
