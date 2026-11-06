//! Per-row tables of a lattice quantity over a contiguous band of nodes.

use std::ops::Range;

/// Values of a per-node lattice quantity, held for one contiguous band of
/// nodes per row.
///
/// Sampled paths spend almost all their time near the centre of each row (see
/// [`RatesCreditTree::usual_rate_nodes`](super::RatesCreditTree::usual_rate_nodes)),
/// so tabulating an expensive quantity over that band turns its computation
/// into a lookup. A node outside its row's band has no entry; the caller
/// computes it on demand from the same expression, so the table changes speed
/// and never a value.
#[derive(Debug, Clone)]
pub struct BandTable<T> {
    /// First entry of each row in `values`.
    starts: Vec<usize>,
    /// Lowest node index held for each row.
    lows: Vec<usize>,
    /// Row entries, concatenated.
    values: Vec<T>,
}

impl<T> Default for BandTable<T> {
    fn default() -> Self {
        Self {
            starts: Vec::new(),
            lows: Vec::new(),
            values: Vec::new(),
        }
    }
}

impl<T: Copy> BandTable<T> {
    /// Tabulate `value_at(step, node)` for every node of each row's band.
    ///
    /// # Arguments
    ///
    /// * `bands` - Node range held for each row, in row order starting at
    ///   step zero; an empty range holds nothing for that row.
    /// * `value_at` - The quantity at a `(step, node)` pair, called once per
    ///   held node in row order.
    ///
    /// # Errors
    ///
    /// Returns the first error from `value_at`.
    pub fn build<E>(
        bands: impl IntoIterator<Item = Range<usize>>,
        mut value_at: impl FnMut(usize, usize) -> Result<T, E>,
    ) -> Result<Self, E> {
        let mut table = Self::default();
        for (step, band) in bands.into_iter().enumerate() {
            table.starts.push(table.values.len());
            table.lows.push(band.start);
            for node in band {
                table.values.push(value_at(step, node)?);
            }
        }
        Ok(table)
    }

    /// The tabulated value of `node` on row `step`.
    ///
    /// # Arguments
    ///
    /// * `step` - Zero-based row index.
    /// * `node` - Node index within the row.
    ///
    /// # Returns
    ///
    /// The value, or `None` when the row is not tabulated or `node` lies
    /// outside its band.
    #[inline]
    pub fn get(&self, step: usize, node: usize) -> Option<T> {
        let offset = node.checked_sub(*self.lows.get(step)?)?;
        let start = *self.starts.get(step)?;
        let end = self
            .starts
            .get(step + 1)
            .copied()
            .unwrap_or(self.values.len());
        let index = start + offset;
        (index < end)
            .then(|| self.values.get(index).copied())
            .flatten()
    }
}
