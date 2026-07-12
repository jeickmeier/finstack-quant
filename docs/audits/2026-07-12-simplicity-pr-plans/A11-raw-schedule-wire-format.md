# Consolidation Plan A11: Standardize on Raw CashFlowSchedule JSON

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

## Metadata

- Audit: 2026-07-12 simplicity audit, Cluster A.
- Findings and hazards: F27 raw-versus-envelope wire formats; H8, H10.
- Risk tier: Tier 4 — intentional public wire/API deletion across Rust, Python, and WASM.
- Estimated net change: -260 to -140 LOC.
- Dependencies: A09 and A10.
- Suggested branch: `codex/a11-raw-schedule-wire-format`.
- Parallel and merge safety: safe beside A02, A05-A07, and A12 after dependencies. Conflicts with any cashflow JSON binding, generated TypeScript, parity contract, or complex-cashflows notebook work.
- Atomicity: **compile-atomic cross-language exception**. Envelope symbols must be removed from Rust exports, Python registration/package/stubs, WASM registration/facade/types, parity, tests, and docs in one commit. Retaining aliases would preserve the parallel API the slice is meant to remove.

## Exact Files

- `finstack-quant/cashflows/src/json.rs`
- `finstack-quant/cashflows/src/lib.rs`
- `finstack-quant-py/src/bindings/cashflows/mod.rs`
- `finstack-quant-py/finstack_quant/cashflows/__init__.py`
- `finstack-quant-py/finstack_quant/cashflows/__init__.pyi`
- `finstack-quant-py/tests/test_cashflows.py`
- `finstack-quant-py/tests/parity/test_contract_topology.py`
- `finstack-quant-py/parity_contract.toml`
- `finstack-quant-py/examples/notebooks/02_pricing/instruments/complex_cashflows.ipynb`
- `finstack-quant-py/docs/notebook-coverage/ALL_NOTEBOOKS_API_COVERAGE_AUDIT.md`
- `finstack-quant-wasm/src/api/cashflows/mod.rs`
- `finstack-quant-wasm/exports/cashflows.js`
- `finstack-quant-wasm/index.d.ts`
- `finstack-quant-wasm/types/generated/CashflowSchedule.ts`
- `finstack-quant-wasm/tests/dts_contract.rs`
- `finstack-quant-wasm/tests/facade/cashflows.test.mjs`
- `finstack-quant-wasm/tests/wasm_cashflows.rs`

## Scope

- Delete `CashflowScheduleEnvelope`, its schema-version constant, and build/validate envelope functions.
- Keep raw `CashFlowSchedule` JSON as the single build, validate, accrue, dated-flow, and bond-input format.
- Delete envelope exports from every Python/WASM registration, facade, stub/type, test, notebook, and parity list.
- Correct generated TypeScript `AccrualConfigJson.method` to the actual Rust serde values `"Linear" | "Compounded"`.
- Remove default-config rounding stamps; callers needing execution provenance must use the existing higher-level result/config envelopes, not a second cashflow wire format.

## Non-Goals

- No new replacement schedule envelope or version wrapper.
- No change to raw schedule validation semantics.
- No lowercase serde rename for `AccrualMethod`; TypeScript follows canonical Rust.
- No `bond_from_cashflows_json` namespace move; A12 owns it.

## Implementation Steps

1. Delete the envelope type, constant, constructors, validators, and Rust re-exports.
2. Remove Python registration/package/stub symbols and update tests/notebook to use raw schedule JSON only.
3. Remove WASM exports/declarations/tests and generated envelope shapes.
4. Correct the TypeScript accrual-method union and add a contract assertion against serialized Rust values.
5. Remove envelope and cross-surface symbol entries from the parity contract/topology expectations.
6. Inventory the workspace for `CashflowScheduleEnvelope`, `build_*_envelope`, and `validate_*_envelope`; only audit/history text may remain.

## Tests to Add or Update

- Rust/Python/WASM raw build -> validate -> accrue -> dated-flow pipeline.
- TypeScript contract test pins `Linear` and `Compounded` exactly.
- Negative test proves an envelope-shaped payload is rejected by the raw validator rather than silently unwrapped.
- Notebook execution uses only raw schedule JSON.

## Full Verification

```bash
rtk mise run all-fmt
rtk mise run rust-lint
rtk mise run rust-test
rtk mise run python-build -- --release
rtk mise run python-lint
rtk mise run python-test
rtk mise run wasm-build
rtk mise run wasm-lint
rtk mise run wasm-test
rtk env UV_CACHE_DIR=/private/tmp/finstack-uv-cache uv run pytest finstack-quant-py/tests/parity -x
rtk mise run gen-check
rtk mise run rust-check-schemas
rtk mise run goldens-test
```

## Binding, Parity, and Serde Impact

- Python: two envelope functions are intentionally removed from `finstack_quant.cashflows`.
- WASM: two envelope functions are intentionally removed from the cashflows facade.
- Parity: root export lists/maps shrink atomically; no compatibility exception remains.
- Serde: raw schedule format is unchanged; the deleted envelope is no longer accepted. TypeScript casing is corrected to match existing Rust serde.

## Rollback

Revert the full cross-language commit. Do not restore only host-language aliases without the Rust implementation and parity entries.

## Done Criteria

- One cashflow schedule wire format remains.
- No public envelope symbol, type, generated interface, test, notebook import, or parity entry remains.
- No cashflow serialization path stamps `FinstackConfig::default()` as if it were the active context.
- TypeScript accrual-method values exactly match Rust JSON.

## Targeted Re-Audit Acceptance

Run `finstack-simplify` across cashflow JSON, Python/WASM bindings, generated types, parity, and notebook references. Accept only when raw schedule JSON is the sole format, no envelope compatibility facade remains, casing matches canonical serde, and no default configuration is stamped into schedule output.
