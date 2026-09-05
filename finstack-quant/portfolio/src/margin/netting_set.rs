//! Netting set types and management.
//!
//! A netting set is a collection of trades that can be netted against each
//! other for margin calculation purposes, typically defined by a master
//! agreement (CSA) or clearing membership.

use finstack_quant_margin::{NettingSetId, OtcMarginSpec, SimmSensitivities};

use crate::types::PositionId;

/// A netting set containing positions for margin aggregation.
///
/// Positions in the same netting set can offset each other's risk
/// for margin calculation purposes.
#[derive(Debug, Clone)]
pub(super) struct NettingSet {
    /// Netting set identifier
    pub(super) id: NettingSetId,
    /// Position IDs in this netting set
    pub(super) positions: Vec<PositionId>,
    /// Margin specification (from CSA or CCP)
    pub(super) margin_spec: Option<OtcMarginSpec>,
    /// Aggregated sensitivities for this netting set
    pub(super) aggregated_sensitivities: Option<SimmSensitivities>,
}

impl NettingSet {
    /// Create a new empty netting set.
    ///
    /// # Arguments
    ///
    /// * `id` - Netting-set identifier, usually driven by CSA or CCP membership.
    ///
    /// # Returns
    ///
    /// Empty netting set with no positions or cached sensitivities.
    #[must_use]
    pub(super) fn new(id: NettingSetId) -> Self {
        Self {
            id,
            positions: Vec::new(),
            margin_spec: None,
            aggregated_sensitivities: None,
        }
    }

    /// Check if the netting set is cleared.
    ///
    /// # Returns
    ///
    /// `true` when the identifier describes a cleared venue rather than a
    /// bilateral agreement.
    #[must_use]
    pub(super) fn is_cleared(&self) -> bool {
        self.id.is_cleared()
    }

    /// Merge sensitivities into this netting set.
    ///
    /// # Arguments
    ///
    /// * `sensitivities` - Additional sensitivities to accumulate.
    pub(super) fn merge_sensitivities(&mut self, sensitivities: &SimmSensitivities) {
        if let Some(ref mut agg) = self.aggregated_sensitivities {
            agg.merge(sensitivities);
        } else {
            self.aggregated_sensitivities = Some(sensitivities.clone());
        }
    }

    /// Clear per-run aggregated sensitivities.
    pub(super) fn reset_sensitivities(&mut self) {
        self.aggregated_sensitivities = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;

    #[test]
    fn test_netting_set_creation() {
        let id = NettingSetId::bilateral("COUNTERPARTY_A", "CSA_001");
        let ns = NettingSet::new(id.clone());

        assert_eq!(ns.id, id);
        assert!(ns.positions.is_empty());
        assert!(!ns.is_cleared());
    }

    #[test]
    fn test_cleared_netting_set() {
        let id = NettingSetId::cleared("LCH");
        let ns = NettingSet::new(id);

        assert!(ns.is_cleared());
    }

    #[test]
    fn test_sensitivities_aggregation() {
        let id = NettingSetId::bilateral("BANK_A", "CSA_001");
        let mut ns = NettingSet::new(id);

        let mut sens1 = SimmSensitivities::new(Currency::USD);
        sens1.add_ir_delta(Currency::USD, "5Y", 100_000.0);

        let mut sens2 = SimmSensitivities::new(Currency::USD);
        sens2.add_ir_delta(Currency::USD, "5Y", -50_000.0);
        sens2.add_ir_delta(Currency::USD, "10Y", 30_000.0);

        // Merge sensitivities
        ns.merge_sensitivities(&sens1);
        ns.merge_sensitivities(&sens2);

        let agg = ns
            .aggregated_sensitivities
            .expect("should have sensitivities");

        // 5Y should be netted: 100,000 - 50,000 = 50,000
        assert_eq!(
            agg.ir_delta.get(&(Currency::USD, "5Y".to_string())),
            Some(&50_000.0)
        );
        // 10Y should be 30,000
        assert_eq!(
            agg.ir_delta.get(&(Currency::USD, "10Y".to_string())),
            Some(&30_000.0)
        );
    }
}
