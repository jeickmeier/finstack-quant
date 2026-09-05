//! Serde wire type for [`super::Performance`]; caches are rebuilt on load.

use super::{invalid_return_series, Performance, TickerSpan};
use crate::dates::{Date, PeriodKind};

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PerformanceState {
    price_dates: Vec<Date>,
    returns: Vec<Vec<f64>>,
    return_spans: Vec<TickerSpan>,
    ticker_names: Vec<String>,
    benchmark_idx: usize,
    frequency: PeriodKind,
    start_idx: usize,
    end_idx: usize,
}

impl TryFrom<PerformanceState> for Performance {
    type Error = crate::error::Error;

    fn try_from(state: PerformanceState) -> Result<Self, Self::Error> {
        let benchmark = state
            .ticker_names
            .get(state.benchmark_idx)
            .ok_or_else(|| {
                invalid_return_series(
                    "<panel>",
                    state.benchmark_idx,
                    "benchmark index out of range",
                )
            })?
            .clone();
        let return_dates = state.price_dates.get(1..).unwrap_or(&[]).to_vec();
        if state.start_idx > state.end_idx || state.end_idx > return_dates.len() {
            return Err(invalid_return_series(
                "<panel>",
                state.start_idx,
                "active window out of range",
            )
            .into());
        }
        let mut perf = Self::assemble(
            state.price_dates,
            return_dates,
            state.returns,
            state.return_spans,
            state.ticker_names,
            Some(&benchmark),
            state.frequency,
        )?;
        perf.start_idx = state.start_idx;
        perf.end_idx = state.end_idx;
        perf.refresh_active_drawdown_cache();
        Ok(perf)
    }
}
