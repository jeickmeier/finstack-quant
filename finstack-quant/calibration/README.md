# finstack-quant-calibration

Quote ingestion, market construction, calibration execution, explicit model
fitting, and cached quote-space recalibration for `finstack-quant`.

The dependency direction is one-way:

```text
calibration -> valuations -> core/models/cashflows
```

`finstack-quant-valuations` owns the object-safe recalibration contract and
pricing concerns. This crate implements that contract with
`CachedRecalibrationProvider`; valuations never depends on this crate.

Public modules:

- `api`: calibration envelopes, engine configuration, reports, and validation.
- `quotes`: raw market quote contracts.
- `recalibration`: cached implementations of the valuations replay port.
- `hull_white`: explicit Hull-White parameter calibration.
- `lmm`: explicit Bermudan LMM base-volatility calibration.
- `versions`: methodology identifiers recorded on `CalibrationReport::model_version`.

Quote-to-instrument construction lives in crate-private `build/`.

Host APIs live at `finstack_quant.calibration` in Python and `calibration` in
the WASM facade. There are no compatibility exports under valuations.

Calibration acceptance uses the caller's configured tolerance and parameter
bounds. Parametric NS/NSS fits use the discount-curve tolerance in PV per unit
notional; an approximate least-squares fit needs an explicitly appropriate
tolerance. Distressed CDS quotes do not widen hazard bounds or tolerances.
Strict quote replay rejects failed fits before returning curves to risk callers.

Parametric calibration fits a single discount curve identified by `curve_id`;
the unsupported separate `discount_curve_id` option has been removed. Hull–White swaption steps
accept ATM quotes only: strikes must match the contractual forward within
`1e-8` in decimal rate units (0.0001 bp).

SABR/SVI reports contain signed residuals keyed by original quote IDs, evaluated
through the published surface interpolation. Target grids must cover the dated
input quotes and meet the configured fit tolerance. Surface expiries use ACT/365F;
equity discounting and dividend PVs use each curve's day count and are normalized
to the surface base date.
