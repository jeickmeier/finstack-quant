# Consolidation Plan A12: Move bond_from_cashflows_json to the Valuations Namespace

**Program index and mandatory merge gate:** [README.md](README.md#mandatory-green-gates)

## Metadata

- Audit: 2026-07-12 simplicity audit, Cluster A.
- Findings and hazards: F27 namespace drift.
- Risk tier: Tier 3 — intentional public namespace move across Rust, Python, and WASM.
- Estimated net change: -60 to +40 LOC.
- Dependencies: A11.
- Suggested branch: `codex/a12-bond-json-namespace`.
- Parallel and merge safety: safe beside A02 and A05-A07 after A11. Conflicts with cashflow/valuations binding registration, bond bindings, parity topology, and the complex-cashflows notebook.
- Atomicity: **compile-atomic cross-language exception**. The Rust canonical path and both host namespaces must move together. No temporary cashflows alias or parity cross-crate exception is allowed in the merged commit.

## Exact Files and Filesets

- `finstack-quant/valuations/src/lib.rs`
- `finstack-quant/valuations/src/instruments/fixed_income/bond/mod.rs`
- New `finstack-quant/valuations/src/instruments/fixed_income/bond/json.rs` if a dedicated bridge module is clearer than `mod.rs`
- `finstack-quant-py/src/bindings/cashflows/mod.rs`
- The existing Python bond binding/registration files selected by:

```bash
rtk rg -l 'Bond|bond' finstack-quant-py/src/bindings/valuations finstack-quant-py/finstack_quant/valuations
```

- `finstack-quant-py/finstack_quant/cashflows/__init__.py`
- `finstack-quant-py/finstack_quant/cashflows/__init__.pyi`
- `finstack-quant-py/tests/test_cashflows.py`
- `finstack-quant-py/tests/test_namespace.py`
- `finstack-quant-py/tests/parity/test_contract_topology.py`
- `finstack-quant-py/parity_contract.toml`
- `finstack-quant-wasm/src/api/cashflows/mod.rs`
- The existing WASM valuations/bond API and facade files selected by:

```bash
rtk rg -l 'Bond|bond' finstack-quant-wasm/src/api/valuations finstack-quant-wasm/exports finstack-quant-wasm/index.d.ts
```

- `finstack-quant-wasm/tests/wasm_cashflows.rs`
- `finstack-quant-wasm/tests/dts_contract.rs`
- `finstack-quant-py/examples/notebooks/02_pricing/instruments/complex_cashflows.ipynb`

## Scope

- Move the Rust helper from the valuations crate root into the fixed-income bond module as `bond_from_cashflows_json`.
- Register/export it under the existing valuations bond namespace in Python and WASM.
- Remove it from cashflows Rust/Python/WASM surfaces.
- Delete the parity contract's cashflows cross-crate exception and pin the function under its canonical valuations owner.
- Update tests and the complex-cashflows notebook to import/call it from valuations.

## Non-Goals

- No change to bond construction, validation, tagged instrument JSON, or arguments.
- No generic instrument factory.
- No compatibility alias in `cashflows` and no root-level Rust re-export after the commit.
- No raw/envelope work; A11 owns it.

## Implementation Steps

1. Move the helper implementation into the bond module and remove the valuations-root function/re-export.
2. Move the PyO3 wrapper and registration from cashflows to the canonical valuations bond module/package.
3. Move the wasm-bindgen wrapper and JS/TypeScript facade export to the valuations bond namespace.
4. Remove the cashflows cross-crate parity entry and add the normal Rust/Python/WASM triplet under valuations.
5. Update namespace tests, cashflow integration tests, TypeScript contract tests, and notebook imports.
6. Inventory all three languages to ensure `bond_from_cashflows_json`/`bondFromCashflowsJson` appears only under valuations/bond ownership.

## Tests to Add or Update

- Rust helper test remains in the bond module and validates raw schedule input.
- Python namespace test asserts presence under valuations and absence under cashflows.
- WASM facade/type contract asserts presence under valuations and absence under cashflows.
- Cross-language behavior test confirms output JSON is byte-equivalent to the pre-move helper.
- Notebook executes with the new import path.

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

- Python: intentional import move from `finstack_quant.cashflows` to the canonical valuations bond package.
- WASM: intentional facade move from `cashflows.bondFromCashflowsJson` to the valuations bond namespace.
- Parity: delete `[crates.cashflows.cross_crate]` entry and record a normal valuations triplet.
- Serde/output: no shape or numerical change.

## Rollback

Revert the entire namespace move. Do not reintroduce only the cashflows host aliases or the parity exception.

## Done Criteria

- Rust implementation lives in the bond module, not `valuations/src/lib.rs`.
- Python and WASM expose it only under valuations/bond.
- Cashflows exports, stubs, types, tests, and parity have no bond-construction symbol.
- Output equivalence and namespace-absence tests pass.

## Targeted Re-Audit Acceptance

Run `finstack-simplify` on Rust module ownership, Python/WASM registrations/facades, and parity. Accept only when the helper has one canonical bond owner, triplet naming is direct, the cross-crate exception is gone, and no compatibility alias recreates namespace drift.
