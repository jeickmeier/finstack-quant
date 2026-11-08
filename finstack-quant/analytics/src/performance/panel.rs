//! The dated panel a [`super::Performance`] is built from.

use finstack_quant_core::dates::{Date, PeriodKind};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Whether a panel's series are prices or simple returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum PanelSeriesKind {
    /// Positive price or wealth levels with an allowed final zero at wipeout;
    /// one per actual valuation date, with interval returns derived pairwise.
    Prices,
    /// Simple decimal returns on adjacent actual valuation intervals;
    /// each column has exactly one fewer observation than the valuation grid.
    Returns,
}

/// A dated panel of price or return series, one series per ticker.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_analytics::{PanelSeriesKind, Performance, PerformancePanel};
/// use finstack_quant_core::dates::Date;
/// use time::Month;
///
/// let dates: Vec<Date> = (1..=4)
///     .map(|d| Date::from_calendar_date(2025, Month::January, d))
///     .collect::<Result<_, _>>()?;
/// let perf = Performance::new(&PerformancePanel {
///     dates,
///     series: vec![vec![100.0, 101.0, 99.0, 102.0]],
///     kind: PanelSeriesKind::Prices,
///     ticker_names: vec!["SPY".into()],
///     benchmark_ticker: None,
///     frequency: None,
/// })?;
/// assert_eq!(perf.get_ticker_names(), &["SPY"]);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields, try_from = "PanelWire")]
pub struct PerformancePanel {
    /// Strictly increasing actual valuation dates; at least two. Prices have
    /// one value per date, while returns cover each adjacent interval.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Vec<finstack_quant_core::wire::DateWire>")
    )]
    pub dates: Vec<Date>,
    /// Ticker-major observations. For Prices, `series[i][t]` is the level at
    /// `dates[t]`; for Returns it is the decimal return on
    /// `(dates[t], dates[t + 1]]`. Leading/trailing missing values are
    /// `NaN` natively and `null` on the wire; each observed span is contiguous.
    #[serde(
        serialize_with = "serialize_series",
        deserialize_with = "deserialize_series"
    )]
    #[cfg_attr(feature = "json-schema", schemars(with = "Vec<Vec<Option<f64>>>"))]
    pub series: Vec<Vec<f64>>,
    /// Whether `series` holds prices or simple returns.
    pub kind: PanelSeriesKind,
    /// Unique ticker names, one per series.
    pub ticker_names: Vec<String>,
    /// Explicit benchmark ticker; omission leaves benchmark context absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub benchmark_ticker: Option<String>,
    /// Explicit nominal observation frequency for annualization and cash
    /// conversion; omission leaves that context absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frequency: Option<PeriodKind>,
}

// Input and output share exactly the same admitted source-panel contract.
#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct PanelWire {
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Vec<finstack_quant_core::wire::DateWire>")
    )]
    dates: Vec<Date>,
    #[cfg_attr(feature = "json-schema", schemars(with = "Vec<Vec<Option<f64>>>"))]
    #[serde(
        serialize_with = "serialize_series",
        deserialize_with = "deserialize_series"
    )]
    series: Vec<Vec<f64>>,
    kind: PanelSeriesKind,
    ticker_names: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    benchmark_ticker: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    frequency: Option<PeriodKind>,
}

impl TryFrom<PanelWire> for PerformancePanel {
    type Error = finstack_quant_core::Error;

    fn try_from(wire: PanelWire) -> Result<Self, Self::Error> {
        let panel = Self {
            dates: wire.dates,
            series: wire.series,
            kind: wire.kind,
            ticker_names: wire.ticker_names,
            benchmark_ticker: wire.benchmark_ticker,
            frequency: wire.frequency,
        };
        super::Performance::new(&panel)?;
        Ok(panel)
    }
}

impl Serialize for PerformancePanel {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        super::Performance::new(self).map_err(serde::ser::Error::custom)?;
        PanelWire {
            dates: self.dates.clone(),
            series: self.series.clone(),
            kind: self.kind,
            ticker_names: self.ticker_names.clone(),
            benchmark_ticker: self.benchmark_ticker.clone(),
            frequency: self.frequency,
        }
        .serialize(serializer)
    }
}

fn serialize_series<S: Serializer>(series: &[Vec<f64>], serializer: S) -> Result<S::Ok, S::Error> {
    if series.iter().flatten().any(|value| value.is_infinite()) {
        return Err(serde::ser::Error::custom(
            "infinite panel observations are invalid",
        ));
    }
    let nullable: Vec<Vec<Option<f64>>> = series
        .iter()
        .map(|row| row.iter().map(|&v| v.is_finite().then_some(v)).collect())
        .collect();
    nullable.serialize(serializer)
}

