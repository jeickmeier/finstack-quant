# Established Codebase Conventions

Reference of patterns observed across the finstack-quant codebase. When reviewing for consistency, deviations from these patterns are candidates for findings.

## Rust Conventions

### Error Handling

| Convention | Example | Crates Using |
|-----------|---------|--------------|
| `thiserror::Error` derive | `#[derive(Error, Debug)]` | All crates |
| `#[non_exhaustive]` on error enums | `#[non_exhaustive] pub enum Error` | All crates |
| `pub type Result<T>` alias | `pub type Result<T> = std::result::Result<T, Error>` | All crates |
| `#[from]` for wrapped errors | `Core(#[from] finstack_quant_core::Error)` | All crates |
| Helper constructors on errors | `Error::missing_curve_with_suggestions(...)` | core (partial) |

**Deviation to watch:** Not all error variants have helper constructors. Only core crate uses them extensively.

### Builder Pattern

| Convention | Dominant Pattern | Deviation |
|-----------|-----------------|-----------|
| Entry point (ID-required) | `Type::builder(id)` | Curve builders, `Schedule::builder(start, end)` |
| Entry point (no required ID) | `Type::builder()` | Via `#[derive(FinancialBuilder)]` for instruments |
| Setter prefix | Bare name (`base_date()`, `knots()`) | `set_` prefix only on mutators (not builders) |
| Terminal method | `.build()` | See Terminal Methods section below |
| Return type | `Result<T>` from `.build()` | `T` directly for simple aggregators (see below) |

**Notes:**
- `Builder::new(...)` is acceptable as an internal constructor, but `Type::builder(...)` should be the public-facing entry point.
- Return type `T` (not `Result<T>`) is acceptable for simple aggregators without validation: `InstrumentCurvesBuilder`, `EquityInstrumentDepsBuilder`, `WaterfallBuilder`, `PayoffBuilder`.

### Terminal Methods

| Method | Where Used | Reason |
|--------|-----------|--------|
| `.build()` | All standard builders | Canonical terminal method |
| `.build_plan(exprs, meta)` | `DagBuilder` | Takes extra parameters; builder is reusable (`&mut self`) |
| `.build_with_curves(curves)` | `CashFlowBuilder` | Convenience overload; `.build()` delegates to `build_with_curves(None)` |

### Trait Naming

| Category | Form | Examples |
|----------|------|---------|
| Market data operations | Gerund | `Discounting`, `Forward`, `Survival` |
| Entity types | Noun | `Instrument`, `Solver`, `TermStructure`, `Payoff` |
| Capabilities | Adjective (`-able`) | `Discountable`, `Bumpable` |

### Trait Bounds

| Trait Category | Expected Bounds | Notes |
|---------------|----------------|-------|
| Public domain traits | `Send + Sync` | `Instrument`, `Pricer`, `TermStructure` |
| Extension traits | None required | `DateExt`, `OffsetDateTimeExt` |
| Provider traits | `Send + Sync` | `FxProvider` |

### Module Organization

| Crate Size | Pattern | Example |
|-----------|---------|---------|
| Large module (>3 submodules) | `mod.rs` with submodules | `core/src/dates/mod.rs` |
| Small module (<3 submodules) | Single file | `portfolio/src/error.rs` |
| Shared code | `common/` subdirectory | `instruments/common/`, `valuations/common/` |

### Re-exports

| Pattern | Where Used | Notes |
|---------|-----------|-------|
| `prelude.rs` | core, valuations | Common imports for downstream users |
| Root `pub use` | core `lib.rs` | `HashMap`, `HashSet`, error types |
| No re-exports | Some internal crates | Access via full module path |

## Python Binding Conventions

| Convention | Pattern | Example |
|-----------|---------|---------|
| Wrapper naming | `Py{Type}` | `PyCurrency`, `PyMoney`, `PyDiscountCurve` |
| Module registration | `pub(crate) fn register()` | Every submodule has one |
| Argument parsing | Centralized `*Arg` types | `CurrencyArg` in `common/args.rs` |
| Error mapping | `map_error()` function | `finstack-quant-py/src/errors.rs` |
| Inner field | `pub(crate) inner: RustType` | Consistent across all wrappers |

