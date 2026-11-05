//! Panel builders shared by the integration modules: the five arguments the
//! former positional constructors took.

use finstack_quant_analytics::{PanelSeriesKind, PerformancePanel};
use finstack_quant_core::dates::{Date, PeriodKind};

fn panel(
    kind: PanelSeriesKind,
    dates: Vec<Date>,
    series: Vec<Vec<f64>>,
    ticker_names: Vec<String>,
    benchmark_ticker: Option<&str>,
    frequency: PeriodKind,
) -> PerformancePanel {
    PerformancePanel {
        dates,
        series,
        kind,
        ticker_names,
        benchmark_ticker: benchmark_ticker.map(str::to_owned),
        frequency: Some(frequency),
    }
}

/// A price panel.
pub fn prices_panel(
    dates: Vec<Date>,
    series: Vec<Vec<f64>>,
    ticker_names: Vec<String>,
    benchmark_ticker: Option<&str>,
    frequency: PeriodKind,
) -> PerformancePanel {
    panel(
        PanelSeriesKind::Prices,
        dates,
        series,
        ticker_names,
        benchmark_ticker,
        frequency,
    )
}

/// A simple-return panel.
pub fn returns_panel(
    dates: Vec<Date>,
    series: Vec<Vec<f64>>,
    ticker_names: Vec<String>,
    benchmark_ticker: Option<&str>,
    frequency: PeriodKind,
) -> PerformancePanel {
    panel(
        PanelSeriesKind::Returns,
        dates,
        series,
        ticker_names,
        benchmark_ticker,
        frequency,
    )
}
