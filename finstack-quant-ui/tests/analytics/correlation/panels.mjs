// Host return panels (Rust PerformancePanel wire values) for the correlation
// fixture, shared by generate.mjs and the component test; a gap is null. 60
// business days from 2024-01-02 of four sleeves' simple daily decimal returns,
// rounded to six places.
const dates = [];
for (let day = Date.UTC(2024, 0, 2); dates.length < 60; day += 86_400_000) {
  const weekday = new Date(day).getUTCDay();
  if (weekday !== 0 && weekday !== 6)
    dates.push(new Date(day).toISOString().slice(0, 10));
}
const round = (value) => Math.round(value * 1e6) / 1e6;
const series = (f) => dates.map((_, i) => round(f(i)));
/** The valid panel: a rates, a credit, an equity and a gold sleeve. */
export const panel = {
  dates,
  series: [
    series((i) => 0.0003 + 0.004 * Math.sin(i / 7)),
    series(
      (i) => 0.0002 + 0.005 * Math.sin(i / 7 + 0.6) + 0.002 * Math.cos(i / 3),
    ),
    series((i) => 0.0005 - 0.006 * Math.sin(i / 7) + 0.003 * Math.sin(i / 4)),
    series((i) => 0.0001 + 0.003 * Math.cos(i / 11)),
  ],
  kind: "returns",
  ticker_names: ["Rates carry", "Credit", "Equity", "Gold"],
  frequency: "daily",
};
const replace = (index, values) =>
  panel.series.map((row, at) => (at === index ? values : row));
/** Panels Rust rejects, each with the reason it does. */
export const failures = {
  // Gold returns the same 0.1% every day: a zero-variance series.
  zeroVariance: {
    ...panel,
    series: replace(
      3,
      dates.map(() => 0.001),
    ),
  },
  // A missing Credit return in the middle of its history.
  interiorGap: {
    ...panel,
    series: replace(
      1,
      panel.series[1].map((value, i) => (i === 30 ? null : value)),
    ),
  },
  // Rates carry stops after 30 days and Gold starts after them: no overlap.
  noOverlap: {
    ...panel,
    series: panel.series.map((row, at) =>
      at === 0
        ? row.map((value, i) => (i < 30 ? value : null))
        : at === 3
          ? row.map((value, i) => (i < 30 ? null : value))
          : row,
    ),
  },
};
