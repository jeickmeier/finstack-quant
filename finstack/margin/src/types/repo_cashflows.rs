//! Margin cashflow generation for repos.

use finstack_core::cashflow::{CFKind, CashFlow};
use finstack_core::dates::Date;
use finstack_core::money::Money;

use super::{MarginCall, RepoMarginSpec};

/// Generate margin-related cashflows for a repo.
///
/// This function generates margin call cashflows based on a series of
/// collateral valuations over the life of the repo.
///
/// # Arguments
///
/// * `spec` - Repo margin specification
/// * `cash_amount` - Cash amount of the repo
/// * `valuations` - Time series of (date, collateral_value) pairs
/// * `currency` - Currency for cashflows
///
/// # Returns
///
/// Vector of margin-related cashflows.
pub fn generate_margin_cashflows(
    spec: &RepoMarginSpec,
    cash_amount: Money,
    valuations: &[(Date, f64)],
    currency: finstack_core::currency::Currency,
) -> Vec<CashFlow> {
    if !spec.has_margining() {
        return vec![];
    }

    let mut cashflows = Vec::new();
    let mut _current_collateral_posted = spec.required_collateral(cash_amount.amount());

    for (i, (date, collateral_value)) in valuations.iter().enumerate() {
        // Check if margin call needed
        let deficit = spec.margin_deficit(cash_amount.amount(), *collateral_value);
        let excess = spec.excess_collateral(cash_amount.amount(), *collateral_value);

        if deficit > 0.0 {
            // Need to deliver additional collateral
            cashflows.push(CashFlow {
                date: *date,
                reset_date: None,
                amount: Money::new(deficit, currency),
                kind: CFKind::VariationMarginPay,
                accrual_factor: 0.0,
                rate: None,
            });
            _current_collateral_posted += deficit;
        } else if excess > 0.0 && i > 0 {
            // Return excess collateral (not on first day)
            cashflows.push(CashFlow {
                date: *date,
                reset_date: None,
                amount: Money::new(excess, currency),
                kind: CFKind::VariationMarginReceive,
                accrual_factor: 0.0,
                rate: None,
            });
            _current_collateral_posted -= excess;
        }
    }

    cashflows
}

/// Generate margin interest cashflows.
///
/// For repos where margin interest is paid on cash margin transfers,
/// this generates the accrued interest cashflows.
///
/// # Arguments
///
/// * `spec` - Repo margin specification
/// * `margin_balances` - Time series of (date, cash_margin_balance) pairs
/// * `currency` - Currency for cashflows
/// * `day_count` - Day count convention for interest calculations (from the repo instrument)
///
/// # Returns
///
/// Vector of margin interest cashflows.
pub fn generate_margin_interest_cashflows(
    spec: &RepoMarginSpec,
    margin_balances: &[(Date, f64)],
    currency: finstack_core::currency::Currency,
    day_count: finstack_core::dates::DayCount,
) -> Vec<CashFlow> {
    if !spec.pays_margin_interest || spec.margin_interest_rate.is_none() {
        return vec![];
    }

    let rate = spec.margin_interest_rate.unwrap_or(0.0);
    let mut cashflows = Vec::new();

    // Calculate interest between consecutive dates using the repo's day count convention.
    // Previously this was hardcoded to Act/360; now it correctly uses the instrument's
    // configured day count (e.g., Act/365F for GBP repos).
    for window in margin_balances.windows(2) {
        if let [prev, curr] = window {
            let year_fraction = day_count
                .year_fraction(
                    prev.0,
                    curr.0,
                    finstack_core::dates::DayCountContext::default(),
                )
                .unwrap_or_else(|_| {
                    // Fallback: use calendar days / 360 if day count calculation fails
                    (curr.0 - prev.0).whole_days() as f64 / 360.0
                });
            let interest = prev.1 * rate * year_fraction;

            if interest.abs() > 0.01 {
                // Only generate if material
                cashflows.push(CashFlow {
                    date: curr.0,
                    reset_date: None,
                    amount: Money::new(interest, currency),
                    kind: CFKind::MarginInterest,
                    accrual_factor: year_fraction,
                    rate: Some(rate),
                });
            }
        }
    }

    cashflows
}