fn deserialize_series<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<Vec<f64>>, D::Error> {
    let nullable = Vec::<Vec<Option<f64>>>::deserialize(deserializer)?;
    Ok(nullable
        .into_iter()
        .map(|row| row.into_iter().map(|v| v.unwrap_or(f64::NAN)).collect())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gaps_travel_as_null_and_unknown_fields_are_rejected() {
        let json = r#"{"dates":["2024-12-31","2025-01-01","2025-01-02"],"series":[[null,0.01]],"kind":"returns","ticker_names":["A"]}"#;
        let panel: PerformancePanel = serde_json::from_str(json).expect("panel");
        assert!(panel.series[0][0].is_nan());
        assert_eq!(serde_json::to_string(&panel).ok().as_deref(), Some(json));
        let unknown = json.replace(r#""kind""#, r#""window":3,"kind""#);
        assert!(serde_json::from_str::<PerformancePanel>(&unknown).is_err());
    }

    fn dates() -> Vec<Date> {
        (1..=3)
            .map(|day| Date::from_calendar_date(2025, time::Month::January, day).unwrap())
            .collect()
    }

    fn panel(values: Vec<f64>) -> PerformancePanel {
        PerformancePanel {
            dates: dates(),
            series: vec![values],
            kind: PanelSeriesKind::Returns,
            ticker_names: vec!["A".into()],
            benchmark_ticker: None,
            frequency: None,
        }
    }

    #[test]
    fn actual_grid_and_optional_context_are_preserved() {
        let perf = super::super::Performance::new(&panel(vec![0.1, -0.1])).unwrap();
        assert_eq!(perf.get_dates(), &dates()[1..]);
        assert_eq!(perf.get_frequency(), None);
        assert_eq!(perf.get_benchmark_idx(), None);
        assert!(perf.get_paired_dates(0).is_err());
        let state = serde_json::to_value(&perf).unwrap();
        assert_eq!(state["price_dates"][0], "2025-01-01");
        let restored: super::super::Performance = serde_json::from_value(state).unwrap();
        assert_eq!(restored.get_returns_for_ticker(0).unwrap(), [0.1, -0.1]);
    }

    #[test]
    fn panel_wire_admission_rejects_invalid_economics_in_both_directions() {
        for values in [
            vec![0.1, f64::INFINITY],
            vec![-1.0, 0.1],
            vec![f64::NAN, f64::NAN],
        ] {
            assert!(serde_json::to_value(panel(values)).is_err());
        }
        for values in ["[0.1,-1.1]", "[-1,0.1]", "[null,null]", "[0.1,\"inf\"]"] {
            let json = format!(
                r#"{{"dates":["2025-01-01","2025-01-02","2025-01-03"],"series":[{values}],"kind":"returns","ticker_names":["A"]}}"#
            );
            assert!(serde_json::from_str::<PerformancePanel>(&json).is_err());
        }
        assert!(serde_json::to_value(panel(vec![0.1, -1.0])).is_ok());
    }

    #[test]
    fn exact_shapes_and_nonblank_names_are_required() {
        assert!(super::super::Performance::new(&panel(vec![0.1, 0.2, 0.3])).is_err());
        let mut invalid = panel(vec![0.1, 0.2]);
        invalid.ticker_names[0] = "  ".into();
        assert!(super::super::Performance::new(&invalid).is_err());
        assert!(serde_json::to_value(invalid).is_err());
    }

    #[test]
    fn real_zero_price_is_terminal_but_positive_near_wipeout_is_not_invented() {
        let mut input = panel(vec![]);
        input.kind = PanelSeriesKind::Prices;
        input.series = vec![vec![100.0, 90.0, 0.0]];
        let perf = super::super::Performance::new(&input).unwrap();
        assert_eq!(perf.get_returns_for_ticker(0).unwrap(), [-0.1, -1.0]);
        input.series = vec![vec![1.0, 1e-20, 1e-20]];
        assert!(super::super::Performance::new(&input).is_err());
    }

    #[test]
    fn range_and_benchmark_failures_leave_state_unchanged() {
        let mut perf = super::super::Performance::new(&panel(vec![0.1, -0.1])).unwrap();
        perf.reset_date_range(dates()[2], dates()[2]).unwrap();
        let before = serde_json::to_value(&perf).unwrap();
        assert!(perf.reset_date_range(dates()[2], dates()[1]).is_err());
        assert!(perf.reset_bench_ticker("missing").is_err());
        assert_eq!(serde_json::to_value(&perf).unwrap(), before);
        perf.reset_bench_ticker("A").unwrap();
        assert_eq!(perf.get_paired_dates(0).unwrap(), &dates()[2..]);
    }

    #[test]
    fn source_state_rejects_unknown_nested_fields_and_impossible_spans() {
        let perf = super::super::Performance::new(&panel(vec![0.1, -0.1])).unwrap();
        let state = serde_json::to_value(&perf).unwrap();
        let mut malformed = state.clone();
        malformed["return_spans"][0]["unexpected"] = serde_json::json!(true);
        assert!(serde_json::from_value::<super::super::Performance>(malformed).is_err());
        let mut malformed = state;
        malformed["return_spans"][0]["end"] = serde_json::json!(3);
        assert!(serde_json::from_value::<super::super::Performance>(malformed).is_err());
    }
}