## WASM Binding Conventions

| Convention | Pattern | Example |
|-----------|---------|---------|
| Wrapper naming | `Js{Type}` | `JsCurrency`, `JsMoney`, `JsBond` |
| JS function naming | `camelCase` via `js_name` | `createStandardRegistry` |
| JS type naming | `PascalCase` via `js_name` | `Bond`, `Currency` |
| Wrapper trait | `InstrumentWrapper` | Consistent `from_inner()` / `inner()` |
| Inner field | `pub(crate) inner: RustType` | Matches Python pattern |

## Naming Patterns

### Financial Domain Terms

When the same concept appears in multiple places, use the same term:

| Concept | Canonical Term | Avoid |
|---------|---------------|-------|
| Interest rate curve | `discount_curve` | `rate_curve`, `yield_curve` (unless specific) |
| Credit risk curve | `hazard_curve` | `default_curve`, `credit_curve` |
| Instrument identifier | `InstrumentId` | `instrument_id` (as a String) |
| Curve identifier | `CurveId` | `curve_id` (as a String) |
| Valuation date | `val_date` | `pricing_date`, `as_of_date` (pick one per context) |

### Constants

| Scope | Style | Example |
|-------|-------|---------|
| `pub const` | `SCREAMING_SNAKE_CASE` | `DEFAULT_MIN_FORWARD_TENOR` |
| Module-level private | `SCREAMING_SNAKE_CASE` (preferred) | Mixed in practice |
| Function-level | `snake_case` | Local computation constants |

## Result-Return Contract

How a public API hands its result back. This is the contract all three
languages implement; deviations from it are findings, not style choices.

| Surface | Computation result | Wire / validator surface |
|---------|-------------------|--------------------------|
| Rust | Typed `…Result` / `…Report` struct. Top-level envelopes stamp `finstack_quant_core::config::ResultsMeta`. | `Result<String>` allowed **only** with a `_json` name suffix **and** a public typed twin |
| Python | Typed `Py*` wrapper | `_json`-suffixed `#[pyfunction]` returning `str` |
| WASM | Plain JS object via `crate::utils::to_js_value`, or a `Js*` handle with getters | `*Json`-suffixed export returning `string` |

### Mandatory accessor set on every result wrapper

| Method | Python | WASM | Notes |
|--------|--------|------|-------|
| Typed field access | `#[getter]` per headline field | getters on `Js*` handles | A wrapper whose only accessor is `to_json` is a JSON string with extra steps |
| JSON out | `to_json() -> String` (compact) | `toJson(): string` | Never `to_string_pretty` |
| JSON in | `#[staticmethod] from_json(json)` | `fromJson(json)` | `#[staticmethod]`, never `#[classmethod]` |
| Pickle | `__reduce__` via `pickle_support::reduce_via_json` | n/a | 49 of 51 `to_json` files already do this |
| pandas frame | `to_dataframe()` | n/a | Primary table; built via `pandas_utils` helpers |
| pandas series | `to_series()` | n/a | **Only** for 1-D labeled vectors |

### Naming rules

| Rule | Correct | Wrong |
|------|---------|-------|
| `_json` / `*Json` suffix means *returns* a JSON string | `allocate_weights_json` | `accrued_interest_json` returning `f64` |
| Non-JSON text returns say so | `plSummaryReportText` | `plSummaryReport` returning prose |
| One primary frame per class | `to_dataframe()` | `to_pandas_long` / `to_pandas_wide` |
| Multi-table classes may add secondaries | `to_dataframe()` + `to_returns_dataframe()` | secondaries with no plain `to_dataframe()` |
| Secondary frames read `to_<thing>_dataframe` | `to_trade_dataframe`, `to_returns_dataframe` | `<thing>_to_dataframe` (sorts away from `to_dataframe`) |
| Orientation is a parameter, not a method | `to_dataframe(orient="wide")` | `to_pandas_wide()` |

