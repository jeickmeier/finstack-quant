//! Shared panel grouping: entity sort and time-key partitions.

use finstack_quant_core::Result;
use std::collections::BTreeMap;

/// Sort row indices by `entity`, then lexicographic `order`, then input index.
pub(crate) fn sorted_indices(entity: &[String], order: &[String]) -> Vec<usize> {
    let mut indices = (0..entity.len()).collect::<Vec<_>>();
    indices.sort_by(|left, right| {
        entity[*left]
            .cmp(&entity[*right])
            .then(order[*left].cmp(&order[*right]))
            .then(left.cmp(right))
    });
    indices
}

/// Visit each contiguous entity run in an already-sorted index.
pub(crate) fn try_for_each_entity(
    entity: &[String],
    indices: &[usize],
    mut visit: impl FnMut(&[usize]) -> Result<()>,
) -> Result<()> {
    let mut start = 0;
    while start < indices.len() {
        let mut end = start + 1;
        while end < indices.len() && entity[indices[end]] == entity[indices[start]] {
            end += 1;
        }
        visit(&indices[start..end])?;
        start = end;
    }
    Ok(())
}

/// Visit each trailing window of at most `window` rows ending at every row.
///
/// `visit` receives the row index at the window end and the window's row
/// indices (in `indices` order).
pub(crate) fn for_each_trailing_window(
    indices: &[usize],
    window: usize,
    mut visit: impl FnMut(usize, &[usize]),
) {
    for (pos, &idx) in indices.iter().enumerate() {
        let start = pos.saturating_sub(window - 1);
        visit(idx, &indices[start..=pos]);
    }
}

/// Fallible variant of [`for_each_trailing_window`].
pub(crate) fn try_for_each_trailing_window(
    indices: &[usize],
    window: usize,
    mut visit: impl FnMut(usize, &[usize]) -> Result<()>,
) -> Result<()> {
    for (pos, &idx) in indices.iter().enumerate() {
        let start = pos.saturating_sub(window - 1);
        visit(idx, &indices[start..=pos])?;
    }
    Ok(())
}

/// Partition row indices into sorted buckets keyed by `K`, preserving row order.
pub(crate) fn partition_by<K: Ord>(keys: impl Iterator<Item = K>) -> BTreeMap<K, Vec<usize>> {
    let mut partitions: BTreeMap<K, Vec<usize>> = BTreeMap::new();
    for (idx, key) in keys.enumerate() {
        partitions.entry(key).or_default().push(idx);
    }
    partitions
}
