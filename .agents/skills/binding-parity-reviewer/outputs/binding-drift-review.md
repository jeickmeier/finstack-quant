# Binding Parity Findings

### Major
- `finstack_valuations::attribution::AttributionResult`: Rust exposes the result type, but Python exports omit the matching stub and `__all__` entry. Users can construct the result from Rust-backed calls but cannot type-check imports consistently. Add the `.pyi` class, package export, and parity contract entry in the same slice.

### Minor
- `get_price`: Python uses `price()` while Rust and WASM follow the `get_*` accessor convention for adjacent valuation result types. Standardize on `get_price` or document the intentional exception in the parity contract.

## Surfaces Checked
- Rust: `finstack/valuations/src/attribution/types.rs`
- PyO3: `finstack-py/src/bindings/valuations/attribution.rs`
- Python stubs/exports: `finstack-py/finstack/valuations/*.pyi`, package `__init__.py`
- WASM/JS/TS: `finstack-wasm/src/api/valuations/attribution.rs`
- Parity contract/tests: `finstack-py/parity_contract.toml`, `finstack-py/tests/parity`
- Examples/docs: not checked

## Verification
- Run `mise run python-build`.
- Run targeted parity tests for valuations bindings.
- Run WASM build/tests if the WASM surface changes.
