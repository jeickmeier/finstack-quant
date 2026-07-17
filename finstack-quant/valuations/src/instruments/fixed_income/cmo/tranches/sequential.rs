//! Sequential pay tranche logic.
//!
//! Sequential tranches receive principal in strict priority order -
//! the first tranche must be completely paid off before the next
//! receives any principal.

use crate::instruments::fixed_income::cmo::types::CmoTranche;

/// Sequential payment order configuration.
#[derive(Debug, Clone)]
pub struct SequentialOrder {
    /// Tranche IDs in payment order
    pub order: Vec<String>,
}

impl SequentialOrder {
    /// Create from tranches (sorts by priority).
    pub fn from_tranches(tranches: &[CmoTranche]) -> Self {
        let mut sorted: Vec<_> = tranches.iter().collect();
        sorted.sort_by_key(|t| t.priority);

        Self {
            order: sorted.iter().map(|t| t.id.clone()).collect(),
        }
    }

    /// Get the next tranche to receive principal.
    pub fn next_to_pay(&self, current_balances: &[(String, f64)]) -> Option<String> {
        for id in &self.order {
            if let Some((_, balance)) = current_balances.iter().find(|(tid, _)| tid == id) {
                if *balance > 0.0 {
                    return Some(id.clone());
                }
            }
        }
        None
    }
}

/// Calculate average life for sequential tranche.
///
/// Average life is the weighted average time to receipt of principal.
///
/// # Arguments
///
/// * `principal_payments` - Vec of (time_in_years, principal_amount)
///
/// # Returns
///
/// Average life in years
pub fn average_life(principal_payments: &[(f64, f64)]) -> f64 {
    let total_principal: f64 = principal_payments.iter().map(|(_, p)| p).sum();

    if total_principal <= 0.0 {
        return 0.0;
    }

    let weighted_sum: f64 = principal_payments.iter().map(|(t, p)| t * p).sum();

    weighted_sum / total_principal
}

/// Estimate average life window for sequential tranche.
///
/// Provides the expected start and end of principal payments
/// based on prepayment assumptions.
///
/// # Arguments
///
/// * `tranche_size` - Principal balance of the sequential tranche whose
///   expected payment window is estimated.
/// * `preceding_balance` - Aggregate unpaid balance of tranches ahead of this
///   tranche in the sequential principal-payment order.
/// * `_total_collateral` - Total collateral balance retained for a common
///   tranche-analysis interface. This simplified timing estimate does not use
///   it directly because `preceding_balance` already determines the start.
/// * `monthly_principal_rate` - Expected monthly scheduled-plus-prepayment
///   principal amount available to the sequential waterfall.
///
/// # Returns
///
/// (start_month, end_month) tuple
pub fn estimate_payment_window(
    tranche_size: f64,
    preceding_balance: f64,
    _total_collateral: f64,
    monthly_principal_rate: f64,
) -> (u32, u32) {
    if monthly_principal_rate <= 0.0 {
        return (0, 360);
    }

    // Start: when preceding tranches are paid off
    let start_month = (preceding_balance / monthly_principal_rate).ceil() as u32;

    // End: when this tranche is paid off
    let end_month = start_month + (tranche_size / monthly_principal_rate).ceil() as u32;

    (start_month, end_month)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::fixed_income::cmo::types::CmoTranche;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::money::Money;

    #[test]
    fn test_sequential_order() {
        let tranches = vec![
            CmoTranche::sequential("A", Money::new(100.0, Currency::USD), 0.04, 1),
            CmoTranche::sequential("B", Money::new(100.0, Currency::USD), 0.05, 2),
            CmoTranche::sequential("C", Money::new(100.0, Currency::USD), 0.06, 3),
        ];

        let order = SequentialOrder::from_tranches(&tranches);

        assert_eq!(order.order, vec!["A", "B", "C"]);
    }

    #[test]
    fn test_next_to_pay() {
        let order = SequentialOrder {
            order: vec!["A".to_string(), "B".to_string(), "C".to_string()],
        };

        // All have balance - A is next
        let balances = vec![
            ("A".to_string(), 100.0),
            ("B".to_string(), 100.0),
            ("C".to_string(), 100.0),
        ];
        assert_eq!(order.next_to_pay(&balances), Some("A".to_string()));

        // A paid off - B is next
        let balances2 = vec![
            ("A".to_string(), 0.0),
            ("B".to_string(), 100.0),
            ("C".to_string(), 100.0),
        ];
        assert_eq!(order.next_to_pay(&balances2), Some("B".to_string()));
    }

    #[test]
    fn test_average_life() {
        // Simple case: all principal at year 5
        let payments = vec![(5.0, 100.0)];
        assert!((average_life(&payments) - 5.0).abs() < 0.001);

        // Split payment
        let payments2 = vec![(2.0, 50.0), (6.0, 50.0)];
        let avg = average_life(&payments2);
        assert!((avg - 4.0).abs() < 0.001); // Weighted average: (2×50 + 6×50)/100 = 4
    }

    #[test]
    fn test_payment_window() {
        // First tranche (no preceding balance)
        let (start, end) = estimate_payment_window(10_000.0, 0.0, 100_000.0, 500.0);
        assert_eq!(start, 0);
        assert_eq!(end, 20); // 10,000 / 500 = 20 months

        // Second tranche (behind first)
        let (start2, end2) = estimate_payment_window(10_000.0, 10_000.0, 100_000.0, 500.0);
        assert_eq!(start2, 20); // After first is paid
        assert_eq!(end2, 40);
    }
}
