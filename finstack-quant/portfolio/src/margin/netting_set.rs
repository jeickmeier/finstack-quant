//! Netting set grouping for one margin run.
//!
//! A netting set is a collection of trades that can be netted against each
//! other for margin calculation purposes, typically defined by a master
//! agreement (CSA) or clearing membership.

use finstack_quant_margin::{NettingSetId, OtcMarginSpec};

use crate::position::Position;

/// The positions of one netting set within a single margin run.
///
/// Positions in the same netting set can offset each other's risk
/// for margin calculation purposes.
#[derive(Debug, Clone)]
pub(super) struct NettingSet<'a> {
    /// Netting set identifier
    pub(super) id: NettingSetId,
    /// Positions in this netting set, in portfolio order
    pub(super) positions: Vec<&'a Position>,
    /// Margin specification (from CSA or CCP)
    pub(super) margin_spec: Option<OtcMarginSpec>,
}

impl NettingSet<'_> {
    /// Create a new empty netting set.
    ///
    /// # Arguments
    ///
    /// * `id` - Netting-set identifier, usually driven by CSA or CCP membership.
    ///
    /// # Returns
    ///
    /// Empty netting set with no positions or margin specification.
    #[must_use]
    pub(super) fn new(id: NettingSetId) -> Self {
        Self {
            id,
            positions: Vec::new(),
            margin_spec: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_netting_set_creation() {
        let id = NettingSetId::bilateral("COUNTERPARTY_A", "CSA_001");
        let ns = NettingSet::new(id.clone());

        assert_eq!(ns.id, id);
        assert!(ns.positions.is_empty());
        assert!(ns.margin_spec.is_none());
    }
}
