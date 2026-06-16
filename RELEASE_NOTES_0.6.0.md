## Finstack Quant 0.6.0

### Executive Summary

This release is a release-readiness cleanup for the Rust/Python/WASM workspace. It removes a deprecated lenient Heston market-parameter resolver, tightens Python packaging metadata and type stubs, updates PyO3/numpy to the advisory-fixed 0.29 line, and fixes documentation and notebook examples so public docs, verifytypes, and all example notebooks execute cleanly.

### Who Should Upgrade

Upgrade if you consume the Python bindings, rely on generated/public documentation, or use the Heston closed-form/equity-option market-parameter path. Python users pick up the PyO3 security update transitively through the rebuilt native extension.

### Breaking Changes

- `finstack_quant_valuations::models::closed_form::heston::HestonParams::from_market` has been removed.
  Use `HestonParams::from_market_strict` and provide all five unitless market scalars explicitly: `HESTON_KAPPA`, `HESTON_THETA`, `HESTON_SIGMA_V`, `HESTON_RHO`, and `HESTON_V0`.

```rust
let params = HestonParams::from_market_strict(&market, r, q)?;
```

### Improvements

- Python package metadata now declares runtime dependencies under `[project]`, so `uv lock`, `uv sync`, pyright, and editable installs agree on dependencies such as pandas.
- Python public stubs now avoid unknown bare `dict` surfaces in attribution, margin, portfolio, and valuations APIs.
- Rust public docs and doctests have been cleaned up across core, cashflows, margin, Monte Carlo, statements, statements-analytics, and valuations.
- Example notebooks have been refreshed for current paths, drawdown episode fields, relative-value score output shape, and seasoned floating-rate fixing requirements.

### Security And Dependency Updates

- `pyo3` and `numpy` Rust crates are updated to `0.29`, resolving the PyO3 advisories reported by `cargo deny`.
- A stale `getrandom 0.2` cargo-deny skip and unused WASM direct dependencies were removed.

### Golden Fixture Note

- Six formula self-test golden pricing fixtures are currently rebaselined to the canonical Python golden runner output. Keep those fixture changes only if the pricing/convention review accepts the deterministic drift; otherwise revert them and triage the underlying pricing differences before tagging.
