"""Stub declarations mirroring ``finstack-quant-py/src/bindings/portfolio/container.rs``."""

from __future__ import annotations

import datetime
from typing import Any
import pandas as pd
from finstack_quant.core.currency import Currency
from finstack_quant.portfolio._materialization import InstrumentArtifactCache, MaterializationReport

class PortfolioBuilder:
    """
    Fluent builder for a runtime ``Portfolio``.

    Start from :meth:`Portfolio.builder`. Each setter returns ``self`` and
    ``build`` consumes the builder.

    Examples
    --------
    >>> import datetime as dt
    >>> from finstack_quant.portfolio import Portfolio
    >>> pf = Portfolio.builder("book", "USD", dt.date(2025, 1, 1)).name("Desk book").build()
    >>> (pf.id, pf.name)
    ('book', 'Desk book')
    """

    def name(self, name: str) -> PortfolioBuilder:
        """
        Set the human-readable portfolio name.

        Parameters
        ----------
        name : str
            Display name stored on the built portfolio.

        Returns
        -------
        PortfolioBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...

    def entity(
        self,
        id: str,
        name: str | None = None,
        tags: dict[str, str] | None = None,
    ) -> PortfolioBuilder:
        """
        Register an entity that positions can reference.

        Parameters
        ----------
        id : str
            Entity identifier used by ``position(..., entity_id=id)``.
        name : str, optional
            Optional display name.
        tags : dict[str, str], optional
            Optional entity tags for grouping and filtering.

        Returns
        -------
        PortfolioBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...

    def position(
        self,
        position_id: str,
        instrument: object,
        quantity: float,
        entity_id: str | None = None,
        unit: str | dict[str, str] | None = None,
        attributes: dict[str, str | float] | None = None,
    ) -> PortfolioBuilder:
        """
        Add a position holding a typed instrument.

        Parameters
        ----------
        position_id : str
            Unique position identifier.
        instrument : object
            Any typed instrument wrapper, or a canonical instrument-envelope
            JSON string. The instrument's own ``id`` becomes ``instrument_id``.
        quantity : float
            Signed holding; its meaning follows ``unit``. Must be finite.
        entity_id : str, optional
            Owning entity registered via ``entity(...)``; ``None`` assigns the
            position to the auto-created standalone entity.
        unit : str or dict, optional
            Position unit: ``"units"`` (default), ``"face_value"``,
            ``"percentage"``, or the mapping ``{"notional": "USD"}`` /
            ``{"notional": None}`` for a (currency-tagged) lot multiplier.
            The bare string ``"notional"`` is rejected, as in the JSON spec.
        attributes : dict[str, str | float], optional
            Position attributes used by grouping and optimization filters.

        Returns
        -------
        PortfolioBuilder
            ``self``, for chaining.

        Raises
        ------
        TypeError
            If ``instrument`` is not a typed instrument, an envelope dict or a JSON string.
        ValueError
            If the instrument payload is invalid, ``quantity`` is not finite,
            ``unit`` is unknown, or the builder was already consumed.
        """
        ...

    def tag(self, key: str, value: str) -> PortfolioBuilder:
        """
        Attach a portfolio-level tag.

        Parameters
        ----------
        key : str
            Portfolio-level tag name used for grouping, filtering, and reporting.
        value : str
            Tag payload stored under ``key`` on the built portfolio.

        Returns
        -------
        PortfolioBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...

    def meta(self, key: str, value: object) -> PortfolioBuilder:
        """
        Attach a JSON-shaped metadata entry.

        Parameters
        ----------
        key : str
            Metadata key.
        value : object
            Any ``json.dumps``-able value stored on the built portfolio.

        Returns
        -------
        PortfolioBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...

    def build(self) -> Portfolio:
        """
        Validate and build the portfolio, consuming the builder.

        Returns
        -------
        Portfolio
            The constructed runtime portfolio.

        Raises
        ------
        PortfolioError
            If a position references an unregistered entity or another
            structural invariant fails.
        ValueError
            If the builder was already consumed.
        """
        ...

    def __repr__(self) -> str:
        """
        Return a concise debug representation.

        Returns
        -------
        str
            ``PortfolioBuilder(...)`` or ``PortfolioBuilder(<consumed>)``.

        Notes
        -----
        This method does not raise.
        """
        ...

