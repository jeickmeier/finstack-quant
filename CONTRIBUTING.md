# Contributing to Finstack Quant

Thanks for your interest in contributing.

## Good First Contributions

- Improve examples and notebooks.
- Add docstrings to Python stubs.
- Add missing test cases for financial conventions.
- Improve error messages at Rust, Python, or WASM boundaries.
- Add small valuation examples.
- Expand glossary and concepts documentation.

## Development Setup

```bash
git clone https://github.com/jeickmeier/finstack-quant.git
cd finstack-quant
mise install
mise run python-sync
```

Run the full local quality gate before opening larger changes:

```bash
mise run all-fmt
mise run all-lint
mise run all-test
```

For narrower changes, use the smallest relevant task first:

| Area | Commands |
|---|---|
| Rust libraries | `mise run rust-build`, `mise run rust-test` |
| Python bindings | `mise run python-build`, `mise run python-test` |
| WASM bindings | `mise run wasm-pkg`, `mise run wasm-test` |
| Examples | `mise run python-examples` |
| Schemas | `mise run rust-check-schemas` |

Run `mise tasks` to see the full task list.

## Project Principles

- Financial logic lives in Rust.
- Python and WASM bindings stay thin: type conversion, wrapper construction,
  error mapping, and module registration only.
- Public APIs require documentation and tests.
- Rust names are canonical. Python names match Rust `snake_case`; WASM names use
  `camelCase`.
- Determinism, convention clarity, and explicit error behavior matter more than
  clever abstractions.

## Binding Changes

When adding or renaming binding surface, update the whole public slice in one
change:

- Rust source, Rust tests, and re-exports.
- PyO3 registration, `__all__`, `.pyi` stubs, and package `__init__.py` exports.
- WASM `#[wasm_bindgen(js_name = ...)]`, TypeScript declarations, facade exports,
  and namespace shims when the surface is part of the WASM subset.
- `finstack-quant-py/parity_contract.toml`.
- Examples, notebooks, and benchmarks that reference the renamed API.

## Documentation

Documentation should describe current behavior, not implementation history.
Prefer concrete inputs, outputs, units, market conventions, errors, and examples.
For Python users, `.pyi` docstrings are the primary IDE-facing API docs.

Useful entry points:

- [`README.md`](README.md)
- [`docs/index.md`](docs/index.md)
- [`docs/REFERENCES.md`](docs/REFERENCES.md)
- [`finstack-quant-py/DOCS_STYLE.md`](finstack-quant-py/DOCS_STYLE.md)
- [`finstack-quant-py/parity_contract.toml`](finstack-quant-py/parity_contract.toml)

## Pull Requests

Keep pull requests focused. Include:

- What changed.
- Why it changed.
- Which checks were run.
- Any remaining verification gaps.

For financial models, pricing, risk, or accounting behavior, include convention
notes and tests that cover edge cases, invalid inputs, and units.
