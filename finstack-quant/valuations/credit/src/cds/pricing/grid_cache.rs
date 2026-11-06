//! Contract-local discount windows for the last hazard knot topology.
use super::engine::{DiscountWindow, IsdaGrid};
use super::helpers::isda_standard_model_boundaries;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::term_structures::HazardCurve;
use finstack_quant_core::{FxHashMap, Result};
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub(super) struct IsdaWindowCache {
    last: Mutex<Option<CachedTopology>>,
    #[cfg(test)]
    builds: std::sync::atomic::AtomicUsize,
}

struct CachedTopology {
    base_date: Date,
    day_count: DayCount,
    knot_times: Vec<u64>,
    grids: FxHashMap<(u64, u64), Arc<PreparedIsdaGrid>>,
}

impl CachedTopology {
    fn matches(&self, curve: &HazardCurve) -> bool {
        self.base_date == curve.base_date()
            && self.day_count == curve.day_count()
            && self.knot_times.len() == curve.len()
            && self
                .knot_times
                .iter()
                .zip(curve.knot_points())
                .all(|(&saved, (t, _))| saved == t.to_bits())
    }
}

pub(super) struct PreparedIsdaGrid {
    pub(super) boundaries: Vec<f64>,
    // Preserve lazy error observation: a discount failure matters only when
    // this window carries positive default probability in the current trial.
    pub(super) discounts: Vec<Result<DiscountWindow>>,
}

impl IsdaWindowCache {
    pub(super) fn get(&self, grid: &IsdaGrid<'_>) -> Result<Arc<PreparedIsdaGrid>> {
        let mut cached = match self.last.lock() {
            Ok(lock) => lock,
            Err(poisoned) => poisoned.into_inner(),
        };
        if cached.as_ref().is_some_and(|last| !last.matches(grid.surv)) {
            *cached = None;
        }
        let last = cached.get_or_insert_with(|| CachedTopology {
            base_date: grid.surv.base_date(),
            day_count: grid.surv.day_count(),
            knot_times: grid.surv.knot_points().map(|(t, _)| t.to_bits()).collect(),
            grids: FxHashMap::default(),
        });
        let key = (grid.t_start.to_bits(), grid.t_end.to_bits());
        if let Some(prepared) = last.grids.get(&key) {
            return Ok(Arc::clone(prepared));
        }
        let boundaries =
            isda_standard_model_boundaries(grid.t_start, grid.t_end, grid.surv, grid.disc)?;
        let discounts = boundaries
            .windows(2)
            .map(|w| grid.discount_window(w[0], w[1], w[1] - w[0]))
            .collect();
        #[cfg(test)]
        self.builds
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let prepared = Arc::new(PreparedIsdaGrid {
            boundaries,
            discounts,
        });
        last.grids.insert(key, Arc::clone(&prepared));
        Ok(prepared)
    }
}

#[cfg(test)]
impl IsdaWindowCache {
    pub(super) fn build_count(&self) -> usize {
        self.builds.load(std::sync::atomic::Ordering::Relaxed)
    }
    pub(super) fn retained_windows(&self) -> usize {
        self.last
            .lock()
            .expect("test lock")
            .as_ref()
            .map_or(0, |t| t.grids.values().map(|g| g.discounts.len()).sum())
    }
}
