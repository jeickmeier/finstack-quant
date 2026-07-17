# AGENTS.md

## Project Structure

- Multi-crate Rust workspace: `finstack-quant/core`, `finstack-quant/analytics`, `finstack-quant/valuations`, `finstack-quant/statements`, `finstack-quant/statements-analytics`, `finstack-quant/scenarios`, `finstack-quant/portfolio`, `finstack-quant/margin`, `finstack-quant/monte_carlo`
- Python bindings in `finstack-quant-py/` (PyO3); WASM bindings in `finstack-quant-wasm/` (wasm-bindgen)
- Python binding Rust code lives under `finstack-quant-py/src/bindings/` (one subdirectory per crate domain)
- WASM binding Rust code lives under `finstack-quant-wasm/src/api/` with a hand-written JS facade at `finstack-quant-wasm/index.js`
- `.pyi` stubs in `finstack-quant-py/finstack_quant/` are derived from contract and binding code; structural parity tests live under `finstack-quant-py/tests/parity`, with behavioral parity tests alongside runtime tests such as `finstack-quant-py/tests/test_core_parity.py`
- Parity contract: `finstack-quant-py/parity_contract.toml`
- Example notebooks in `finstack-quant-py/examples/notebooks/`; runner script: `run_all_notebooks.py`

## Build and Tooling

- `uv` is the Python package manager; use `uv run` when running Python functions
- Tasks are defined in `mise.toml`; invoke them with `mise run <task>` (or `mise r <task>`). List everything with `mise tasks`. Common ones: `mise run all-fmt`, `mise run all-lint`, `mise run all-test`, `mise run python-build` (dev profile, fast compile), `mise run python-build -- --release` (release; faster runtime).
- Python test tasks rebuild the extension with the fast dev profile before running pytest. Use `mise run python-build -- --release` only for release validation, performance-sensitive runs, or when explicitly requested. Use `mise run python-sync` first (or whenever Python deps change) to refresh the `uv` virtualenv.
- Pre-commit runs `cargo clippy` and `cargo deny check` (Rust supply-chain: advisories + licenses + bans)
- CI additionally runs OSV-Scanner across `Cargo.lock`, `uv.lock`, and `package-lock.json` for cross-ecosystem CVE coverage
- Clippy runs with `-D warnings`; all warnings are treated as errors

## Clippy Strictness

- `#![deny(clippy::unwrap_used)]`, `#![deny(clippy::panic)]`, `#![forbid(unsafe_code)]` in binding crate
- `too_many_arguments` threshold is 7; use a params struct for more
- `-D missing_docs` is enabled; all public struct fields need doc comments
- `doc_overindented_list_items`: list item continuations use 2-space indent, not aligned to preceding text
- Fix lint/type/test errors before resorting to `#[allow(...)]` as last resort

## Public API Documentation Contract

- Every public Rust function, associated function, trait method, and constructor
  with one or more caller-supplied inputs must include a `# Arguments` section.
- Document every input by its exact Rust parameter name in a Markdown list. Each
  entry must explain the value's purpose and, where applicable, its units,
  conventions, accepted range or shape, defaults, lookup semantics, ownership,
  and effects on state or calculation results. Do not use tautologies such as
  "the input value" or merely repeat the parameter type.
- Financial inputs must state their representation and market convention (for
  example, decimal rate versus percentage or basis points, date/day-count
  convention, currency, curve role, and bump unit). Document identifier and
  optional-input fallback behavior where it changes the result.
- The same bar applies to host-language APIs. Python `.pyi` stubs and
  pure-Python modules must give every public class, classmethod, free function,
  method, and property a substantive summary. Document each parameter, every
  non-``None`` return value, and the exception types a caller should catch,
  including the conditions that raise them. A module example plus a runnable
  doctest for every public class, classmethod, and free function is required;
  class examples may cover ordinary instance accessors. State units,
  conventions, accepted strings, shapes, missing-data behavior, defaults, and
  host-language differences where they affect use. WASM-exposed APIs must
  preserve equivalent input guidance in Rustdoc/TypeScript-facing
  documentation.
- Python errors must name the mapped public exception (`ValueError`, `KeyError`,
  `RuntimeError`, or a documented domain-specific subclass) rather than a
  generic "error". Confirm the mapping in `finstack-quant-py/src/errors.rs`
  before documenting it.
- `mise run rust-doc`, `mise run python-doc`, and `mise run wasm-doc` enforce
  the exact-parameter and substantive-description standard; `python-doc` also
  enforces summaries, return descriptions, error behavior, and required usage
  examples. Update documentation in the same change as any public signature.

