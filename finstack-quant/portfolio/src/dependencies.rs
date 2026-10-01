//! Portfolio-level dependency index for selective repricing.
//!
//! Provides a normalized key model ([`MarketFactorKey`]) and an inverted index
//! ([`DependencyIndex`]) that maps market factor keys to the set of portfolio
//! positions that depend on them. The index is built from each instrument's
//! [`finstack_quant_valuations::instruments::MarketDependencies`] and enables
//! efficient lookup of affected positions
//! when a subset of market data changes.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::HashMap;
use finstack_quant_core::HashSet;
use finstack_quant_valuations::instruments::MarketDependencies;
use finstack_quant_valuations::instruments::RatesCurveKind;

/// Normalized market factor key for portfolio-level dependency tracking.
///
/// Each variant captures enough information to uniquely identify one atomic
/// market data input.  The key space is intentionally broader than curves
/// alone so the index can route spot, vol, FX, and series changes without
/// a second abstraction layer.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum MarketFactorKey {
    /// A curve-like factor identified by ID and kind.
    Curve {
        /// Curve identifier matching [`CurveId`] in market data.
        id: CurveId,
        /// Curve kind (discount, forward, credit, or inflation).
        kind: RatesCurveKind,
    },
    /// Base-correlation curve used by credit-index loss pricing.
    BaseCorrelation(CurveId),
    /// Composite credit-index data resolved against the request market.
    CreditIndex(CurveId),
    /// Equity, commodity, or other spot price identifier.
    Spot(String),
    /// Volatility surface identifier.
    VolSurface(String),
    /// FX pair (base/quote currencies).
    Fx {
        /// Base currency.
        base: Currency,
        /// Quote currency.
        quote: Currency,
    },
    /// Scalar time series identifier (e.g., OHLC history for realized variance).
    Series(String),
}

impl MarketFactorKey {
    /// Create a curve key from a `CurveId` and [`RatesCurveKind`].
    ///
    /// # Arguments
    ///
    /// * `id` - Exact curve identifier used by the instrument and market snapshot.
    /// * `kind` - Curve role distinguishing discount, forward, credit, and inflation inputs.
    ///
    /// # Returns
    ///
    /// Curve market-factor key.
    pub fn curve(id: CurveId, kind: RatesCurveKind) -> Self {
        Self::Curve { id, kind }
    }

    /// Create a spot key.
    ///
    /// # Arguments
    ///
    /// * `id` - Exact scalar identifier for the equity, commodity, or other spot input.
    ///
    /// # Returns
    ///
    /// Spot market-factor key.
    pub fn spot(id: impl Into<String>) -> Self {
        Self::Spot(id.into())
    }

    /// Create a vol-surface key.
    ///
    /// # Arguments
    ///
    /// * `id` - Exact volatility-surface identifier in market data.
    ///
    /// # Returns
    ///
    /// Volatility-surface market-factor key.
    pub fn vol_surface(id: impl Into<String>) -> Self {
        Self::VolSurface(id.into())
    }

    /// Create an FX-pair key.
    ///
    /// # Arguments
    ///
    /// * `base` - Currency whose unit is quoted by the FX rate.
    /// * `quote` - Currency in which one unit of `base` is expressed.
    ///
    /// # Returns
    ///
    /// FX market-factor key.
    pub fn fx(base: Currency, quote: Currency) -> Self {
        Self::Fx { base, quote }
    }

    /// Create a time-series key.
    ///
    /// # Arguments
    ///
    /// * `id` - Exact scalar time-series identifier in market data.
    ///
    /// # Returns
    ///
    /// Scalar-series market-factor key.
    pub fn series(id: impl Into<String>) -> Self {
        Self::Series(id.into())
    }
}

/// Flatten a [`MarketDependencies`] into a deduplicated set of [`MarketFactorKey`]s.
///
/// # Arguments
///
/// * `deps` - Instrument dependency description to normalize.
///
/// # Returns
///
/// Deduplicated normalized key set.
pub fn flatten_dependencies(deps: &MarketDependencies) -> HashSet<MarketFactorKey> {
    let mut keys = HashSet::default();

    for (curve_id, kind) in deps.curves.all_with_kind() {
        keys.insert(MarketFactorKey::curve(curve_id, kind));
    }
    for index_id in &deps.credit_index_ids {
        keys.insert(MarketFactorKey::CreditIndex(index_id.clone()));
    }
    for spot_id in &deps.market_scalar_ids {
        keys.insert(MarketFactorKey::Spot(spot_id.clone()));
    }
    for vol_id in deps.unique_vol_surface_ids() {
        keys.insert(MarketFactorKey::VolSurface(vol_id.as_str().to_string()));
    }
    for pair in &deps.fx_pairs {
        keys.insert(MarketFactorKey::Fx {
            base: pair.base,
            quote: pair.quote,
        });
    }
    for series_id in &deps.series_ids {
        keys.insert(MarketFactorKey::Series(series_id.clone()));
    }

    keys
}

