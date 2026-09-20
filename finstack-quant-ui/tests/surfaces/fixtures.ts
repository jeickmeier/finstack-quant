import type { StoredSurface } from "../../registry/components/vol-surface-chart/stored";
export const storedSurface: StoredSurface = {
  id: "Supplied surface",
  expiries: [0.5, 1, 2],
  strikes: [80, 100, 120],
  secondary_axis: "strike",
  quote_type: "black_lognormal",
  interpolation_mode: "total_variance",
  vols_row_major: [0.21, 0.2, 0.23, 0.22, 0.24, 0.26, 0.25, 0.27, 0.28],
};
export const tenorSurface: StoredSurface = {
  ...storedSurface,
  id: "Normal tenor",
  strikes: [1, 5, 10],
  secondary_axis: "tenor",
  quote_type: "normal",
  vols_row_major: [
    0.005, 0.006, 0.007, 0.008, 0.009, 0.01, 0.011, 0.012, 0.013,
  ],
};
export const fxQuotes = {
  id: "EURUSD",
  expiries: [0.5, 1],
  atm_vols: [0.08, 0.09],
  rr_25d: [0.01, 0.012],
  bf_25d: [0.005, 0.006],
  rr_10d: null,
  bf_10d: null,
};
