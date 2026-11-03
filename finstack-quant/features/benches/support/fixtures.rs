//! Deterministic panel fixtures shared by features Criterion targets.
//!
//! No RNG crate and no clock. A seeded generator is required: these inputs
//! feed the same code paths the correctness tests pin.
//!
//! Compiled independently into each bench binary via `#[path]`, so items used
//! by only one target look unused to the other.
#![allow(dead_code)]

use serde_json::{Value, json};

/// Default research-year panel used by the absolute-cost benches: 100 names
/// times 252 daily observations (25,200 rows).
pub const HOT_ENTITIES: usize = 100;
/// Daily observations in the default research-year panel.
pub const HOT_OBS: usize = 252;
/// Default trailing window (one quarter of daily bars).
pub const HOT_WINDOW: usize = 63;
/// Default OLS factor count for neutralize / residual benches.
pub const HOT_FACTORS: usize = 3;
/// Industry-style groups for the grouped cross-section bench.
pub const HOT_GROUPS: usize = 10;

/// Splitmix64-style iteration mapped into `(-0.02, 0.02)`.
pub fn synthetic_returns(n: usize, seed: u64) -> Vec<f64> {
    let mut state = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    (0..n)
        .map(|_| {
            state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            let u = ((z ^ (z >> 31)) as f64) / (u64::MAX as f64);
            (u - 0.5) * 0.04
        })
        .collect()
}

/// ISO-8601 calendar days starting at 2020-01-01, one string per observation.
pub fn iso_dates(n: usize) -> Vec<String> {
    let mut year = 2020_i32;
    let mut month = 1_u32;
    let mut day = 1_u32;
    let mut dates = Vec::with_capacity(n);
    for _ in 0..n {
        dates.push(format!("{year:04}-{month:02}-{day:02}"));
        day += 1;
        let dim = days_in_month(year, month);
        if day > dim {
            day = 1;
            month += 1;
            if month > 12 {
                month = 1;
                year += 1;
            }
        }
    }
    dates
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap(year) => 29,
        _ => 28,
    }
}

fn is_leap(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

/// Date-major research panel consumed by every public features entry point.
pub struct FeaturePanel {
    /// Row-aligned return-like observations; ~5% missing.
    pub values: Vec<Option<f64>>,
    /// Compounded positive levels for `drawdown`.
    pub levels: Vec<Option<f64>>,
    /// Second return column for pairwise rolling stats.
    pub other: Vec<Option<f64>>,
    /// Strictly positive vols for `risk_scaled_weights`.
    pub volatility: Vec<Option<f64>>,
    /// Entity key, date-major (interleaved across names).
    pub entity: Vec<String>,
    /// Lexicographic order key (ISO-8601 date), aligned with `values`.
    pub order: Vec<String>,
    /// Cross-section partition key (same strings as `order`).
    pub time_key: Vec<String>,
    /// `(time, industry)` group labels for grouped cross-sections.
    pub groups: Vec<String>,
    /// Full-rank per-date exposure columns for OLS helpers.
    pub exposures: Vec<Vec<Option<f64>>>,
    /// Total row count (`n_entities * n_obs`).
    pub n_rows: usize,
}

/// Build a date-major panel: all names on day 0, then all names on day 1, …
///
/// # Arguments
///
/// * `n_entities` - Distinct names in the cross-section.
/// * `n_obs` - Daily observations per name.
/// * `n_factors` - Exposure columns; must be less than `n_entities` so each
///   date's OLS design is overdetermined when an intercept is fit.
/// * `seed` - Deterministic splitmix64 seed for the return draws.
#[must_use]
pub fn feature_panel(n_entities: usize, n_obs: usize, n_factors: usize, seed: u64) -> FeaturePanel {
    assert!(n_entities > 0, "panel needs at least one entity");
    assert!(n_obs > 0, "panel needs at least one observation");
    assert!(
        n_entities > n_factors + 1,
        "need more names than OLS columns"
    );

    let n_rows = n_entities * n_obs;
    let dates = iso_dates(n_obs);
    let entity_ids = (0..n_entities)
        .map(|entity| format!("E{entity:04}"))
        .collect::<Vec<_>>();
    let group_ids = (0..HOT_GROUPS)
        .map(|group| format!("G{group:02}"))
        .collect::<Vec<_>>();

    let returns = synthetic_returns(n_rows, seed);
    let other_raw = synthetic_returns(n_rows, seed.wrapping_add(1));

    let mut values = Vec::with_capacity(n_rows);
    let mut levels = Vec::with_capacity(n_rows);
    let mut other = Vec::with_capacity(n_rows);
    let mut volatility = Vec::with_capacity(n_rows);
    let mut entity = Vec::with_capacity(n_rows);
    let mut order = Vec::with_capacity(n_rows);
    let mut time_key = Vec::with_capacity(n_rows);
    let mut groups = Vec::with_capacity(n_rows);
    let mut exposures = vec![Vec::with_capacity(n_rows); n_factors];
    let mut running_level = vec![100.0; n_entities];

    for (obs, date) in dates.iter().enumerate() {
        for name in 0..n_entities {
            let idx = obs * n_entities + name;
            let missing = idx % 20 == 19;
            let ret = returns[idx];
            let value = if missing { None } else { Some(ret) };
            if let Some(r) = value {
                running_level[name] *= 1.0 + r;
            }
            values.push(value);
            levels.push(if missing {
                None
            } else {
                Some(running_level[name])
            });
            other.push(if missing { None } else { Some(other_raw[idx]) });
            volatility.push(if missing {
                None
            } else {
                Some(0.01 + other_raw[idx].abs())
            });
            entity.push(entity_ids[name].clone());
            order.push(date.clone());
            time_key.push(date.clone());
            groups.push(group_ids[name % HOT_GROUPS].clone());
            for (factor, column) in exposures.iter_mut().enumerate() {
                let loading = entity_loading(name, n_entities, factor, ret);
                column.push(if missing { None } else { Some(loading) });
            }
        }
    }

    FeaturePanel {
        values,
        levels,
        other,
        volatility,
        entity,
        order,
        time_key,
        groups,
        exposures,
        n_rows,
    }
}

fn entity_loading(name: usize, n_entities: usize, factor: usize, ret: f64) -> f64 {
    let size = name as f64 / n_entities as f64;
    match factor {
        0 => size,
        1 => ((name * 7) % n_entities) as f64 / n_entities as f64,
        _ => 0.25 * ret + 0.1 * ((name * (factor + 3)) % 11) as f64,
    }
}

/// Default 100 × 252 panel with three exposure columns.
#[must_use]
pub fn hot_panel() -> FeaturePanel {
    feature_panel(HOT_ENTITIES, HOT_OBS, HOT_FACTORS, 42)
}

/// JSON `{ "window": w, "min_periods": w }`.
#[must_use]
pub fn window_params(window: usize) -> Value {
    json!({ "window": window, "min_periods": window })
}

/// JSON `{ "span": span }` for pandas-span EWMA ops.
#[must_use]
pub fn span_params(span: f64) -> Value {
    json!({ "span": span })
}