class Portfolio:
    """
    Built runtime portfolio. Cheap to clone; pass directly to pipeline functions.

    Build once with :meth:`from_spec` and reuse across ``value_portfolio``,
    ``aggregate_full_cashflows``, ``aggregate_metrics``, and
    ``apply_scenario_and_revalue`` to skip the per-call spec parse + index
    rebuild.

    Examples
    --------
    >>> import datetime as dt
    >>> from finstack_quant.portfolio import Portfolio
    >>> pf = Portfolio.builder("book", "USD", dt.date(2025, 1, 1)).build()
    >>> (pf.id, pf.base_currency, pf.as_of, len(pf))
    ('book', 'USD', datetime.date(2025, 1, 1), 0)
    """

    @staticmethod
    def builder(
        id: str,
        base_currency: Currency | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
    ) -> PortfolioBuilder:
        """
        Start a typed, fluent portfolio builder.

        Parameters
        ----------
        id : str
            Portfolio identifier.
        base_currency : Currency | str
            Reporting currency (ISO-4217 code or ``Currency``) used for every
            base-currency rollup.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date; ISO 8601 strings are accepted.

        Returns
        -------
        PortfolioBuilder
            Builder whose ``position`` / ``entity`` / ``tag`` / ``meta``
            setters chain and whose ``build()`` returns the ``Portfolio``.

        Raises
        ------
        ValueError
            If ``base_currency`` is not a valid ISO-4217 code or ``as_of`` is
            not a date.

        Examples
        --------
        >>> from finstack_quant.portfolio import Portfolio
        >>> pf = Portfolio.builder("book", "EUR", "2025-01-01").name("Desk").build()
        >>> (pf.name, pf.base_currency)
        ('Desk', 'EUR')
        """
        ...

    @staticmethod
    def from_spec(spec_json: str) -> Portfolio:
        """
        Parse a ``PortfolioSpec`` JSON string into a runtime portfolio.

        Parameters
        ----------
        spec_json : str
            Portfolio specification JSON, including positions, base currency,
            and as-of date, to validate and compile for reuse.

        Returns
        -------
        Portfolio
            Validated runtime portfolio built from ``spec_json`` with lookup indices rebuilt.

        Raises
        ------
        ValueError
            If ``spec_json`` is malformed or does not match the ``PortfolioSpec`` schema.
        PortfolioError
            If the decoded positions or portfolio identifiers violate portfolio
            construction invariants.

        Examples
        --------
        >>> from finstack_quant.portfolio import Portfolio
        >>> spec = '{"id":"empty","base_currency":"USD","as_of":"2025-01-01","entities":{},"positions":[]}'
        >>> portfolio = Portfolio.from_spec(spec)
        >>> (portfolio.id, portfolio.base_currency, len(portfolio))
        ('empty', 'USD', 0)
        """
        ...

    @staticmethod
    def from_materialization(
        bundle: str | bytes | bytearray,
        cache: InstrumentArtifactCache | None = None,
    ) -> tuple[Portfolio, MaterializationReport]:
        """
        Parse and build a strict persisted portfolio bundle in one native call.

        Parsing, validation, unique instrument decoding, position construction,
        and index rebuilding run while the GIL is released. Pass a reusable
        cache to share decoded instruments across bundles.

        Parameters
        ----------
        bundle : str | bytes | bytearray
            Complete UTF-8 JSON document using the
            ``finstack_quant.portfolio_materialization/1`` contract.
        cache : InstrumentArtifactCache | None, default None
            Shared artifact cache, or ``None`` to use an ephemeral bounded
            cache for this call.

        Returns
        -------
        tuple[Portfolio, MaterializationReport]
            Frozen reusable portfolio handle and immutable load report.

        Raises
        ------
        TypeError
            If ``bundle`` is not ``str``, ``bytes``, or ``bytearray``.
        ContractValidationError
            If JSON parsing or structured contract validation fails. The
            exception's ``report`` attribute contains diagnostic dictionaries.
        ContractLimitExceededError
            If the bundle exceeds a configured resource bound.
        PortfolioError
            If native portfolio construction fails outside contract validation.

        Examples
        --------
        >>> import json
        >>> from finstack_quant.portfolio import Portfolio
        >>> bundle = {
        ...     "schema": "finstack_quant.portfolio_materialization/1",
        ...     "portfolio": {"id": "empty", "base_currency": "USD", "as_of": "2025-01-01", "entities": {}},
        ...     "instruments": [],
        ...     "positions": [],
        ... }
        >>> built, report = Portfolio.from_materialization(json.dumps(bundle))
        >>> (built.id, report.positions)
        ('empty', 0)
        """
        ...

    @staticmethod
    def validate_materialization(
        bundle: str | bytes | bytearray,
        cache: InstrumentArtifactCache | None = None,
    ) -> MaterializationReport | dict[str, object]:
        """
        Strictly validate a materialization bundle without building a portfolio.

        Twin of the WASM ``Portfolio.validateMaterialization``: contract
        diagnostics are *returned* rather than raised, so ingestion-form
        callers get structured feedback without exception handling.

        Parameters
        ----------
        bundle : str | bytes | bytearray
            Complete UTF-8 JSON document using the
            ``finstack_quant.portfolio_materialization/1`` contract.
        cache : InstrumentArtifactCache | None, default None
            Shared artifact cache, or ``None`` to use an ephemeral bounded
            cache for this call.

        Returns
        -------
        MaterializationReport | dict[str, object]
            On success, a :class:`MaterializationReport` whose
            ``build_positions`` and ``index_build`` phase counters are always
            zero (those phases are outside this API). When validation finds
            contract errors, the Rust ``ValidationReport`` in its serde form
            (the same object WASM returns): a ``dict`` with ``diagnostics``
            (list of diagnostic dicts carrying ``code``, ``phase``,
            ``severity``, ``pointer``, ``message`` and the optional context
            keys) and ``truncated`` (bool).

        Raises
        ------
        TypeError
            If ``bundle`` is not ``str``, ``bytes``, or ``bytearray``.
        ContractLimitExceededError
            If the bundle exceeds a configured resource bound.
        PortfolioError
            If a native failure occurs outside contract validation.

        Examples
        --------
        >>> import json
        >>> from finstack_quant.portfolio import Portfolio
        >>> bundle = {
        ...     "schema": "finstack_quant.portfolio_materialization/1",
        ...     "portfolio": {"id": "empty", "base_currency": "USD", "as_of": "2025-01-01", "entities": {}},
        ...     "instruments": [],
        ...     "positions": [],
        ... }
        >>> report = Portfolio.validate_materialization(json.dumps(bundle))
        >>> report.positions
        0
        """
        ...

    @property
    def id(self) -> str:
        """
        Stable identifier of this portfolio in the registry.

        Returns
        -------
        str
            Stable identifier of this portfolio in the registry.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def name(self) -> str | None:
        """
        Human-readable portfolio name.

        Returns
        -------
        str | None
            Display name, or ``None`` when the portfolio has no name.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def as_of(self) -> datetime.date:
        """
        Portfolio valuation date.

        Returns
        -------
        datetime.date
            The as-of date every pipeline function values the book on.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def tags(self) -> dict[str, str]:
        """
        Portfolio-level tags in insertion order.

        Returns
        -------
        dict[str, str]
            Tag key/value pairs attached through ``PortfolioBuilder.tag``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def meta(self) -> dict[str, Any]:
        """
        Portfolio-level metadata as a JSON-shaped mapping.

        Returns
        -------
        dict[str, Any]
            Metadata entries attached through ``PortfolioBuilder.meta``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def entity_ids(self) -> list[str]:
        """
        Entity identifiers in registration order.

        Returns
        -------
        list[str]
            Includes the auto-created standalone entity when any position
            omitted ``entity_id``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def position_ids(self) -> list[str]:
        """
        Position identifiers in portfolio order.

        Returns
        -------
        list[str]
            One id per position, in the order positions were added.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def base_currency(self) -> str:
        """
        Base currency code used for valuation and aggregation.

        Returns
        -------
        str
            Base currency code used for valuation and aggregation.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __len__(self) -> int:
        """Number of positions in the built portfolio.
        Returns
        -------
        int
        """
        ...

    def to_spec(self) -> dict[str, Any]:
        """
        Return the canonical ``PortfolioSpec`` of the portfolio as a plain dict.

        Mirrors Rust ``Portfolio::to_spec``. The dict (or its JSON form from
        :meth:`to_json`) is the input accepted by :meth:`from_spec`.

        Returns
        -------
        dict[str, Any]
            ``PortfolioSpec`` wire object: identifier, base currency, valuation
            date, entities, books and positions with their instrument specs.

        Raises
        ------
        ValueError
            If the specification cannot be converted to Python objects.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize the portfolio back to its canonical ``PortfolioSpec`` JSON.

        Returns
        -------
        str
            Compact canonical JSON of :meth:`to_spec`, accepted by
            :meth:`from_spec`.

        Raises
        ------
        ValueError
            If the specification cannot be serialized.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

def parse_portfolio_spec_json(json_str: str) -> str:
    """
    Parse and canonicalize a ``PortfolioSpec`` from JSON.

    Parameters
    ----------
    json_str : str
        Portfolio specification JSON to validate and normalize into canonical
        Rust serialization.

    Returns
    -------
    str
        Canonical compact JSON with defaults normalized and fields emitted in Rust wire order.

    Raises
    ------
    ValueError
        If ``json_str`` is malformed or does not match the ``PortfolioSpec`` schema.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import parse_portfolio_spec_json
    >>> spec = '{"id":"empty","base_currency":"USD","as_of":"2025-01-01","entities":{},"positions":[]}'
    >>> json.loads(parse_portfolio_spec_json(spec))["id"]
    'empty'
    """
    ...
