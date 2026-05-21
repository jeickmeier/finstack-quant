# finstack-py documentation style

PyO3 bindings for the `finstack` Rust workspace. Users see docs via `help()`, IDE
tooltips (`.pyi`), and notebooks. Wording should read as Python docstrings and match
the Rust source semantics.

Counterpart: [`finstack-wasm/DOCS_STYLE.md`](../finstack-wasm/DOCS_STYLE.md).

### Where docs live

- **Source of truth**: Rust `///` doc comments on `#[pyfunction]`, `#[pyclass]`, and `#[pymethods]` items in `finstack-py/src/bindings/**`.
- **PyO3 mapping**: PyO3 forwards `///` doc comments verbatim into Python `__doc__`. Whatever you write in the Rust source is what users see at the Python REPL via `help(thing)`.
- **Type stubs**: `.pyi` files in `finstack-py/finstack/**`. These provide IDE tooltips, mypy typing, and a richer docstring surface than what fits naturally in Rust comments.
- **Notebooks**: `finstack-py/examples/notebooks/` — long-form learning material, indexed by level.
- **Parity contract**: `finstack-py/parity_contract.toml` — the exact Python API surface that parity tests pin.

### Required sections per binding

For every `#[pyfunction]`, `#[pyclass]`, classmethod, instance method, property:

#### 1. Summary

One sentence. Reads as a Python docstring would.

#### 2. Parameters / Returns / Raises

Use NumPy-style sections in `.pyi` files (this is what IDEs render best). In Rust binding `///` comments, use the standard rustdoc sections (`# Arguments`, `# Returns`, `# Errors`) — PyO3 forwards them as-is and they are still readable at `help(thing)`.

```rust
/// Construct a Money amount in the given currency.
///
/// # Arguments
///
/// * `amount` - Numeric amount, finite (no NaN or infinity).
/// * `currency` - Either a [`PyCurrency`] or an ISO-4217 code string.
///
/// # Returns
///
/// A new `Money` value pinned to `currency`.
///
/// # Errors
///
/// Raises `ValueError` if `amount` is non-finite or `currency` is not a
/// valid ISO-4217 code.
```

NumPy style in `.pyi`:

```python
def __init__(self, amount: float, currency: Currency | str) -> None:
    """Construct a Money amount in the given currency.

    Parameters
    ----------
    amount : float
        Numeric amount, finite (no NaN or infinity).
    currency : Currency or str
        Either a Currency or an ISO-4217 code string.

    Raises
    ------
    ValueError
        If amount is non-finite or currency is not a valid ISO-4217 code.

    Examples
    --------
    >>> Money(100.0, "USD")
    Money(100.00, USD)
    """
    ...
```

#### 3. Examples