/// Resolve a credit-index aggregate to every curve its pricing data references.
///
/// Index and issuer hazards are included together because instrument dependency
/// declarations do not encode which loss model reads each curve. Issuer curves
/// are sorted and deduplicated so assignment reports remain deterministic.
///
/// # Arguments
///
/// * `market` - Request market snapshot containing the referenced credit-index data.
/// * `index_id` - Exact credit-index identifier used to look up its hazard and correlation curves.
///
/// # Errors
///
/// Propagates a missing credit-index lookup error.
pub(crate) fn credit_index_keys(
    market: &MarketContext,
    index_id: &CurveId,
) -> finstack_quant_core::Result<Vec<MarketFactorKey>> {
    let index = market.get_credit_index(index_id.as_str())?;
    let mut hazards = std::collections::BTreeSet::from([index.index_credit_curve.id().clone()]);
    if let Some(issuers) = &index.issuer_credit_curves {
        hazards.extend(issuers.values().map(|curve| curve.id().clone()));
    }
    let mut keys: Vec<_> = hazards
        .into_iter()
        .map(|id| MarketFactorKey::curve(id, RatesCurveKind::Credit))
        .collect();
    keys.push(MarketFactorKey::BaseCorrelation(
        index.base_correlation_curve.id().clone(),
    ));
    Ok(keys)
}

/// Normalize instrument dependencies and resolve composite credit-index inputs.
///
/// # Arguments
///
/// * `deps` - Instrument dependency description; credit-index inputs expand to all
///   aggregate and issuer hazards plus the base-correlation curve.
/// * `market` - Request market snapshot supplying the current credit-index constituents.
///
/// # Errors
///
/// Propagates a missing credit-index lookup error.
pub(crate) fn resolved_dependencies(
    deps: &MarketDependencies,
    market: &MarketContext,
) -> finstack_quant_core::Result<HashSet<MarketFactorKey>> {
    let mut keys = flatten_dependencies(deps);
    for index_id in &deps.credit_index_ids {
        keys.remove(&MarketFactorKey::CreditIndex(index_id.clone()));
        keys.extend(credit_index_keys(market, index_id)?);
    }
    Ok(keys)
}

fn finalize_dependency_map(
    staged: HashMap<MarketFactorKey, HashSet<usize>>,
) -> HashMap<MarketFactorKey, Vec<usize>> {
    staged
        .into_iter()
        .map(|(key, indices)| {
            let mut indices: Vec<_> = indices.into_iter().collect();
            indices.sort_unstable();
            (key, indices)
        })
        .collect()
}

/// Inverted index mapping market factor keys to affected position indices.
///
/// Stored alongside the `position_index` on [`Portfolio`](crate::portfolio::Portfolio)
/// as a derived, non-serialized cache.  The index maps each [`MarketFactorKey`]
/// to the position indices whose instruments depend on that key.
///
/// Positions whose `market_dependencies()` returned an error are tracked
/// separately in [`unresolved`](Self::unresolved) and are conservatively
/// included in every `affected_positions` query. An empty dependency set is
/// no longer treated as unresolved: `market_dependencies` is a required trait
/// method, so empty means the instrument genuinely reads no market data.
#[derive(Debug, Clone, Default)]
pub struct DependencyIndex {
    inner: HashMap<MarketFactorKey, Vec<usize>>,
    /// Position indices whose instruments failed to report dependencies.
    /// These are always included in any affected-position query as a
    /// conservative fallback.
    unresolved: Vec<usize>,
    /// Number of positions this index was built/extended for. Lets callers
    /// detect a stale index (positions mutated without a matching index
    /// update) before trusting a selective-repricing query.
    indexed_positions: usize,
}

