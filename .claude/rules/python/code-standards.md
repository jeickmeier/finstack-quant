---
trigger: model_decision
description: When python code standards are needed
globs:
---

# Finstack Quant Python Bindings — Code Standards

Standards for the `finstack-quant-py` Python bindings (PyO3-based).

## Goals

- No new business logic in bindings. Bindings are thin wrappers over Rust crates.
- Deterministic behavior; no hidden non‑determinism or global state leaks.
- Currency‑safety: never perform cross‑currency arithmetic in the bindings.
- Deny `unsafe`; match core error semantics via idiomatic Python exceptions.

## Canonical API Rule

Rust is the single source of truth for all API topology and naming:

- The binding module tree under `src/bindings/` mirrors the Rust umbrella crate structure exactly.
- Type and function names in Python match their Rust names exactly (e.g. Rust `sharpe` stays `sharpe`, not `sharpe_ratio`; Rust `Date` stays `Date`, not a host-specific alias).
- No convenience re‑exports at `finstack_quant.*` unless the Rust umbrella root exports them.
- No legacy aliases or compatibility paths.

Two documented deviations from strict crate-mirroring, both recorded in
`parity_contract.toml`:

- `finstack_quant.valuations.correlation` is a **merged** namespace. Most of it
  mirrors `finstack_quant_valuations::correlation` (copulas, `CreditExposure`,
  portfolio-loss simulation). The shared correlation-matrix helpers
  (`validate_correlation_matrix`, `nearest_correlation_matrix`, `Error`) have their
  canonical Rust home in `finstack_quant_analytics::correlation` and are re-exported
  through `finstack_quant_valuations::correlation`. Python/WASM keep the historical
  `valuations.correlation` namespace.
- `reporting` is a pure-Python presentation layer (tear sheets, tables, charts) with
  no Rust crate; it is explicitly exempt from crate-mirroring and has no WASM parity.

See `docs/superpowers/specs/2026-04-10-rust-canonical-api-alignment-design.md` for the full spec.

## Module Layout and Registration

### Source Tree

All binding Rust code lives under `finstack-quant-py/src/bindings/`:

```
finstack-quant-py/src/
  lib.rs            # thin entrypoint: mod bindings; delegates to bindings::register_root
  bindings/
    mod.rs          # register_root() — registers all crate domains
    core/           # finstack_quant::core bindings
    analytics/      # finstack_quant::analytics bindings
    attribution/
    cashflows/
    covenants/
    factor_model/
    features/
    margin/         # finstack_quant::margin bindings
    monte_carlo/
    valuations/     # finstack_quant::valuations bindings
    statements/     # finstack_quant::statements bindings
    statements_analytics/
    portfolio/
    scenarios/
  errors.rs         # centralized error mapping
```

### Registration Pattern

Each crate domain has a `register(py, parent)` function:

```rust
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};
use pyo3::Bound;

pub(crate) fn register<'py>(py: Python<'py>, parent: &Bound<'py, PyModule>) -> PyResult<()> {
    let module = PyModule::new(py, "analytics")?;
    module.add_function(wrap_pyfunction!(sharpe, &module)?)?;
    module.add_function(wrap_pyfunction!(max_drawdown, &module)?)?;
    module.setattr("__all__", PyList::new(py, ["sharpe", "max_drawdown"])?)?;
    parent.add_submodule(&module)?;
    parent.setattr("analytics", &module)?;
    Ok(())
}
```

Rules:

- Set `__all__` via `PyList` directly in registration; do not return export lists.
- Keep `__all__` exhaustive and sorted; expose only public APIs.
- Every module sets `__doc__`.

### Python Package Root

`finstack-quant-py/finstack_quant/__init__.py` exposes the 14 Rust domains plus
the pure-Python `reporting` namespace:

```python
__all__ = (
    "analytics", "attribution", "cashflows", "core", "covenants",
    "factor_model", "features", "margin", "monte_carlo", "portfolio",
    "reporting", "scenarios", "statements", "statements_analytics",
    "valuations",
)
```

No leaf types at `finstack_quant.*`.

## Type Wrapping Pattern

```rust
#[pyclass(module = "finstack_quant.core.currency", name = "Currency", frozen)]
#[derive(Clone)]
pub struct PyCurrency {
    pub(crate) inner: finstack_quant_core::currency::Currency,
}

#[pymethods]
impl PyCurrency {
    #[new]
    #[pyo3(text_signature = "(code)")]
    fn new(code: &str) -> PyResult<Self> {
        let inner = code.parse().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[getter]
    fn code(&self) -> String { self.inner.code().to_string() }
}
```

## Error Mapping

Convert core errors via `errors.rs`:

- Missing id → `KeyError`
- Validation/argument errors → `ValueError`
- Calibration/operational failures → `RuntimeError`
- Never `unwrap()` on user inputs; use `?` with `core_to_py`.

## API Design

- Names: snake_case for functions; PascalCase for classes/enums.
- Constructors: use `#[new]` for primary constructor.
- Builders: expose `Type.builder(...)` as the single entry point.
- Prefer immutable containers; expose builders (`*Builder`) for fluent mutation.
- Avoid surprising coercions. Be explicit about accepted types.

## Docstrings

- Always provide `#[pyo3(text_signature = "...")]` on public functions and constructors.
- Add module `__doc__` and class/method docstrings with NumPy-style sections.
- Include at least one example for nontrivial APIs; keep outputs realistic and stable.

## Performance and Safety

- Do not add heavy computation in bindings; delegate to Rust crates.
- Release the GIL only inside core (already handled in Rust).
- Avoid unnecessary clones; clone only when semantically needed.

## Tests and Stubs

- Structural parity tests under `finstack-quant-py/tests/parity/` validate namespace topology against `finstack-quant-py/parity_contract.toml`; behavioral parity cases live alongside runtime tests such as `finstack-quant-py/tests/test_core_parity.py`.
- Build locally: `uv run maturin develop --release`.
- `.pyi` stubs in `finstack-quant-py/finstack_quant/` are derived from the contract and binding code.

## Review Checklist

- [ ] Public APIs have `text_signature` and docstrings.
- [ ] Errors mapped via `core_to_py`; no `unwrap` on user inputs.
- [ ] No cross‑currency math; no business logic in bindings.
- [ ] `__all__` set in registration; module registered under correct parent.
- [ ] Type and function names match Rust exactly.
- [ ] `cargo fmt`/`cargo clippy` clean; `uv run maturin develop` succeeds.
