# Consistency Review: attribution bindings

## Summary

Found one major naming drift across Rust, Python, and WASM attribution accessors.

## Findings

### Major Naming: accessor drift

**Where:** Rust `get_price`, Python `price`, WASM `getPrice`

**Pattern A:** `get_*` accessor convention is used by adjacent valuation result types.

**Pattern B:** Python exposes a bare noun accessor for the same concept.

**Recommendation:** Standardize on `get_price` for Rust/Python and `getPrice` for WASM, then update stubs and parity tests.

## Convention Inventory

- Accessors: `get_*`
- WASM names: `camelCase` through `js_name`
- Python names: Rust-compatible `snake_case`