impl DependencyIndex {
    /// Build the dependency index from a slice of positions.
    ///
    /// Iterates all positions, calls `instrument.market_dependencies()`,
    /// flattens each into normalized keys, and records the position index.
    /// Instruments that return an error from `market_dependencies()` are
    /// tracked as unresolved and conservatively included in every query.
    ///
    /// # Arguments
    ///
    /// * `positions` - Ordered portfolio positions whose instrument dependencies
    ///   determine the resulting zero-based position indices.
    ///
    /// # Returns
    ///
    /// Newly built dependency index.
    pub fn build(positions: &[crate::position::Position]) -> Self {
        let mut staged: HashMap<MarketFactorKey, HashSet<usize>> = HashMap::default();
        let mut unresolved = Vec::new();

        for (idx, position) in positions.iter().enumerate() {
            let Ok(deps) = position.instrument.market_dependencies() else {
                unresolved.push(idx);
                continue;
            };

            let keys = flatten_dependencies(&deps);
            for key in keys {
                staged.entry(key).or_default().insert(idx);
            }
        }

        let inner = finalize_dependency_map(staged);
        Self {
            inner,
            unresolved,
            indexed_positions: positions.len(),
        }
    }

    /// Incrementally index a single appended position (avoids full rebuild).
    ///
    /// `idx` must equal the current [`indexed_position_count`](Self::indexed_position_count)
    /// — the index only supports appends, never in-place replacement, because
    /// replacing a position would leave its old dependency keys behind.
    ///
    /// # Arguments
    ///
    /// * `idx` - Positional index in the portfolio's position vector.
    /// * `position` - The position to index.
    pub fn add_position(&mut self, idx: usize, position: &crate::position::Position) {
        debug_assert_eq!(
            idx, self.indexed_positions,
            "DependencyIndex only supports appends; idx must equal the indexed \
             position count (got {idx}, expected {})",
            self.indexed_positions
        );
        self.indexed_positions = self.indexed_positions.max(idx + 1);

        let Ok(deps) = position.instrument.market_dependencies() else {
            self.unresolved.push(idx);
            return;
        };

        let keys = flatten_dependencies(&deps);

        // `idx` is the new, strictly-increasing appended index (enforced by the
        // append-only contract asserted above) and `flatten_dependencies` yields
        // each key at most once per call, so `idx` can never already be present
        // in an entry. Push unconditionally: the previous `entry.contains(&idx)`
        // guard was a dead O(entry_len) scan that made repeated `add_position`
        // calls O(n²) for a widely-shared factor. Each entry therefore stays
        // sorted and duplicate-free, matching `finalize_dependency_map`.
        for key in keys {
            self.inner.entry(key).or_default().push(idx);
        }
    }

    /// Number of positions this index currently covers.
    ///
    /// Selective-repricing callers should check this equals
    /// `portfolio.positions().len()` before trusting an affected-position
    /// query; a mismatch means the index is stale.
    ///
    /// # Returns
    ///
    /// Count of positions reflected in the index.
    pub fn indexed_position_count(&self) -> usize {
        self.indexed_positions
    }

    /// Look up position indices affected by a single market factor key.
    ///
    /// # Arguments
    ///
    /// * `key` - Exact normalized market input to look up. Composite credit-index
    ///   keys remain indexed by identity; use `affected_positions` to resolve
    ///   their underlying curves against a request market.
    ///
    /// # Returns
    ///
    /// Slice of matching position indices, or an empty slice when the key is absent.
    pub fn positions_for_key(&self, key: &MarketFactorKey) -> &[usize] {
        self.inner.get(key).map_or(&[], |v| v.as_slice())
    }

    /// Collect the deduplicated, sorted union of position indices affected by
    /// any of the supplied keys, plus all unresolved positions.
    ///
    /// Composite credit-index dependencies are resolved against `market`, so
    /// changing an index hazard, issuer hazard, or base correlation reprices
    /// every position that consumes the index.
    ///
    /// # Arguments
    ///
    /// * `keys` - Changed market inputs, including composite index replacements.
    /// * `market` - Request market snapshot supplying the current credit-index
    ///   curve references; missing referenced indices return an error.
    ///
    /// # Returns
    ///
    /// Sorted affected-position indices.
    ///
    /// # Errors
    ///
    /// Returns the market-data lookup error if a tracked credit-index aggregate
    /// is absent from the request market.
    pub fn affected_positions(
        &self,
        keys: &[MarketFactorKey],
        market: &MarketContext,
    ) -> finstack_quant_core::Result<Vec<usize>> {
        let mut seen = HashSet::default();
        let mut result = Vec::new();

        for &idx in &self.unresolved {
            if seen.insert(idx) {
                result.push(idx);
            }
        }

        let has_fx_change = keys
            .iter()
            .any(|key| matches!(key, MarketFactorKey::Fx { .. }));
        for key in keys {
            for &idx in self.positions_for_key(key) {
                if seen.insert(idx) {
                    result.push(idx);
                }
            }
        }
        for (indexed_key, indices) in &self.inner {
            let MarketFactorKey::CreditIndex(index_id) = indexed_key else {
                continue;
            };
            if !keys.contains(indexed_key)
                && !credit_index_keys(market, index_id)?
                    .iter()
                    .any(|key| keys.contains(key))
            {
                continue;
            }
            for &idx in indices {
                if seen.insert(idx) {
                    result.push(idx);
                }
            }
        }
        if has_fx_change {
            // An observed FX quote can feed any triangulated cross through the
            // matrix pivot. Bumped matrices also discard derived observations
            // before rebuilding. Without a stable, authoritative path
            // manifest, every FX-dependent instrument is therefore potentially
            // affected by any FX quote change. Scan the index once even when a
            // stress manifest contains multiple FX pairs.
            for (indexed_key, indices) in &self.inner {
                if matches!(indexed_key, MarketFactorKey::Fx { .. }) {
                    for &idx in indices {
                        if seen.insert(idx) {
                            result.push(idx);
                        }
                    }
                }
            }
        }

        result.sort_unstable();
        Ok(result)
    }

