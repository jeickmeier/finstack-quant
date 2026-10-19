# Python binding documentation

Python users see API docs through `.pyi` stubs (IDE hover, signature help,
mypy) and through PyO3 `///` comments forwarded into `help()`. Stubs are the
primary IntelliSense surface. Runtime `__doc__` comes from the Rust binding
comments.

## Surfaces

| Surface | Location | Role |
|---------|----------|------|
| Stubs | `finstack-quant-py/finstack_quant/**/*.pyi` | IDE docs, types, doctest examples |
| Pure-Python modules | `finstack-quant-py/finstack_quant/**/*.py` | Same bar as stubs when there is no `.pyi` |
| PyO3 rustdoc | `finstack-quant-py/src/bindings/**` | `help()` at the REPL |
| Parity contract | `finstack-quant-py/parity_contract.toml` | Public names that must stay in sync |
| Academic sources | `docs/REFERENCES.md` | Canonical papers and market standards |

Thin re-export shims need only a module docstring; symbol docs live on the
compiled type or the stub.

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
| PyO3 placeholder prose | included in `python-doc` |

`python-doc` rejects fabricated generator boilerplate, tautological
parameter text, and missing `Raises` / “does not raise” notes on every
public callable (including instance methods, properties, and no-arg
helpers). `python-test --doctest` executes `>>>` examples against the compiled
`finstack_quant` package.
