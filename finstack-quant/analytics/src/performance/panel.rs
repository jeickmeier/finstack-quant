//! The dated panel a [`super::Performance`] is built from.

use finstack_quant_core::dates::{Date, PeriodKind};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Whether a panel's series are prices or simple returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum PanelSeriesKind {
    /// Strictly positive prices, one per date; returns are derived pairwise,
    /// so the first date anchors the first return.
    Prices,
    /// Simple decimal returns, one per return-aligned date; a synthetic prior
    /// date one frequency step before the first anchors the holding period.
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PerformancePanel {
    /// Strictly ascending observation dates, one per value of every series.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Vec<finstack_quant_core::wire::DateWire>")
    )]
    pub dates: Vec<Date>,
    /// Ticker-major series: `series[i][t]` is ticker `i` at `dates[t]`. A
    /// ticker may have leading or trailing gaps (`NaN`, `null` on the wire)
    /// but its observed span must be contiguous.
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
    /// Benchmark ticker; omitted uses the first ticker.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub benchmark_ticker: Option<String>,
    /// Observation frequency used for annualization; omitted uses
    /// [`crate::DEFAULT_FREQUENCY`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frequency: Option<PeriodKind>,
}

fn serialize_series<S: Serializer>(series: &[Vec<f64>], serializer: S) -> Result<S::Ok, S::Error> {
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
        let json = r#"{"dates":["2025-01-01","2025-01-02"],"series":[[null,0.01]],"kind":"returns","ticker_names":["A"]}"#;
        let panel: PerformancePanel = serde_json::from_str(json).expect("panel");
        assert!(panel.series[0][0].is_nan());
        assert_eq!(serde_json::to_string(&panel).ok().as_deref(), Some(json));
        let unknown = json.replace(r#""kind""#, r#""window":3,"kind""#);
        assert!(serde_json::from_str::<PerformancePanel>(&unknown).is_err());
    }
}