    /// Position indices whose instruments failed to report dependencies.
    ///
    /// # Returns
    ///
    /// Slice of unresolved position indices.
    pub fn unresolved(&self) -> &[usize] {
        &self.unresolved
    }

    /// Total number of distinct market factor keys tracked.
    ///
    /// # Returns
    ///
    /// Number of normalized factor keys stored in the index.
    pub fn factor_count(&self) -> usize {
        self.inner.len()
    }

    /// Check whether the index is empty (no resolved keys and no unresolved positions).
    ///
    /// # Returns
    ///
    /// `true` when the index contains no keys and no unresolved positions.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty() && self.unresolved.is_empty()
    }

    /// Iterate over all tracked market factor keys and their position indices.
    ///
    /// # Returns
    ///
    /// Iterator over normalized factor keys and matching position-index slices.
    pub fn iter(&self) -> impl Iterator<Item = (&MarketFactorKey, &[usize])> {
        self.inner.iter().map(|(k, v)| (k, v.as_slice()))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn credit_index_fixture() -> (
        finstack_quant_valuations::instruments::CdsTranche,
        MarketContext,
        finstack_quant_core::dates::Date,
    ) {
        use finstack_quant_core::market_data::term_structures::{
            BaseCorrelationCurve, CreditIndexData, DiscountCurve, HazardCurve,
        };
        use std::sync::Arc;
        let as_of = time::macros::date!(2025 - 01 - 01);
        let discount = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (1.0, 0.95), (5.0, 0.8), (10.0, 0.6)])
            .build()
            .expect("discount");
        let hazard = HazardCurve::builder("INDEX-HZ")
            .base_date(as_of)
            .recovery_rate(0.4)
            .knots([(1.0, 0.01), (3.0, 0.015), (5.0, 0.02), (10.0, 0.025)])
            .build()
            .expect("hazard");
        let correlation = BaseCorrelationCurve::builder("INDEX-BC")
            .knots([
                (3.0, 0.25),
                (7.0, 0.30),
                (10.0, 0.34),
                (15.0, 0.40),
                (30.0, 0.50),
            ])
            .build()
            .expect("base correlation");
        let index = CreditIndexData::builder()
            .num_constituents(125)
            .recovery_rate(0.4)
            .index_credit_curve(Arc::new(hazard.clone()))
            .base_correlation_curve(Arc::new(correlation.clone()))
            .build()
            .expect("index");
        let market = MarketContext::new()
            .insert(discount)
            .insert(hazard)
            .insert(correlation)
            .insert_credit_index("INDEX", index)
            .expect("credit index dependencies");
        let mut tranche =
            finstack_quant_valuations::instruments::CdsTranche::example().expect("tranche");
        tranche.credit_index_id = "INDEX".into();
        (tranche, market, as_of)
    }

    #[test]
    fn credit_index_routing_resolves_hazard_issuer_and_correlation_on_request_market() {
        use crate::position::{Position, PositionUnit};
        use finstack_quant_core::market_data::term_structures::HazardCurve;
        use std::sync::Arc;
        let (tranche, market, as_of) = credit_index_fixture();
        let position = Position::new(
            "tranche",
            crate::types::DUMMY_ENTITY_ID,
            "tranche",
            Arc::new(tranche),
            1.0,
            PositionUnit::Units,
        )
        .expect("position");
        let dependency_index = DependencyIndex::build(&[position]);
        let issuer = HazardCurve::builder("ISSUER-HZ")
            .base_date(as_of)
            .recovery_rate(0.4)
            .knots([(1.0, 0.01), (5.0, 0.02)])
            .build()
            .expect("issuer");
        let mut index = market
            .get_credit_index("INDEX")
            .expect("index")
            .as_ref()
            .clone();
        index.num_constituents = 1;
        index.issuer_credit_curves = Some(std::collections::BTreeMap::from_iter([(
            "issuer".to_string(),
            Arc::new(issuer.clone()),
        )]));
        let market = market
            .insert(issuer)
            .insert_credit_index("INDEX", index)
            .expect("credit index dependencies");
        for key in [
            MarketFactorKey::curve("INDEX-HZ".into(), RatesCurveKind::Credit),
            MarketFactorKey::curve("ISSUER-HZ".into(), RatesCurveKind::Credit),
            MarketFactorKey::BaseCorrelation("INDEX-BC".into()),
            MarketFactorKey::CreditIndex("INDEX".into()),
        ] {
            assert_eq!(
                dependency_index
                    .affected_positions(&[key], &market)
                    .expect("routing"),
                vec![0]
            );
        }
        assert!(dependency_index
            .affected_positions(
                &[MarketFactorKey::curve(
                    "INDEX-HZ".into(),
                    RatesCurveKind::Credit
                )],
                &MarketContext::new(),
            )
            .is_err());
    }

    #[test]
    fn flatten_empty_deps() {
        let deps = MarketDependencies::new();
        let keys = flatten_dependencies(&deps);
        assert!(keys.is_empty());
    }

    #[test]
    fn flatten_deduplicates() {
        let mut deps = MarketDependencies::new();
        deps.add_discount_curve("USD");
        deps.add_forward_curve("USD");
        deps.add_discount_curve("USD");
        deps.add_forward_curve("USD");
        deps.add_market_scalar_id("SPX");
        deps.add_market_scalar_id("SPX");

        let keys = flatten_dependencies(&deps);
        let curve_count = keys
            .iter()
            .filter(|k| matches!(k, MarketFactorKey::Curve { .. }))
            .count();
        let spot_count = keys
            .iter()
            .filter(|k| matches!(k, MarketFactorKey::Spot(_)))
            .count();
        assert_eq!(curve_count, 2, "discount + forward for USD");
        assert_eq!(spot_count, 1, "SPX deduplicated");
        assert_eq!(keys.len(), 3, "2 curves + 1 spot");
    }

    #[test]
    fn flatten_preserves_inflation_curve_kind() {
        let mut deps = MarketDependencies::new();
        deps.add_inflation_curve("US-CPI");

        let keys = flatten_dependencies(&deps);
        assert!(keys.contains(&MarketFactorKey::curve(
            "US-CPI".into(),
            RatesCurveKind::Inflation,
        )));
        assert!(!keys.contains(&MarketFactorKey::curve(
            "US-CPI".into(),
            RatesCurveKind::Forward,
        )));
    }

    #[test]
    fn dependency_index_empty_portfolio() {
        let index = DependencyIndex::build(&[]);
        assert!(index.is_empty());
        assert_eq!(index.factor_count(), 0);
        assert!(index.unresolved().is_empty());
    }

    #[test]
    fn finalize_dependency_map_sorts_and_deduplicates_indices() {
        let key = MarketFactorKey::spot("SPX");
        let mut staged: HashMap<MarketFactorKey, HashSet<usize>> = HashMap::default();
        staged
            .entry(key.clone())
            .or_default()
            .extend([3usize, 1, 3, 2]);

        let finalized = finalize_dependency_map(staged);

        assert_eq!(finalized.get(&key).map(Vec::as_slice), Some(&[1, 2, 3][..]));
    }

    #[test]
    fn affected_positions_conservatively_includes_all_fx_dependencies() {
        let direct = MarketFactorKey::fx(Currency::EUR, Currency::USD);
        let triangulated_cross = MarketFactorKey::fx(Currency::EUR, Currency::JPY);
        let mut staged: HashMap<MarketFactorKey, HashSet<usize>> = HashMap::default();
        staged.entry(direct).or_default().insert(7);
        staged.entry(triangulated_cross).or_default().insert(11);
        let index = DependencyIndex {
            inner: finalize_dependency_map(staged),
            unresolved: Vec::new(),
            indexed_positions: 12,
        };

        assert_eq!(
            index
                .affected_positions(
                    &[MarketFactorKey::fx(Currency::USD, Currency::EUR)],
                    &MarketContext::new()
                )
                .expect("dependency routing"),
            vec![7, 11],
        );
    }
}
