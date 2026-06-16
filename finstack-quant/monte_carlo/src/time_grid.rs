//! Time grid types from [`finstack_quant_core::math::time_grid`].
//!
//! This module re-exports the core implementation so Monte Carlo callers can depend on
//! `finstack-monte_carlo` alone. For non-MC code, import [`TimeGrid`] directly
//! from `finstack-quant-core`.

pub use finstack_quant_core::math::time_grid::{
    map_date_to_step, map_dates_to_steps, map_exercise_dates_to_steps, TimeGrid, TimeGridError,
};
