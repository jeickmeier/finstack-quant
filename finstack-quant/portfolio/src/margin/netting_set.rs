//! Netting set types and management.
//!
//! A netting set is a collection of trades that can be netted against each
//! other for margin calculation purposes, typically defined by a master
//! agreement (CSA) or clearing membership.

use finstack_quant_core::HashMap;
use finstack_quant_margin::{NettingSetId, OtcMarginSpec, SimmSensitivities};

use crate::position::Position;
use crate::types::PositionId;

/// A netting set containing positions for margin aggregation.
///
/// Positions in the same netting set can offset each other's risk
/// for margin calculation purposes.
#[derive(Debug, Clone)]
pub struct NettingSet {
    /// Netting set identifier
    pub id: NettingSetId,
    /// Position IDs in this netting set
    pub positions: Vec<PositionId>,
    /// Margin specification (from CSA or CCP)
    pub margin_spec: Option<OtcMarginSpec>,
    /// Aggregated sensitivities for this netting set
    pub aggregated_sensitivities: Option<SimmSensitivities>,
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
    pub fn new(id: NettingSetId) -> Self {
        Self {
            id,
            positions: Vec::new(),
            margin_spec: None,
            aggregated_sensitivities: None,
        }
    }

    /// Create a new netting set with margin specification.
    ///
    /// # Arguments
    ///
    /// * `spec` - OTC margin specification associated with the set.
    ///
    /// # Returns
    ///
    /// The updated netting set for fluent chaining.
    #[must_use]
    pub fn with_margin_spec(mut self, spec: OtcMarginSpec) -> Self {
        self.margin_spec = Some(spec);
        self
    }

    /// Add a position to the netting set.
    ///
    /// # Arguments
    ///
    /// * `position_id` - Position to append.
    pub fn add_position(&mut self, position_id: PositionId) {
        self.positions.push(position_id);
    }

    /// Get the number of positions in this netting set.
    ///
    /// # Returns
    ///
    /// Count of directly assigned positions.
    #[must_use]
    pub fn position_count(&self) -> usize {
        self.positions.len()
    }

    /// Check if the netting set is cleared.
    ///
    /// # Returns
    ///
    /// `true` when the identifier describes a cleared venue rather than a
    /// bilateral agreement.
    #[must_use]
    pub fn is_cleared(&self) -> bool {
        self.id.is_cleared()
    }

    /// Merge sensitivities into this netting set.
    ///
    /// # Arguments
    ///
    /// * `sensitivities` - Additional sensitivities to accumulate.
    pub fn merge_sensitivities(&mut self, sensitivities: &SimmSensitivities) {
        if let Some(ref mut agg) = self.aggregated_sensitivities {
            agg.merge(sensitivities);
        } else {
            self.aggregated_sensitivities = Some(sensitivities.clone());
        }
    }

    /// Clear per-run aggregated sensitivities.
    pub fn reset_sensitivities(&mut self) {
        self.aggregated_sensitivities = None;
    }
}

/// Manager for organizing positions into netting sets.
///
/// Automatically groups positions based on their margin specifications
/// and CSA/CCP memberships.
#[derive(Debug, Default)]
pub struct NettingSetManager {
    /// Netting sets indexed by their ID
    netting_sets: HashMap<NettingSetId, NettingSet>,
    /// Default netting set for positions without margin specs
    default_set: Option<NettingSetId>,
}

impl NettingSetManager {
    /// Create a new empty netting set manager.
    ///
    /// # Returns
    ///
    /// Manager with no tracked netting sets and no default set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a default netting set for positions without margin specs.
    ///
    /// # Arguments
    ///
    /// * `id` - Identifier of the fallback netting set.
    ///
    /// # Returns
    ///
    /// The updated manager for fluent chaining.
    pub fn with_default_set(mut self, id: NettingSetId) -> Self {
        self.default_set = Some(id.clone());
        self.netting_sets
            .entry(id.clone())
            .or_insert_with(|| NettingSet::new(id));
        self
    }