### Type suffix policy

| Suffix | Means |
|--------|-------|
| `…Result` | Computation output |
| `…Report` | Diagnostics / validation output |
| `…Envelope` | Wire container (spec in, or result out) |

Do not mass-rename existing types to fit; apply to new types and fix genuine
duplicate names (two types sharing one name in the workspace).

### Serialization rules

- **WASM**: every `JsValue` return goes through `crate::utils::to_js_value`
  (`json_compatible`). Raw `serde_wasm_bindgen::to_value` serializes maps as ES
  `Map`s, which `JSON.stringify` silently drops — it is banned outside
  `utils/mod.rs`.
- **Numeric vectors** cross the WASM boundary as `Float64Array` (return
  `Box<[f64]>`, not `Vec<f64>`).
- **Compact JSON** everywhere. Pretty-printing is presentation; callers can do
  it themselves.
- **Absent values** are `Option<T>` (→ `undefined`), never `JsValue::NULL`.
  `Result` is for *failed*, `Option` is for *absent*.

## Known Intentional Deviations

Document any places where divergence from the dominant pattern is intentional:

### Result-Return Contract
- **Schema-document emitters keep `to_string_pretty`**: `schema.rs`,
  `schema_registry.rs`, and the per-domain `*_schema` functions emit JSON
  Schema documents meant to be read by humans and LLMs and written to files.
  `to_json()` on a *class* is a wire format and stays compact; a `*_schema()`
  *document* stays pretty. Same for the calibration canonical-echo validator.
- **`CalibrationResult.to_json` returns `Bound<'py, PyString>`, not `String`**:
  it serves a cached Python string so repeated access is allocation-free.
  Python sees `str` either way, so the contract is satisfied; the difference is
  Rust-internal.
- **`DatedSeries` JSON is not verbatim `inner`**: the wrapper carries a
  `value_column` metric label that the Rust type does not, so it serializes
  through a private wire struct. Without this, `from_json` would silently
  relabel a rolling-Sharpe series.
- **`SimmCalculator::calculate_from_sensitivities` keeps its `_result` twin**:
  the base fn returns `(f64, HashMap<String, Money>)`, not a bare scalar, and
  collapsing it would force an `as_of: Date` parameter onto the public
  `im_profile_from_simm` and cascade into its binding, stub, and tests. The
  other three margin pairs were collapsed.

### Error & Module Structure
- `error/mod.rs` in core: Uses subdirectory because error module has `inputs.rs` and `suggestions.rs` submodules (justified by size). Valuations uses flat `error.rs` as a re-export facade.
- `prelude.rs` only in core/valuations: Other crates are too small to benefit from a prelude.

### Debug & Display
- `MarketContext` has a manual `Debug` impl that shows collection sizes instead of full contents -- intentional to avoid dumping large data structures in debug output.

### Serde Qualification
- Both `serde::Serialize` (fully qualified in derives) and `Serialize` (after `use serde::{Serialize, Deserialize}` import) are acceptable. No standardization required.

### Instrument Module Structure
- `parameters.rs` is only present for instruments with multiple pricing models or complex configuration (credit derivatives, options, swaptions). Simpler instruments embed parameters in `types.rs`.
- `pricing/` subdirectory vs `pricer.rs`: Use a directory when an instrument has 3+ distinct pricing engines; use a single `pricer.rs` file otherwise.

### Binding Names
- Acronyms in type names use CamelCase (`CdsIndex`, `CdsTranche`, `CdsOption`, `FiIndexTotalReturnSwap`), matching `InstrumentType` variants and C-CASE; Rust, Python and JS names are identical.

### Documentation
- Module READMEs: valuations has more READMEs than core; core uses inline doc comments instead. Both approaches are acceptable.
