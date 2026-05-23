---
trigger: model_decision
description: When python code standards are needed
globs:
---
# Finstack Python Bindings — Code Standards

Standards for the `finstack-py` Python bindings (PyO3-based).

## Goals

- No new business logic in bindings. Bindings are thin wrappers over Rust crates.
- Deterministic behavior; no hidden non‑determinism or global state leaks.
- Currency‑safety: never perform cross‑currency arithmetic in the bindings.
- Deny `unsafe`; match core error semantics via idiomatic Python exceptions.

## Canonical API Rule

Rust is the single source of truth for all API topology and naming:

- The binding module tree under `src/bindings/` mirrors the Rust umbrella crate structure exactly.
- Type and function names in Python match their Rust names exactly (e.g. Rust `sharpe` stays `sharpe`, not `sharpe_ratio`; Rust `Date` stays `Date`, not `FsDate`).
- No convenience re‑exports at `finstack.*` unless the Rust umbrella root exports them.
- No legacy aliases or compatibility paths.

See `docs/superpowers/specs/2026-04-10-rust-canonical-api-alignment-design.md` for the full spec.

## Module Layout and Registration

### Source Tree

All binding Rust code lives under `finstack-py/src/bindings/`:

```
finstack-py/src/
  lib.rs            # thin entrypoint: mod bindings; delegates to bindings::register_root
  bindings/
    mod.rs          # register_root() — registers all crate domains
    core/           # finstack::core bindings
    analytics/      # finstack::analytics bindings
    margin/         # finstack::margin bindings
    valuations/     # finstack::valuations bindings
    statements/     # finstack::statements bindings
    statements_analytics/
    portfolio/
    scenarios/
    correlation/
    monte_carlo/
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

`finstack-py/finstack/__init__.py` exposes only the 10 umbrella domains:

```python
__all__ = (
    "core", "analytics", "margin", "valuations", "statements",
    "statements_analytics", "portfolio", "scenarios", "correlation", "monte_carlo",
)
```

No leaf types at `finstack.*`.

## Type Wrapping Pattern

```rust
#[pyclass(module = "finstack.core.currency", name = "Currency", frozen)]
#[derive(Clone)]
pub struct PyCurrency {
    pub(crate) inner: finstack_core::currency::Currency,
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

- Structural parity tests under `finstack-py/tests/parity/` validate namespace topology against `finstack-py/parity_contract.toml`; behavioral parity cases live alongside runtime tests such as `finstack-py/tests/test_core_parity.py`.
- Build locally: `uv run maturin develop --release`.
- `.pyi` stubs in `finstack-py/finstack/` are derived from the contract and binding code.

## Review Checklist

- [ ] Public APIs have `text_signature` and docstrings.
- [ ] Errors mapped via `core_to_py`; no `unwrap` on user inputs.
- [ ] No cross‑currency math; no business logic in bindings.
- [ ] `__all__` set in registration; module registered under correct parent.
- [ ] Type and function names match Rust exactly.
- [ ] `cargo fmt`/`cargo clippy` clean; `uv run maturin develop` succeeds.
