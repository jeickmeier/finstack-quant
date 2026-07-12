# C02 — Retire the Derivative SABR Calibration Path

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

## Metadata

- **Source audit:** `docs/audits/2026-07-12-core-cashflows-valuations-simplicity-audit.md`
- **Findings / hazards:** F2
- **Tier:** 3
- **Estimated net LOC:** approximately -650, deletion-dominated; fewer than 100 non-delete edits
- **Dependencies:** C01
- **Branch:** `codex/simplify-c02-retire-derivative-sabr`
- **Parallel / merge safety:** may run in parallel with C03, C06, and C08 after C01. Conflicts only with other SABR calibration or SABR binding work.

## Exact files

- `finstack-quant/valuations/src/models/volatility/sabr/calibration.rs`
- `finstack-quant/valuations/src/models/volatility/sabr_derivatives.rs` (delete)
- `finstack-quant/valuations/src/models/volatility/mod.rs`
- `finstack-quant-py/src/bindings/valuations/sabr.rs`
- `finstack-quant-wasm/src/api/valuations/sabr.rs`

## Scope

- Keep `calibrate`, `calibrate_shifted`, `calibrate_auto_shift`, and `calibrate_with_atm_pinning` as the one SABR calibration family.
- Delete `SABRMarketData`, `SABRCalibrationDerivatives`, their module/re-exports, and all `*_with_derivatives` calibrator methods.
- Remove Python and WASM `check_smile_lengths`; let the Rust calibrator own validation and map its error through existing host adapters.

## Non-goals

- Do not change objective weighting, initial guesses, shift selection, optimizer tolerance, iteration caps, ATM interpolation, or ATM pinning.
- Do not rename Python/WASM `SabrCalibrator` methods or alter TypeScript/stub signatures.

## Implementation steps

1. Remove derivative-method implementations and imports from `calibration.rs`.
2. Delete `sabr_derivatives.rs` and its public module/re-exports.
3. Remove binding-owned vector-length checks from both host bindings.
4. Preserve error category mapping: Rust validation errors become Python `ValueError` and WASM `JsValue` errors through the centralized adapters.
5. Retain the C01 canonical golden cases as the replacement coverage.

## Behavior / golden tests

- C01’s unshifted, shifted, auto-shifted, and ATM-pinned SABR cases must remain bitwise equal where deterministic or within the pinned tolerance.
- Python and WASM invalid-length tests must preserve host error types and the stable `length ... must match` fragment.

## Focused verification

```bash
rtk cargo test -p finstack-quant-valuations sabr
rtk cargo test -p finstack-quant-wasm sabr
rtk uv run pytest finstack-quant-py/tests/test_valuations_new_bindings.py -k sabr
```

## Full verification

```bash
rtk mise run all-fmt
rtk mise run all-lint
rtk mise run all-test
rtk mise run python-build -- --release
```

## Binding / parity / serde impact

- Python/WASM public SABR symbols and signatures stay unchanged; only validation ownership moves to Rust.
- `finstack-quant-py/parity_contract.toml`, Python stubs, `index.d.ts`, and JS exports require no symbol changes and must remain green.
- No serde types are changed. Removing the public Rust derivative DTOs is a deliberate semver-visible deletion.

## Rollback

- Revert this PR to restore the derivative module and binding prechecks. No persisted state changes.

## Done criteria

- The standard calibrator is the sole algorithmic implementation.
- Both bindings delegate length validation to Rust.
- No production caller or test imports the derivative DTOs.

## Targeted re-audit acceptance

```bash
rtk rg -n "sabr_derivatives|SABRCalibrationDerivatives|SABRMarketData|with_derivatives|check_smile_lengths" finstack-quant/valuations/src finstack-quant-py/src finstack-quant-wasm/src
```

The command must return no production hits.