/// Convert margin calls to cashflows.
///
/// Transforms margin call events into classified cashflows for
/// inclusion in the full cashflow schedule.
pub fn margin_calls_to_cashflows(calls: &[MarginCall]) -> Vec<CashFlow> {
    calls
        .iter()
        .map(|call| {
            let kind = match call.call_type {
                super::MarginCallType::InitialMargin => CFKind::InitialMarginPost,
                super::MarginCallType::VariationMarginDelivery | super::MarginCallType::TopUp => {
                    CFKind::VariationMarginPay
                }
                super::MarginCallType::VariationMarginReturn => CFKind::VariationMarginReceive,
                super::MarginCallType::Substitution => CFKind::CollateralSubstitutionOut,
            };

            CashFlow {
                date: call.settlement_date,
                reset_date: Some(call.call_date),
                amount: call.amount,
                kind,
                accrual_factor: 0.0,
                rate: None,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_core::currency::Currency;
    use time::Month;

    fn test_date(y: i32, m: u8, d: u8) -> Date {
        Date::from_calendar_date(y, Month::try_from(m).expect("valid month"), d)
            .expect("valid date")
    }

    #[test]
    fn no_cashflows_for_none_margin_type() {
        let spec = RepoMarginSpec::none();
        let cash = Money::new(100_000_000.0, Currency::USD);
        let valuations = vec![
            (test_date(2025, 1, 15), 102_000_000.0),
            (test_date(2025, 1, 16), 100_000_000.0),
        ];

        let cashflows = generate_margin_cashflows(&spec, cash, &valuations, Currency::USD);
        assert!(cashflows.is_empty());
    }

    #[test]
    fn generates_margin_call_on_deficit() {
        let spec = RepoMarginSpec::mark_to_market(1.02, 0.01).expect("registry should load");
        let cash = Money::new(100_000_000.0, Currency::USD);
        let valuations = vec![
            (test_date(2025, 1, 15), 102_000_000.0), // Adequate
            (test_date(2025, 1, 16), 100_000_000.0), // Deficit of 2M
        ];

        let cashflows = generate_margin_cashflows(&spec, cash, &valuations, Currency::USD);

        // Should have one margin call for the 2M deficit
        assert_eq!(cashflows.len(), 1);
        assert_eq!(cashflows[0].kind, CFKind::VariationMarginPay);
        assert_eq!(cashflows[0].amount.amount(), 2_000_000.0);
    }

    #[test]
    fn generates_margin_return_on_excess() {
        let spec = RepoMarginSpec::mark_to_market(1.02, 0.01).expect("registry should load");
        let cash = Money::new(100_000_000.0, Currency::USD);
        let valuations = vec![
            (test_date(2025, 1, 15), 102_000_000.0), // Adequate
            (test_date(2025, 1, 16), 105_000_000.0), // Excess of 3M
        ];

        let cashflows = generate_margin_cashflows(&spec, cash, &valuations, Currency::USD);

        // Should have one margin return for the 3M excess
        assert_eq!(cashflows.len(), 1);
        assert_eq!(cashflows[0].kind, CFKind::VariationMarginReceive);
        assert_eq!(cashflows[0].amount.amount(), 3_000_000.0);
    }

    #[test]
    fn margin_interest_generation() {
        use finstack_core::dates::DayCount;

        let mut spec = RepoMarginSpec::mark_to_market(1.02, 0.01).expect("registry should load");
        spec.margin_interest_rate = Some(0.05); // 5% annual

        let margin_balances = vec![
            (test_date(2025, 1, 15), 2_000_000.0), // Day 1
            (test_date(2025, 1, 22), 2_000_000.0), // Day 8 (7 days later)
        ];

        let cashflows = generate_margin_interest_cashflows(
            &spec,
            &margin_balances,
            Currency::USD,
            DayCount::Act360,
        );

        // Interest = 2M * 5% * (7/360) ≈ 1944.44
        assert_eq!(cashflows.len(), 1);
        assert_eq!(cashflows[0].kind, CFKind::MarginInterest);
        assert!((cashflows[0].amount.amount() - 1944.44).abs() < 1.0);
    }

    #[test]
    fn margin_interest_respects_day_count() {
        use finstack_core::dates::DayCount;

        let mut spec = RepoMarginSpec::mark_to_market(1.02, 0.01).expect("registry should load");
        spec.margin_interest_rate = Some(0.05); // 5% annual

        let margin_balances = vec![
            (test_date(2025, 1, 15), 2_000_000.0),
            (test_date(2025, 1, 22), 2_000_000.0), // 7 days later
        ];

        let cf_360 = generate_margin_interest_cashflows(
            &spec,
            &margin_balances,
            Currency::USD,
            DayCount::Act360,
        );
        let cf_365 = generate_margin_interest_cashflows(
            &spec,
            &margin_balances,
            Currency::USD,
            DayCount::Act365F,
        );

        // Act/360 gives higher interest than Act/365F for same period
        assert!(cf_360[0].amount.amount() > cf_365[0].amount.amount());
        // Act/365F: 2M * 5% * (7/365) ≈ 1917.81
        assert!((cf_365[0].amount.amount() - 1917.81).abs() < 1.0);
    }
}