## Architecture: Binding Layer

- Rust is the canonical API design. Type and function names in Python/WASM must match Rust exactly (exceptions only for documented host-language collisions)
- All logic stays in Rust crates; bindings do only type conversion, wrapper construction, error mapping
- Python binding tree: `finstack-quant-py/src/bindings/{core,analytics,margin,...}/`; `lib.rs` delegates to `bindings::register_root`
- WASM binding tree: `finstack-quant-wasm/src/api/{core_ns,analytics,margin,...}/`; public API via `index.js` facade, not raw pkg/
- Wrapper pattern: `pub(crate) inner: RustType` with `from_inner()` constructor
- Error handling: centralized `core_to_py()` in `errors.rs` (Python), `JsValue::from_str` (WASM); never use `.unwrap()` or `.expect()` in non-test binding code
- Module registration: every submodule sets `__all__` via `PyList` in `register()`; no dynamic export discovery
- Builder pattern: fluent chaining (e.g., `Type.builder(id).field(val).build()`)
- **WASM exposes a strict subset of `finstack-quant-core`** (currently `currency`, `dates`, `market_data`, `math`, `money`, `types`). Python tracks the full crate surface; WASM is opt-in per module. The agreed subset is documented in `[wasm_core_subset]` in `finstack-quant-py/parity_contract.toml` — update it whenever the WASM core surface changes.

## API Conventions

- Accessors use `get_*` naming (e.g., `get_discount()`, `get_forward()`, `get_price()`)
- Metric keys are fully qualified: `bucketed_dv01::USD-OIS::10y`, `cs01::ACME-HZD`, `pv01::usd_ois`
- Z-spread CS01 for bonds uses instrument ID as key (e.g., `cs01::BOND_A`), not `z_spread`
- Bond CS01 without a hazard curve uses z-spread bump method (market convention)

## Naming Strategy

- **Prefer simple, short names across Rust / Python / WASM.** The canonical Rust name should read well as the Python and WASM binding name. If a Rust name is long or awkward (e.g. `period_stats_from_returns`, `rolling_var_forecasts_with_method`), that is a signal the Rust name itself should be shortened, not that the binding should rename it.
- **Triplet consistency is mandatory.** Rust `snake_case` ↔ Python `snake_case` (identical) ↔ WASM `camelCase` (via `#[wasm_bindgen(js_name = ...)]`). `period_stats` / `period_stats` / `periodStats`, not a mix.
- **Short name = canonical / most-common variant.** When multiple variants of one concept exist, give the short name to the variant most binding users will call. Example:
  - `period_stats(returns: &[f64])` — canonical, takes raw flat returns (exposed in Python/WASM)
  - `period_stats_from_grouped(grouped: &[(PeriodId, f64)])` — specialized grouped-input variant (Rust-internal)
  - `rolling_var_forecasts(..., VarMethod)` — canonical, enum-dispatched (exposed)
  - `rolling_var_forecasts_with_fn(..., fn)` — specialized closure variant (Rust-internal)
- **Descriptive suffixes for specialized variants:** use `_from_<input>` (alternate input shape), `_with_<thing>` (alternate dispatch mechanism), `_unchecked` (invariant-skipping). Suffixes are only for the non-canonical variants; the short base name belongs to the one exposed through bindings.
- **Accessors still use `get_*`** (see above) — naming-strategy shortening does not override the `get_*` convention.
- **When renaming, propagate everywhere in one slice:** Rust source + Rust tests + re-exports → PyO3 `#[pyfunction]` + `__all__` + `.pyi` + `__init__.py` → WASM `#[wasm_bindgen(js_name=...)]` + `index.d.ts` + `exports/*.js` → `finstack-quant-py/parity_contract.toml` + benchmarks + notebooks. Verify with `mise run all-fmt && mise run all-lint && mise run all-test && mise run python-build`.

## Workflow Preferences

- Preferred flow: Audit/Review → Plan → Implement (in that order)
- When a plan file exists: do NOT edit the plan file; do not recreate todos that already exist; mark todos as `in_progress` when starting each one
- During a plan, validate each completed task with the smallest targeted tests and focused lint/type checks that cover its changes. Run full test suites only once, at the very end of the plan, after targeted checks are clean.
- User reports issues by pasting terminal output (clippy, cargo deny, test failures) rather than describing them
- When moving files, use `mv` in terminal and update all import references; then lint and format
