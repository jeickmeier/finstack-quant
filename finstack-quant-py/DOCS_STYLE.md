# Python binding documentation

Python users see API docs through `.pyi` stubs (IDE hover, signature help,
mypy) and through `help()` / IPython `obj?` at runtime. The stub is the single
source of every docstring: `build.rs` extracts each stub docstring into
`$OUT_DIR/pydoc/<module>/<qualname>.txt`, and a PyO3 owner takes it with
`#[doc = pydoc!("finstack_quant.valuations.market", "ConventionRegistry")]`
(module docstrings: `pydoc!(module, "__module__")`). Edit the stub, never Rust
prose, to change what `help()` shows; a key with no stub docstring fails to
compile. Modules not yet converted still forward PyO3 `///` comments into
`__doc__`; `tests/test_runtime_docs.py` lists the converted ones.

## Surfaces

| Surface | Location | Role |
|---------|----------|------|
| Stubs | `finstack-quant-py/finstack_quant/**/*.pyi` | IDE docs, types, doctest examples |
| Pure-Python modules | `finstack-quant-py/finstack_quant/**/*.py` | Same bar as stubs when there is no `.pyi` |
| PyO3 bindings | `finstack-quant-py/src/bindings/**` | `pydoc!` pulls the stub text into `help()`; `///` only in modules not yet converted |
| Parity contract | `finstack-quant-py/parity_contract.toml` | Public names that must stay in sync |
| Academic sources | `docs/REFERENCES.md` | Canonical papers and market standards |

Do not add a `.py` re-export shim for a compiled module: register it with
`Exposure::Compiled` and put its module docstring in the stub. Keep a `.py`
only when it adds pure-Python code.

## Stub layout mirrors the Rust bindings

Each Rust binding file has exactly one stub twin, so a declaration is found
by the same path on both sides:

- A public module's `__init__.pyi` (or `<name>.pyi` when the module draws on
  one Rust file) holds the module docstring and example, what the module's own
  Rust file defines (`mod.rs`, or the single `<name>.rs`), shared aliases and
  constants, the `X as X` re-exports, and `__all__`.
- Every other Rust file of the module gets a private fragment with the same
  name: `bindings/portfolio/brinson.rs` → `portfolio/_brinson.pyi`, and nested
  folders mirror as private folders:
  `bindings/portfolio/optimization/enums.rs` →
  `portfolio/_optimization/enums.pyi` (no `__init__.pyi` in private folders).
  `bindings/valuations/` hosts `valuations.instruments`, so its files map to
  `valuations/instruments/_<file>.pyi`.
- A fragment opens with a one-line docstring naming its Rust twin, imports
  siblings absolutely (`from finstack_quant.portfolio._results import
  PortfolioResult`) and shared aliases from the public module, and keeps
  private typing helpers next to their first user.
- The `pydoc!` key, and the `module =` argument of doc-keyed macros such as
  `wire_methods!` and `instrument_pricing_methods!`, name the fragment
  (`pydoc!("finstack_quant.portfolio._brinson", "BrinsonPeriodResult")`). The
  `#[pyclass(module = ...)]` attribute stays the public module, which is what
  `__module__`, `repr` and pickling see.

Adding a Rust binding file means adding its fragment and the re-export in the
public `__init__.pyi`; tests read a module's declarations through
`tests/stub_helpers.py` rather than one file.

## Required content

Every public class, classmethod, free function, and constructor documents:

1. A one-line summary (at least 16 characters, not a section heading).
2. Every caller-supplied parameter: meaning, units or market convention,
   accepted strings, shape, and defaults.
3. Every non-`None` return: shape, alignment, units, and missing-data
   behavior (`None`, `NaN`, `inf`, placeholder).
4. Exception behavior. Name the public type from
   `finstack-quant-py/src/errors.rs` (`ValueError`, `KeyError`,
   `RuntimeError`, or a documented subclass such as `AnalyticsError` or
   `PortfolioError`) and the condition that raises it. Every public callable
   that cannot fail must say it does not raise and what it returns instead
   (`None`, `NaN`, `inf`, or stored state).
5. A runnable `>>>` doctest on every module, public class, classmethod, and
   free function. Class examples may cover ordinary instance accessors.
   Import-only or `callable(...)` / `.__name__` examples are rejected.

Match the docstring flavor already used in the file (NumPy `Parameters` /
`Returns` / `Raises` in stubs; Google `Args:` / `Returns:` / `Raises:` in
pure-Python helpers such as `features/dataframe.py`). Do not mix flavors in
one module.

Financial and numerical APIs cite `docs/REFERENCES.md` anchors in a
`Sources` section when they implement a named model or market convention.

## Conventions to state explicitly

- **Rates**: decimal (`0.05` = 5%) vs basis points vs continuously compounded.
- **Dates**: role of each date (`as_of` vs issue vs maturity vs accrual).
- **Curves**: required IDs in `MarketContext` (for example `"USD-OIS"`).
- **Quotes**: clean vs dirty, percent-of-par vs absolute.
- **Money**: `Money` stores `Decimal` in Rust. Python construction accepts
  `decimal.Decimal`, `float`, or `int`; `amount_decimal` is the lossless
  view. See [`INVARIANTS.md`](../INVARIANTS.md) §1.

## Builders

Python builders mutate in place **and** return the same instance, so fluent
chaining matches Rust:

```python
from datetime import date
from finstack_quant.core.dates import ScheduleBuilder, StubKind

schedule = (
    ScheduleBuilder(date(2025, 1, 15), date(2030, 1, 15))
    .frequency("3M")
    .stub_rule(StubKind.SHORT_FRONT)
    .build()
)
```

Document that setters return the same builder, not a copy, unless the
binding is an explicit copy-on-write `with_*` method.

## Checks

| Check | Command |
|-------|---------|
| Stub completeness | `mise run python-doc` |
| Stub doctests | `mise run python-test --doctest` |
| PyO3 placeholder prose (unconverted `///` only) | included in `python-doc` |
| Runtime `__doc__` equals the stub | `pytest finstack-quant-py/tests/test_runtime_docs.py` |

`python-doc` rejects fabricated generator boilerplate, tautological
parameter text, and missing `Raises` / “does not raise” notes on every
public callable (including instance methods, properties, and no-arg
helpers). `python-test --doctest` executes `>>>` examples against the compiled
`finstack_quant` package.