    /// Add a position to the appropriate netting set.
    ///
    /// If the position has a margin spec with a netting set ID, it will be
    /// added to that set. Otherwise, it will be added to the default set
    /// (if configured) or skipped.
    pub fn add_position(
        &mut self,
        position: &Position,
        netting_set_id: Option<NettingSetId>,
        margin_spec: Option<OtcMarginSpec>,
    ) {
        let ns_id = netting_set_id.or_else(|| self.default_set.clone());

        if let Some(id) = ns_id {
            let ns = self
                .netting_sets
                .entry(id.clone())
                .or_insert_with(|| NettingSet::new(id));
            if let Some(spec) = margin_spec {
                match &ns.margin_spec {
                    None => ns.margin_spec = Some(spec),
                    Some(existing) if existing != &spec => {
                        tracing::warn!(
                            netting_set_id = ?ns.id,
                            "Conflicting margin specs registered for netting set; keeping first spec"
                        );
                    }
                    Some(_) => {}
                }
            }
            ns.add_position(position.position_id.clone());
        } else {
            tracing::warn!(
                position_id = %position.position_id,
                "Position has no netting set ID and no default set configured — excluded from margin calculation"
            );
        }
    }

    /// Get a netting set by ID.
    ///
    /// # Returns
    ///
    /// Borrowed netting set, if present.
    #[must_use]
    pub fn get(&self, id: &NettingSetId) -> Option<&NettingSet> {
        self.netting_sets.get(id)
    }

    /// Get a mutable reference to a netting set by ID.
    ///
    /// # Returns
    ///
    /// Mutable borrowed netting set, if present.
    pub fn get_mut(&mut self, id: &NettingSetId) -> Option<&mut NettingSet> {
        self.netting_sets.get_mut(id)
    }

    /// Iterate over all netting sets.
    ///
    /// # Returns
    ///
    /// Iterator over netting-set identifiers and their contents.
    pub fn iter(&self) -> impl Iterator<Item = (&NettingSetId, &NettingSet)> {
        self.netting_sets.iter()
    }

    /// Get the number of netting sets.
    ///
    /// # Returns
    ///
    /// Number of tracked netting sets.
    #[must_use]
    pub fn count(&self) -> usize {
        self.netting_sets.len()
    }

    /// Get all netting set IDs.
    ///
    /// # Returns
    ///
    /// Iterator over tracked identifiers.
    pub fn ids(&self) -> impl Iterator<Item = &NettingSetId> {
        self.netting_sets.keys()
    }

    /// Get or create a netting set.
    ///
    /// # Arguments
    ///
    /// * `id` - Identifier of the desired netting set.
    ///
    /// # Returns
    ///
    /// Mutable reference to the existing or newly created netting set.
    pub fn get_or_create(&mut self, id: NettingSetId) -> &mut NettingSet {
        self.netting_sets
            .entry(id.clone())
            .or_insert_with(|| NettingSet::new(id))
    }

    /// Merge sensitivities into a netting set.
    ///
    /// # Arguments
    ///
    /// * `netting_set_id` - Target netting set.
    /// * `sensitivities` - Sensitivities to merge into the target set.
    pub fn merge_sensitivities(
        &mut self,
        netting_set_id: &NettingSetId,
        sensitivities: &SimmSensitivities,
    ) {
        if let Some(ns) = self.netting_sets.get_mut(netting_set_id) {
            ns.merge_sensitivities(sensitivities);
        }
    }

    /// Clear all per-run aggregated sensitivities.
    pub fn reset_sensitivities(&mut self) {
        for netting_set in self.netting_sets.values_mut() {
            netting_set.reset_sensitivities();
        }
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
    fn test_netting_set_manager() {
        let mut manager = NettingSetManager::new();

        let bilateral_id = NettingSetId::bilateral("BANK_A", "CSA_001");
        let cleared_id = NettingSetId::cleared("CME");

        // Create netting sets
        manager.get_or_create(bilateral_id.clone());
        manager.get_or_create(cleared_id);

        assert_eq!(manager.count(), 2);

        // Add position to bilateral set
        let ns = manager.get_mut(&bilateral_id).expect("should exist");
        ns.add_position("POS_001".into());
        ns.add_position("POS_002".into());

        assert_eq!(
            manager
                .get(&bilateral_id)
                .expect("should exist")
                .position_count(),
            2
        );
    }

    #[test]
    fn test_sensitivities_aggregation() {
        let id = NettingSetId::bilateral("BANK_A", "CSA_001");
        let mut ns = NettingSet::new(id);

        // Create two sets of sensitivities
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