Required for every public class, classmethod, and free function. Examples should be runnable as `>>>` doctests in `.pyi` (we don't run them automatically yet, but write them so we can opt into pytest doctest later).

For `#[pymethods]` accessor patterns where the example is identical to a sibling, you may reference the class-level example instead of duplicating.

#### 4. Conventions (when applicable)

State explicitly:

- **Rates**: decimal (`0.05` = 5%) vs basis points (`500.0` = 5%) vs continuously compounded.
- **Dates**: role of each (`as_of` vs `issue` vs `maturity` vs `accrual_*`).
- **Curves**: required IDs in `MarketContext` (e.g. `"USD-OIS"`).
- **Quote convention**: clean vs dirty, percent-of-par vs absolute.
- **Decimal vs float**: per [`INVARIANTS.md`](../INVARIANTS.md) §1, money values that flow to accounting / settlement / regulatory capital MUST be `Decimal` at the Rust boundary; bindings expose `f64`. Document if a Python user needs to convert back to `decimal.Decimal` for downstream work.

### Financial documentation rules (non-negotiable)

Mirror exactly the language in `finstack-wasm/DOCS_STYLE.md` so the triplets read identically across Rust / Python / WASM:

- **Rates**: always state whether inputs are decimal (e.g. `0.05`) or bps (e.g. `120.0`).
- **Dates**: clarify the role of each date (`as_of` valuation date vs `issue`/`start` vs `maturity`).
- **Curves**: document expected IDs and required market data (what must exist in `MarketContext`).
- **Prices**: clarify quote convention (clean vs dirty, percent-of-par vs absolute).
- **Sign conventions**: see [`INVARIANTS.md`](../INVARIANTS.md) §3 for cashflow sign convention by context.

### Builder pattern: in-place mutation, not fluent self-return

This is **the** Python-vs-Rust quirk:

- **Rust**: builders use fluent self-return (`builder.frequency(x).stub_rule(y).build()`).
- **Python (PyO3)**: PyO3 method bindings cannot return `&mut Self` cleanly, so Python builders are exposed with **in-place mutation** — methods return `None`, you call them sequentially on the same object, then call `.build()`.

Document this on every builder class in both the Rust binding source and the `.pyi`:

```python
class ScheduleBuilder:
    """Fluent builder for constructing date schedules.

    Note
    ----
    Methods on this class mutate the builder in place and return ``None``.
    Call them sequentially rather than chaining.

    Examples
    --------
    >>> from finstack.core.dates import ScheduleBuilder, BusinessDayConvention
    >>> from finstack.core.dates import StubKind
    >>> b = ScheduleBuilder(start_date, end_date)
    >>> b.frequency("3M")
    >>> b.stub_rule(StubKind.SHORT_FRONT)
    >>> b.adjust_with(BusinessDayConvention.MODIFIED_FOLLOWING, "usny")
    >>> schedule = b.build()
    """
```

### Dunder methods

Every PyO3-exposed class will eventually surface `__repr__`, `__str__`, `__hash__`, and rich-comparison dunders. Pick one rule and apply it consistently:

**Rule**: every dunder gets a one-line `///` doc comment in the Rust source, even if obvious. This costs 8-16 characters per method and prevents inconsistency drift across files.

Examples:

```rust
/// Return ``repr(self)``.
fn __repr__(&self) -> String { ... }

/// Return ``str(self)``.
fn __str__(&self) -> String { ... }

/// Hash by canonical key components.
fn __hash__(&self) -> u64 { ... }

/// Equality and ordering by canonical key.
fn __richcmp__(&self, other: &Self, op: CompareOp) -> bool { ... }
```

In `.pyi` stubs, dunders generally don't need a docstring (`...` is fine) — the IDE behaviour is intuitive and Python convention is to not document them.

### Naming consistency

Per [`AGENTS.md`](../AGENTS.md):

- Rust `snake_case` ↔ Python `snake_case` — **identical**, no rename.
- Use `#[pyo3(name = "…")]` only when forced by a Python collision (none in core today).
- Type names (Rust `Money` ↔ Python `Money`) are identical.

When you find yourself wanting to rename in the binding, rename the Rust source instead. See AGENTS.md §"Naming Strategy".

### Error conversion contract

Fallible bindings route through `finstack-py/src/errors.rs`:

- `core_to_py` — `finstack_core::Error` → `ValueError` (full source chain in message).
- `analytics_to_py` — same chain → `AnalyticsError` (`ValueError` subclass).
- `portfolio_to_py` — `finstack_portfolio::Error` → `PortfolioError` or a narrower
  subclass (`FinstackValuationError`, `FinstackFxError`, `FinstackOptimizationError`).
- `display_to_py` — any `Display` type → `ValueError`.
- `serde_json_to_py` — JSON parse/serialize boundaries with a context prefix.

Some modules define additional exceptions (e.g. `CholeskyError` in `core.math.linalg`,
`CalibrationEnvelopeError` in valuations calibration). Document the type users should
catch in `# Errors` / `Raises`.

Do not use `.unwrap()` or `.expect()` in non-test binding code.

### `.pyi` stub minimum bar

Every public binding needs a `.pyi` entry with:

- Full type annotations on every parameter and return.
- A docstring matching the binding's `///` comment (NumPy-style is preferred).
- Inclusion in the module's `__all__` list.
- Consistency with `finstack-py/parity_contract.toml`.

### Templates

#### Static method on a namespace class

```python
class scoring:
    @staticmethod
    def altman_z_score(
        working_capital_to_ta: float,
        retained_earnings_to_ta: float,
        ebit_to_ta: float,
        market_equity_to_book_liab: float,
        sales_to_ta: float,
    ) -> tuple[float, str, float]:
        """Original Altman Z-Score (1968) for public manufacturers.

        Parameters
        ----------
        working_capital_to_ta : float
            Working capital / total assets (X1).
        retained_earnings_to_ta : float
            Retained earnings / total assets (X2).
        ebit_to_ta : float
            EBIT / total assets (X3).
        market_equity_to_book_liab : float
            Market equity / book liabilities (X4).
        sales_to_ta : float
            Sales / total assets (X5).

        Returns
        -------
        tuple[float, str, float]
            ``(score, zone, implied_pd)`` with ``zone`` in
            ``{"safe", "grey", "distress"}``.

        Raises
        ------
        ValueError
            If any ratio is non-finite.

        Examples
        --------
        >>> from finstack.core.credit import scoring
        >>> score, zone, pd = scoring.altman_z_score(0.2, 0.3, 0.15, 1.5, 1.0)
        >>> zone
        'safe'
        """
        ...
```

#### Class with builder

```python
class ScheduleBuilder:
    """Fluent builder for date schedules.

    Methods mutate in place and return ``None``; call them sequentially.

    Parameters
    ----------
    start : Date
        Schedule effective date.
    end : Date
        Schedule terminal date.

    Examples
    --------
    >>> from finstack.core.dates import (
    ...     ScheduleBuilder, BusinessDayConvention, StubKind,
    ... )
    >>> b = ScheduleBuilder(start, end)
    >>> b.frequency("3M")
    >>> b.stub_rule(StubKind.SHORT_FRONT)
    >>> b.adjust_with(BusinessDayConvention.MODIFIED_FOLLOWING, "usny")
    >>> schedule = b.build()
    """

    def __init__(self, start: Date, end: Date) -> None: ...

    def frequency(self, freq: str) -> None:
        """Set the period frequency (e.g. "3M", "6M", "1Y").

        Parameters
        ----------
        freq : str
            Tenor string parseable by :class:`Tenor`.
        """
        ...

    def build(self) -> Schedule:
        """Finalize and return the constructed schedule.

        Raises
        ------
        ValueError
            If the configured parameters are inconsistent (e.g. frequency
            not set, or stub rule incompatible with the date range).
        """
        ...
```

### Workflow

When adding a new binding:

1. Write the Rust binding `///` comment first. Match the Rust source it wraps in semantics.
2. Add the `.pyi` stub with NumPy-style docstring + type annotations.
3. Update `__all__` in the `.pyi` and the `register()` function.
4. Update `parity_contract.toml` if the new binding is in the parity-tested surface.
5. Run `mise run python-build` and `mise run all-test`.

When changing an existing binding:

1. Update the `///` comment.
2. Update the `.pyi` docstring and stubs.
3. Re-run parity tests.

When the binding's behaviour matches the Rust API exactly (the common case), keep the docstrings short — the canonical reference is the Rust source. When the Python surface diverges (in-place builders, dunder-method conventions, error type mapping), document the difference loudly so users don't bounce off it.
