"""
Portfolio construction, valuation, optimization, cashflows, scenarios, and metrics.

Examples
--------
>>> from finstack_quant.portfolio import Portfolio
>>> spec = '{"id":"empty","base_currency":"USD","as_of":"2025-01-01","entities":{},"positions":[]}'
>>> (Portfolio.from_spec(spec).id, len(Portfolio.from_spec(spec)))
('empty', 0)
"""

from __future__ import annotations

import datetime
from typing import Any

import numpy as np
import numpy.typing as npt
import pandas as pd

from finstack_quant.attribution import PnlAttribution
from finstack_quant.core.currency import Currency
from finstack_quant.core.market_data import DiscountCurve, MarketContext
from finstack_quant.core.money import Money
from finstack_quant.core.table import ArrowTable
from finstack_quant.models.factor.credit import CreditFactorModel
from finstack_quant.models.factor.risk import RiskDecomposition
from finstack_quant.portfolio import schema as schema
from finstack_quant.scenarios import ApplicationReport, ScenarioSpec

__all__ = [
    "BrinsonPeriodResult",
    "CandidatePosition",
    "CarinoLinkedAttribution",
    "Constraint",
    "ContractLimitExceededError",
    "ContractValidationError",
    "CreditVolReport",
    "DurationCellTable",
    "ExcessReturnResult",
    "FactorAssignmentReport",
    "FactorBrinsonResult",
    "FactorContributionDelta",
    "FactorPnlProfile",
    "FactorRiskDecomposition",
    "FiAttributionResult",
    "FiCarinoLinkedResult",
    "FiReconciliationReport",
    "FinstackError",
    "FxError",
    "GridAttributionResult",
    "GridCarinoLinkedResult",
    "Inequality",
    "InstrumentArtifactCache",
    "LevelVolContribution",
    "LinkedReturn",
    "MalformedContractSchemaError",
    "MaterializationReport",
    "MetricExpr",
    "MissingContractVersionError",
    "MissingMetricPolicy",
    "Objective",
    "OptimizationStatus",
    "PerPositionMetric",
    "Portfolio",
    "PortfolioAttribution",
    "PortfolioBuilder",
    "PortfolioCashflows",
    "PortfolioError",
    "PortfolioMetrics",
    "PortfolioOptimizationResult",
    "PortfolioOptimizationSpec",
    "PortfolioResult",
    "PortfolioValuation",
    "PositionAssignment",
    "PositionFilter",
    "PositionValue",
    "PositionVolContribution",
    "ReconciliationReport",
    "ReplayResult",
    "ScenarioPnl",
    "ScenarioPnlBatchItem",
    "SensitivityMatrix",
    "StressResult",
    "TradeDirection",
    "TradeSpec",
    "TradeType",
    "TradeUniverse",
    "UnmatchedEntry",
    "UnsupportedContractVersionError",
    "ValuationError",
    "WeightAllocationResult",
    "WeightingScheme",
    "WhatIfResult",
    "aggregate_full_cashflows",
    "aggregate_metrics",
    "aggregate_metrics_json",
    "allocate_weights",
    "allocate_weights_json",
    "apply_scenario_and_revalue",
    "attribute_portfolio_pnl",
    "brinson_fachler",
    "brinson_fachler_json",
    "build_credit_vol_report",
    "build_portfolio_from_spec_json",
    "campisi_attribution",
    "campisi_attribution_json",
    "campisi_carino_link",
    "campisi_carino_link_from_snapshots",
    "campisi_carino_link_from_snapshots_json",
    "campisi_carino_link_json",
    "campisi_reconciliation_check",
    "campisi_reconciliation_check_json",
    "carino_link",
    "carino_link_json",
    "cell_returns_from_curves",
    "cell_returns_from_curves_json",
    "cell_returns_from_reference",
    "cell_returns_from_reference_json",
    "compute_factor_sensitivities",
    "compute_pnl_profiles",
    "decompose_factor_risk",
    "excess_returns",
    "excess_returns_json",
    "factor_brinson_attribution",
    "factor_brinson_attribution_json",
    "factor_stress",
    "grid_attribution",
    "grid_attribution_json",
    "grid_carino_link",
    "grid_carino_link_json",
    "mwr_xirr",
    "net_in_currency_by_date",
    "optimize_portfolio",
    "parse_portfolio_spec_json",
    "position_what_if",
    "replay_portfolio",
    "replay_portfolio_json",
    "scenario_pnl",
    "scenario_pnl_batch",
    "scenario_pnl_batch_json",
    "schema",
    "twrr_linked",
    "twrr_linked_json",
    "twrr_modified_dietz",
    "validate_allocation_json",
    "value_portfolio",
]

class FinstackError(ValueError):
    """
    Base class for the library's named exceptions.

    Catch this in a single ``except FinstackError`` clause to handle any error
    the library raises through one of its own exception classes, rather than
    enumerating them.

    It derives from ``ValueError``, so every subclass keeps ``ValueError`` in
    its MRO and existing ``except ValueError`` clauses are unaffected. Bare
    ``ValueError`` / ``KeyError`` raised by the lower-level mapping helpers are
    deliberately not reparented and are *not* caught by this class.

    Its canonical home is ``finstack_quant.core``; it is re-exported here
    alongside the portfolio exception family.

    Examples
    --------
    >>> from finstack_quant.portfolio import FinstackError, PortfolioError
    >>> issubclass(PortfolioError, FinstackError)
    True
    >>> issubclass(FinstackError, ValueError)
    True
    """

class PortfolioError(FinstackError):
    """
    Portfolio validation or calculation failure.

    Examples
    --------
    >>> from finstack_quant.portfolio import PortfolioError
    >>> str(PortfolioError("invalid portfolio"))
    'invalid portfolio'
    """

class ValuationError(PortfolioError):
    """
    Portfolio valuation failure.

    Examples
    --------
    >>> from finstack_quant.portfolio import ValuationError
    >>> str(ValuationError("valuation failed"))
    'valuation failed'
    """

class FxError(PortfolioError):
    """
    Portfolio FX conversion or market-data failure.

    Examples
    --------
    >>> from finstack_quant.portfolio import FxError
    >>> str(FxError("missing FX rate"))
    'missing FX rate'
    """

class ContractValidationError(FinstackError):
    """
    Persisted contract validation failure with structured diagnostics.

    ``report`` is a list of dictionaries with stable ``code`` and optional
    RFC 6901 ``pointer`` entries suitable for form and ingestion error UIs.

    Examples
    --------
    >>> from finstack_quant.portfolio import ContractValidationError
    >>> issubclass(ContractValidationError, ValueError)
    True
    """

    report: list[dict[str, Any]]

class UnsupportedContractVersionError(ContractValidationError):
    """
    Persisted payload uses an unsupported positive contract version.

    Examples
    --------
    >>> from finstack_quant.portfolio import ContractValidationError, UnsupportedContractVersionError
    >>> issubclass(UnsupportedContractVersionError, ContractValidationError)
    True
    """

class MissingContractVersionError(ContractValidationError):
    """
    Strict persisted payload omits its required contract version.

    Examples
    --------
    >>> from finstack_quant.portfolio import ContractValidationError, MissingContractVersionError
    >>> issubclass(MissingContractVersionError, ContractValidationError)
    True
    """

class MalformedContractSchemaError(ContractValidationError):
    """
    Persisted payload has a malformed or mismatched schema marker.

    Examples
    --------
    >>> from finstack_quant.portfolio import ContractValidationError, MalformedContractSchemaError
    >>> issubclass(MalformedContractSchemaError, ContractValidationError)
    True
    """

class ContractLimitExceededError(ContractValidationError):
    """
    Persisted payload exceeds a configured byte, count, or depth bound.

    Examples
    --------
    >>> from finstack_quant.portfolio import ContractValidationError, ContractLimitExceededError
    >>> issubclass(ContractLimitExceededError, ContractValidationError)
    True
    """

class InstrumentArtifactCache:
    """
    Reusable bounded cache for decoded content-addressed instruments.

    ``capacity`` bounds retained artifacts; encoded source data is also bounded
    at 64 MiB. Instances are frozen and thread-safe.

    Examples
    --------
    >>> from finstack_quant.portfolio import InstrumentArtifactCache
    >>> cache = InstrumentArtifactCache(5_000)
    >>> (len(cache), cache.size, cache.decode_count)
    (0, 0, 0)
    """

    def __init__(self, capacity: int | None = None) -> None:
        """
        Create an empty cache with an explicit artifact-entry bound.

        Parameters
        ----------
        capacity : int, optional
            Maximum decoded artifacts retained by the cache. ``None`` falls
            through to the native ``InstrumentArtifactCache::default`` bound
            (currently 4,096 entries), matching the WASM constructor.
            Benchmarks should pass the fixture's unique-artifact count
            explicitly.

        Raises
        ------
        TypeError
            If ``capacity`` is not an integer.
        OverflowError
            If ``capacity`` is negative or exceeds the native ``usize`` range.

        Returns
        -------
        None
            Constructors initialize the cache in place.
        """
    @property
    def size(self) -> int:
        """
        Number of decoded artifacts currently retained.

        Returns
        -------
        int
            Entry count between zero and the configured capacity.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
    @property
    def decode_count(self) -> int:
        """
        Cumulative number of successful cache-miss decodes.

        Returns
        -------
        int
            Non-negative decode count for this cache instance.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
    def __len__(self) -> int:
        """
        Return the number of retained decoded artifacts.

        Returns
        -------
        int
            Same value as :attr:`size`.
        """

class MaterializationReport:
    """
    Immutable outcome metadata for one successful materialization call.

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
    >>> _, report = Portfolio.from_materialization(json.dumps(bundle))
    >>> (report.positions, report.unique_instruments)
    (0, 0)
    """

    @staticmethod
    def from_json(json: str) -> MaterializationReport:
        """
        Deserialize materialization metadata from canonical JSON.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        MaterializationReport
            Reconstructed report.

        Raises
        ------
        ValueError
            If the payload is malformed.

        Examples
        --------
        >>> from finstack_quant.portfolio import MaterializationReport
        >>> try:
        ...     MaterializationReport.from_json("{}")
        ... except ValueError:
        ...     print("missing fields")
        missing fields
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this report to compact canonical JSON.

        Returns
        -------
        str
            JSON accepted by :meth:`from_json`; also backs ``pickle``.

        Notes
        -----
        This method does not raise for a well-formed report.
        """
        ...

    @property
    def diagnostics(self) -> list[dict[str, Any]]:
        """
        Return bounded non-fatal structured diagnostics.

        Returns
        -------
        list[dict[str, Any]]
            Diagnostic dictionaries with stable codes, phases, severities,
            messages, optional JSON pointers, and artifact context.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
    @property
    def truncated(self) -> bool:
        """
        Whether diagnostics were omitted because the bound was reached.

        Returns
        -------
        bool
            ``True`` when one or more diagnostics were not retained.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
    @property
    def unique_instruments(self) -> int:
        """
        Number of unique instrument artifacts in the source bundle.

        Returns
        -------
        int
            Non-negative artifact count.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
    @property
    def positions(self) -> int:
        """
        Number of ordered positions materialized.

        Returns
        -------
        int
            Non-negative position count.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
    @property
    def dependencies(self) -> int:
        """
        Sum of normalized market-dependency keys over unique artifacts.

        Returns
        -------
        int
            Non-negative dependency count before position fan-out.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
    @property
    def cache_hits(self) -> int:
        """
        Number of unique artifacts served from the supplied cache.

        Returns
        -------
        int
            Non-negative hit count no greater than ``unique_instruments``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
    @property
    def input_bytes(self) -> int:
        """
        Number of UTF-8 bytes in the supplied materialization bundle.

        Returns
        -------
        int
            Positive encoded byte count for the source document.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
    @property
    def timing_available(self) -> bool:
        """
        Whether every phase used an available monotonic host clock.

        Returns
        -------
        bool
            ``True`` for native Python builds, where ``time::Instant`` backs
            every phase measurement.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
    @property
    def phase_nanos(self) -> dict[str, int]:
        """
        Return sequential materialization phase timings.

        Returns
        -------
        dict[str, int]
            Nanosecond timings for parse, version validation, instrument
            decode, position build, and index build phases.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the materialization counters as a single-row DataFrame.

        The report is a flat set of counters, so it yields exactly one row —
        concatenate rows across loads to chart materialization cost over time.
        The nested ``phase_nanos`` mapping is flattened into one column per
        phase; the structured :attr:`diagnostics` payload is not included.

        Columns: ``unique_instruments``, ``positions``, ``dependencies``,
        ``cache_hits``, ``input_bytes``, ``truncated``, ``timing_available``,
        ``phase_parse_nanos``, ``phase_validate_versions_nanos``,
        ``phase_decode_instruments_nanos``, ``phase_build_positions_nanos``,
        ``phase_index_build_nanos``.

        Returns
        -------
        pd.DataFrame
            Exactly one row. The ``phase_*`` columns are zero — not measured
            durations — whenever ``timing_available`` is ``False``.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
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
            contract errors, a ``dict`` with ``diagnostics`` (list of
            diagnostic dicts) and ``truncated`` (bool).

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

    def to_spec_json(self) -> str:
        """
        Serialize the portfolio back to its canonical ``PortfolioSpec`` JSON.
        Returns
        -------
        str
            Compact canonical JSON reconstructed as a ``PortfolioSpec``.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class PortfolioAttribution:
    """
    Rust-computed portfolio P&L attribution in portfolio base currency.

    Examples
    --------
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.portfolio import Portfolio, attribute_portfolio_pnl
    >>> spec = '{"id":"empty","base_currency":"USD","as_of":"2025-01-01","entities":{},"positions":[]}'
    >>> result = attribute_portfolio_pnl(
    ...     Portfolio.from_spec(spec), MarketContext(), MarketContext(), "2025-01-01", "2025-01-02", "parallel"
    ... )
    >>> result.total_pnl.amount == 0
    True
    """

    def to_json(self) -> str:
        """
        Serialize the complete canonical attribution payload.

        Returns
        -------
        str
            Canonical JSON representation of this `PortfolioAttribution`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @staticmethod
    def from_json(json: str) -> PortfolioAttribution:
        """
        Deserialize a `PortfolioAttribution` from JSON produced by :meth:`to_json`.

        Completes the wire round-trip, which is also what makes this type
        picklable.

        Parameters
        ----------
        json : str
            Canonical JSON produced by :meth:`to_json`.

        Returns
        -------
        PortfolioAttribution
            Validated instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If ``json`` is malformed or does not match the serialized schema.

        Examples
        --------
        >>> from finstack_quant.portfolio import PortfolioAttribution
        >>> try:
        ...     PortfolioAttribution.from_json("{}")
        ... except ValueError as exc:
        ...     "missing field" in str(exc)
        True
        """
        ...

    def by_position_json(self) -> str:
        """
        Serialize nested attribution in Rust ``IndexMap`` position order.

        Returns
        -------
        str
            Compact position-keyed attribution JSON in canonical insertion order.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def reconciliation_check(self, tolerance: float) -> ReconciliationReport:
        """
        Check that the factor buckets plus FX translation reconcile to ``total_pnl``.

        Parameters
        ----------
        tolerance : float
            Absolute tolerance in base-currency units (``0.01`` for one cent).

        Returns
        -------
        ReconciliationReport
            Typed report with ``total_residual``, ``is_reconciled`` and
            ``tolerance``; ``is_reconciled`` is forced ``False`` when
            ``result_invalid`` is set.

        Notes
        -----
        This method does not raise; it returns the computed report.
        """
        ...

    @property
    def by_position(self) -> dict[str, PnlAttribution]:
        """
        Per-position attributions in each instrument's native currency.

        Returns
        -------
        dict[str, PnlAttribution]
            Typed attributions keyed by position id in canonical order. They
            exclude FX translation, so they do not sum to the base-currency
            aggregates.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_position_dataframe(self) -> pd.DataFrame:
        """
        Export per-position native-currency attributions, one row per position.

        Returns
        -------
        pd.DataFrame
            Columns ``position_id``, ``currency``, ``total_pnl``, ``carry``,
            ``rates_curves_pnl``, ``credit_curves_pnl``,
            ``inflation_curves_pnl``, ``correlations_pnl``, ``fx_pnl``,
            ``cross_factor_pnl``, ``vol_pnl``, ``model_params_pnl``,
            ``market_scalars_pnl``, ``residual``, ``result_invalid``.

        Raises
        ------
        ValueError
            If the frame cannot be built.
        """
        ...

    def explain(self) -> str:
        """
        Human-readable explanation tree of the portfolio-level buckets.

        Returns
        -------
        str
            Multi-line tree listing each bucket and its share of ``total_pnl``.

        Notes
        -----
        This method does not raise; it formats stored values.
        """
        ...

    @property
    def rates_detail(self) -> dict[str, Any] | None:
        """
        Aggregate rates-curve detail (per-curve breakdown).

        Returns
        -------
        dict[str, Any] | None
            JSON-shaped detail, or ``None`` when the method did not produce it.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def credit_detail(self) -> dict[str, Any] | None:
        """
        Aggregate credit-curve detail.

        Returns
        -------
        dict[str, Any] | None
            JSON-shaped detail, or ``None`` when absent.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def inflation_detail(self) -> dict[str, Any] | None:
        """
        Aggregate inflation-curve detail.

        Returns
        -------
        dict[str, Any] | None
            JSON-shaped detail, or ``None`` when absent.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def correlations_detail(self) -> dict[str, Any] | None:
        """
        Aggregate correlation detail.

        Returns
        -------
        dict[str, Any] | None
            JSON-shaped detail, or ``None`` when absent.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def fx_detail(self) -> dict[str, Any] | None:
        """
        Aggregate FX detail.

        Returns
        -------
        dict[str, Any] | None
            JSON-shaped detail, or ``None`` when absent.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def vol_detail(self) -> dict[str, Any] | None:
        """
        Aggregate volatility detail.

        Returns
        -------
        dict[str, Any] | None
            JSON-shaped detail, or ``None`` when absent.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def scalars_detail(self) -> dict[str, Any] | None:
        """
        Aggregate market-scalar detail.

        Returns
        -------
        dict[str, Any] | None
            JSON-shaped detail, or ``None`` when absent.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the portfolio-level factor totals as a single-row DataFrame.

        Every ``Money`` aggregate is flattened to a float column plus one
        shared ``currency`` column; the per-position nested breakdown stays on
        :meth:`by_position_json`.

        Columns: ``currency``, ``total_pnl``, ``carry``, ``rates_curves_pnl``,
        ``credit_curves_pnl``, ``inflation_curves_pnl``, ``correlations_pnl``,
        ``fx_pnl``, ``fx_translation_pnl``, ``cross_factor_pnl``, ``vol_pnl``,
        ``model_params_pnl``, ``market_scalars_pnl``, ``residual``,
        ``result_invalid``.

        Returns
        -------
        pd.DataFrame
            Exactly one row; concatenate rows across attribution runs to build
            a P&L time series.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    @property
    def total_pnl(self) -> Money:
        """
        Total portfolio P&L between the two market snapshots.

        Returns
        -------
        Money
            Base-currency total; the factor components below decompose it.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def carry(self) -> Money:
        """
        Carry (time decay plus coupon/dividend income) component of total P&L.

        Returns
        -------
        Money
            Base-currency carry. Positive when the passage of time earns income.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def rates_curves_pnl(self) -> Money:
        """
        P&L attributed to interest-rate (discount and forward) curve moves.

        Returns
        -------
        Money
            Base-currency rates component. Excludes credit and inflation curves, which are reported separately.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def credit_curves_pnl(self) -> Money:
        """
        P&L attributed to credit-spread / hazard-rate curve moves.

        Returns
        -------
        Money
            Base-currency credit component, from spread and hazard-rate moves only.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def inflation_curves_pnl(self) -> Money:
        """
        P&L attributed to inflation curve moves.

        Returns
        -------
        Money
            Base-currency inflation component, from index-curve moves only.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def correlations_pnl(self) -> Money:
        """
        P&L attributed to correlation-input moves.

        Returns
        -------
        Money
            Base-currency correlation component, from correlation inputs only.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def fx_pnl(self) -> Money:
        """
        P&L attributed to FX spot/forward moves on FX-sensitive instruments.

        Returns
        -------
        Money
            Distinct from :attr:`fx_translation_pnl`, which covers restating
            unchanged foreign-currency values into the base currency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def fx_translation_pnl(self) -> Money:
        """
        P&L from translating non-base-currency values at the new FX rates.

        Returns
        -------
        Money
            Base-currency translation effect only; economic FX exposure is reported in ``fx_pnl``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def cross_factor_pnl(self) -> Money:
        """
        Second-order P&L from joint moves of two or more factors.

        Returns
        -------
        Money
            Base-currency interaction term. Non-zero only when factors move jointly, and not attributable to any single factor.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def vol_pnl(self) -> Money:
        """
        P&L attributed to implied-volatility surface moves.

        Returns
        -------
        Money
            Base-currency volatility component, from implied-vol surface moves only.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def model_params_pnl(self) -> Money:
        """
        P&L attributed to changes in calibrated model parameters.

        Returns
        -------
        Money
            Base-currency model component, from recalibration rather than market moves.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def market_scalars_pnl(self) -> Money:
        """
        P&L attributed to scalar market inputs (spots, dividends, recoveries).

        Returns
        -------
        Money
            Base-currency scalar component, covering inputs that are not curves or surfaces.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def residual(self) -> Money:
        """
        Unexplained P&L left after every factor has been attributed.

        Returns
        -------
        Money
            ``total_pnl`` less the sum of all factor components. Check it
            against a tolerance with :meth:`reconciliation_check`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def result_invalid(self) -> bool:
        """
        Whether Rust flagged the attribution as unusable.

        Returns
        -------
        bool
            ``True`` when at least one position produced non-finite
            sensitivities or failed residual computation. Aggregating an
            invalid result across instruments is not meaningful.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __repr__(self) -> str: ...

class PortfolioValuation:
    """
    Typed wrapper around a ``PortfolioValuation`` result.

    Wrap the JSON returned by :func:`value_portfolio` once and pass the typed
    object to :func:`aggregate_metrics` to skip re-parsing.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import PortfolioValuation
    >>> doc = {
    ...     "as_of": "2025-01-01",
    ...     "position_values": {},
    ...     "total_base_currency": {"amount": "0", "currency": "USD"},
    ...     "by_entity": {},
    ... }
    >>> PortfolioValuation.from_json(json.dumps(doc)).total_value
    0.0
    """

    @staticmethod
    def from_json(valuation_json: str) -> PortfolioValuation:
        """
        Deserialize a ``PortfolioValuation`` from JSON.

        Parameters
        ----------
        valuation_json : str
            Canonical valuation payload returned by ``value_portfolio`` or an
            equivalent serialized portfolio valuation.

        Returns
        -------
        PortfolioValuation
            Validated `PortfolioValuation` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or cannot be deserialized into the documented portfolio type.

        Examples
        --------
        >>> import json
        >>> from finstack_quant.portfolio import PortfolioValuation
        >>> doc = {
        ...     "as_of": "2025-01-01",
        ...     "position_values": {},
        ...     "total_base_currency": {"amount": "0", "currency": "USD"},
        ...     "by_entity": {},
        ... }
        >>> valuation = PortfolioValuation.from_json(json.dumps(doc))
        >>> (valuation.total_value, valuation.base_currency)
        (0.0, 'USD')
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this valuation to canonical JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `PortfolioValuation`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def total_value(self) -> float:
        """
        Total portfolio value in ``base_currency``.

        Returns
        -------
        float
            Total portfolio value in ``base_currency``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def base_currency(self) -> str:
        """
        Base currency code for this valuation.

        Returns
        -------
        str
            Base currency code for this valuation.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def as_of(self) -> datetime.date:
        """
        Pricing date at which every position in this valuation was marked.

        Returns
        -------
        datetime.date
            Calendar date carried through from the valued portfolio; all
            discounting, accruals and FX conversions in the result are as of
            this date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def position_values(self) -> dict[str, PositionValue]:
        """
        Per-position valuations keyed by position id, in valuation order.

        Returns
        -------
        dict[str, PositionValue]
            Typed per-position values.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def by_entity(self) -> dict[str, Money]:
        """
        Base-currency totals by entity id.

        Returns
        -------
        dict[str, Money]
            Entity rollups in valuation order.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def degraded_positions(self) -> list[str]:
        """
        Positions that fell back to PV-only valuation.

        Returns
        -------
        list[str]
            Position ids whose requested risk metrics could not be computed.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def fx_collapse_policy(self) -> str:
        """
        FX conversion policy stamped on the base-currency rollups.

        Returns
        -------
        str
            Serde name of the applied ``FxConversionPolicy`` (for example
            ``"cashflow_date"``).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def has_degraded_risk(self) -> bool:
        """
        Whether any position carries incomplete risk metrics.

        Returns
        -------
        bool
            ``True`` when ``degraded_positions`` is non-empty.

        Notes
        -----
        This accessor does not raise; it returns the derived value.
        """
        ...

    def get_position_value(self, position_id: str) -> PositionValue:
        """
        Look up one position's valuation.

        Parameters
        ----------
        position_id : str
            Position identifier.

        Returns
        -------
        PositionValue
            Typed valuation for the position.

        Raises
        ------
        KeyError
            If ``position_id`` was not valued.
        """
        ...

    def get_entity_value(self, entity_id: str) -> Money:
        """
        Look up one entity's base-currency total.

        Parameters
        ----------
        entity_id : str
            Entity identifier.

        Returns
        -------
        Money
            Base-currency rollup for the entity.

        Raises
        ------
        KeyError
            If ``entity_id`` has no valued positions.
        """
        ...

    def to_arrow_positions(self) -> ArrowTable:
        """
        Export per-position values via Arrow (zero-copy for consumers).

        Columns: ``position_id``, ``entity_id``, ``value_native``,
        ``value_base``, ``currency_native``, ``currency_base``, ``risk_metrics_complete``, ``risk_error`` (see
        ``finstack_quant_portfolio::positions_to_table``).

        Returns
        -------
        ArrowTable
            Arrow table with one row per valued position.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export per-position values as a pandas DataFrame.

        One row per entry of ``position_values``. Built from the same
        ``positions_to_table`` envelope that backs :meth:`to_arrow_positions`,
        so the two exits cannot drift apart.

        Columns: ``position_id``, ``entity_id``, ``value_native``,
        ``value_base``, ``currency_native``, ``currency_base``.

        Returns
        -------
        pd.DataFrame
            Values are floats and currency codes are strings. Prefer
            :meth:`to_arrow_positions` when a zero-copy handoff matters more
            than pandas ergonomics.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def __len__(self) -> int:
        """Number of valued positions.
        Returns
        -------
        int
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class PortfolioCashflows:
    """
    Typed wrapper around a ``PortfolioCashflows`` ladder.

    Returned by :func:`aggregate_full_cashflows`; survives multiple drill-in
    calls (``events_json``, ``by_date_json``, ``issues_json``,
    :meth:`collapse_to_base_by_date_kind`) without re-parsing.

    Examples
    --------
    >>> from finstack_quant.portfolio import PortfolioCashflows
    >>> cashflows = PortfolioCashflows.from_json(
    ...     '{"events":[],"by_position":{},"by_date":{},"position_summaries":{},"issues":[]}'
    ... )
    >>> (cashflows.num_positions, cashflows.num_issues)
    (0, 0)
    """

    @staticmethod
    def from_json(cashflows_json: str) -> PortfolioCashflows:
        """
        Deserialize a cashflow ladder from JSON.

        Parameters
        ----------
        cashflows_json : str
            Canonical classified-cashflow payload returned by the aggregation
            API or an equivalent serialized ladder.

        Returns
        -------
        PortfolioCashflows
            Validated `PortfolioCashflows` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or cannot be deserialized into the documented portfolio type.

        Examples
        --------
        >>> from finstack_quant.portfolio import PortfolioCashflows
        >>> cashflows = PortfolioCashflows.from_json(
        ...     '{"events":[],"by_position":{},"by_date":{},"position_summaries":{},"issues":[]}'
        ... )
        >>> (cashflows.num_positions, cashflows.num_issues)
        (0, 0)
        """
        ...

    def to_json(self) -> str:
        """
        Serialize the full cashflow ladder to canonical JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `PortfolioCashflows`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def events_json(self) -> str:
        """
        Return all dated cashflow events as JSON.
        Returns
        -------
        str
            Compact JSON array of all dated cashflow events in ladder order.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def by_date_json(self) -> str:
        """
        Return cashflows grouped by date as JSON.
        Returns
        -------
        str
            Compact JSON for totals grouped by payment date, currency, and cashflow kind.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def issues_json(self) -> str:
        """
        Return cashflow aggregation or FX-conversion issues as JSON.
        Returns
        -------
        str
            Compact JSON array of cashflow-schedule extraction issues.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the flat ``events`` ladder as a pandas DataFrame.

        One row per dated cashflow event, in the ladder's canonical
        payment-date order. ``amount`` is flattened to a float column plus a
        ``currency`` column rather than a nested dict, so a multi-currency
        ladder stays currency-safe: group by ``currency`` before summing.

        Columns: ``position_id``, ``instrument_id``, ``instrument_type``,
        ``date`` (ISO 8601 string), ``amount`` (position-scaled), ``currency``,
        ``kind`` (``"fixed"``, ``"float_reset"``, ``"notional"``, ...),
        ``reset_date`` (ISO 8601 string, ``None`` outside floating coupons),
        ``accrual_factor``, ``rate`` (``None`` when the event carries no rate).

        Returns
        -------
        pd.DataFrame
            One row per event; ``len(df)`` equals ``len(cashflows)``.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    @property
    def num_positions(self) -> int:
        """
        Number of positions represented in the ladder.
        Returns
        -------
        int
            Number of positions with schedule entries, including empty schedules.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def num_issues(self) -> int:
        """
        Number of diagnostic issues recorded on the ladder.
        Returns
        -------
        int
            Number of cashflow-schedule extraction issues recorded in the ladder.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def events(self) -> list[dict[str, Any]]:
        """
        Flat position-scaled events in payment-date order.

        Returns
        -------
        list[dict[str, Any]]
            JSON-shaped events with ``position_id``, ``instrument_id``,
            ``instrument_type``, ``date``, ``amount`` (``{"amount",
            "currency"}``), ``kind``, ``reset_date``, ``accrual_factor``,
            ``rate``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def by_position(self) -> dict[str, list[dict[str, Any]]]:
        """
        Per-position event drill-down.

        Returns
        -------
        dict[str, list[dict[str, Any]]]
            Events keyed by position id.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def by_date(self) -> dict[str, dict[str, dict[str, dict[str, str]]]]:
        """
        Totals by payment date, then ISO currency, then cashflow kind.

        Returns
        -------
        dict[str, dict[str, dict[str, dict[str, str]]]]
            ``{date_iso: {currency: {kind: {"amount", "currency"}}}}``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def issues(self) -> list[dict[str, Any]]:
        """
        Extraction issues recorded during aggregation.

        Returns
        -------
        list[dict[str, Any]]
            Entries with ``position_id``, ``instrument_id``,
            ``instrument_type``, ``kind`` and ``message``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def fx_collapse_policy(self) -> str:
        """
        FX policy stamped for :meth:`collapse_to_base_by_date_kind`.

        Returns
        -------
        str
            Serde name of the applied ``CashflowFxPolicy``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_issues_dataframe(self) -> pd.DataFrame:
        """
        Export the extraction issues as a ``DataFrame``.

        Returns
        -------
        pd.DataFrame
            Columns ``position_id``, ``instrument_id``, ``instrument_type``,
            ``kind``, ``message``; zero rows when there are no issues.

        Raises
        ------
        ValueError
            If the frame cannot be built.
        """
        ...

    def net_in_currency_by_date(self, currency: Currency | str) -> list[tuple[str, float]]:
        """
        Net same-currency amounts across kinds for each payment date.

        Parameters
        ----------
        currency : Currency | str
            ISO-4217 currency whose per-date kind buckets are summed.

        Returns
        -------
        list[tuple[str, float]]
            ``(ISO date, net amount)`` pairs in ladder order; dates with no
            flows in ``currency`` are omitted.

        Raises
        ------
        ValueError
            If ``currency`` is not a valid ISO-4217 code.
        """
        ...

    def collapse_to_base_by_date_kind(
        self,
        market: MarketContext | str,
        base_currency: Currency | str,
        as_of: datetime.date | str,
        discount_curves: dict[str, str] | None = None,
    ) -> pd.DataFrame:
        """
        Collapse the ladder to a base-currency ``(date, kind)`` table.

        Payments on or before ``as_of`` use spot FX at ``as_of``. Later
        payments use the CIP forward ``F(T) = S × DF_from(T) / DF_base(T)``.

        Parameters
        ----------
        market : MarketContext or str
            Market context object or JSON providing the FX matrix and
            discount curves used for spot and CIP-forward conversion.
        base_currency : Currency | str
            Currency into which each classified cashflow is converted.
        as_of : datetime.date | str
            Spot/forward split date, either a date-like object or an ISO
            8601 string.
        discount_curves : dict[str, str] | None
            Optional map of ISO currency code to discount-curve id. A missing
            map or missing currency uses the ISO code as the curve id
            (``market.get_discount("EUR")``).

        Returns
        -------
        pd.DataFrame
            Columns ``date`` (ISO 8601 string), ``kind``, ``amount`` (float in
            ``base_currency``), ``currency``; one row per ``(date, kind)``.

        Raises
        ------
        TypeError
            If ``market`` is neither a ``MarketContext`` nor a JSON string.
        ValueError
            If the market JSON, currency, date, or a ``discount_curves`` key is
            invalid.
        PortfolioError
            If an FX rate, discount curve, or discount factor required for
            base-currency conversion is unavailable.
        """
        ...

    def collapse_to_base_by_date_kind_json(
        self,
        market: MarketContext | str,
        base_currency: Currency | str,
        as_of: datetime.date | str,
        discount_curves: dict[str, str] | None = None,
    ) -> str:
        """
        Wire twin of :meth:`collapse_to_base_by_date_kind`.

        Parameters
        ----------
        market : MarketContext or str
            Market context object or JSON providing FX and discount curves.
        base_currency : Currency | str
            Target currency of the collapsed ladder.
        as_of : datetime.date | str
            Spot/forward split date.
        discount_curves : dict[str, str] | None
            Optional ``{currency_code: curve_id}`` overrides.

        Returns
        -------
        str
            Nested ``{date: {kind: {"amount", "currency"}}}`` JSON.

        Raises
        ------
        PortfolioError
            If an FX rate or discount factor required for conversion is
            unavailable.
        """
        ...

    def __len__(self) -> int:
        """Number of cashflow events.
        Returns
        -------
        int
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class PortfolioResult:
    """
    Typed wrapper around a ``PortfolioResult`` envelope.

    Use the scalar accessors (``total_value``, ``get_metric``) to read single
    values without re-parsing the JSON envelope.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import PortfolioResult
    >>> valuation = {
    ...     "as_of": "2025-01-01",
    ...     "position_values": {},
    ...     "total_base_currency": {"amount": "0", "currency": "USD"},
    ...     "by_entity": {},
    ... }
    >>> rounding = {
    ...     "mode": "bankers",
    ...     "ingest_scale_by_currency": {},
    ...     "output_scale_by_currency": {},
    ...     "tolerances": {"rate_epsilon": 1e-12, "generic_epsilon": 1e-10},
    ...     "version": 1,
    ... }
    >>> doc = {
    ...     "schema_version": 1,
    ...     "valuation": valuation,
    ...     "metrics": {"aggregated": {}, "by_position": {}},
    ...     "meta": {"numeric_mode": "f64", "rounding": rounding},
    ... }
    >>> PortfolioResult.from_json(json.dumps(doc)).total_value
    0.0
    """

    def __init__(self, valuation: PortfolioValuation, metrics: PortfolioMetrics) -> None:
        """
        Assemble a result envelope from a valuation and its aggregated metrics.

        Parameters
        ----------
        valuation : PortfolioValuation
            Output of :func:`value_portfolio`.
        metrics : PortfolioMetrics
            Output of :func:`aggregate_metrics` for the same valuation.

        Notes
        -----
        The ``meta`` stamp is taken from the default ``FinstackConfig``. This
        constructor does not raise.

        Examples
        --------
        >>> import json
        >>> from finstack_quant.portfolio import PortfolioMetrics, PortfolioResult, PortfolioValuation
        >>> valuation = PortfolioValuation.from_json(
        ...     json.dumps({
        ...         "as_of": "2025-01-01",
        ...         "position_values": {},
        ...         "total_base_currency": {"amount": "0", "currency": "USD"},
        ...         "by_entity": {},
        ...     })
        ... )
        >>> metrics = PortfolioMetrics.from_json('{"aggregated": {}, "by_position": {}}')
        >>> PortfolioResult(valuation, metrics).total_value
        0.0
        """
        ...

    @property
    def valuation(self) -> PortfolioValuation:
        """
        The valuation component.

        Returns
        -------
        PortfolioValuation
            Per-position values and totals.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def metrics(self) -> PortfolioMetrics:
        """
        The aggregated-metrics component.

        Returns
        -------
        PortfolioMetrics
            Portfolio-wide metric totals.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def meta(self) -> dict[str, Any]:
        """
        Calculation metadata stamp.

        Returns
        -------
        dict[str, Any]
            Numeric mode, rounding context and library version.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @staticmethod
    def from_json(result_json: str) -> PortfolioResult:
        """
        Deserialize a portfolio result envelope from JSON.

        Parameters
        ----------
        result_json : str
            Canonical portfolio-result JSON containing total value and metrics.

        Returns
        -------
        PortfolioResult
            Validated `PortfolioResult` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or cannot be deserialized into the documented portfolio type.

        Examples
        --------
        >>> import json
        >>> from finstack_quant.portfolio import PortfolioResult
        >>> valuation = {
        ...     "as_of": "2025-01-01",
        ...     "position_values": {},
        ...     "total_base_currency": {"amount": "0", "currency": "USD"},
        ...     "by_entity": {},
        ... }
        >>> rounding = {
        ...     "mode": "bankers",
        ...     "ingest_scale_by_currency": {},
        ...     "output_scale_by_currency": {},
        ...     "tolerances": {"rate_epsilon": 1e-12, "generic_epsilon": 1e-10},
        ...     "version": 1,
        ... }
        >>> doc = {
        ...     "schema_version": 1,
        ...     "valuation": valuation,
        ...     "metrics": {"aggregated": {}, "by_position": {}},
        ...     "meta": {"numeric_mode": "f64", "rounding": rounding},
        ... }
        >>> PortfolioResult.from_json(json.dumps(doc)).total_value
        0.0
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this result envelope to canonical JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `PortfolioResult`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def total_value(self) -> float:
        """
        Total value stored in the result envelope.

        Returns
        -------
        float
            Total value stored in the result envelope.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def get_metric(self, metric_id: str) -> float | None:
        """
        Return a metric value, or ``None`` when it is absent.

        Parameters
        ----------
        metric_id : str
            Fully qualified metric key, such as ``"pv01::usd_ois"``.

        Returns
        -------
        float | None
            Aggregated scalar stored under ``metric_id``, or ``None`` when the
            result contains no matching metric.

        Notes
        -----
        This method does not raise; a missing result is ``None`` rather than an exception.
        """
        ...

    def require_metric(self, metric_id: str) -> float:
        """
        Return a metric value, raising ``KeyError`` when it is absent.

        Parameters
        ----------
        metric_id : str
            Fully qualified metric key that must be present in the result.

        Returns
        -------
        float
            Aggregated scalar stored under the fully qualified ``metric_id``.

        Raises
        ------
        KeyError
            If ``metric_id`` is absent from the result.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class PortfolioMetrics:
    """
    Typed wrapper around Rust-aggregated portfolio metrics.

    Examples
    --------
    >>> from finstack_quant.portfolio import PortfolioMetrics
    >>> metrics = PortfolioMetrics.from_json('{"aggregated":{},"by_position":{}}')
    >>> metrics.metric_series("pv")
    []
    """

    @staticmethod
    def from_json(metrics_json: str) -> PortfolioMetrics:
        """
        Deserialize canonical ``PortfolioMetrics`` JSON.

        Parameters
        ----------
        metrics_json : str
            Canonical metric payload returned by ``aggregate_metrics``.

        Returns
        -------
        PortfolioMetrics
            Validated `PortfolioMetrics` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or cannot be deserialized into the documented portfolio type.

        Examples
        --------
        >>> from finstack_quant.portfolio import PortfolioMetrics
        >>> metrics = PortfolioMetrics.from_json('{"aggregated":{},"by_position":{}}')
        >>> metrics.metric_series("pv")
        []
        """
        ...

    def to_json(self) -> str:
        """
        Serialize canonical ``PortfolioMetrics`` JSON.

        Returns
        -------
        str
            Canonical JSON representation of this `PortfolioMetrics`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def aggregated(self) -> dict[str, Any]:
        """
        Portfolio-wide aggregated metrics keyed by metric id.

        Returns
        -------
        dict[str, Any]
            Each value carries ``metric_id``, ``total`` and the ``by_entity``
            breakdown, in canonical Rust ``IndexMap`` insertion order. Only
            summable metrics are aggregated.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def by_position(self) -> dict[str, Any]:
        """
        Raw per-position metric values keyed by position id.

        Returns
        -------
        dict[str, Any]
            Each value carries the position's native ``currency`` — non-summable
            metrics are quoted in it — and its ``metrics`` mapping.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def skipped_metrics(self) -> list[Any]:
        """
        Metric values excluded from aggregation because they were non-finite.

        Returns
        -------
        list[Any]
            A non-empty list means aggregation is incomplete; each entry records
            the ``position_id``, ``metric_id`` and the offending ``value``.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def degraded_positions(self) -> list[str]:
        """
        Positions that carried no risk measures because their valuation fell
        back to PV-only.

        Returns
        -------
        list[str]
            Position identifiers contributing zero to every total without a
            ``skipped_metrics`` entry — the only signal that the aggregate is
            partial.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def unaggregated_metrics(self) -> list[str]:
        """
        Metric identifiers present in ``by_position`` that were not aggregated
        into a portfolio total because they are not summable across positions.

        Returns
        -------
        list[str]
            Sorted, de-duplicated metric ids (for example ``ytm`` or
            ``duration``); per-position values remain in ``by_position``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def metric_series(
        self,
        base: str,
    ) -> list[tuple[list[str], float, dict[str, float]]]:
        """
        Return decoded components, total, and entity values in wire order.

        Entity mappings preserve Rust ``IndexMap`` insertion order. Canonical wire keys decode to unique coordinates.

        Parameters
        ----------
        base : str
            Metric namespace prefix used to select matching metric series.

        Returns
        -------
        list[tuple[list[str], float, dict[str, float]]]
            Ordered ``(components, total, by_entity)`` entries below ``base``;
            the scalar base entry is excluded.

        Notes
        -----
        An unknown canonical prefix returns an empty list.

        Raises
        ------
        ValueError
            If the base metric key is not canonically encoded.
        """
        ...

    def get_metric(self, metric_id: str) -> dict[str, Any] | None:
        """
        One aggregated metric.

        Parameters
        ----------
        metric_id : str
            Fully qualified metric key (for example ``"dv01"``).

        Returns
        -------
        dict[str, Any] | None
            ``{"metric_id", "total", "by_entity"}`` or ``None`` when the
            metric was not aggregated.

        Notes
        -----
        This method does not raise; an absent key yields ``None``.
        """
        ...

    def get_position_metrics(self, position_id: str) -> dict[str, Any] | None:
        """
        One position's raw metrics.

        Parameters
        ----------
        position_id : str
            Position identifier.

        Returns
        -------
        dict[str, Any] | None
            ``{"currency", "metrics"}`` or ``None`` when absent.

        Notes
        -----
        This method does not raise; an absent key yields ``None``.
        """
        ...

    def get_total(self, metric_id: str) -> float | None:
        """
        Portfolio total of one aggregated metric.

        Parameters
        ----------
        metric_id : str
            Fully qualified metric key.

        Returns
        -------
        float | None
            Sum across positions, or ``None`` when absent.

        Notes
        -----
        This method does not raise; an absent key yields ``None``.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Primary ``DataFrame`` view: the aggregated metrics table.

        Returns
        -------
        pd.DataFrame
            Columns ``metric_id``, ``total`` (delegates to
            :meth:`to_aggregated_dataframe`).

        Raises
        ------
        ValueError
            If the frame cannot be built.
        """
        ...

    def to_aggregated_dataframe(self) -> pd.DataFrame:
        """
        Export the portfolio-wide aggregated metrics as a pandas DataFrame.

        One row per entry of the ``aggregated`` map, in canonical Rust
        ``IndexMap`` insertion order. Built from the canonical Rust
        ``aggregated_metrics_to_table`` envelope so the frame cannot drift
        from the Rust table export. The per-entity breakdown is not
        flattened here — reach it through :meth:`metric_series`.

        Columns: ``metric_id``, ``total`` (sum across positions; only summable
        metrics are aggregated).

        Returns
        -------
        pd.DataFrame
            One row per aggregated metric.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_position_dataframe(self) -> pd.DataFrame:
        """
        Export the raw per-position metric values in long format.

        One row per ``(position, metric)`` pair — the row count is the total
        number of metric values across positions, not the number of positions.
        Pivot with ``df.pivot(index="position_id", columns="metric_id",
        values="value")`` for a wide view. Built from the canonical Rust
        ``metrics_to_table`` envelope so the frame cannot drift from the Rust
        table export.

        Columns: ``metric_id``, ``position_id``, ``currency`` (the position's
        native currency; non-summable metrics are quoted in it), ``value``.

        Returns
        -------
        pd.DataFrame
            Long-format per-position metric values.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def __repr__(self) -> str: ...

class ScenarioPnl:
    """
    Scenario-attributable profit and loss, in the portfolio base currency.

    Returned by :func:`scenario_pnl` alongside the scenario
    :class:`~finstack_quant.scenarios.ApplicationReport`. Positions added or
    removed by the scenario are zero-filled against the missing side, so
    :attr:`by_position` always sums to :attr:`total`.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import ScenarioPnl
    >>> doc = {"total": {"amount": "0", "currency": "USD"}, "by_position": {}}
    >>> pnl = ScenarioPnl.from_json(json.dumps(doc))
    >>> (pnl.total, pnl.currency, pnl.by_position)
    (0.0, 'USD', {})
    """

    @property
    def total(self) -> float:
        """
        Total scenario P&L in the portfolio base currency.

        Returns
        -------
        float
            Sum of :attr:`by_position`, quoted in :attr:`currency`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def currency(self) -> str:
        """
        Base currency the P&L is denominated in.

        Returns
        -------
        str
            ISO 4217 code of the portfolio base currency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def by_position(self) -> dict[str, float]:
        """
        Per-position scenario P&L, keyed by position id.

        Returns
        -------
        dict[str, float]
            Base-currency amounts in the canonical Rust ``IndexMap``
            iteration order (stressed positions first, then base-only
            positions), matching :meth:`to_dataframe`, :meth:`to_series`,
            and :meth:`to_json`. Sums to :attr:`total`.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Per-position P&L ladder as a pandas DataFrame.

        Columns: ``position_id``, ``pnl``. The schema is pinned, so an empty
        ladder keeps the same dtypes as a populated one.

        Returns
        -------
        pd.DataFrame
            One row per position, with float ``pnl`` in :attr:`currency`.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_series(self) -> pd.Series:
        """
        Per-position P&L as a pandas Series indexed by position id.

        Returns
        -------
        pd.Series
            Float base-currency amounts named ``pnl``.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this P&L ladder to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation, suitable for a matching
            :meth:`from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @staticmethod
    def from_json(json: str) -> ScenarioPnl:
        """
        Deserialize a ``ScenarioPnl`` from JSON.

        Parameters
        ----------
        json : str
            Canonical payload produced by :meth:`to_json`, or the ``pnl``
            member of a :func:`scenario_pnl_batch` entry.

        Returns
        -------
        ScenarioPnl
            Validated instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or does not match the serialized
            ``ScenarioPnl`` schema.

        Examples
        --------
        >>> import json as _json
        >>> from finstack_quant.portfolio import ScenarioPnl
        >>> doc = {"total": {"amount": "0", "currency": "USD"}, "by_position": {}}
        >>> ScenarioPnl.from_json(_json.dumps(doc)).total
        0.0
        """
        ...

class ScenarioPnlBatchItem:
    """
    One ordered result from :func:`scenario_pnl_batch`.

    Carries the scenario identifier, its typed :class:`ScenarioPnl` ladder,
    and the scenario :class:`~finstack_quant.scenarios.ApplicationReport`.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import ScenarioPnlBatchItem
    >>> doc = {
    ...     "scenario_id": "up_10bp",
    ...     "pnl": {"total": {"amount": "0", "currency": "USD"}, "by_position": {}},
    ...     "report": {
    ...         "operations_applied": 0,
    ...         "user_operations": 0,
    ...         "expanded_operations": 0,
    ...         "changes": {
    ...             "market_targets": [],
    ...             "changed_instrument_indices": [],
    ...             "as_of_changed": False,
    ...             "portfolio_shape_changed": False,
    ...             "all_dirty": False,
    ...         },
    ...         "warnings": [],
    ...     },
    ... }
    >>> ScenarioPnlBatchItem.from_json(json.dumps(doc)).scenario_id
    'up_10bp'
    """

    @property
    def scenario_id(self) -> str:
        """
        Identifier copied from the input scenario.

        Returns
        -------
        str
            The stored label from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def pnl(self) -> ScenarioPnl:
        """
        Scenario-attributable portfolio P&L ladder.

        Returns
        -------
        ScenarioPnl
            Typed P&L ladder in the portfolio base currency.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def report(self) -> ApplicationReport:
        """
        Application provenance and warnings for this scenario.

        Returns
        -------
        ApplicationReport
            Scenario application report with counts and warnings.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Per-position P&L ladder as a pandas DataFrame.

        Columns: ``scenario_id``, ``position_id``, ``pnl``. The schema is pinned, so an empty result keeps the
        same dtypes as a populated one.

        Returns
        -------
        pd.DataFrame
            One row per entry, with the pinned column schema above.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this result to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation, suitable for a matching
            :meth:`from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @staticmethod
    def from_json(json: str) -> ScenarioPnlBatchItem:
        """
        Deserialize a ``ScenarioPnlBatchItem`` from canonical JSON.

        Parameters
        ----------
        json : str
            Canonical payload produced by :meth:`to_json` or the paired
            ``*_json`` wire function.

        Returns
        -------
        ScenarioPnlBatchItem
            Validated instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or does not match the serialized
            ``ScenarioPnlBatchItem`` schema.

        Examples
        --------
        >>> from finstack_quant.portfolio import ScenarioPnlBatchItem
        >>> doc = (
        ...     '{"scenario_id": "up", "pnl": {"total": {"amount": "0",'
        ...     ' "currency": "USD"}, "by_position": {}}, "report":'
        ...     ' {"operations_applied": 0, "user_operations": 0,'
        ...     ' "expanded_operations": 0, "changes": {"market_targets": [],'
        ...     ' "changed_instrument_indices": [], "as_of_changed": false,'
        ...     ' "portfolio_shape_changed": false, "all_dirty": false},'
        ...     ' "warnings": []}}'
        ... )
        >>> ScenarioPnlBatchItem.from_json(doc).scenario_id
        'up'
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

def build_portfolio_from_spec_json(spec_json: str) -> str:
    """
    Build a runtime portfolio from JSON and return the round-tripped spec.

    Prefer :meth:`Portfolio.from_spec` for real work — it returns the typed
    object that pipeline functions reuse without rebuilding.

    Parameters
    ----------
    spec_json : str
        Portfolio specification JSON to validate, compile, and serialize back.

    Returns
    -------
    str
        Canonical compact JSON for the validated, constructed portfolio specification.

    Raises
    ------
    ValueError
        If ``spec_json`` is malformed or does not match the ``PortfolioSpec`` schema.
    PortfolioError
        If the decoded positions or portfolio identifiers violate portfolio
        construction invariants.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import build_portfolio_from_spec_json
    >>> spec = '{"id":"empty","base_currency":"USD","as_of":"2025-01-01","entities":{},"positions":[]}'
    >>> json.loads(build_portfolio_from_spec_json(spec))["positions"]
    []
    """
    ...

def aggregate_metrics(
    valuation: PortfolioValuation | str,
    base_currency: Currency | str,
    market: MarketContext | str,
    as_of: datetime.date | str,
) -> PortfolioMetrics:
    """
    Aggregate portfolio metrics from a valuation.

    Accepts a typed :class:`PortfolioValuation` (fast path) or a JSON string.

    Parameters
    ----------
    valuation : PortfolioValuation or str
        Typed valuation or canonical valuation JSON to aggregate.
    base_currency : Currency | str
        ISO base-currency code in which aggregate values and metrics are stated.
    market : MarketContext or str
        Market context object or JSON supplying conversion and market inputs.
    as_of : datetime.date | str
        Valuation date used to resolve date-dependent market data, either a
        date-like object or an ISO 8601 string.

    Returns
    -------
    PortfolioMetrics
        Typed aggregate-metrics wrapper with ``aggregated`` /
        ``by_position`` getters and DataFrame exits; use
        :func:`aggregate_metrics_json` for the raw wire string.

    Raises
    ------
    TypeError
        If ``valuation`` or ``market`` is neither its typed wrapper nor a JSON string.
    ValueError
        If supplied JSON, ``base_currency``, or ``as_of`` is invalid.
    PortfolioError
        If the valuation base currency or date is inconsistent with the
        requested aggregation context.
    FxError
        If an FX rate required for base-currency aggregation is unavailable.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.portfolio import Portfolio, aggregate_metrics, value_portfolio
    >>> spec = '{"id":"empty","base_currency":"USD","as_of":"2025-01-01","entities":{},"positions":[]}'
    >>> valuation = value_portfolio(Portfolio.from_spec(spec), MarketContext())
    >>> aggregate_metrics(valuation, "USD", MarketContext(), "2025-01-01").aggregated
    {}
    """
    ...

def aggregate_metrics_json(
    valuation: PortfolioValuation | str,
    base_currency: Currency | str,
    market: MarketContext | str,
    as_of: datetime.date | str,
) -> str:
    """
    Aggregate portfolio metrics from a valuation and return wire JSON.

    Wire twin of :func:`aggregate_metrics`: same inputs and validation,
    returning the canonical JSON string instead of the typed wrapper.

    Parameters
    ----------
    valuation : PortfolioValuation or str
        Typed valuation or canonical valuation JSON to aggregate.
    base_currency : Currency | str
        ISO base-currency code in which aggregate values are stated.
    market : MarketContext or str
        Market context object or JSON supplying conversion inputs.
    as_of : datetime.date | str
        Valuation date, either a date-like object or an ISO 8601 string.

    Returns
    -------
    str
        JSON-serialized ``PortfolioMetrics`` wire document.

    Raises
    ------
    TypeError
        If ``valuation`` or ``market`` is neither its typed wrapper nor a
        JSON string.
    ValueError
        If supplied JSON, ``base_currency``, or ``as_of`` is invalid.
    PortfolioError
        If the valuation is inconsistent with the aggregation context.
    FxError
        If a required FX rate is unavailable.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.portfolio import Portfolio, aggregate_metrics_json, value_portfolio
    >>> spec = '{"id":"empty","base_currency":"USD","as_of":"2025-01-01","entities":{},"positions":[]}'
    >>> valuation = value_portfolio(Portfolio.from_spec(spec), MarketContext())
    >>> json.loads(aggregate_metrics_json(valuation, "USD", MarketContext(), "2025-01-01"))["aggregated"]
    {}
    """
    ...

def value_portfolio(
    portfolio: Portfolio | str,
    market: MarketContext | str,
    strict_risk: bool = True,
    metrics: list[str] | None = None,
) -> PortfolioValuation:
    """
    Value a portfolio and return its typed result.

    Typed ``Portfolio`` and ``MarketContext`` inputs avoid rebuilding runtime
    state, while the result can be passed directly to :func:`aggregate_metrics`.

    Parameters
    ----------
    portfolio : Portfolio or str
        Built portfolio or canonical ``PortfolioSpec`` JSON to value.
    market : MarketContext or str
        Market context object or JSON supplying curves, quotes, and FX data.
    strict_risk : bool, default True
        Whether absent or failed risk calculations abort the valuation rather
        than being recorded as diagnostics. The default matches Rust: a
        standard-metric risk run fails closed. Set ``False`` only for an
        intentional PV-preserving fallback.
    metrics : list[str] or None, default None
        Exact metric identifiers to compute. ``None`` requests the standard
        portfolio risk set; an empty list performs PV-only valuation. Names
        are validated strictly against the standard ``MetricId`` set; an
        unknown name raises ``ValueError`` listing the available metrics.

    Returns
    -------
    PortfolioValuation
        Typed valuation result backed directly by the Rust calculation.

    Raises
    ------
    ValueError
        If a requested metric name is not a standard metric identifier.
    PortfolioError
        If portfolio construction, market lookup, FX conversion, pricing, or
        strict risk evaluation fails.

    Examples
    --------
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.portfolio import Portfolio, value_portfolio
    >>> spec = '{"id":"empty","base_currency":"USD","as_of":"2025-01-01","entities":{},"positions":[]}'
    >>> value_portfolio(Portfolio.from_spec(spec), MarketContext()).total_value
    0.0
    """
    ...

def aggregate_full_cashflows(
    portfolio: Portfolio | str,
    market: MarketContext | str,
    allow_partial: bool = False,
) -> PortfolioCashflows:
    """
    Build the full classified cashflow ladder for the portfolio.

    Returns a typed :class:`PortfolioCashflows` wrapper; call ``to_json()``
    to get the raw ladder or use the typed accessors to drill in.

    Parameters
    ----------
    portfolio : Portfolio or str
        Built portfolio or canonical ``PortfolioSpec`` JSON to expand.
    market : MarketContext or str
        Market context object or JSON needed for instrument cashflow generation.
    allow_partial : bool, default False
        When ``False``, any schedule-construction issue aborts the call.
        When ``True``, remaining positions still contribute to the ladder
        and issues are returned on :class:`PortfolioCashflows`.

    Returns
    -------
    PortfolioCashflows
        Typed event ladder with per-position, per-date, and issue drill-downs.

    Raises
    ------
    TypeError
        If ``portfolio`` or ``market`` is neither its typed wrapper nor a JSON string.
    ValueError
        If supplied JSON is malformed or cashflow aggregation cannot represent
        a monetary result.
    PortfolioError
        If a JSON portfolio cannot be constructed from its decoded specification,
        or if any position fails schedule construction while ``allow_partial``
        is ``False``.

    Examples
    --------
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.portfolio import Portfolio, aggregate_full_cashflows
    >>> spec = '{"id":"empty","base_currency":"USD","as_of":"2025-01-01","entities":{},"positions":[]}'
    >>> cashflows = aggregate_full_cashflows(Portfolio.from_spec(spec), MarketContext())
    >>> (cashflows.num_positions, cashflows.num_issues)
    (0, 0)
    """
    ...

def net_in_currency_by_date(
    cashflows: PortfolioCashflows | str,
    currency: Currency | str,
) -> list[tuple[str, float]]:
    """
    Net same-currency cashflow amounts across kinds for each payment date.

    Parameters
    ----------
    cashflows : PortfolioCashflows | str
        A :class:`PortfolioCashflows` ladder (no parse), full cashflow-ladder
        JSON, or a ``{date: {ccy: {kind: money}}}`` object (optionally wrapped
        as ``{"by_date": ...}``). Kind keys are opaque strings; amounts may be
        JSON numbers or decimal strings.
    currency : Currency | str
        ISO-4217 code selecting which per-date currency bucket to net.

    Returns
    -------
    list[tuple[str, float]]
        ``(ISO date, net amount)`` pairs sorted by date. Dates with no flows
        in ``currency`` are omitted.

    Raises
    ------
    TypeError
        If ``cashflows`` is neither a ``PortfolioCashflows`` nor a string.
    ValueError
        If ``cashflows`` is not JSON, ``currency`` is unknown, or ``by_date``
        is not an object.
    PortfolioError
        If cashflow JSON cannot be interpreted as a classified ladder.

    Examples
    --------
    >>> from finstack_quant.portfolio import net_in_currency_by_date
    >>> net_in_currency_by_date(
    ...     '{"by_date":{"2025-01-15":{"USD":{"notional":{"amount":"-100","currency":"USD"}}}}}',
    ...     "USD",
    ... )
    [('2025-01-15', -100.0)]
    """
    ...

def apply_scenario_and_revalue(
    portfolio: Portfolio | str,
    scenario: ScenarioSpec | str,
    market: MarketContext | str,
) -> tuple[PortfolioValuation, ApplicationReport]:
    """
    Apply a scenario and revalue the portfolio.

    Parameters
    ----------
    portfolio : Portfolio or str
        Built portfolio or canonical ``PortfolioSpec`` JSON to revalue.
    scenario : ScenarioSpec | str
        Typed :class:`~finstack_quant.scenarios.ScenarioSpec` or its canonical
        JSON string describing market-data shocks.
    market : MarketContext or str
        Base market context object or JSON to shock before revaluation.

    Returns
    -------
    tuple[PortfolioValuation, ApplicationReport]
        The revalued portfolio and the scenario application report. Call
        ``.to_json()`` on either for its wire form.

    Raises
    ------
    TypeError
        If ``portfolio`` or ``market`` is neither its typed wrapper nor a JSON string.
    ValueError
        If portfolio, scenario, or market JSON is malformed or schema-incompatible.
    PortfolioError
        If portfolio construction, scenario application, market lookup, FX
        conversion, or valuation fails.

    Examples
    --------
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.portfolio import Portfolio, apply_scenario_and_revalue
    >>> spec = '{"id":"empty","base_currency":"USD","as_of":"2025-01-01","entities":{},"positions":[]}'
    >>> scenario = '{"id":"s","name":"S","operations":[]}'
    >>> valuation, report = apply_scenario_and_revalue(Portfolio.from_spec(spec), scenario, MarketContext())
    >>> (valuation.total_value, report.operations_applied)
    (0.0, 0)
    """
    ...

def scenario_pnl(
    portfolio: Portfolio | str,
    scenario: ScenarioSpec | str,
    market: MarketContext | str,
) -> tuple[ScenarioPnl, ApplicationReport]:
    """
    Compute the profit and loss attributable to a scenario.

    Values the portfolio against the unshocked market and against the
    scenario-shocked market, then reports the base-currency difference per
    position and in total. Positions added or removed by the scenario are
    zero-filled against the missing side, so ``by_position`` always sums to
    ``total``.

    Parameters
    ----------
    portfolio : Portfolio or str
        Built portfolio or canonical ``PortfolioSpec`` JSON valued on both the
        unshocked and shocked legs.
    scenario : ScenarioSpec | str
        Typed :class:`~finstack_quant.scenarios.ScenarioSpec` or its canonical
        JSON string describing the market-data shocks whose profit-and-loss
        impact is measured.
    market : MarketContext or str
        Unshocked market context object or JSON used as the base leg and as the
        source the scenario operations are applied to.

    Returns
    -------
    tuple[ScenarioPnl, ApplicationReport]
        The P&L ladder (base-currency ``total`` and ``by_position`` amounts)
        and the scenario application report carrying which operations were
        applied. :class:`ScenarioPnl` offers ``to_dataframe()`` and
        ``to_series()``; call ``.to_json()`` on either for its wire form.

    Raises
    ------
    TypeError
        If ``portfolio`` or ``market`` is neither its typed wrapper nor a JSON string.
    ValueError
        If a JSON input is malformed or does not match its serialized schema.
    PortfolioError
        If scenario application or either valuation fails, including for
        missing market data or mismatched base and shocked currencies.

    Examples
    --------
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.portfolio import Portfolio, scenario_pnl
    >>> spec = '{"id":"empty","base_currency":"USD","as_of":"2025-01-01","entities":{},"positions":[]}'
    >>> pnl, report = scenario_pnl(Portfolio.from_spec(spec), '{"id":"s","name":"S","operations":[]}', MarketContext())
    >>> (pnl.total, pnl.currency, pnl.by_position)
    (0.0, 'USD', {})
    """
    ...

def scenario_pnl_batch(
    portfolio: Portfolio | str,
    scenarios: list[ScenarioSpec | str] | str,
    market: MarketContext | str,
) -> list[ScenarioPnlBatchItem]:
    """
    Compute ordered scenario P&L results while reusing one base valuation.

    The Rust portfolio engine values the unshocked portfolio once, applies
    each scenario independently, and returns results in the same order as the
    input JSON array. It is the batch counterpart to :func:`scenario_pnl`;
    use it when more than one scenario is evaluated against one portfolio and
    market snapshot.

    Parameters
    ----------
    portfolio : Portfolio or str
        Built portfolio or canonical ``PortfolioSpec`` JSON. The typed form
        avoids rebuilding the portfolio for the batch.
    scenarios : list[ScenarioSpec | str] | str
        Ordered scenarios as a sequence of typed ``ScenarioSpec`` objects or
        canonical JSON strings, or one canonical JSON array string. Order is
        preserved exactly; an empty batch returns an empty list without
        valuation.
    market : MarketContext or str
        Unshocked market context or canonical market JSON used for the shared
        base valuation and each scenario application.

    Returns
    -------
    list[ScenarioPnlBatchItem]
        One ordered item per input scenario, each carrying ``scenario_id``,
        a typed ``pnl`` (:class:`ScenarioPnl`) and ``report``
        (:class:`~finstack_quant.scenarios.ApplicationReport`); use
        :func:`scenario_pnl_batch_json` for the raw wire string.

    Raises
    ------
    ValueError
        If ``scenarios`` is malformed or cannot deserialize to an ordered
        array of valid ``ScenarioSpec`` values.
    PortfolioError
        If scenario application, valuation, or base-currency P&L differencing
        fails. The reported error is for the earliest failing input scenario.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.portfolio import Portfolio, scenario_pnl_batch
    >>> spec = '{"id":"empty","base_currency":"USD","as_of":"2025-01-01","entities":{},"positions":[]}'
    >>> scenario_pnl_batch(Portfolio.from_spec(spec), "[]", MarketContext())
    []
    """
    ...

def scenario_pnl_batch_json(
    portfolio: Portfolio | str,
    scenarios: list[ScenarioSpec | str] | str,
    market: MarketContext | str,
) -> str:
    """
    Compute ordered batch scenario P&L and return wire JSON.

    Wire twin of :func:`scenario_pnl_batch`: same inputs and validation,
    returning the canonical JSON array string instead of typed items.

    Parameters
    ----------
    portfolio : Portfolio or str
        Built portfolio or canonical ``PortfolioSpec`` JSON.
    scenarios : list[ScenarioSpec | str] | str
        Ordered scenarios (typed objects, JSON strings, or one JSON array
        string); order is preserved.
    market : MarketContext or str
        Unshocked market context or canonical market JSON.

    Returns
    -------
    str
        Canonical JSON array. Each item has ``scenario_id``, ``pnl`` (the
        same base-currency ``ScenarioPnl`` shape returned by
        :func:`scenario_pnl`), and ``report``. ``"[]"`` in, ``"[]"`` out.

    Raises
    ------
    ValueError
        If ``scenarios`` cannot deserialize to an ordered array of
        valid ``ScenarioSpec`` values.
    PortfolioError
        If scenario application, valuation, or base-currency P&L
        differencing fails.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.portfolio import Portfolio, scenario_pnl_batch_json
    >>> spec = '{"id":"empty","base_currency":"USD","as_of":"2025-01-01","entities":{},"positions":[]}'
    >>> json.loads(scenario_pnl_batch_json(Portfolio.from_spec(spec), "[]", MarketContext()))
    []
    """
    ...

def attribute_portfolio_pnl(
    portfolio: Portfolio | str,
    market_t0: MarketContext | str,
    market_t1: MarketContext | str,
    as_of_t0: datetime.date | str,
    as_of_t1: datetime.date | str,
    method: str | dict[str, Any],
    config: dict[str, Any] | str | None = None,
) -> PortfolioAttribution:
    """
    Attribute portfolio P&L with Rust-owned aggregation and FX translation.

    Parameters
    ----------
    portfolio : Portfolio or str
        Built portfolio or canonical ``PortfolioSpec`` JSON being attributed.
    market_t0 : MarketContext or str
        Opening market context object or JSON at the start of the P&L interval.
    market_t1 : MarketContext or str
        Closing market context object or JSON at the end of the P&L interval.
    as_of_t0 : datetime.date | str
        ISO-8601 opening valuation date associated with ``market_t0``.
    as_of_t1 : datetime.date | str
        ISO-8601 closing valuation date associated with ``market_t1``.
    method : str or dict[str, Any]
        Attribution-method name or method configuration understood by Rust.
    config : dict[str, Any] or str or None
        Optional attribution configuration mapping or canonical JSON payload.

    Returns
    -------
    PortfolioAttribution
        Base-currency P&L decomposition by position, market move, and residual.

    Raises
    ------
    TypeError
        If a portfolio, market, method, or configuration input has an unsupported type.
    ValueError
        If supplied JSON, either ISO date, or the attribution method or
        configuration is invalid.
    PortfolioError
        If portfolio construction, valuation, FX conversion, or attribution fails.

    Examples
    --------
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.portfolio import Portfolio, attribute_portfolio_pnl
    >>> spec = '{"id":"empty","base_currency":"USD","as_of":"2025-01-01","entities":{},"positions":[]}'
    >>> result = attribute_portfolio_pnl(
    ...     Portfolio.from_spec(spec), MarketContext(), MarketContext(), "2025-01-01", "2025-01-02", "parallel"
    ... )
    >>> result.total_pnl.amount == 0
    True
    """
    ...

class WeightAllocationResult:
    """
    Strategy weight-allocation result.

    Returned by :func:`allocate_weights`.

    Examples
    --------
    >>> from finstack_quant.portfolio import allocate_weights
    >>> spec = '{"scheme":"equal","total_capital":1000.0,"strategies":[{"id":"a"},{"id":"b"}]}'
    >>> [row["weight"] for row in allocate_weights(spec).allocations]
    [0.5, 0.5]
    """

    @property
    def scheme(self) -> str:
        """
        Allocation scheme applied (e.g. ``"inverse_volatility"``).

        Returns
        -------
        str
            The stored label from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def allocations(self) -> list[dict[str, object]]:
        """
        Per-strategy allocation rows as records.

        Returns
        -------
        list[dict[str, object]]
            JSON-shaped view of the canonical Rust field.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def diagnostics(self) -> dict[str, object]:
        """
        Portfolio-level diagnostics (``weights_sum``, ``leverage``).

        Returns
        -------
        dict[str, object]
            JSON-shaped view of the canonical Rust field.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Per-strategy allocations as a pandas DataFrame.

        Columns: ``id``, ``weight``, ``capital``, ``volatility``, ``risk_contribution``. The schema is pinned, so an empty result keeps the
        same dtypes as a populated one.

        Returns
        -------
        pd.DataFrame
            One row per entry, with the pinned column schema above.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this result to canonical JSON.

        Returns
        -------
        str
            Canonical JSON wire document, byte-identical to the paired
            ``*_json`` function's output for the same inputs.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @staticmethod
    def from_json(json: str) -> WeightAllocationResult:
        """
        Deserialize a ``WeightAllocationResult`` from JSON produced by :meth:`to_json`.

        Completes the wire round-trip, which is also what makes this type
        picklable.

        Parameters
        ----------
        json : str
            Canonical JSON produced by :meth:`to_json`.

        Returns
        -------
        WeightAllocationResult
            Validated instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If ``json`` is malformed or does not match the serialized schema.

        Examples
        --------
        >>> from finstack_quant.portfolio import WeightAllocationResult, allocate_weights
        >>> spec = '{"scheme":"equal","total_capital":1000.0,"strategies":[{"id":"a"},{"id":"b"}]}'
        >>> result = allocate_weights(spec)
        >>> WeightAllocationResult.from_json(result.to_json()).to_json() == result.to_json()
        True
        """
        ...

def allocate_weights(spec_json: str | dict[str, Any] | list[Any] | pd.DataFrame) -> WeightAllocationResult:
    """
    Allocate strategy weights from a JSON specification.

    The specification contains a ``scheme`` (for example
    ``"inverse_volatility"``), ``total_capital``, and a list of strategy
    objects with ``id`` and return/history fields required by the selected
    scheme. The Rust allocator computes normalized weights and money amounts;
    Python only passes the JSON through.

    Parameters
    ----------
    spec_json : str | dict | list | pandas.DataFrame
        JSON-serialized allocation specification.

    Returns
    -------
    WeightAllocationResult
        Typed result with ``scheme``, ``allocations`` and ``diagnostics``
        getters plus ``to_dataframe()``; use :func:`allocate_weights_json`
        for the raw wire string.

    Raises
    ------
    ValueError
        If the JSON is malformed, required fields are missing, the
        scheme is unsupported, or the selected scheme cannot be evaluated.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import allocate_weights
    >>> spec = '{"scheme":"equal","total_capital":1000.0,"strategies":[{"id":"a"},{"id":"b"}]}'
    >>> [entry["weight"] for entry in allocate_weights(spec).allocations]
    [0.5, 0.5]
    """
    ...

def allocate_weights_json(spec_json: str | dict[str, Any] | list[Any] | pd.DataFrame) -> str:
    """
    Allocate strategy weights from a JSON specification as wire JSON.

    Wire twin of :func:`allocate_weights`: same input and validation,
    returning the canonical JSON string instead of the typed wrapper.

    Parameters
    ----------
    spec_json : str | dict | list | pandas.DataFrame
        JSON-serialized ``WeightAllocationSpec`` selecting the scheme,
        strategy inputs, and any covariance data.

    Returns
    -------
    str
        JSON-serialized ``WeightAllocationResult`` wire document.

    Raises
    ------
    ValueError
        If the JSON is malformed, required fields are missing, the scheme is
        unsupported, or the selected scheme cannot be evaluated.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import allocate_weights_json
    >>> spec = '{"scheme":"equal","total_capital":1000.0,"strategies":[{"id":"a"},{"id":"b"}]}'
    >>> [entry["weight"] for entry in json.loads(allocate_weights_json(spec))["allocations"]]
    [0.5, 0.5]
    """
    ...

def validate_allocation_json(spec_json: str | dict[str, Any] | list[Any] | pd.DataFrame) -> str:
    """
    Validate a strategy allocation JSON specification.

    Performs the same Rust-side parse and semantic validation used by
    :func:`allocate_weights` without computing allocations.

    Parameters
    ----------
    spec_json : str | dict | list | pandas.DataFrame
        JSON-serialized allocation specification.

    Returns
    -------
    str
        Canonical compact JSON of the accepted specification.

    Raises
    ------
    ValueError
        If the specification is malformed or invalid.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import validate_allocation_json
    >>> spec = '{"scheme":"equal","total_capital":1000.0,"strategies":[{"id":"a"},{"id":"b"}]}'
    >>> json.loads(validate_allocation_json(spec))["scheme"]
    'equal'
    """
    ...

class ReplayResult:
    """
    Full output of a historical portfolio replay run.

    Returned by :func:`replay_portfolio`. Carries per-step valuations, the
    aggregate summary, and any best-effort skipped snapshots.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import ReplayResult
    >>> money = {"amount": "0", "currency": "USD"}
    >>> doc = {
    ...     "steps": [],
    ...     "summary": {
    ...         "start_date": "2025-01-15",
    ...         "end_date": "2025-01-15",
    ...         "num_steps": 0,
    ...         "start_value": money,
    ...         "end_value": money,
    ...         "total_mtm_pnl": money,
    ...         "max_mtm_drawdown": money,
    ...         "max_mtm_drawdown_pct": 0.0,
    ...         "max_mtm_drawdown_peak_date": "2025-01-15",
    ...         "max_mtm_drawdown_trough_date": "2025-01-15",
    ...     },
    ... }
    >>> ReplayResult.from_json(json.dumps(doc)).summary["num_steps"]
    0
    """

    @staticmethod
    def from_json(json: str) -> ReplayResult:
        """
        Deserialize a replay result from canonical JSON.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json` or :func:`replay_portfolio_json`.

        Returns
        -------
        ReplayResult
            Reconstructed result.

        Raises
        ------
        ValueError
            If the payload is malformed.

        Examples
        --------
        >>> from finstack_quant.portfolio import ReplayResult
        >>> try:
        ...     ReplayResult.from_json("{}")
        ... except ValueError:
        ...     print("missing fields")
        missing fields
        """
        ...

    @property
    def steps(self) -> list[dict[str, object]]:
        """
        Per-step output as records, full valuations included.

        Returns
        -------
        list[dict[str, object]]
            JSON-shaped view of the canonical Rust field.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def summary(self) -> dict[str, object]:
        """
        Aggregate statistics across the full replay.

        Returns
        -------
        dict[str, object]
            JSON-shaped view of the canonical Rust field.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def skipped_dates(self) -> list[list[object]]:
        """
        Best-effort skipped snapshots as ``(date, reason)`` pairs.

        Returns
        -------
        list[list[object]]
            ``[date, reason]`` pairs; empty for strict-mode runs.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Per-step value and P&L ladder as a pandas DataFrame.

        Columns: ``date``, ``value``, ``daily_mtm_pnl``, ``cumulative_mtm_pnl``. The schema is pinned, so an empty result keeps the
        same dtypes as a populated one.

        Returns
        -------
        pd.DataFrame
            One row per entry, with the pinned column schema above.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this result to canonical JSON.

        Returns
        -------
        str
            Canonical JSON wire document, byte-identical to the paired
            ``*_json`` function's output for the same inputs.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

def replay_portfolio(
    portfolio: Portfolio | str,
    snapshots: list[tuple[datetime.date | str, MarketContext | str]] | list[dict[str, Any]] | str,
    config: dict[str, Any] | str | None = None,
    mode: str | None = None,
) -> ReplayResult:
    """
    Replay a portfolio through dated market snapshots.

    Parameters
    ----------
    portfolio : Portfolio or str
        Typed :class:`Portfolio` or JSON ``PortfolioSpec``.
    snapshots : list[tuple[date, MarketContext | str]] | list[dict] | str
        Dated market snapshots in ascending order: ``(date, market)`` pairs
        (dates as ``datetime.date`` or ISO strings), JSON-shaped
        ``{"date": "YYYY-MM-DD", "market": {...}}`` dicts, or the canonical
        JSON array string.
    config : dict | str | None
        ``ReplayConfig`` as a dict or JSON string (``mode``,
        ``attribution_method``, ``valuation_options``, ``on_error``).
    mode : str | None
        Shorthand for ``config={"mode": mode}``: ``"pv_only"``,
        ``"pv_and_pnl"`` or ``"full_attribution"``. Ignored when ``config``
        is given.

    Returns
    -------
    ReplayResult
        Typed result with ``steps``, ``summary`` and ``skipped_dates``
        getters plus ``to_dataframe()``; use :func:`replay_portfolio_json`
        for the raw wire string.

    Raises
    ------
    ValueError
        If neither ``config`` nor ``mode`` is supplied, or the snapshots or
        config are malformed (an empty timeline is rejected).
    PortfolioError
        If a snapshot valuation fails under a strict error policy.

    Examples
    --------
    >>> from finstack_quant.portfolio import replay_portfolio
    >>> spec = '{"id":"empty","base_currency":"USD","as_of":"2025-01-01","entities":{},"positions":[]}'
    >>> try:
    ...     replay_portfolio(spec, [], mode="pv_only")
    ... except ValueError as exc:
    ...     print("must be non-empty" in str(exc))
    True
    """
    ...

def replay_portfolio_json(
    portfolio: Portfolio | str,
    snapshots: list[tuple[datetime.date | str, MarketContext | str]] | list[dict[str, Any]] | str,
    config: dict[str, Any] | str | None = None,
    mode: str | None = None,
) -> str:
    """
    Replay a portfolio through dated market snapshots and return wire JSON.

    Wire twin of :func:`replay_portfolio`: same inputs and validation,
    returning the canonical JSON string instead of the typed wrapper.

    Parameters
    ----------
    portfolio : Portfolio or str
        Typed :class:`Portfolio` or JSON ``PortfolioSpec``.
    snapshots : list[tuple[date, MarketContext | str]] | list[dict] | str
        Dated market snapshots (see :func:`replay_portfolio`).
    config : dict | str | None
        ``ReplayConfig`` as a dict or JSON string.
    mode : str | None
        Shorthand for ``config={"mode": mode}``.

    Returns
    -------
    str
        JSON-serialized ``ReplayResult`` wire document.

    Raises
    ------
    ValueError
        If neither ``config`` nor ``mode`` is supplied or an input is
        malformed.
    PortfolioError
        If a snapshot valuation fails under a strict error policy.

    Examples
    --------
    >>> from finstack_quant.portfolio import replay_portfolio_json
    >>> spec = '{"id":"empty","base_currency":"USD","as_of":"2025-01-01","entities":{},"positions":[]}'
    >>> try:
    ...     replay_portfolio_json(spec, "[]", '{"mode":"pv_only"}')
    ... except ValueError as exc:
    ...     print("must be non-empty" in str(exc))
    True
    """
    ...

class BrinsonPeriodResult:
    """
    Single-period Brinson-Fachler attribution result.

    Returned by :func:`brinson_fachler`. Sector effects preserve the input
    sector order; the three effect totals sum to
    :attr:`total_excess_return`.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import brinson_fachler
    >>> sectors = [
    ...     {
    ...         "sector": "A",
    ...         "portfolio_weight": 1.0,
    ...         "benchmark_weight": 1.0,
    ...         "portfolio_return": 0.02,
    ...         "benchmark_return": 0.01,
    ...     }
    ... ]
    >>> brinson_fachler(json.dumps(sectors)).total_excess_return
    0.01
    """

    @property
    def sectors(self) -> list[dict[str, object]]:
        """
        Per-sector effects as records, in the order supplied.

        Returns
        -------
        list[dict[str, object]]
            JSON-shaped view of the canonical Rust field.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def total_allocation(self) -> float:
        """
        Sum of allocation effects across sectors.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def total_selection(self) -> float:
        """
        Sum of selection effects across sectors.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def total_interaction(self) -> float:
        """
        Sum of interaction effects across sectors.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def portfolio_return(self) -> float:
        """
        Portfolio total return for the period.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def benchmark_return(self) -> float:
        """
        Benchmark total return for the period.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def total_excess_return(self) -> float:
        """
        Active return; equals the sum of the three effect totals.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Per-sector effects as a pandas DataFrame.

        Columns: ``sector``, ``allocation``, ``selection``, ``interaction``, ``total``. The schema is pinned, so an empty result keeps the
        same dtypes as a populated one.

        Returns
        -------
        pd.DataFrame
            One row per entry, with the pinned column schema above.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this result to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation, suitable for a matching
            :meth:`from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @staticmethod
    def from_json(json: str) -> BrinsonPeriodResult:
        """
        Deserialize a ``BrinsonPeriodResult`` from canonical JSON.

        Parameters
        ----------
        json : str
            Canonical payload produced by :meth:`to_json` or the paired
            ``*_json`` wire function.

        Returns
        -------
        BrinsonPeriodResult
            Validated instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or does not match the serialized
            ``BrinsonPeriodResult`` schema.

        Examples
        --------
        >>> from finstack_quant.portfolio import BrinsonPeriodResult
        >>> doc = '{"sectors": [], "total_allocation": 0.0, "total_selection": 0.0, "total_interaction": 0.0, "portfolio_return": 0.0, "benchmark_return": 0.0, "total_excess_return": 0.0}'
        >>> BrinsonPeriodResult.from_json(doc).total_excess_return
        0.0
        """
        ...

class CarinoLinkedAttribution:
    """
    Carino-linked multi-period Brinson attribution result.

    Returned by :func:`carino_link`. The linked per-sector effects sum to the
    geometrically compounded active return exactly.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import carino_link
    >>> period = [
    ...     {
    ...         "sector": "A",
    ...         "portfolio_weight": 1.0,
    ...         "benchmark_weight": 1.0,
    ...         "portfolio_return": 0.02,
    ...         "benchmark_return": 0.01,
    ...     }
    ... ]
    >>> result = carino_link(json.dumps([period, period]))
    >>> round(result.linked_selection, 4)
    0.0203
    """

    @property
    def periods(self) -> list[dict[str, object]]:
        """
        Per-period decompositions as records, in chronological order.

        Returns
        -------
        list[dict[str, object]]
            JSON-shaped view of the canonical Rust field.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def portfolio_return_compounded(self) -> float:
        """
        Geometrically compounded portfolio return.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def benchmark_return_compounded(self) -> float:
        """
        Geometrically compounded benchmark return.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def linked_sectors(self) -> list[dict[str, object]]:
        """
        Per-sector Carino-smoothed effects summed across periods.

        Returns
        -------
        list[dict[str, object]]
            JSON-shaped view of the canonical Rust field.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def linked_allocation(self) -> float:
        """
        Sum of per-sector linked allocation effects.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def linked_selection(self) -> float:
        """
        Sum of per-sector linked selection effects.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def linked_interaction(self) -> float:
        """
        Sum of per-sector linked interaction effects.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Linked per-sector effects as a pandas DataFrame.

        Columns: ``sector``, ``allocation``, ``selection``, ``interaction``, ``total``. The schema is pinned, so an empty result keeps the
        same dtypes as a populated one.

        Returns
        -------
        pd.DataFrame
            One row per entry, with the pinned column schema above.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this result to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation, suitable for a matching
            :meth:`from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @staticmethod
    def from_json(json: str) -> CarinoLinkedAttribution:
        """
        Deserialize a ``CarinoLinkedAttribution`` from canonical JSON.

        Parameters
        ----------
        json : str
            Canonical payload produced by :meth:`to_json` or the paired
            ``*_json`` wire function.

        Returns
        -------
        CarinoLinkedAttribution
            Validated instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or does not match the serialized
            ``CarinoLinkedAttribution`` schema.

        Examples
        --------
        >>> from finstack_quant.portfolio import CarinoLinkedAttribution
        >>> doc = '{"periods": [], "portfolio_return_compounded": 0.0, "benchmark_return_compounded": 0.0, "linked_sectors": [], "linked_allocation": 0.0, "linked_selection": 0.0, "linked_interaction": 0.0}'
        >>> CarinoLinkedAttribution.from_json(doc).linked_allocation
        0.0
        """
        ...

def brinson_fachler(sectors_json: str | dict[str, Any] | list[Any] | pd.DataFrame) -> BrinsonPeriodResult:
    """
    Compute single-period Brinson-Fachler attribution from sector JSON.

    Parameters
    ----------
    sectors_json : str | dict | list | pandas.DataFrame
        JSON array of ``SectorPeriod`` objects with ``sector``,
        ``portfolio_weight``, ``benchmark_weight``, ``portfolio_return``, and
        ``benchmark_return`` fields. Returns are simple decimal returns for the
        period (e.g. ``0.02`` for +2%).

    Returns
    -------
    BrinsonPeriodResult
        Typed result with per-sector effects, effect totals, and
        ``to_dataframe()`` / ``to_json()`` exits; use
        :func:`brinson_fachler_json` for the raw wire string.

    Raises
    ------
    PortfolioError
        If sector weights do not sum to one or returns are invalid.
    ValueError
        If ``sectors_json`` is malformed.

    Sources
    -------
    See ``docs/REFERENCES.md#brinson-fachler-1985``.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import brinson_fachler
    >>> sectors = [
    ...     {
    ...         "sector": "A",
    ...         "portfolio_weight": 1.0,
    ...         "benchmark_weight": 1.0,
    ...         "portfolio_return": 0.02,
    ...         "benchmark_return": 0.01,
    ...     }
    ... ]
    >>> brinson_fachler(json.dumps(sectors)).total_excess_return
    0.01
    """
    ...

def brinson_fachler_json(sectors_json: str | dict[str, Any] | list[Any] | pd.DataFrame) -> str:
    """
    Compute single-period Brinson-Fachler attribution and return wire JSON.

    Wire twin of :func:`brinson_fachler`: same inputs and validation,
    returning the canonical JSON string instead of the typed wrapper.

    Parameters
    ----------
    sectors_json : str | dict | list | pandas.DataFrame
        JSON array of ``SectorPeriod`` objects; same schema as
        :func:`brinson_fachler`.

    Returns
    -------
    str
        JSON-serialized ``BrinsonPeriodResult`` wire document.

    Raises
    ------
    PortfolioError
        If sector weights do not sum to one or returns are invalid.
    ValueError
        If ``sectors_json`` is malformed.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import brinson_fachler_json
    >>> sectors = [
    ...     {
    ...         "sector": "A",
    ...         "portfolio_weight": 1.0,
    ...         "benchmark_weight": 1.0,
    ...         "portfolio_return": 0.02,
    ...         "benchmark_return": 0.01,
    ...     }
    ... ]
    >>> json.loads(brinson_fachler_json(json.dumps(sectors)))["total_excess_return"]
    0.01
    """
    ...

def carino_link(periods_json: str | dict[str, Any] | list[Any] | pd.DataFrame) -> CarinoLinkedAttribution:
    """
    Compute Carino-linked multi-period Brinson attribution from period JSON.

    Parameters
    ----------
    periods_json : str | dict | list | pandas.DataFrame
        JSON array of periods, where each period is an array of ``SectorPeriod``
        objects (same schema as :func:`brinson_fachler`).

    Returns
    -------
    CarinoLinkedAttribution
        Typed result with linked per-sector effects and compounded returns;
        use :func:`carino_link_json` for the raw wire string.

    Raises
    ------
    PortfolioError
        If any period fails Brinson validation.
    ValueError
        If ``periods_json`` is malformed.

    Sources
    -------
    See ``docs/REFERENCES.md#carino-1999``.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import carino_link
    >>> period = [
    ...     {
    ...         "sector": "A",
    ...         "portfolio_weight": 1.0,
    ...         "benchmark_weight": 1.0,
    ...         "portfolio_return": 0.02,
    ...         "benchmark_return": 0.01,
    ...     }
    ... ]
    >>> result = carino_link(json.dumps([period, period]))
    >>> round(result.linked_selection, 4)
    0.0203
    """
    ...

class FiAttributionResult:
    """
    Single-period Campisi benchmark-relative attribution result.

    Returned by :func:`campisi_attribution`. The five effect totals sum to
    :attr:`active_return`; sector effects preserve first-seen order.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import campisi_attribution
    >>> snap = [
    ...     {
    ...         "sector": "GOVT",
    ...         "weight": 1.0,
    ...         "total_return": 0.02,
    ...         "yield_annual": 0.04,
    ...         "modified_duration": 5.0,
    ...         "spread_duration": 0.0,
    ...         "spread": 0.0,
    ...         "delta_treasury_yield": -0.001,
    ...         "delta_spread": 0.0,
    ...     }
    ... ]
    >>> result = campisi_attribution(json.dumps(snap), json.dumps(snap), '{"period_years":0.25}')
    >>> round(result.active_return, 6)
    0.0
    """

    @property
    def sectors(self) -> list[dict[str, object]]:
        """
        Per-sector benchmark-relative effects as records.

        Returns
        -------
        list[dict[str, object]]
            JSON-shaped view of the canonical Rust field.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def portfolio_components(self) -> dict[str, object]:
        """
        Portfolio-side absolute Campisi split (carry, treasury, spread, selection, total).

        Returns
        -------
        dict[str, object]
            JSON-shaped view of the canonical Rust field.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def benchmark_components(self) -> dict[str, object]:
        """
        Benchmark-side absolute Campisi split (carry, treasury, spread, selection, total).

        Returns
        -------
        dict[str, object]
            JSON-shaped view of the canonical Rust field.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def portfolio_return(self) -> float:
        """
        Portfolio total return for the period.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def benchmark_return(self) -> float:
        """
        Benchmark total return for the period.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def active_return(self) -> float:
        """
        Active return; portfolio minus benchmark total return.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def total_allocation(self) -> float:
        """
        Sum of sector allocation effects.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def total_active_carry(self) -> float:
        """
        Sum of sector active carry effects.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def total_active_treasury(self) -> float:
        """
        Sum of sector active treasury effects.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def total_active_spread(self) -> float:
        """
        Sum of sector active spread effects.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def total_selection(self) -> float:
        """
        Sum of sector selection effects.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Per-sector effects as a pandas DataFrame.

        Columns: ``sector``, ``portfolio_weight``, ``benchmark_weight``, ``portfolio_return``, ``benchmark_return``, ``allocation``, ``active_carry``, ``active_treasury``, ``active_spread``, ``selection``, ``total_active``. The schema is pinned, so an empty result keeps the
        same dtypes as a populated one.

        Returns
        -------
        pd.DataFrame
            One row per entry, with the pinned column schema above.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this result to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation, suitable for a matching
            :meth:`from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @staticmethod
    def from_json(json: str) -> FiAttributionResult:
        """
        Deserialize a ``FiAttributionResult`` from canonical JSON.

        Parameters
        ----------
        json : str
            Canonical payload produced by :meth:`to_json` or the paired
            ``*_json`` wire function.

        Returns
        -------
        FiAttributionResult
            Validated instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or does not match the serialized
            ``FiAttributionResult`` schema.

        Examples
        --------
        >>> from finstack_quant.portfolio import FiAttributionResult
        >>> doc = '{"sectors": [], "portfolio_components": {"carry": 0.0, "treasury": 0.0, "spread": 0.0, "selection": 0.0, "total": 0.0}, "benchmark_components": {"carry": 0.0, "treasury": 0.0, "spread": 0.0, "selection": 0.0, "total": 0.0}, "portfolio_return": 0.0, "benchmark_return": 0.0, "active_return": 0.0, "total_allocation": 0.0, "total_active_carry": 0.0, "total_active_treasury": 0.0, "total_active_spread": 0.0, "total_selection": 0.0}'
        >>> FiAttributionResult.from_json(doc).active_return
        0.0
        """
        ...

class FiCarinoLinkedResult:
    """
    Multi-period Carino-linked Campisi attribution result.

    Returned by :func:`campisi_carino_link` and
    :func:`campisi_carino_link_from_snapshots`. The five linked totals sum to
    the geometrically compounded active return.

    Examples
    --------
    >>> from finstack_quant.portfolio import FiCarinoLinkedResult
    >>> doc = (
    ...     '{"periods": [], "portfolio_return_compounded": 0.0404,'
    ...     ' "benchmark_return_compounded": 0.0201, "linked_sectors": [],'
    ...     ' "linked_allocation": 0.0, "linked_active_carry": 0.0,'
    ...     ' "linked_active_treasury": 0.0, "linked_active_spread": 0.0,'
    ...     ' "linked_selection": 0.0}'
    ... )
    >>> FiCarinoLinkedResult.from_json(doc).portfolio_return_compounded
    0.0404
    """

    @property
    def periods(self) -> list[dict[str, object]]:
        """
        Per-period single-period results as records, in chronological order.

        Returns
        -------
        list[dict[str, object]]
            JSON-shaped view of the canonical Rust field.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def portfolio_return_compounded(self) -> float:
        """
        Geometrically compounded portfolio return.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def benchmark_return_compounded(self) -> float:
        """
        Geometrically compounded benchmark return.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def linked_sectors(self) -> list[dict[str, object]]:
        """
        Per-sector linked effects as records.

        Returns
        -------
        list[dict[str, object]]
            JSON-shaped view of the canonical Rust field.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def linked_allocation(self) -> float:
        """
        Sum of linked allocation effects.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def linked_active_carry(self) -> float:
        """
        Sum of linked active carry effects.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def linked_active_treasury(self) -> float:
        """
        Sum of linked active treasury effects.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def linked_active_spread(self) -> float:
        """
        Sum of linked active spread effects.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def linked_selection(self) -> float:
        """
        Sum of linked selection effects.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Linked per-sector effects as a pandas DataFrame.

        Columns: ``sector``, ``allocation``, ``active_carry``, ``active_treasury``, ``active_spread``, ``selection``, ``total_active``. The schema is pinned, so an empty result keeps the
        same dtypes as a populated one.

        Returns
        -------
        pd.DataFrame
            One row per entry, with the pinned column schema above.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this result to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation, suitable for a matching
            :meth:`from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @staticmethod
    def from_json(json: str) -> FiCarinoLinkedResult:
        """
        Deserialize a ``FiCarinoLinkedResult`` from canonical JSON.

        Parameters
        ----------
        json : str
            Canonical payload produced by :meth:`to_json` or the paired
            ``*_json`` wire function.

        Returns
        -------
        FiCarinoLinkedResult
            Validated instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or does not match the serialized
            ``FiCarinoLinkedResult`` schema.

        Examples
        --------
        >>> from finstack_quant.portfolio import FiCarinoLinkedResult
        >>> doc = '{"periods": [], "portfolio_return_compounded": 0.0, "benchmark_return_compounded": 0.0, "linked_sectors": [], "linked_allocation": 0.0, "linked_active_carry": 0.0, "linked_active_treasury": 0.0, "linked_active_spread": 0.0, "linked_selection": 0.0}'
        >>> FiCarinoLinkedResult.from_json(doc).linked_selection
        0.0
        """
        ...

class FiReconciliationReport:
    """
    Reconciliation report for the five Campisi effect totals.

    Returned by :func:`campisi_reconciliation_check`. A floating-point sanity
    gate, not a model check: the Campisi decomposition reconciles by
    construction because selection is the residual.

    Examples
    --------
    >>> from finstack_quant.portfolio import FiReconciliationReport
    >>> doc = '{"total_residual": 0.0, "is_reconciled": true, "tolerance": 1e-10}'
    >>> FiReconciliationReport.from_json(doc).is_reconciled
    True
    """

    @property
    def total_residual(self) -> float:
        """
        Active return minus the sum of the five effect totals.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def is_reconciled(self) -> bool:
        """
        Whether the residual is within tolerance.

        Returns
        -------
        bool
            The stored flag from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def tolerance(self) -> float:
        """
        Absolute tolerance that was applied, in return units.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Single-row pandas DataFrame view of the report.

        Columns: ``total_residual``, ``is_reconciled``, ``tolerance``. The schema is pinned, so an empty result keeps the
        same dtypes as a populated one.

        Returns
        -------
        pd.DataFrame
            One row per entry, with the pinned column schema above.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this result to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation, suitable for a matching
            :meth:`from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @staticmethod
    def from_json(json: str) -> FiReconciliationReport:
        """
        Deserialize a ``FiReconciliationReport`` from canonical JSON.

        Parameters
        ----------
        json : str
            Canonical payload produced by :meth:`to_json` or the paired
            ``*_json`` wire function.

        Returns
        -------
        FiReconciliationReport
            Validated instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or does not match the serialized
            ``FiReconciliationReport`` schema.

        Examples
        --------
        >>> from finstack_quant.portfolio import FiReconciliationReport
        >>> doc = '{"total_residual": 0.0, "is_reconciled": true, "tolerance": 1e-10}'
        >>> FiReconciliationReport.from_json(doc).is_reconciled
        True
        """
        ...

def carino_link_json(periods_json: str | dict[str, Any] | list[Any] | pd.DataFrame) -> str:
    """
    Compute Carino-linked multi-period Brinson attribution and return wire JSON.

    Wire twin of :func:`carino_link`: same inputs and validation, returning
    the canonical JSON string instead of the typed wrapper.

    Parameters
    ----------
    periods_json : str | dict | list | pandas.DataFrame
        JSON array of periods of ``SectorPeriod`` objects; same schema as
        :func:`carino_link`.

    Returns
    -------
    str
        JSON-serialized ``CarinoLinkedAttribution`` wire document.

    Raises
    ------
    PortfolioError
        If any period fails Brinson validation.
    ValueError
        If ``periods_json`` is malformed.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import carino_link_json
    >>> period = [
    ...     {
    ...         "sector": "A",
    ...         "portfolio_weight": 1.0,
    ...         "benchmark_weight": 1.0,
    ...         "portfolio_return": 0.02,
    ...         "benchmark_return": 0.01,
    ...     }
    ... ]
    >>> result = json.loads(carino_link_json(json.dumps([period, period])))
    >>> round(result["linked_selection"], 4)
    0.0203
    """
    ...

def campisi_attribution(
    portfolio_json: str | dict[str, Any] | list[Any] | pd.DataFrame,
    benchmark_json: str | dict[str, Any] | list[Any] | pd.DataFrame,
    config_json: str | dict[str, Any] | list[Any] | pd.DataFrame,
) -> FiAttributionResult:
    """
    Compute single-period Campisi fixed-income benchmark attribution.

    Binds Rust ``finstack_quant_portfolio::campisi_attribution``.

    Parameters
    ----------
    portfolio_json : str | dict | list | pandas.DataFrame
        JSON array of ``FiPositionSnapshot`` objects with ``sector``,
        ``weight``, ``total_return``, ``yield_annual``, ``modified_duration``,
        ``spread_duration``, ``spread``, ``delta_treasury_yield``, and
        ``delta_spread`` fields (decimals; durations in years). Unknown fields
        are rejected and weights must sum to one. ``spread_duration`` must be
        the canonical quote-reproducing Z-spread duration; ``spread`` and
        ``delta_spread`` must use the matching ``z_spread`` basis. OAS,
        G-spread, and discount-margin values are incompatible. The direct JSON
        shape has no metric-ID provenance, so this binding cannot detect a
        mislabeled numeric spread basis.
    benchmark_json : str | dict | list | pandas.DataFrame
        JSON array of ``FiPositionSnapshot`` objects for the benchmark, subject
        to the same quote-reproducing Z-spread basis contract.
    config_json : str | dict | list | pandas.DataFrame
        JSON object whose only field is ``period_years`` (e.g. ``0.25``). It is
        required — there is no default — and unknown keys are rejected. The
        spread convention is not configurable: the Campisi spread effect
        ``-SD * delta_spread`` is identical under the absolute and
        Duration-Times-Spread conventions, so a mode switch could only relabel
        an identical result.

    Returns
    -------
    FiAttributionResult
        Typed result with per-sector effects, the five effect totals, and
        ``to_dataframe()`` / ``to_json()`` exits; use
        :func:`campisi_attribution_json` for the raw wire string.

    Raises
    ------
    PortfolioError
        If weights do not sum to one, inputs are non-finite, ``period_years``
        is not finite and positive, either side is empty, or a sector present
        on either side has ``|net sector weight| <= 1e-6 * gross absolute
        sector weight``. The spread *level* is unconstrained: zero and negative
        Z-spreads are accepted, because the spread effect never divides by the
        level. Spread-basis provenance cannot be validated from the numeric JSON
        shape.
    ValueError
        If any JSON argument is malformed.

    Sources
    -------
    See ``docs/REFERENCES.md#campisi-2000``.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import campisi_attribution
    >>> portfolio = [
    ...     {
    ...         "sector": "GOVT",
    ...         "weight": 1.0,
    ...         "total_return": 0.02,
    ...         "yield_annual": 0.04,
    ...         "modified_duration": 5.0,
    ...         "spread_duration": 0.0,
    ...         "spread": 0.0,
    ...         "delta_treasury_yield": -0.001,
    ...         "delta_spread": 0.0,
    ...     }
    ... ]
    >>> benchmark = [
    ...     {
    ...         "sector": "GOVT",
    ...         "weight": 1.0,
    ...         "total_return": 0.01,
    ...         "yield_annual": 0.03,
    ...         "modified_duration": 4.0,
    ...         "spread_duration": 0.0,
    ...         "spread": 0.0,
    ...         "delta_treasury_yield": -0.001,
    ...         "delta_spread": 0.0,
    ...     }
    ... ]
    >>> result = campisi_attribution(json.dumps(portfolio), json.dumps(benchmark), '{"period_years":0.25}')
    >>> round(result.active_return, 4)
    0.01
    """
    ...

def campisi_attribution_json(
    portfolio_json: str | dict[str, Any] | list[Any] | pd.DataFrame,
    benchmark_json: str | dict[str, Any] | list[Any] | pd.DataFrame,
    config_json: str | dict[str, Any] | list[Any] | pd.DataFrame,
) -> str:
    """
    Compute single-period Campisi attribution and return wire JSON.

    Wire twin of :func:`campisi_attribution`: same inputs and validation,
    returning the canonical JSON string instead of the typed wrapper.

    Parameters
    ----------
    portfolio_json : str | dict | list | pandas.DataFrame
        JSON array of ``FiPositionSnapshot`` objects; same schema and
        Z-spread basis contract as :func:`campisi_attribution`.
    benchmark_json : str | dict | list | pandas.DataFrame
        JSON array of ``FiPositionSnapshot`` objects for the benchmark side.
    config_json : str | dict | list | pandas.DataFrame
        JSON ``FiAttributionConfig`` whose only field is ``period_years``.

    Returns
    -------
    str
        JSON-serialized ``FiAttributionResult`` wire document.

    Raises
    ------
    PortfolioError
        If the canonical Rust validation rejects the inputs (same conditions
        as :func:`campisi_attribution`).
    ValueError
        If any JSON argument is malformed.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import campisi_attribution_json
    >>> snap = [
    ...     {
    ...         "sector": "GOVT",
    ...         "weight": 1.0,
    ...         "total_return": 0.02,
    ...         "yield_annual": 0.04,
    ...         "modified_duration": 5.0,
    ...         "spread_duration": 0.0,
    ...         "spread": 0.0,
    ...         "delta_treasury_yield": -0.001,
    ...         "delta_spread": 0.0,
    ...     }
    ... ]
    >>> result = json.loads(campisi_attribution_json(json.dumps(snap), json.dumps(snap), '{"period_years":0.25}'))
    >>> round(result["active_return"], 6)
    0.0
    """
    ...

def campisi_carino_link(periods_json: str | dict[str, Any] | list[Any] | pd.DataFrame) -> FiCarinoLinkedResult:
    """
    Carino-link already-computed single-period Campisi attribution results.

    Binds Rust ``finstack_quant_portfolio::campisi_carino_link``.

    Each period result already carries its own applied ``period_years``, so
    periods of *different* lengths link correctly here (e.g. act/365 calendar
    months: 31/365, 28/365, 31/365). Use this entry point for real day counts;
    :func:`campisi_carino_link_from_snapshots` applies one shared
    ``period_years`` to every period and is only correct for equal-length
    periods.

    Parameters
    ----------
    periods_json : str | dict | list | pandas.DataFrame
        JSON array of ``FiAttributionResult`` objects in chronological order,
        each the parsed output of :func:`campisi_attribution_json` (or
        ``FiAttributionResult.to_json()``). Every period
        must carry the same sector ordering. Unknown fields are rejected.

    Returns
    -------
    FiCarinoLinkedResult
        Typed result whose five linked totals sum to the geometrically
        compounded active return; use :func:`campisi_carino_link_json` for
        the raw wire string.

    Raises
    ------
    PortfolioError
        If ``periods_json`` is empty; sector orderings differ; a consumed
        top-level return/effect, per-sector linked effect, or sector
        ``total_active`` is non-finite; ``active_return`` disagrees with the
        portfolio-minus-benchmark return; a sector ``total_active`` disagrees
        with its five effects; sector effects do not reconcile to their
        declared top-level totals; the five totals do not reconcile to
        ``active_return`` within the overflow-safe scaled-L1 tolerance; a
        reconciliation residual is non-finite; or a return is at or below
        -100% (outside the Carino domain).
    ValueError
        If ``periods_json`` is malformed or does not match the
        ``FiAttributionResult`` schema.

    Sources
    -------
    See ``docs/REFERENCES.md#carino-1999``.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import campisi_attribution_json, campisi_carino_link
    >>> portfolio = [
    ...     {
    ...         "sector": "GOVT",
    ...         "weight": 1.0,
    ...         "total_return": 0.02,
    ...         "yield_annual": 0.04,
    ...         "modified_duration": 5.0,
    ...         "spread_duration": 0.0,
    ...         "spread": 0.0,
    ...         "delta_treasury_yield": -0.001,
    ...         "delta_spread": 0.0,
    ...     }
    ... ]
    >>> benchmark = [
    ...     {
    ...         "sector": "GOVT",
    ...         "weight": 1.0,
    ...         "total_return": 0.01,
    ...         "yield_annual": 0.03,
    ...         "modified_duration": 4.0,
    ...         "spread_duration": 0.0,
    ...         "spread": 0.0,
    ...         "delta_treasury_yield": -0.001,
    ...         "delta_spread": 0.0,
    ...     }
    ... ]
    >>> period = json.loads(
    ...     campisi_attribution_json(json.dumps(portfolio), json.dumps(benchmark), '{"period_years":0.25}')
    ... )
    >>> linked = campisi_carino_link(json.dumps([period, period]))
    >>> round(linked.linked_selection, 6)
    0.013195
    """
    ...

def campisi_carino_link_json(periods_json: str | dict[str, Any] | list[Any] | pd.DataFrame) -> str:
    """
    Carino-link single-period Campisi results and return wire JSON.

    Wire twin of :func:`campisi_carino_link`: same inputs and validation,
    returning the canonical JSON string instead of the typed wrapper.

    Parameters
    ----------
    periods_json : str | dict | list | pandas.DataFrame
        JSON array of ``FiAttributionResult`` objects in chronological
        order; same schema as :func:`campisi_carino_link`.

    Returns
    -------
    str
        JSON-serialized ``FiCarinoLinkedResult`` wire document.

    Raises
    ------
    PortfolioError
        If linking validation rejects the periods (same conditions as
        :func:`campisi_carino_link`).
    ValueError
        If ``periods_json`` is malformed or does not match the
        ``FiAttributionResult`` schema.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import campisi_attribution_json, campisi_carino_link_json
    >>> snap = [
    ...     {
    ...         "sector": "GOVT",
    ...         "weight": 1.0,
    ...         "total_return": 0.02,
    ...         "yield_annual": 0.04,
    ...         "modified_duration": 5.0,
    ...         "spread_duration": 0.0,
    ...         "spread": 0.0,
    ...         "delta_treasury_yield": -0.001,
    ...         "delta_spread": 0.0,
    ...     }
    ... ]
    >>> period = json.loads(campisi_attribution_json(json.dumps(snap), json.dumps(snap), '{"period_years":0.25}'))
    >>> linked = json.loads(campisi_carino_link_json(json.dumps([period, period])))
    >>> round(linked["linked_selection"], 6)
    0.0
    """
    ...

def campisi_carino_link_from_snapshots(
    periods_json: str | dict[str, Any] | list[Any] | pd.DataFrame,
    config_json: str | dict[str, Any] | list[Any] | pd.DataFrame,
) -> FiCarinoLinkedResult:
    """
    Compute Carino-linked multi-period Campisi attribution from period JSON.

    Binds Rust ``finstack_quant_portfolio::campisi_carino_link_from_snapshots``.

    One ``config_json`` — and therefore one ``period_years`` — is applied to
    every period, so this entry point is only correct when all periods have the
    same length. For mixed-length periods (real day counts), compute each
    period with :func:`campisi_attribution` and link the results with
    :func:`campisi_carino_link`.

    Parameters
    ----------
    periods_json : str | dict | list | pandas.DataFrame
        JSON array of period objects, each with ``portfolio`` and
        ``benchmark`` arrays of ``FiPositionSnapshot`` (same schema as
        :func:`campisi_attribution`).
    config_json : str | dict | list | pandas.DataFrame
        JSON ``FiAttributionConfig`` shared across periods; ``period_years`` is
        its only field and is required (no default).

    Returns
    -------
    FiCarinoLinkedResult
        Typed result whose five linked totals sum to the geometrically
        compounded active return; use
        :func:`campisi_carino_link_from_snapshots_json` for the raw wire
        string.

    Raises
    ------
    PortfolioError
        If ``periods_json`` is empty, any period fails Campisi validation, or
        sector orderings differ across periods.
    ValueError
        If any JSON argument is malformed.

    Sources
    -------
    See ``docs/REFERENCES.md#carino-1999``.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import campisi_carino_link_from_snapshots
    >>> portfolio = [
    ...     {
    ...         "sector": "GOVT",
    ...         "weight": 1.0,
    ...         "total_return": 0.02,
    ...         "yield_annual": 0.04,
    ...         "modified_duration": 5.0,
    ...         "spread_duration": 0.0,
    ...         "spread": 0.0,
    ...         "delta_treasury_yield": -0.001,
    ...         "delta_spread": 0.0,
    ...     }
    ... ]
    >>> benchmark = [
    ...     {
    ...         "sector": "GOVT",
    ...         "weight": 1.0,
    ...         "total_return": 0.01,
    ...         "yield_annual": 0.03,
    ...         "modified_duration": 4.0,
    ...         "spread_duration": 0.0,
    ...         "spread": 0.0,
    ...         "delta_treasury_yield": -0.001,
    ...         "delta_spread": 0.0,
    ...     }
    ... ]
    >>> periods = json.dumps([{"portfolio": portfolio, "benchmark": benchmark}])
    >>> result = campisi_carino_link_from_snapshots(periods, '{"period_years":0.25}')
    >>> round(result.linked_selection, 4)
    0.0065
    """
    ...

def campisi_carino_link_from_snapshots_json(
    periods_json: str | dict[str, Any] | list[Any] | pd.DataFrame,
    config_json: str | dict[str, Any] | list[Any] | pd.DataFrame,
) -> str:
    """
    Compute snapshot-level Carino-linked Campisi attribution as wire JSON.

    Wire twin of :func:`campisi_carino_link_from_snapshots`: same inputs and
    validation, returning the canonical JSON string instead of the typed
    wrapper.

    Parameters
    ----------
    periods_json : str | dict | list | pandas.DataFrame
        JSON array of period objects with ``portfolio`` and ``benchmark``
        snapshot arrays; same schema as
        :func:`campisi_carino_link_from_snapshots`.
    config_json : str | dict | list | pandas.DataFrame
        JSON ``FiAttributionConfig`` shared across periods.

    Returns
    -------
    str
        JSON-serialized ``FiCarinoLinkedResult`` wire document.

    Raises
    ------
    PortfolioError
        If any period fails Campisi validation or sector orderings differ.
    ValueError
        If any JSON argument is malformed.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import campisi_carino_link_from_snapshots_json
    >>> snap = [
    ...     {
    ...         "sector": "GOVT",
    ...         "weight": 1.0,
    ...         "total_return": 0.02,
    ...         "yield_annual": 0.04,
    ...         "modified_duration": 5.0,
    ...         "spread_duration": 0.0,
    ...         "spread": 0.0,
    ...         "delta_treasury_yield": -0.001,
    ...         "delta_spread": 0.0,
    ...     }
    ... ]
    >>> periods = json.dumps([{"portfolio": snap, "benchmark": snap}])
    >>> result = json.loads(campisi_carino_link_from_snapshots_json(periods, '{"period_years":0.25}'))
    >>> round(result["linked_selection"], 6)
    0.0
    """
    ...

def campisi_reconciliation_check(
    result_json: str | dict[str, Any] | list[Any] | pd.DataFrame, tolerance: float
) -> FiReconciliationReport:
    """
    Reconcile the five Campisi effect totals against the active return.

    Binds the Rust method
    ``finstack_quant_portfolio::FiAttributionResult::reconciliation_check``
    (a method in Rust, exposed here as a JSON function on the namespace).

    The Campisi decomposition reconciles by construction because selection is
    the residual, so this is a floating-point sanity gate rather than a model
    check. Without it callers must re-sum ``total_allocation``,
    ``total_active_carry``, ``total_active_treasury``, ``total_active_spread``
    and ``total_selection`` by hand.

    Parameters
    ----------
    result_json : str | dict | list | pandas.DataFrame
        JSON ``FiAttributionResult``, as returned by
        :func:`campisi_attribution_json` (or
        ``FiAttributionResult.to_json()``). Unknown fields are rejected.
    tolerance : float
        Absolute tolerance in return units; ``1e-10`` is appropriate for
        return-space values.

    Returns
    -------
    FiReconciliationReport
        Typed report with ``total_residual`` (``active_return`` minus the
        five totals), ``is_reconciled`` and the applied ``tolerance``; use
        :func:`campisi_reconciliation_check_json` for the raw wire string.

    Raises
    ------
    ValueError
        If ``result_json`` is malformed or carries unknown fields.

    Sources
    -------
    See ``docs/REFERENCES.md#campisi-2000``.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import campisi_attribution_json, campisi_reconciliation_check
    >>> portfolio = [
    ...     {
    ...         "sector": "GOVT",
    ...         "weight": 1.0,
    ...         "total_return": 0.02,
    ...         "yield_annual": 0.04,
    ...         "modified_duration": 5.0,
    ...         "spread_duration": 0.0,
    ...         "spread": 0.0,
    ...         "delta_treasury_yield": -0.001,
    ...         "delta_spread": 0.0,
    ...     }
    ... ]
    >>> benchmark = [
    ...     {
    ...         "sector": "GOVT",
    ...         "weight": 1.0,
    ...         "total_return": 0.01,
    ...         "yield_annual": 0.03,
    ...         "modified_duration": 4.0,
    ...         "spread_duration": 0.0,
    ...         "spread": 0.0,
    ...         "delta_treasury_yield": -0.001,
    ...         "delta_spread": 0.0,
    ...     }
    ... ]
    >>> result = campisi_attribution_json(json.dumps(portfolio), json.dumps(benchmark), '{"period_years":0.25}')
    >>> campisi_reconciliation_check(result, 1e-10).is_reconciled
    True
    """
    ...

def campisi_reconciliation_check_json(
    result_json: str | dict[str, Any] | list[Any] | pd.DataFrame, tolerance: float
) -> str:
    """
    Reconcile the five Campisi effect totals and return wire JSON.

    Wire twin of :func:`campisi_reconciliation_check`: same inputs and
    validation, returning the canonical JSON string instead of the typed
    report.

    Parameters
    ----------
    result_json : str | dict | list | pandas.DataFrame
        JSON ``FiAttributionResult``, as returned by
        :func:`campisi_attribution_json`.
    tolerance : float
        Absolute tolerance in return units; ``1e-10`` is appropriate for
        return-space values.

    Returns
    -------
    str
        JSON-serialized ``FiReconciliationReport`` wire document.

    Raises
    ------
    ValueError
        If ``result_json`` is malformed or carries unknown fields.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import campisi_attribution_json, campisi_reconciliation_check_json
    >>> snap = [
    ...     {
    ...         "sector": "GOVT",
    ...         "weight": 1.0,
    ...         "total_return": 0.02,
    ...         "yield_annual": 0.04,
    ...         "modified_duration": 5.0,
    ...         "spread_duration": 0.0,
    ...         "spread": 0.0,
    ...         "delta_treasury_yield": -0.001,
    ...         "delta_spread": 0.0,
    ...     }
    ... ]
    >>> result = campisi_attribution_json(json.dumps(snap), json.dumps(snap), '{"period_years":0.25}')
    >>> json.loads(campisi_reconciliation_check_json(result, 1e-10))["is_reconciled"]
    True
    """
    ...

class DurationCellTable:
    """
    Duration-cell base-return table.

    Returned by :func:`cell_returns_from_reference` and
    :func:`cell_returns_from_curves`; pass ``to_json()`` to
    :func:`excess_returns` as the base table.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import cell_returns_from_reference
    >>> reference = [{"duration": 1.0, "total_return": 0.02}]
    >>> table = cell_returns_from_reference(json.dumps(reference), "UST", '{"width":2.0}')
    >>> table.cells[0]["base_return"]
    0.02
    """

    @property
    def base_label(self) -> str:
        """
        Label identifying the reference universe (e.g. ``"UST"``).

        Returns
        -------
        str
            The stored label from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def cells(self) -> list[dict[str, object]]:
        """
        Cells as records, in ascending duration order.

        Returns
        -------
        list[dict[str, object]]
            JSON-shaped view of the canonical Rust field.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Cells as a pandas DataFrame.

        Columns: ``label``, ``lower``, ``upper``, ``base_return``, ``observed``. The schema is pinned, so an empty result keeps the
        same dtypes as a populated one.

        Returns
        -------
        pd.DataFrame
            One row per entry, with the pinned column schema above.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this result to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation, suitable for a matching
            :meth:`from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @staticmethod
    def from_json(json: str) -> DurationCellTable:
        """
        Deserialize a ``DurationCellTable`` from canonical JSON.

        Parameters
        ----------
        json : str
            Canonical payload produced by :meth:`to_json` or the paired
            ``*_json`` wire function.

        Returns
        -------
        DurationCellTable
            Validated instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or does not match the serialized
            ``DurationCellTable`` schema.

        Examples
        --------
        >>> from finstack_quant.portfolio import DurationCellTable
        >>> doc = '{"base_label": "UST", "cells": []}'
        >>> DurationCellTable.from_json(doc).base_label
        'UST'
        """
        ...

class ExcessReturnResult:
    """
    Per-position and portfolio-level duration-matched credit excess returns.

    Returned by :func:`excess_returns`.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import cell_returns_from_reference, excess_returns
    >>> reference = [{"duration": 1.0, "total_return": 0.02}]
    >>> table = cell_returns_from_reference(json.dumps(reference), "UST", '{"width":2.0}')
    >>> positions = [{"id": "B1", "weight": 1.0, "duration": 1.0, "total_return": 0.03}]
    >>> result = excess_returns(json.dumps(positions), table.to_json())
    >>> round(result.portfolio_excess_return, 4)
    0.01
    """

    @property
    def base_label(self) -> str:
        """
        Label of the base curve the excess returns were measured against.

        Returns
        -------
        str
            The stored label from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def positions(self) -> list[dict[str, object]]:
        """
        Per-position results as records, in input order.

        Returns
        -------
        list[dict[str, object]]
            JSON-shaped view of the canonical Rust field.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def portfolio_total_return(self) -> float:
        """
        Weight-weighted portfolio total return.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def portfolio_base_return(self) -> float:
        """
        Weight-weighted portfolio base return.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def portfolio_excess_return(self) -> float:
        """
        Weight-weighted portfolio excess return.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Per-position excess returns as a pandas DataFrame.

        Columns: ``id``, ``cell``, ``base_return``, ``excess_return``. The schema is pinned, so an empty result keeps the
        same dtypes as a populated one.

        Returns
        -------
        pd.DataFrame
            One row per entry, with the pinned column schema above.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this result to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation, suitable for a matching
            :meth:`from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @staticmethod
    def from_json(json: str) -> ExcessReturnResult:
        """
        Deserialize a ``ExcessReturnResult`` from canonical JSON.

        Parameters
        ----------
        json : str
            Canonical payload produced by :meth:`to_json` or the paired
            ``*_json`` wire function.

        Returns
        -------
        ExcessReturnResult
            Validated instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or does not match the serialized
            ``ExcessReturnResult`` schema.

        Examples
        --------
        >>> from finstack_quant.portfolio import ExcessReturnResult
        >>> doc = '{"base_label": "UST", "positions": [], "portfolio_total_return": 0.0, "portfolio_base_return": 0.0, "portfolio_excess_return": 0.0}'
        >>> ExcessReturnResult.from_json(doc).base_label
        'UST'
        """
        ...

def cell_returns_from_reference(
    reference_json: str | dict[str, Any] | list[Any] | pd.DataFrame,
    base_label: str,
    config_json: str | dict[str, Any] | list[Any] | pd.DataFrame,
) -> DurationCellTable:
    """
    Build a duration-cell base-return table from a reference universe.

    Binds Rust ``finstack_quant_portfolio::cell_returns_from_reference``
    (Dynkin, Hyman & Vankudre 1998, Appendix B): buckets ``reference_json``
    into fixed-width duration cells and averages each cell's member total
    returns, interpolating interior gaps and flat-extrapolating
    leading/trailing gaps.

    Parameters
    ----------
    reference_json : str | dict | list | pandas.DataFrame
        JSON array of ``ReferenceReturn`` objects (``duration``,
        ``total_return``, both decimals with duration in years); must be
        non-empty. Unknown fields are rejected.
    base_label : str
        Label identifying the resulting curve (e.g. ``"UST"``), carried
        through to the output's ``base_label`` for policy visibility.
    config_json : str | dict | list | pandas.DataFrame
        JSON ``CellConfig``; ``width`` is its only field (cell width in
        years, finite and positive) and is required — there is no default.

    Returns
    -------
    DurationCellTable
        Typed cell table with ``to_dataframe()`` / ``to_json()`` exits; use
        :func:`cell_returns_from_reference_json` for the raw wire string.

    Raises
    ------
    PortfolioError
        If ``reference_json`` is empty, ``config.width`` is not finite and
        positive, any reference entry has a non-finite ``total_return`` or a
        non-finite/negative ``duration``, the width produces two numerically
        distinct cells that collide on their one-decimal label, or the
        largest reference ``duration`` divided by ``config.width`` would
        require more than an internal sanity bound of cells (100,000; a
        units mistake, e.g. days instead of years, rather than a legitimate
        duration grid).
    ValueError
        If any JSON argument is malformed or carries unknown fields.

    Sources
    -------
    See ``docs/REFERENCES.md#dynkin-hyman-vankudre-1998``.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import cell_returns_from_reference
    >>> reference = [{"duration": 1.0, "total_return": 0.02}]
    >>> table = cell_returns_from_reference(json.dumps(reference), "UST", '{"width":2.0}')
    >>> table.cells[0]["base_return"]
    0.02
    """
    ...

def cell_returns_from_reference_json(
    reference_json: str | dict[str, Any] | list[Any] | pd.DataFrame,
    base_label: str,
    config_json: str | dict[str, Any] | list[Any] | pd.DataFrame,
) -> str:
    """
    Build a reference-universe duration-cell table and return wire JSON.

    Wire twin of :func:`cell_returns_from_reference`: same inputs and
    validation, returning the canonical JSON string instead of the typed
    table.

    Parameters
    ----------
    reference_json : str | dict | list | pandas.DataFrame
        JSON array of ``ReferenceReturn`` objects; same schema as
        :func:`cell_returns_from_reference`.
    base_label : str
        Label identifying the resulting curve (e.g. ``"UST"``).
    config_json : str | dict | list | pandas.DataFrame
        JSON ``CellConfig``; ``width`` is its only field and is required.

    Returns
    -------
    str
        JSON-serialized ``DurationCellTable`` wire document.

    Raises
    ------
    PortfolioError
        If the canonical Rust validation rejects the inputs (same conditions
        as :func:`cell_returns_from_reference`).
    ValueError
        If any JSON argument is malformed or carries unknown fields.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import cell_returns_from_reference_json
    >>> reference = [{"duration": 1.0, "total_return": 0.02}]
    >>> table = json.loads(cell_returns_from_reference_json(json.dumps(reference), "UST", '{"width":2.0}'))
    >>> table["cells"][0]["base_return"]
    0.02
    """
    ...

def cell_returns_from_curves(
    start: DiscountCurve,
    end: DiscountCurve,
    horizon_years: float,
    max_duration: float,
    base_label: str,
    config_json: str | dict[str, Any] | list[Any] | pd.DataFrame,
) -> DurationCellTable:
    """
    Build a duration-cell base-return table from start/end discount curves.

    Binds Rust ``finstack_quant_portfolio::cell_returns_from_curves``: each
    cell's base return is the holding-period return of a hypothetical
    zero-coupon position bought at the cell midpoint off ``start`` and
    revalued off ``end`` after ``horizon_years`` have elapsed. Every
    resulting cell is observed (a curve has a discount factor at every
    queried point), unlike the reference-universe path in
    :func:`cell_returns_from_reference`.

    Parameters
    ----------
    start : DiscountCurve
        Discount curve observed at the start of the holding period.
    end : DiscountCurve
        Discount curve observed ``horizon_years`` later, at period end.
    horizon_years : float
        Length of the holding period, in years; must be finite and positive.
    max_duration : float
        Upper bound of the duration grid, in years; must be finite and
        strictly greater than ``horizon_years``.
    base_label : str
        Label identifying the base curve (e.g. ``"UST"``, ``"USD-SOFR"``),
        stamped into the result purely for policy visibility.
    config_json : str | dict | list | pandas.DataFrame
        JSON ``CellConfig``; ``width`` is its only field and is required.

    Returns
    -------
    DurationCellTable
        Typed cell table with ``to_dataframe()`` / ``to_json()`` exits; use
        :func:`cell_returns_from_curves_json` for the raw wire string.

    Raises
    ------
    PortfolioError
        If ``config.width`` or ``horizon_years`` is not finite and positive,
        ``max_duration`` does not strictly exceed ``horizon_years``,
        ``max_duration`` divided by ``config.width`` would require more than
        an internal sanity bound of cells (100,000; a units mistake rather
        than a legitimate duration grid), a cell's midpoint does not exceed
        ``horizon_years`` (unavoidable whenever ``config.width`` is not
        strictly greater than ``2 * horizon_years``, since the first cell's
        midpoint is ``config.width / 2``), either curve produces a
        non-positive/non-finite discount factor at a queried time, or the
        width produces colliding one-decimal cell labels.
    ValueError
        If ``config_json`` is malformed.

    Sources
    -------
    See ``docs/REFERENCES.md#dynkin-hyman-vankudre-1998``.

    Examples
    --------
    >>> from datetime import date
    >>> import json
    >>> from finstack_quant.core.market_data import DiscountCurve
    >>> from finstack_quant.portfolio import cell_returns_from_curves
    >>> start = DiscountCurve.flat("start", date(2025, 1, 1), 0.02)
    >>> end = DiscountCurve.flat("end", date(2025, 4, 1), 0.03)
    >>> table = cell_returns_from_curves(start, end, 0.25, 2.0, "UST", '{"width":1.0}')
    >>> len(table.cells)
    2
    """
    ...

def cell_returns_from_curves_json(
    start: DiscountCurve,
    end: DiscountCurve,
    horizon_years: float,
    max_duration: float,
    base_label: str,
    config_json: str | dict[str, Any] | list[Any] | pd.DataFrame,
) -> str:
    """
    Build a curve-snapshot duration-cell table and return wire JSON.

    Wire twin of :func:`cell_returns_from_curves`: same inputs and
    validation, returning the canonical JSON string instead of the typed
    table.

    Parameters
    ----------
    start : DiscountCurve
        Discount curve observed at the start of the holding period.
    end : DiscountCurve
        Discount curve observed ``horizon_years`` later, at period end.
    horizon_years : float
        Length of the holding period, in years; finite and positive.
    max_duration : float
        Upper bound of the duration grid, in years; strictly greater than
        ``horizon_years``.
    base_label : str
        Label identifying the base curve, stamped for policy visibility.
    config_json : str | dict | list | pandas.DataFrame
        JSON ``CellConfig``; ``width`` is its only field and is required.

    Returns
    -------
    str
        JSON-serialized ``DurationCellTable`` wire document.

    Raises
    ------
    PortfolioError
        If the canonical Rust validation rejects the inputs (same conditions
        as :func:`cell_returns_from_curves`).
    ValueError
        If ``config_json`` is malformed.

    Examples
    --------
    >>> from datetime import date
    >>> import json
    >>> from finstack_quant.core.market_data import DiscountCurve
    >>> from finstack_quant.portfolio import cell_returns_from_curves_json
    >>> start = DiscountCurve.flat("start", date(2025, 1, 1), 0.02)
    >>> end = DiscountCurve.flat("end", date(2025, 4, 1), 0.03)
    >>> table = json.loads(cell_returns_from_curves_json(start, end, 0.25, 2.0, "UST", '{"width":1.0}'))
    >>> len(table["cells"])
    2
    """
    ...

def excess_returns(
    positions_json: str | dict[str, Any] | list[Any] | pd.DataFrame,
    table_json: str | dict[str, Any] | list[Any] | pd.DataFrame,
) -> ExcessReturnResult:
    """
    Compute duration-matched credit excess returns against a base-return table.

    Binds Rust ``finstack_quant_portfolio::excess_returns`` (Dynkin, Hyman &
    Vankudre 1998, Appendix B): each position's ``duration`` is matched to
    its duration cell in ``table_json`` and the position's excess return is
    ``total_return - cell.base_return``, the credit-specific component of
    performance isolated from the general level/shape move of the base curve.

    Parameters
    ----------
    positions_json : str | dict | list | pandas.DataFrame
        JSON array of ``ExcessReturnPosition`` objects (``id``, ``weight``,
        ``duration``, ``total_return``); weights must sum to ``1.0`` within
        ``1e-6``.
    table_json : str | dict | list | pandas.DataFrame
        JSON ``DurationCellTable``, as returned by
        :func:`cell_returns_from_reference_json`,
        :func:`cell_returns_from_curves_json`, or
        ``DurationCellTable.to_json()``.

    Returns
    -------
    ExcessReturnResult
        Typed result with per-position and portfolio-level
        total/base/excess returns; use :func:`excess_returns_json` for the
        raw wire string.

    Raises
    ------
    PortfolioError
        If ``table_json`` has no cells; a cell has an empty or duplicate
        label, non-finite bounds or base return, a negative lower bound, a
        non-positive width, non-ascending lower bounds, or overlaps its
        predecessor; a position has a non-finite
        weight/duration/total_return; a position's duration falls outside
        every cell, including a valid gap (the error names the position); or
        the position weights do not sum to ``1.0`` within tolerance.
    ValueError
        If any JSON argument is malformed.

    Sources
    -------
    See ``docs/REFERENCES.md#dynkin-hyman-vankudre-1998``.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import cell_returns_from_reference, excess_returns
    >>> reference = [{"duration": 1.0, "total_return": 0.02}]
    >>> table = cell_returns_from_reference(json.dumps(reference), "UST", '{"width":2.0}')
    >>> positions = [{"id": "B1", "weight": 1.0, "duration": 1.0, "total_return": 0.03}]
    >>> result = excess_returns(json.dumps(positions), table.to_json())
    >>> round(result.portfolio_excess_return, 4)
    0.01
    """
    ...

def excess_returns_json(
    positions_json: str | dict[str, Any] | list[Any] | pd.DataFrame,
    table_json: str | dict[str, Any] | list[Any] | pd.DataFrame,
) -> str:
    """
    Compute duration-matched credit excess returns and return wire JSON.

    Wire twin of :func:`excess_returns`: same inputs and validation,
    returning the canonical JSON string instead of the typed wrapper.

    Parameters
    ----------
    positions_json : str | dict | list | pandas.DataFrame
        JSON array of ``ExcessReturnPosition`` objects; same schema as
        :func:`excess_returns`.
    table_json : str | dict | list | pandas.DataFrame
        JSON ``DurationCellTable`` base-return table.

    Returns
    -------
    str
        JSON-serialized ``ExcessReturnResult`` wire document.

    Raises
    ------
    PortfolioError
        If the canonical Rust validation rejects the table or positions
        (same conditions as :func:`excess_returns`).
    ValueError
        If any JSON argument is malformed.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import cell_returns_from_reference_json, excess_returns_json
    >>> reference = [{"duration": 1.0, "total_return": 0.02}]
    >>> table = cell_returns_from_reference_json(json.dumps(reference), "UST", '{"width":2.0}')
    >>> positions = [{"id": "B1", "weight": 1.0, "duration": 1.0, "total_return": 0.03}]
    >>> result = json.loads(excess_returns_json(json.dumps(positions), table))
    >>> round(result["portfolio_excess_return"], 4)
    0.01
    """
    ...

class GridAttributionResult:
    """
    Single-period hierarchical duration-cell x sector grid attribution result.

    Returned by :func:`grid_attribution`. ``total_curve``, ``total_sector``
    and ``total_selection`` sum to :attr:`active_return` to floating-point
    precision for well-conditioned inputs.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import grid_attribution
    >>> portfolio = [{"cell": "0-3", "sector": "GOVT", "weight": 1.0, "total_return": 0.02}]
    >>> benchmark = [{"cell": "0-3", "sector": "GOVT", "weight": 1.0, "total_return": 0.01}]
    >>> result = grid_attribution(json.dumps(portfolio), json.dumps(benchmark))
    >>> round(result.total_selection, 4)
    0.01
    """

    @property
    def portfolio_return(self) -> float:
        """
        Portfolio total return for the period.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def benchmark_return(self) -> float:
        """
        Benchmark total return for the period.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def active_return(self) -> float:
        """
        Active return; portfolio minus benchmark total return.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def curve_effects(self) -> list[dict[str, object]]:
        """
        Per-cell curve effects as records, in first-appearance order.

        Returns
        -------
        list[dict[str, object]]
            JSON-shaped view of the canonical Rust field.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def sector_effects(self) -> list[dict[str, object]]:
        """
        Per-(cell, sector) allocation effects as records.

        Returns
        -------
        list[dict[str, object]]
            JSON-shaped view of the canonical Rust field.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def selection_effects(self) -> list[dict[str, object]]:
        """
        Per-(cell, sector) selection effects as records, aligned with ``sector_effects``.

        Returns
        -------
        list[dict[str, object]]
            JSON-shaped view of the canonical Rust field.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def total_curve(self) -> float:
        """
        Sum of the per-cell curve effects.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def total_sector(self) -> float:
        """
        Sum of the sector allocation effects.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def total_selection(self) -> float:
        """
        Sum of the selection effects.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Per-cell curve effects as a pandas DataFrame.

        Columns: ``cell``, ``portfolio_weight``, ``benchmark_weight``, ``benchmark_cell_return``, ``curve_effect``. The schema is pinned, so an empty result keeps the
        same dtypes as a populated one.
        The primary frame is the duration-cell axis; the two (cell,
        sector) tables come from :meth:`to_sector_effects_dataframe` and
        :meth:`to_selection_effects_dataframe`.

        Returns
        -------
        pd.DataFrame
            One row per entry, with the pinned column schema above.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_sector_effects_dataframe(self) -> pd.DataFrame:
        """
        Per-(cell, sector) allocation effects as a pandas DataFrame.

        Columns: ``cell``, ``sector``, ``allocation_effect``. The schema is pinned, so an empty result keeps the
        same dtypes as a populated one.

        Returns
        -------
        pd.DataFrame
            One row per entry, with the pinned column schema above.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_selection_effects_dataframe(self) -> pd.DataFrame:
        """
        Per-(cell, sector) selection effects as a pandas DataFrame.

        Columns: ``cell``, ``sector``, ``selection_effect``. The schema is pinned, so an empty result keeps the
        same dtypes as a populated one.

        Returns
        -------
        pd.DataFrame
            One row per entry, with the pinned column schema above.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this result to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation, suitable for a matching
            :meth:`from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @staticmethod
    def from_json(json: str) -> GridAttributionResult:
        """
        Deserialize a ``GridAttributionResult`` from canonical JSON.

        Parameters
        ----------
        json : str
            Canonical payload produced by :meth:`to_json` or the paired
            ``*_json`` wire function.

        Returns
        -------
        GridAttributionResult
            Validated instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or does not match the serialized
            ``GridAttributionResult`` schema.

        Examples
        --------
        >>> from finstack_quant.portfolio import GridAttributionResult
        >>> doc = '{"portfolio_return": 0.0, "benchmark_return": 0.0, "active_return": 0.0, "curve_effects": [], "sector_effects": [], "selection_effects": [], "total_curve": 0.0, "total_sector": 0.0, "total_selection": 0.0}'
        >>> GridAttributionResult.from_json(doc).active_return
        0.0
        """
        ...

class GridCarinoLinkedResult:
    """
    Multi-period Carino-linked hierarchical grid attribution result.

    Returned by :func:`grid_carino_link`. Only the three top-level effects
    are linked; their sum reconstructs the compounded active return exactly.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import grid_attribution_json, grid_carino_link
    >>> portfolio = [{"cell": "0-3", "sector": "GOVT", "weight": 1.0, "total_return": 0.02}]
    >>> benchmark = [{"cell": "0-3", "sector": "GOVT", "weight": 1.0, "total_return": 0.01}]
    >>> period = json.loads(grid_attribution_json(json.dumps(portfolio), json.dumps(benchmark)))
    >>> result = grid_carino_link(json.dumps([period, period]))
    >>> round(result.linked_selection, 4)
    0.0203
    """

    @property
    def periods(self) -> list[dict[str, object]]:
        """
        Per-period single-period results as records, in chronological order.

        Returns
        -------
        list[dict[str, object]]
            JSON-shaped view of the canonical Rust field.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def portfolio_return_compounded(self) -> float:
        """
        Geometrically compounded portfolio return.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def benchmark_return_compounded(self) -> float:
        """
        Geometrically compounded benchmark return.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def linked_curve(self) -> float:
        """
        Sum of per-period Carino-scaled curve effects.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def linked_sector(self) -> float:
        """
        Sum of per-period Carino-scaled sector allocation effects.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def linked_selection(self) -> float:
        """
        Sum of per-period Carino-scaled selection effects.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Single-row pandas DataFrame view of the linked totals.

        Columns: ``portfolio_return_compounded``, ``benchmark_return_compounded``, ``linked_curve``, ``linked_sector``, ``linked_selection``. The schema is pinned, so an empty result keeps the
        same dtypes as a populated one.

        Returns
        -------
        pd.DataFrame
            One row per entry, with the pinned column schema above.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this result to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation, suitable for a matching
            :meth:`from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @staticmethod
    def from_json(json: str) -> GridCarinoLinkedResult:
        """
        Deserialize a ``GridCarinoLinkedResult`` from canonical JSON.

        Parameters
        ----------
        json : str
            Canonical payload produced by :meth:`to_json` or the paired
            ``*_json`` wire function.

        Returns
        -------
        GridCarinoLinkedResult
            Validated instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or does not match the serialized
            ``GridCarinoLinkedResult`` schema.

        Examples
        --------
        >>> from finstack_quant.portfolio import GridCarinoLinkedResult
        >>> doc = '{"periods": [], "portfolio_return_compounded": 0.0, "benchmark_return_compounded": 0.0, "linked_curve": 0.0, "linked_sector": 0.0, "linked_selection": 0.0}'
        >>> GridCarinoLinkedResult.from_json(doc).linked_curve
        0.0
        """
        ...

def grid_attribution(
    portfolio_json: str | dict[str, Any] | list[Any] | pd.DataFrame,
    benchmark_json: str | dict[str, Any] | list[Any] | pd.DataFrame,
) -> GridAttributionResult:
    """
    Compute a single-period hierarchical duration-cell x sector grid attribution.

    Binds Rust ``finstack_quant_portfolio::grid_attribution`` (Dynkin, Hyman
    & Vankudre 1998, Appendix A): decomposes active return into a per-cell
    curve (positioning) effect, a within-cell sector allocation effect, and
    a security-selection residual per (cell, sector).

    Parameters
    ----------
    portfolio_json : str | dict | list | pandas.DataFrame
        JSON array of ``GridPosition`` objects (``cell``, ``sector``,
        ``weight``, ``total_return``) for the portfolio side; weights must
        sum to ``1.0`` within ``1e-6``.
    benchmark_json : str | dict | list | pandas.DataFrame
        JSON array of ``GridPosition`` objects for the benchmark side; same
        weight-sum requirement.

    Returns
    -------
    GridAttributionResult
        Typed result whose ``total_curve``,
        ``total_sector`` and ``total_selection`` sum to ``active_return`` to
        floating-point precision for well-conditioned inputs; among
        *accepted* inputs (those that clear the near-zero-net-weight check
        below), the reconciliation residual grows the closer any bucket's
        net weight sits to that check's own rejection boundary — see the
        Rust module docs' measured residuals for magnitudes.

    Raises
    ------
    PortfolioError
        If any weight or return is non-finite, either side's weights don't
        sum to ``1.0`` within tolerance, or a (cell) or (cell, sector)
        bucket has positions on a side but nets to a weight that is zero, or
        numerically near zero (within ``1e-6`` relative to its own gross
        weight), which is undefined-or-explosive to attribute (the error
        names the bucket and the side).
    ValueError
        If either JSON argument is malformed or carries unknown fields.

    Sources
    -------
    See ``docs/REFERENCES.md#dynkin-hyman-vankudre-1998``.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import grid_attribution
    >>> portfolio = [{"cell": "0-3", "sector": "GOVT", "weight": 1.0, "total_return": 0.02}]
    >>> benchmark = [{"cell": "0-3", "sector": "GOVT", "weight": 1.0, "total_return": 0.01}]
    >>> result = grid_attribution(json.dumps(portfolio), json.dumps(benchmark))
    >>> round(result.total_selection, 4)
    0.01
    """
    ...

def grid_attribution_json(
    portfolio_json: str | dict[str, Any] | list[Any] | pd.DataFrame,
    benchmark_json: str | dict[str, Any] | list[Any] | pd.DataFrame,
) -> str:
    """
    Compute a single-period grid attribution and return wire JSON.

    Wire twin of :func:`grid_attribution`: same inputs and validation,
    returning the canonical JSON string instead of the typed wrapper.

    Parameters
    ----------
    portfolio_json : str | dict | list | pandas.DataFrame
        JSON array of ``GridPosition`` objects for the portfolio side; same
        schema as :func:`grid_attribution`.
    benchmark_json : str | dict | list | pandas.DataFrame
        JSON array of ``GridPosition`` objects for the benchmark side.

    Returns
    -------
    str
        JSON-serialized ``GridAttributionResult`` wire document.

    Raises
    ------
    PortfolioError
        If the canonical Rust validation rejects the inputs (same conditions
        as :func:`grid_attribution`).
    ValueError
        If either JSON argument is malformed or carries unknown fields.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import grid_attribution_json
    >>> portfolio = [{"cell": "0-3", "sector": "GOVT", "weight": 1.0, "total_return": 0.02}]
    >>> benchmark = [{"cell": "0-3", "sector": "GOVT", "weight": 1.0, "total_return": 0.01}]
    >>> result = json.loads(grid_attribution_json(json.dumps(portfolio), json.dumps(benchmark)))
    >>> round(result["total_selection"], 4)
    0.01
    """
    ...

def grid_carino_link(periods_json: str | dict[str, Any] | list[Any] | pd.DataFrame) -> GridCarinoLinkedResult:
    """
    Carino-link multi-period hierarchical grid attribution results.

    Binds Rust ``finstack_quant_portfolio::grid_carino_link`` (Carino 1999):
    applies Carino smoothing to a chronological sequence of single-period
    :func:`grid_attribution` results so the three top-level effects
    (``linked_curve``, ``linked_sector``, ``linked_selection``) sum exactly
    to the geometrically compounded active return. Only the three top-level
    effects are linked; per-cell / per-(cell, sector) multi-period linking
    is out of scope.

    Parameters
    ----------
    periods_json : str | dict | list | pandas.DataFrame
        JSON array of ``GridAttributionResult`` objects, in chronological
        order, each the wire output of :func:`grid_attribution_json` (or
        ``GridAttributionResult.to_json()``).

    Returns
    -------
    GridCarinoLinkedResult
        Typed result with the three linked effects and compounded returns;
        use :func:`grid_carino_link_json` for the raw wire string.

    Raises
    ------
    PortfolioError
        If ``periods_json`` is empty; any consumed return or top-level effect
        is non-finite; ``active_return`` disagrees with the portfolio-minus-
        benchmark return; the three effect totals do not reconcile to
        ``active_return`` within an overflow-safe scaled-L1 tolerance; a
        return-identity or reconciliation residual is non-finite; or any
        per-period or compounded return is at or below -100% (outside the
        Carino domain).
    ValueError
        If ``periods_json`` is malformed or does not match the
        ``GridAttributionResult`` schema.

    Sources
    -------
    See ``docs/REFERENCES.md#carino-1999`` and
    ``docs/REFERENCES.md#dynkin-hyman-vankudre-1998``.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import grid_attribution_json, grid_carino_link
    >>> portfolio = [{"cell": "0-3", "sector": "GOVT", "weight": 1.0, "total_return": 0.02}]
    >>> benchmark = [{"cell": "0-3", "sector": "GOVT", "weight": 1.0, "total_return": 0.01}]
    >>> period = json.loads(grid_attribution_json(json.dumps(portfolio), json.dumps(benchmark)))
    >>> result = grid_carino_link(json.dumps([period, period]))
    >>> round(result.linked_selection, 4)
    0.0203
    """
    ...

def grid_carino_link_json(periods_json: str | dict[str, Any] | list[Any] | pd.DataFrame) -> str:
    """
    Carino-link multi-period grid attribution results and return wire JSON.

    Wire twin of :func:`grid_carino_link`: same inputs and validation,
    returning the canonical JSON string instead of the typed wrapper.

    Parameters
    ----------
    periods_json : str | dict | list | pandas.DataFrame
        JSON array of ``GridAttributionResult`` objects in chronological
        order; same schema as :func:`grid_carino_link`.

    Returns
    -------
    str
        JSON-serialized ``GridCarinoLinkedResult`` wire document.

    Raises
    ------
    PortfolioError
        If linking validation rejects the periods (same conditions as
        :func:`grid_carino_link`).
    ValueError
        If ``periods_json`` is malformed or does not match the
        ``GridAttributionResult`` schema.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import grid_attribution_json, grid_carino_link_json
    >>> portfolio = [{"cell": "0-3", "sector": "GOVT", "weight": 1.0, "total_return": 0.02}]
    >>> benchmark = [{"cell": "0-3", "sector": "GOVT", "weight": 1.0, "total_return": 0.01}]
    >>> period = json.loads(grid_attribution_json(json.dumps(portfolio), json.dumps(benchmark)))
    >>> result = json.loads(grid_carino_link_json(json.dumps([period, period])))
    >>> round(result["linked_selection"], 4)
    0.0203
    """
    ...

class FactorBrinsonResult:
    """
    Jeet-Partani factor-Brinson unified attribution result.

    Returned by :func:`factor_brinson_attribution`.
    ``allocation + selection`` reconstructs :attr:`active_return`.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import factor_brinson_attribution
    >>> inputs = {
    ...     "asset_ids": ["A"],
    ...     "asset_returns": [0.02],
    ...     "exposures": [1.0],
    ...     "factor_names": ["Market"],
    ...     "portfolio_weights": [1.0],
    ...     "benchmark_weights": [1.0],
    ... }
    >>> factor_brinson_attribution(json.dumps(inputs), [0.02]).active_return
    0.0
    """

    @property
    def portfolio_return(self) -> float:
        """
        Portfolio total return for the period.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def benchmark_return(self) -> float:
        """
        Benchmark total return for the period.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def active_return(self) -> float:
        """
        Active return; portfolio minus benchmark total return.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def allocation(self) -> float:
        """
        Factor (allocation) contribution to the active return.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def selection(self) -> float:
        """
        Specific (selection) contribution to the active return.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def factor_contributions(self) -> list[dict[str, object]]:
        """
        Per-factor breakdown of ``allocation`` as records.

        Returns
        -------
        list[dict[str, object]]
            JSON-shaped view of the canonical Rust field.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def asset_contributions(self) -> list[dict[str, object]]:
        """
        Per-asset breakdown of ``selection`` as records.

        Returns
        -------
        list[dict[str, object]]
            JSON-shaped view of the canonical Rust field.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Per-factor contributions as a pandas DataFrame.

        Columns: ``factor``, ``active_loading``, ``factor_return``, ``contribution``. The schema is pinned, so an empty result keeps the
        same dtypes as a populated one.
        The primary frame is the factor axis; the per-asset selection
        breakdown comes from :meth:`to_asset_contributions_dataframe`.

        Returns
        -------
        pd.DataFrame
            One row per entry, with the pinned column schema above.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_asset_contributions_dataframe(self) -> pd.DataFrame:
        """
        Per-asset specific contributions as a pandas DataFrame.

        Columns: ``asset``, ``specific_return``, ``active_weight``, ``contribution``. The schema is pinned, so an empty result keeps the
        same dtypes as a populated one.

        Returns
        -------
        pd.DataFrame
            One row per entry, with the pinned column schema above.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this result to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation, suitable for a matching
            :meth:`from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @staticmethod
    def from_json(json: str) -> FactorBrinsonResult:
        """
        Deserialize a ``FactorBrinsonResult`` from canonical JSON.

        Parameters
        ----------
        json : str
            Canonical payload produced by :meth:`to_json` or the paired
            ``*_json`` wire function.

        Returns
        -------
        FactorBrinsonResult
            Validated instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or does not match the serialized
            ``FactorBrinsonResult`` schema.

        Examples
        --------
        >>> from finstack_quant.portfolio import FactorBrinsonResult
        >>> doc = '{"portfolio_return": 0.0, "benchmark_return": 0.0, "active_return": 0.0, "allocation": 0.0, "selection": 0.0, "factor_contributions": [], "asset_contributions": []}'
        >>> FactorBrinsonResult.from_json(doc).allocation
        0.0
        """
        ...

def factor_brinson_attribution(
    input_json: str | dict[str, Any] | list[Any] | pd.DataFrame, factor_returns: list[float]
) -> FactorBrinsonResult:
    """
    Compute Jeet-Partani (2023) factor-Brinson unified attribution.

    Binds Rust ``finstack_quant_portfolio::factor_brinson_attribution``:
    generalizes classical Brinson-Fachler allocation/selection to continuous
    factor exposures by replacing the sector partition with a
    factor-exposure matrix and a caller-supplied benchmark factor-return
    vector.

    Parameters
    ----------
    input_json : str | dict | list | pandas.DataFrame
        JSON ``FactorBrinsonInput`` with ``asset_ids``, ``asset_returns``,
        ``exposures`` (row-major ``n_assets x n_factors``), ``factor_names``,
        ``portfolio_weights`` and ``benchmark_weights``. Each weight vector
        must sum to ``1.0`` within ``1e-6``; weights may be negative (short
        positions).
    factor_returns : list[float]
        Caller-supplied benchmark factor returns ``f_b``, length
        ``input.factor_names``. Typically fit with
        :func:`finstack_quant.analytics.constrained_least_squares` (using
        benchmark weights) so the completeness condition below holds.

    Returns
    -------
    FactorBrinsonResult
        Typed result with ``allocation``, ``selection``, and their
        per-factor / per-asset breakdowns; use
        :func:`factor_brinson_attribution_json` for the raw wire string.

    Raises
    ------
    PortfolioError
        If any array has the wrong length relative to ``n_assets`` /
        ``n_factors``, either ``n_assets`` or ``n_factors`` is zero, any
        input value is non-finite, portfolio or benchmark weights don't sum
        to ``1.0`` within tolerance, or the Jeet-Partani completeness
        residual ``|h_b'eps_b|`` exceeds tolerance — meaning the supplied
        ``factor_returns`` do not fully explain the benchmark return. The
        error directs callers to
        :func:`finstack_quant.analytics.constrained_least_squares`.
    ValueError
        If ``input_json`` is malformed or carries unknown fields.

    Sources
    -------
    See ``docs/REFERENCES.md#jeet-partani-2023``.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import factor_brinson_attribution
    >>> inputs = {
    ...     "asset_ids": ["A"],
    ...     "asset_returns": [0.02],
    ...     "exposures": [1.0],
    ...     "factor_names": ["Market"],
    ...     "portfolio_weights": [1.0],
    ...     "benchmark_weights": [1.0],
    ... }
    >>> factor_brinson_attribution(json.dumps(inputs), [0.02]).active_return
    0.0
    """
    ...

def factor_brinson_attribution_json(
    input_json: str | dict[str, Any] | list[Any] | pd.DataFrame, factor_returns: list[float]
) -> str:
    """
    Compute factor-Brinson unified attribution and return wire JSON.

    Wire twin of :func:`factor_brinson_attribution`: same inputs and
    validation, returning the canonical JSON string instead of the typed
    wrapper.

    Parameters
    ----------
    input_json : str | dict | list | pandas.DataFrame
        JSON ``FactorBrinsonInput``; same schema as
        :func:`factor_brinson_attribution`.
    factor_returns : list[float]
        Caller-supplied benchmark factor returns, length
        ``input.factor_names``.

    Returns
    -------
    str
        JSON-serialized ``FactorBrinsonResult`` wire document.

    Raises
    ------
    PortfolioError
        If the canonical Rust validation rejects the inputs (same conditions
        as :func:`factor_brinson_attribution`).
    ValueError
        If ``input_json`` is malformed or carries unknown fields.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import factor_brinson_attribution_json
    >>> inputs = {
    ...     "asset_ids": ["A"],
    ...     "asset_returns": [0.02],
    ...     "exposures": [1.0],
    ...     "factor_names": ["Market"],
    ...     "portfolio_weights": [1.0],
    ...     "benchmark_weights": [1.0],
    ... }
    >>> json.loads(factor_brinson_attribution_json(json.dumps(inputs), [0.02]))["active_return"]
    0.0
    """
    ...

class LinkedReturn:
    """
    Result of geometrically linking TWRR sub-period returns.

    Returned by :func:`twrr_linked`. For horizons below one year
    :attr:`annualised` mirrors :attr:`cumulative` (GIPS 2020 reports such
    horizons cumulatively).

    Examples
    --------
    >>> from finstack_quant.portfolio import twrr_linked
    >>> result = twrr_linked("[0.05,0.03]", horizon_years=1.0)
    >>> (round(result.cumulative, 4), result.num_periods)
    (0.0815, 2)
    """

    @property
    def cumulative(self) -> float:
        """
        Cumulative return over the full horizon.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def annualised(self) -> float:
        """
        Annualised return; mirrors ``cumulative`` below one year.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def num_periods(self) -> int:
        """
        Number of sub-periods that were linked.

        Returns
        -------
        int
            The stored count from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Single-row pandas DataFrame view of the linked return.

        Columns: ``cumulative``, ``annualised``, ``num_periods``. The schema is pinned, so an empty result keeps the
        same dtypes as a populated one.

        Returns
        -------
        pd.DataFrame
            One row per entry, with the pinned column schema above.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this result to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation, suitable for a matching
            :meth:`from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @staticmethod
    def from_json(json: str) -> LinkedReturn:
        """
        Deserialize a ``LinkedReturn`` from canonical JSON.

        Parameters
        ----------
        json : str
            Canonical payload produced by :meth:`to_json` or the paired
            ``*_json`` wire function.

        Returns
        -------
        LinkedReturn
            Validated instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or does not match the serialized
            ``LinkedReturn`` schema.

        Examples
        --------
        >>> from finstack_quant.portfolio import LinkedReturn
        >>> doc = '{"cumulative": 0.0815, "annualised": 0.0815, "num_periods": 2}'
        >>> LinkedReturn.from_json(doc).num_periods
        2
        """
        ...

def twrr_modified_dietz(
    period: dict[str, Any] | str | None = None,
    *,
    beginning_market_value: float | None = None,
    ending_market_value: float | None = None,
    cashflows: list[tuple[float, float]] | None = None,
) -> float:
    """
    Compute a Modified-Dietz TWRR sub-period return.

    Parameters
    ----------
    period : dict | str | None
        Complete ``TwrrPeriod`` (``beginning_market_value``,
        ``ending_market_value``, ``cashflows: [{amount,
        fraction_of_period_remaining}]``) as a dict or JSON string. Omit it
        to build the period from the keyword arguments instead.
    beginning_market_value : float | None
        PV at period start (used when ``period`` is omitted).
    ending_market_value : float | None
        PV at period end (used when ``period`` is omitted).
    cashflows : list[tuple[float, float]] | None
        External flows as ``(amount, fraction_of_period_remaining)`` pairs:
        positive amount = contribution into the portfolio; the fraction in
        ``[0, 1]`` weights the flow by time remaining. Defaults to none.

    Returns
    -------
    float
        Sub-period time-weighted return as a decimal.

    Raises
    ------
    ValueError
        If the period is malformed, a cashflow weight lies outside ``[0, 1]``,
        the Dietz denominator is non-positive, or neither ``period`` nor both
        market values are supplied.

    Examples
    --------
    >>> from finstack_quant.portfolio import twrr_modified_dietz
    >>> twrr_modified_dietz(beginning_market_value=100.0, ending_market_value=110.0)
    0.1
    >>> twrr_modified_dietz({"beginning_market_value": 100.0, "ending_market_value": 110.0, "cashflows": []})
    0.1
    """
    ...

def twrr_linked(returns_json: str | dict[str, Any] | list[Any] | pd.DataFrame, horizon_years: float) -> LinkedReturn:
    """
    Geometrically link TWRR sub-period returns over a horizon.

    Parameters
    ----------
    returns_json : str | dict | list | pandas.DataFrame
        JSON array of sub-period decimal returns (e.g. from Modified Dietz).
    horizon_years : float
        Reporting horizon in years used to annualize the linked return.

    Returns
    -------
    LinkedReturn
        Typed result with ``cumulative``, ``annualised`` and
        ``num_periods``; use :func:`twrr_linked_json` for the raw wire
        string.

    Raises
    ------
    ValueError
        If ``returns_json`` is malformed, any sub-period return is non-finite
        or at most -1, or the compounded growth factor is non-positive.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import twrr_linked
    >>> result = twrr_linked("[0.05,0.03]", horizon_years=1.0)
    >>> round(result.cumulative, 4)
    0.0815
    """
    ...

def twrr_linked_json(returns_json: str | dict[str, Any] | list[Any] | pd.DataFrame, horizon_years: float) -> str:
    """
    Geometrically link TWRR sub-period returns and return wire JSON.

    Wire twin of :func:`twrr_linked`: same inputs and validation, returning
    the canonical JSON string instead of the typed wrapper.

    Parameters
    ----------
    returns_json : str | dict | list | pandas.DataFrame
        JSON array of sub-period decimal returns.
    horizon_years : float
        Reporting horizon in years used to annualize the linked return.

    Returns
    -------
    str
        JSON-serialized ``LinkedReturn`` wire document.

    Raises
    ------
    ValueError
        If ``returns_json`` is malformed, any sub-period return is
        non-finite or at most -1, or the compounded growth factor is non-positive.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import twrr_linked_json
    >>> result = json.loads(twrr_linked_json("[0.05,0.03]", 1.0))
    >>> round(result["cumulative"], 4)
    0.0815
    """
    ...

def mwr_xirr(
    cashflows: list[tuple[datetime.date | str, float]] | list[dict[str, Any]] | pd.DataFrame | str,
) -> float:
    """
    Compute the money-weighted return (XIRR, Act/365F) from dated cashflows.

    Parameters
    ----------
    cashflows : list[tuple[date, float]] | list[dict] | pd.DataFrame | str
        Dated flows from the investor's cash account: contributions negative,
        terminal value / distributions positive. Accepts ``(date, amount)``
        pairs (dates as ``datetime.date`` or ISO strings), dicts with
        ``date`` and ``amount`` keys, a DataFrame with those columns, or the
        canonical JSON array string.

    Returns
    -------
    float
        Internal rate of return as a decimal annualized rate.

    Raises
    ------
    ValueError
        If the flows are malformed, lack a sign change, or XIRR does not
        converge.

    Examples
    --------
    >>> import datetime as dt
    >>> from finstack_quant.portfolio import mwr_xirr
    >>> round(mwr_xirr([(dt.date(2025, 1, 1), -100.0), (dt.date(2026, 1, 1), 110.0)]), 6)
    0.1
    >>> round(mwr_xirr('[{"date":"2025-01-01","amount":-100.0},{"date":"2026-01-01","amount":110.0}]'), 6)
    0.1
    """
    ...

# factor_model typed result classes (Slice 8)

class FactorContributionDelta:
    """
    Per-factor contribution change between a baseline and a scenario.

    Examples
    --------
    >>> from finstack_quant.portfolio import FactorContributionDelta
    >>> delta = FactorContributionDelta.from_json('{"factor_id":"Rates","absolute_change":0.1,"relative_change":0.2}')
    >>> (delta.factor_id, delta.absolute_change)
    ('Rates', 0.1)
    """

    @classmethod
    def from_json(cls, json_str: str) -> FactorContributionDelta:
        """
        Deserialize a factor contribution delta from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized baseline-to-scenario factor delta, normally
            produced by ``FactorContributionDelta.to_json``.

        Returns
        -------
        FactorContributionDelta
            Validated `FactorContributionDelta` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or cannot be deserialized into the documented portfolio type.

        Examples
        --------
        >>> from finstack_quant.portfolio import FactorContributionDelta
        >>> delta = FactorContributionDelta.from_json(
        ...     '{"factor_id":"Rates","absolute_change":0.1,"relative_change":0.2}'
        ... )
        >>> delta.relative_change
        0.2
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this factor contribution delta to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `FactorContributionDelta`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def factor_id(self) -> str:
        """
        Identifier of the contributing risk factor in the model.

        Returns
        -------
        str
            Identifier of the contributing risk factor in the model.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def absolute_change(self) -> float:
        """
        Absolute contribution change.

        Returns
        -------
        float
            Absolute contribution change.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def relative_change(self) -> float:
        """
        Relative contribution change.

        Returns
        -------
        float
            Relative contribution change.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class WhatIfResult:
    """
    Result of a position what-if scenario.

    Examples
    --------
    >>> from finstack_quant.portfolio import WhatIfResult
    >>> risk = '{"total_risk":1.0,"measure":"variance","factor_contributions":[],"residual_risk":1.0,"position_factor_contributions":[],"position_residual_contributions":[]}'
    >>> result = WhatIfResult.from_json('{"before":' + risk + ',"after":' + risk + ',"delta":[]}')
    >>> result.delta
    []
    """

    @classmethod
    def from_json(cls, json_str: str) -> WhatIfResult:
        """
        Deserialize a what-if result from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized before-and-after risk result, normally
            produced by ``WhatIfResult.to_json``.

        Returns
        -------
        WhatIfResult
            Validated `WhatIfResult` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or cannot be deserialized into the documented portfolio type.

        Examples
        --------
        >>> from finstack_quant.portfolio import WhatIfResult
        >>> risk = '{"total_risk":1.0,"measure":"variance","factor_contributions":[],"residual_risk":1.0,"position_factor_contributions":[],"position_residual_contributions":[]}'
        >>> result = WhatIfResult.from_json('{"before":' + risk + ',"after":' + risk + ',"delta":[]}')
        >>> result.before.total_risk
        1.0
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this what-if result to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `WhatIfResult`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def before(self) -> RiskDecomposition:
        """
        Baseline risk decomposition.
        Returns
        -------
        RiskDecomposition

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def after(self) -> RiskDecomposition:
        """
        Post-scenario risk decomposition.
        Returns
        -------
        RiskDecomposition

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def delta(self) -> list[FactorContributionDelta]:
        """
        Per-factor contribution changes.
        Returns
        -------
        list[FactorContributionDelta]

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the per-factor contribution changes as a pandas DataFrame.

        One row per entry of :attr:`delta`. The baseline and scenario
        decompositions are not flattened here — reach them through
        :attr:`before` / :attr:`after` and their own frame accessors.

        Columns: ``factor_id``, ``absolute_change`` (risk-measure units),
        ``relative_change`` (dimensionless fraction, not a percentage).

        Returns
        -------
        pd.DataFrame
            One row per changed factor.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class StressResult:
    """
    Result of a factor-stress scenario.

    Examples
    --------
    >>> from finstack_quant.portfolio import StressResult
    >>> risk = '{"total_risk":1.0,"measure":"variance","factor_contributions":[],"residual_risk":1.0,"position_factor_contributions":[],"position_residual_contributions":[]}'
    >>> result = StressResult.from_json(
    ...     '{"total_pnl":-10.0,"position_pnl":[["P1",-10.0]],"stressed_decomposition":' + risk + "}"
    ... )
    >>> result.total_pnl
    -10.0
    """

    @classmethod
    def from_json(cls, json_str: str) -> StressResult:
        """
        Deserialize a stress result from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized stressed P&L and decomposition result, normally
            produced by ``StressResult.to_json``.

        Returns
        -------
        StressResult
            Validated `StressResult` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or cannot be deserialized into the documented portfolio type.

        Examples
        --------
        >>> from finstack_quant.portfolio import StressResult
        >>> risk = '{"total_risk":1.0,"measure":"variance","factor_contributions":[],"residual_risk":1.0,"position_factor_contributions":[],"position_residual_contributions":[]}'
        >>> result = StressResult.from_json(
        ...     '{"total_pnl":-10.0,"position_pnl":[["P1",-10.0]],"stressed_decomposition":' + risk + "}"
        ... )
        >>> result.position_pnl
        [('P1', -10.0)]
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this stress result to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `StressResult`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def total_pnl(self) -> float:
        """
        Total portfolio P&L under the stress scenario.

        Returns
        -------
        float
            Total portfolio P&L under the stress scenario.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def position_pnl(self) -> list[tuple[str, float]]:
        """
        Position identifier and scenario P&L pairs from the stress run.
        Returns
        -------
        list[tuple[str, float]]

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def stressed_decomposition(self) -> RiskDecomposition:
        """
        Risk decomposition after applying the stress.
        Returns
        -------
        RiskDecomposition

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the per-position stressed P&L as a pandas DataFrame.

        One row per entry of :attr:`position_pnl`. The scenario totals stay on
        :attr:`total_pnl` and :attr:`stressed_decomposition`.

        Columns: ``position_id``, ``pnl`` (portfolio-currency amount; a loss
        is negative).

        Returns
        -------
        pd.DataFrame
            One row per position with a stressed P&L contribution.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class PositionAssignment:
    """
    Matched factor assignments for a single portfolio position.

    The full ``(dependency, factor_id)`` pairs are available as JSON via
    :meth:`mappings_json`; matched factor identifiers are accessible directly
    via the :attr:`factor_ids` property.

    Examples
    --------
    >>> from finstack_quant.portfolio import PositionAssignment
    >>> assignment = PositionAssignment.from_json('{"position_id":"P1","mappings":[]}')
    >>> (assignment.position_id, assignment.n_mappings)
    ('P1', 0)
    """

    @classmethod
    def from_json(cls, json_str: str) -> PositionAssignment:
        """
        Deserialize a position assignment from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized factor assignment for one position, normally
            produced by ``PositionAssignment.to_json``.

        Returns
        -------
        PositionAssignment
            Validated `PositionAssignment` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or cannot be deserialized into the documented portfolio type.

        Examples
        --------
        >>> from finstack_quant.portfolio import PositionAssignment
        >>> PositionAssignment.from_json('{"position_id":"P1","mappings":[]}').factor_ids
        []
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this position assignment to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `PositionAssignment`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def position_id(self) -> str:
        """
        Portfolio position identifier.

        Returns
        -------
        str
            Portfolio position identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def n_mappings(self) -> int:
        """
        Number of dependency-to-factor mappings.

        Returns
        -------
        int
            Number of dependency-to-factor mappings.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def mappings_json(self) -> str:
        """
        Return detailed dependency-to-factor mappings as JSON.
        Returns
        -------
        str
            Compact JSON for matched ``(dependency, factor_id, beta)`` triples.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def factor_ids(self) -> list[str]:
        """
        Matched factor identifiers.

        Returns
        -------
        list[str]
            Matched factor identifiers.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class UnmatchedEntry:
    """
    Single unmatched dependency surfaced during assignment.

    Examples
    --------
    >>> from finstack_quant.portfolio import UnmatchedEntry
    >>> entry = UnmatchedEntry.from_json('{"position_id":"P1","dependency":{"fx_pair":{"base":"USD","quote":"EUR"}}}')
    >>> entry.position_id
    'P1'
    """

    @classmethod
    def from_json(cls, json_str: str) -> UnmatchedEntry:
        """
        Deserialize an unmatched dependency entry from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized unmatched-factor diagnostic, normally produced
            by ``UnmatchedEntry.to_json``.

        Returns
        -------
        UnmatchedEntry
            Validated `UnmatchedEntry` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or cannot be deserialized into the documented portfolio type.

        Examples
        --------
        >>> from finstack_quant.portfolio import UnmatchedEntry
        >>> entry = UnmatchedEntry.from_json(
        ...     '{"position_id":"P1","dependency":{"fx_pair":{"base":"USD","quote":"EUR"}}}'
        ... )
        >>> "fx_pair" in entry.to_json()
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this unmatched entry to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `UnmatchedEntry`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def position_id(self) -> str:
        """
        Portfolio position identifier.

        Returns
        -------
        str
            Portfolio position identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def dependency_json(self) -> str:
        """
        Return the unmatched dependency payload as JSON.
        Returns
        -------
        str
            Compact JSON for the unmatched market-data dependency.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class FactorAssignmentReport:
    """
    Assignment results for a portfolio-level factor mapping pass.

    Examples
    --------
    >>> from finstack_quant.portfolio import FactorAssignmentReport
    >>> report = FactorAssignmentReport.from_json('{"assignments":[{"position_id":"P1","mappings":[]}],"unmatched":[]}')
    >>> len(report.assignments)
    1
    """

    @classmethod
    def from_json(cls, json_str: str) -> FactorAssignmentReport:
        """
        Deserialize a factor assignment report from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized factor-model assignment report, normally
            produced by ``FactorAssignmentReport.to_json``.

        Returns
        -------
        FactorAssignmentReport
            Validated `FactorAssignmentReport` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or cannot be deserialized into the documented portfolio type.

        Examples
        --------
        >>> from finstack_quant.portfolio import FactorAssignmentReport
        >>> report = FactorAssignmentReport.from_json(
        ...     '{"assignments":[{"position_id":"P1","mappings":[]}],"unmatched":[]}'
        ... )
        >>> report.unmatched
        []
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this factor assignment report to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `FactorAssignmentReport`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def assignments(self) -> list[PositionAssignment]:
        """
        Matched assignments by position.
        Returns
        -------
        list[PositionAssignment]

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def unmatched(self) -> list[UnmatchedEntry]:
        """
        Dependencies that could not be mapped to factors.
        Returns
        -------
        list[UnmatchedEntry]

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the matched assignments as a long-format DataFrame.

        One row per ``(dependency, factor_id, beta)`` mapping, so a position
        matched by a hierarchical matcher (e.g. credit) contributes one row
        per level. Row count is the sum of ``n_mappings`` across
        :attr:`assignments`, not the number of positions.

        Columns: ``position_id``, ``factor_id``, ``beta`` (factor loading),
        ``dependency_json`` — the ``MarketDependency`` variant tree is
        deliberately kept as a JSON string rather than exploded into columns,
        matching :meth:`PositionAssignment.mappings_json`.

        Returns
        -------
        pd.DataFrame
            One row per matched mapping.

        Raises
        ------
        ValueError
            If a market dependency cannot be serialized to JSON.
        """
        ...

    def to_unmatched_dataframe(self) -> pd.DataFrame:
        """
        Export the unmatched dependencies as a pandas DataFrame.

        Columns: ``position_id``, ``dependency_json`` (the unmatched
        ``MarketDependency`` as a JSON string).

        Returns
        -------
        pd.DataFrame
            One row per entry of :attr:`unmatched`; a zero-row frame (columns
            still present) means every dependency matched a configured factor.

        Raises
        ------
        ValueError
            If a market dependency cannot be serialized to JSON.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class LevelVolContribution:
    """
    Aggregated risk contribution for a single hierarchy level.

    Examples
    --------
    >>> from finstack_quant.portfolio import LevelVolContribution
    >>> try:
    ...     LevelVolContribution()
    ... except TypeError as exc:
    ...     print(exc)
    cannot create 'finstack_quant.portfolio.LevelVolContribution' instances
    """

    @property
    def level_name(self) -> str:
        """
        Name of the hierarchy level that received this volatility contribution.

        Returns
        -------
        str
            Name of the hierarchy level that received this volatility contribution.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def total(self) -> float:
        """
        Total contribution for this level.

        Returns
        -------
        float
            Total contribution for this level.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def by_bucket(self) -> dict[str, float]:
        """
        Contribution by hierarchy bucket.

        Returns
        -------
        dict[str, float]
            Contribution by hierarchy bucket.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class PositionVolContribution:
    """
    Per-position vol breakdown under :class:`CreditVolReport`.

    Examples
    --------
    >>> from finstack_quant.portfolio import PositionVolContribution
    >>> try:
    ...     PositionVolContribution()
    ... except TypeError as exc:
    ...     print(exc)
    cannot create 'finstack_quant.portfolio.PositionVolContribution' instances
    """

    @property
    def position_id(self) -> str:
        """
        Portfolio position identifier.

        Returns
        -------
        str
            Portfolio position identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def factor_total(self) -> float:
        """
        Factor-driven volatility contribution.

        Returns
        -------
        float
            Factor-driven volatility contribution.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def idiosyncratic(self) -> float:
        """
        Idiosyncratic volatility contribution.

        Returns
        -------
        float
            Idiosyncratic volatility contribution.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def total(self) -> float:
        """
        Total position volatility contribution.

        Returns
        -------
        float
            Total position volatility contribution.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class CreditVolReport:
    """
    Aggregated vol report grouped by hierarchy level.

    Examples
    --------
    >>> from finstack_quant.portfolio import CreditVolReport
    >>> try:
    ...     CreditVolReport()
    ... except TypeError as exc:
    ...     print(exc)
    cannot create 'finstack_quant.portfolio.CreditVolReport' instances
    """

    @property
    def total(self) -> float:
        """
        Total portfolio volatility under the report measure.

        Returns
        -------
        float
            Total portfolio volatility under the report measure.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def measure_json(self) -> str:
        """
        Risk measure specification as JSON.

        Returns
        -------
        str
            Risk measure specification as JSON.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def generic(self) -> float:
        """
        Generic factor contribution.

        Returns
        -------
        float
            Generic factor contribution.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def idiosyncratic_total(self) -> float:
        """
        Aggregate idiosyncratic contribution.

        Returns
        -------
        float
            Aggregate idiosyncratic contribution.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def by_level(self) -> list[LevelVolContribution]:
        """
        Volatility contribution by hierarchy level.
        Returns
        -------
        list[LevelVolContribution]

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def by_position(self) -> list[PositionVolContribution] | None:
        """
        Optional per-position volatility contributions.
        Returns
        -------
        list[PositionVolContribution] or None

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Primary table: the per-hierarchy-level rollup.

        Alias of :meth:`to_level_dataframe`. Every tabular result type in the library
        answers ``to_dataframe()``; the optional per-position breakdown stays on
        :meth:`to_position_dataframe`.

        Returns
        -------
        pd.DataFrame
            The same frame :meth:`to_level_dataframe` returns.

        Examples
        --------
        >>> frame = result.to_dataframe()  # doctest: +SKIP

        Notes
        -----
        This alias does not raise; it delegates to the method named above.
        """
        ...

    def to_level_dataframe(self) -> pd.DataFrame:
        """
        Export the per-hierarchy-level rollup as a pandas DataFrame.

        One row per entry of :attr:`by_level`. The per-bucket drill-down stays
        on ``LevelVolContribution.by_bucket``, which is already a plain dict.

        Columns: ``level_name``, ``total`` (level contribution in the units of
        :attr:`measure_json`).

        Returns
        -------
        pd.DataFrame
            One row per hierarchy level.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_position_dataframe(self) -> pd.DataFrame:
        """
        Export the optional per-position breakdown as a pandas DataFrame.

        One row per entry of :attr:`by_position`. When the report was built
        without ``by_position=True`` the result is a zero-row frame that still
        carries the full column schema, so downstream ``df[...]`` selections
        keep working instead of raising ``KeyError``.

        Columns: ``position_id``, ``factor_total`` (systematic),
        ``idiosyncratic`` (issuer-specific), ``total`` (their sum) — all in
        the units of :attr:`measure_json`.

        Returns
        -------
        pd.DataFrame
            One row per position, or zero rows when :attr:`by_position` is
            ``None``.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

def factor_stress(
    portfolio: Portfolio | str,
    market: MarketContext | str,
    factor_model_config_json: str,
    as_of: datetime.date | str,
    stresses: list[tuple[str, float]],
) -> StressResult:
    """
    Run a factor-stress scenario and revalue the portfolio.

    Builds the Rust factor model from ``factor_model_config_json``, analyzes
    the base portfolio, computes sensitivities, applies the requested factor
    shifts, and returns the stressed result.

    Parameters
    ----------
    portfolio : Portfolio or str
        Portfolio instance or JSON portfolio specification accepted
        by the compiled portfolio extractor.
    market : MarketContext or str
        MarketContext instance or JSON market context accepted by the
        compiled market extractor.
    factor_model_config_json : str
        JSON-encoded ``finstack_quant_models::factor::FactorModelConfig``.
    as_of : datetime.date | str
        Calculation date, either a date-like object or an ISO 8601 string.
    stresses : list[tuple[str, float]]
        ``(factor_id, shift)`` pairs. Factor IDs must match the
        configured model; shifts use the Rust factor model's units for that
        factor.

    Returns
    -------
    StressResult
        StressResult containing base/stressed portfolio risk and per-factor
        deltas.

    Raises
    ------
    ValueError
        If JSON parsing, date parsing, model construction, market
        lookup, or portfolio valuation fails.
    TypeError
        If ``portfolio`` or ``market`` cannot be converted to the
        expected Rust types.

    Examples
    --------
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.portfolio import factor_stress
    >>> portfolio = '{"id":"empty","base_currency":"USD","as_of":"2025-01-01","entities":{},"positions":[]}'
    >>> try:
    ...     factor_stress(portfolio, MarketContext(), "{}", "2025-01-01", [])
    ... except ValueError as exc:
    ...     print("missing field `factors`" in str(exc))
    True
    """
    ...

def position_what_if(
    portfolio: Portfolio | str,
    market: MarketContext | str,
    factor_model_config_json: str,
    as_of: datetime.date | str,
    changes: list[dict[str, Any]],
) -> WhatIfResult:
    """
    Run position remove/resize what-if analysis.

    The Python binding accepts JSON-like dictionaries for remove and resize
    changes, then delegates the sensitivity reallocation and result generation
    to Rust.

    Parameters
    ----------
    portfolio : Portfolio or str
        Portfolio instance or JSON portfolio specification accepted
        by the compiled portfolio extractor.
    market : MarketContext or str
        MarketContext instance or JSON market context accepted by the
        compiled market extractor.
    factor_model_config_json : str
        JSON-encoded ``finstack_quant_models::factor::FactorModelConfig``.
    as_of : datetime.date | str
        Calculation date, either a date-like object or an ISO 8601 string.
    changes : list[dict[str, Any]]
        List of dictionaries. Remove changes use
        ``{"kind": "remove", "position_id": "..."}``; resize changes use
        ``{"kind": "resize", "position_id": "...", "new_quantity": 123.0}``.
        Supply at most one final change per position; duplicate IDs raise ValueError.

    Returns
    -------
    WhatIfResult
        WhatIfResult with base and scenario risk decomposition deltas.

    Raises
    ------
    ValueError
        If a change kind is unknown, resize omits
        ``new_quantity``, JSON/config parsing fails, or
        Rust factor-model evaluation fails.
    TypeError
        If ``portfolio`` or ``market`` cannot be converted to the
        expected Rust types.

    Examples
    --------
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.portfolio import position_what_if
    >>> portfolio = '{"id":"empty","base_currency":"USD","as_of":"2025-01-01","entities":{},"positions":[]}'
    >>> try:
    ...     position_what_if(portfolio, MarketContext(), "{}", "2025-01-01", [])
    ... except ValueError as exc:
    ...     print("missing field `factors`" in str(exc))
    True
    """
    ...

def build_credit_vol_report(
    decomposition: RiskDecomposition,
    model: CreditFactorModel,
    by_position: bool = False,
) -> CreditVolReport:
    """
    Build a credit volatility report from decomposition outputs.

    Aggregates a Rust ``RiskDecomposition`` against the supplied credit factor
    model hierarchy into generic, level, idiosyncratic, and optional
    per-position volatility contributions.

    Parameters
    ----------
    decomposition : RiskDecomposition
        Factor risk decomposition to summarize.
    model : CreditFactorModel
        Credit factor model whose hierarchy and factor taxonomy label
        the report levels and buckets.
    by_position : bool, default False
        Include per-position contribution rows when ``True``.

    Returns
    -------
    CreditVolReport
        CreditVolReport with total, generic, level, idiosyncratic, and optional
        position-level volatility contribution fields.

    Notes
    -----
    This helper does not raise; it aggregates the supplied decomposition in-process.

    Examples
    --------
    >>> from finstack_quant.models.factor.risk import RiskDecomposition
    >>> from finstack_quant.portfolio import build_credit_vol_report
    >>> risk = RiskDecomposition.from_json(
    ...     '{"total_risk":1.0,"measure":"variance","factor_contributions":[],"residual_risk":1.0,"position_factor_contributions":[],"position_residual_contributions":[]}'
    ... )
    >>> try:
    ...     build_credit_vol_report(risk, object())
    ... except TypeError as exc:
    ...     print("CreditFactorModel" in str(exc))
    True
    """
    ...

class WeightingScheme:
    """
    How optimization weights are defined.

    Examples
    --------
    >>> from finstack_quant.portfolio import WeightingScheme
    >>> WeightingScheme.value_weight().label
    'value_weight'
    """

    @classmethod
    def value_weight(cls) -> WeightingScheme:
        """
        Weight positions by market value.

        Returns
        -------
        WeightingScheme
            Weights as shares of gross absolute base-currency portfolio PV.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.portfolio import WeightingScheme
        >>> WeightingScheme.value_weight().label
        'value_weight'
        """
        ...

    @classmethod
    def notional_weight(cls) -> WeightingScheme:
        """
        Weight positions by instrument deal notional times lot scale.

        Returns
        -------
        WeightingScheme
            Weights as signed shares of
            absolute deal notional converted to portfolio base currency at the
            valuation date, multiplied by ``scale_factor()``.
            Instruments that do not expose deal notional fail; there is no
            fallback to ``scale_factor()`` as a dollar proxy.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.portfolio import WeightingScheme
        >>> WeightingScheme.notional_weight().label
        'notional_weight'
        """
        ...

    @classmethod
    def unit_scaling(cls) -> WeightingScheme:
        """
        Use unit scaling rather than value/notional scaling.

        Returns
        -------
        WeightingScheme
            Existing weights as quantity multipliers starting at one, and candidate
            weights as quantities starting at zero. Turnover is the sum of absolute
            multiplier changes for existing holdings.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.portfolio import WeightingScheme
        >>> WeightingScheme.unit_scaling().label
        'unit_scaling'
        """
        ...

    @property
    def label(self) -> str:
        """
        Rust enum label for this weighting scheme.

        Returns
        -------
        str
            Rust enum label for this weighting scheme.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __eq__(self, other: object) -> bool:
        """Structural equality on the underlying Rust variant.

        Returns
        -------
        bool
        """
        ...

    def __hash__(self) -> int:
        """Hash consistent with :meth:`__eq__` (usable as dict/set keys).
        Returns
        -------
        int
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class MissingMetricPolicy:
    """
    Policy for handling positions missing required metrics.

    Examples
    --------
    >>> from finstack_quant.portfolio import MissingMetricPolicy
    >>> MissingMetricPolicy.strict().label
    'strict'
    """

    @classmethod
    def zero(cls) -> MissingMetricPolicy:
        """
        Treat missing metric values as zero.

        Returns
        -------
        MissingMetricPolicy
            Policy substituting ``0.0`` for each missing required metric.

        Notes
        -----
        This factory does not raise; it returns a new instance with the documented defaults.

        Examples
        --------
        >>> from finstack_quant.portfolio import MissingMetricPolicy
        >>> MissingMetricPolicy.zero().label
        'zero'
        """
        ...

    @classmethod
    def exclude(cls) -> MissingMetricPolicy:
        """
        Exclude positions with missing required metrics.

        Returns
        -------
        MissingMetricPolicy
            Policy that freezes missing-metric positions at their current
            weights and drops them from ``WeightedSum`` and
            ``ValueWeightedAverage`` coefficient vectors (coefficient 0,
            omitted from a value-weighted-average denominator).

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.portfolio import MissingMetricPolicy
        >>> MissingMetricPolicy.exclude().label
        'exclude'
        """
        ...

    @classmethod
    def strict(cls) -> MissingMetricPolicy:
        """
        Reject optimization when required metrics are missing.

        Returns
        -------
        MissingMetricPolicy
            Policy failing optimization when any required metric is missing.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.portfolio import MissingMetricPolicy
        >>> MissingMetricPolicy.strict().label
        'strict'
        """
        ...

    @property
    def label(self) -> str:
        """
        Rust enum label for this policy.

        Returns
        -------
        str
            Rust enum label for this policy.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __eq__(self, other: object) -> bool:
        """Structural equality on the underlying Rust variant.

        Returns
        -------
        bool
        """
        ...

    def __hash__(self) -> int:
        """Hash consistent with :meth:`__eq__` (usable as dict/set keys).
        Returns
        -------
        int
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class Inequality:
    """
    Inequality / equality operator (`<=`, `>=`, `==`).

    Examples
    --------
    >>> from finstack_quant.portfolio import Inequality
    >>> Inequality.le().label
    'le'
    """

    def __init__(self, op: str) -> None:
        """
        Parse an inequality from its name or symbol.

        Parameters
        ----------
        op : str
            ``"le"``/``"<="``, ``"ge"``/``">="`` or ``"eq"``/``"=="``.

        Raises
        ------
        ValueError
            For any other text.

        Examples
        --------
        >>> from finstack_quant.portfolio import Inequality
        >>> Inequality("<=") == Inequality.le()
        True
        """
        ...

    @classmethod
    def le(cls) -> Inequality:
        """
        Less-than-or-equal inequality.

        Returns
        -------
        Inequality
            Operator encoding ``lhs <= rhs``.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.portfolio import Inequality
        >>> Inequality.le().label
        'le'
        """
        ...

    @classmethod
    def ge(cls) -> Inequality:
        """
        Greater-than-or-equal inequality.

        Returns
        -------
        Inequality
            Operator encoding ``lhs >= rhs``.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.portfolio import Inequality
        >>> Inequality.ge().label
        'ge'
        """
        ...

    @classmethod
    def eq(cls) -> Inequality:
        """
        Equality constraint requiring the metric to equal a bound.

        Returns
        -------
        Inequality
            Operator encoding ``lhs == rhs``.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.portfolio import Inequality
        >>> Inequality.eq().label
        'eq'
        """
        ...

    @property
    def label(self) -> str:
        """
        Canonical inequality operator label such as ``<=``.

        Returns
        -------
        str
            Operator name used in optimization constraint expressions.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __eq__(self, other: object) -> bool:
        """Structural equality on the underlying Rust variant.

        Returns
        -------
        bool
        """
        ...

    def __hash__(self) -> int:
        """Hash consistent with :meth:`__eq__` (usable as dict/set keys).
        Returns
        -------
        int
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class TradeDirection:
    """
    Trade direction (buy/sell/hold).

    Examples
    --------
    >>> from finstack_quant.portfolio import TradeDirection
    >>> TradeDirection.buy().label
    'buy'
    """

    @classmethod
    def buy(cls) -> TradeDirection:
        """
        Direction for increasing an instrument exposure.

        Returns
        -------
        TradeDirection
            Direction for increasing an instrument exposure.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.portfolio import TradeDirection
        >>> TradeDirection.buy().label
        'buy'
        """
        ...

    @classmethod
    def sell(cls) -> TradeDirection:
        """
        Direction for decreasing an instrument exposure.

        Returns
        -------
        TradeDirection
            Direction for decreasing an instrument exposure.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.portfolio import TradeDirection
        >>> TradeDirection.sell().label
        'sell'
        """
        ...

    @classmethod
    def hold(cls) -> TradeDirection:
        """
        Hold/no-trade direction.

        Returns
        -------
        TradeDirection
            Direction representing no change in exposure.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.portfolio import TradeDirection
        >>> TradeDirection.hold().label
        'hold'
        """
        ...

    @property
    def label(self) -> str:
        """
        Buy/sell/short label for the trade direction.

        Returns
        -------
        str
            Buy/sell/short label for the trade direction.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __eq__(self, other: object) -> bool:
        """Structural equality on the underlying Rust variant.

        Returns
        -------
        bool
        """
        ...

    def __hash__(self) -> int:
        """Hash consistent with :meth:`__eq__` (usable as dict/set keys).
        Returns
        -------
        int
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class TradeType:
    """
    Trade type (existing/new-position/close-out).

    Examples
    --------
    >>> from finstack_quant.portfolio import TradeType
    >>> TradeType.existing().label
    'existing'
    """

    @classmethod
    def existing(cls) -> TradeType:
        """
        Trade an existing position.

        Returns
        -------
        TradeType
            Trade type for adjusting an existing portfolio position.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.portfolio import TradeType
        >>> TradeType.existing().label
        'existing'
        """
        ...

    @classmethod
    def new_position(cls) -> TradeType:
        """
        Open a new candidate position.

        Returns
        -------
        TradeType
            Trade type for adding a candidate instrument as a new position.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.portfolio import TradeType
        >>> TradeType.new_position().label
        'new_position'
        """
        ...

    @classmethod
    def close_out(cls) -> TradeType:
        """
        Close an existing position.

        Returns
        -------
        TradeType
            Trade type for fully closing an existing position.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.portfolio import TradeType
        >>> TradeType.close_out().label
        'close_out'
        """
        ...

    @property
    def label(self) -> str:
        """
        Human-readable label for the trade-type variant.

        Returns
        -------
        str
            Human-readable label for the trade-type variant.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __eq__(self, other: object) -> bool:
        """Structural equality on the underlying Rust variant.

        Returns
        -------
        bool
        """
        ...

    def __hash__(self) -> int:
        """Hash consistent with :meth:`__eq__` (usable as dict/set keys).
        Returns
        -------
        int
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class PerPositionMetric:
    """
    Per-position metric source for optimization expressions.

    Examples
    --------
    >>> from finstack_quant.portfolio import PerPositionMetric
    >>> PerPositionMetric.pv_base().kind
    'pv_base'
    """

    @classmethod
    def metric(cls, metric_id: str) -> PerPositionMetric:
        """
        Use a standard valuation metric by its ``MetricId`` string.

        Parameters
        ----------
        metric_id : str
            Standard metric identifier, such as ``"dv01"`` or
            ``"duration_mod"``. Validated strictly against the standard
            metric set; for namespaced or custom measure keys (for example
            ``"dv01::USD-OIS"``) use :meth:`custom_key`, which reads the same
            ``measures`` map.

        Returns
        -------
        PerPositionMetric
            Metric source reading ``metric_id`` from each position valuation.

        Raises
        ------
        ValueError
            If ``metric_id`` is not a standard metric name; the message lists
            the available metrics.

        Examples
        --------
        >>> from finstack_quant.portfolio import PerPositionMetric
        >>> PerPositionMetric.metric("duration_mod").kind
        'metric'
        """
        ...

    @classmethod
    def custom_key(cls, key: str) -> PerPositionMetric:
        """
        Use a custom per-position metric key.

        Parameters
        ----------
        key : str
            Custom metric key emitted by the portfolio valuation pipeline.

        Returns
        -------
        PerPositionMetric
            Metric source reading the custom measure named ``key``.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.portfolio import PerPositionMetric
        >>> PerPositionMetric.custom_key("desk_metric").kind
        'custom_key'
        """
        ...

    @classmethod
    def pv_base(cls) -> PerPositionMetric:
        """
        Use present value converted to portfolio base currency.

        Returns
        -------
        PerPositionMetric
            Metric source using each scaled position PV in portfolio base currency.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.portfolio import PerPositionMetric
        >>> PerPositionMetric.pv_base().kind
        'pv_base'
        """
        ...

    @classmethod
    def pv_native(cls) -> PerPositionMetric:
        """
        Use present value in the instrument native currency.

        Returns
        -------
        PerPositionMetric
            Metric source using each scaled position PV in its native currency.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.portfolio import PerPositionMetric
        >>> PerPositionMetric.pv_native().kind
        'pv_native'
        """
        ...

    @classmethod
    def attribute(cls, key: str) -> PerPositionMetric:
        """
        Use a position attribute as a metric source.

        Parameters
        ----------
        key : str
            Attribute name stored on positions, whose numeric values become the
            metric for selected positions.

        Returns
        -------
        PerPositionMetric
            Metric source reading the numeric position attribute named ``key``.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.portfolio import PerPositionMetric
        >>> PerPositionMetric.attribute("sector").kind
        'attribute'
        """
        ...

    @classmethod
    def attribute_indicator(
        cls,
        key: str,
        op: str,
        text: str | None = None,
        number: float | None = None,
    ) -> PerPositionMetric:
        """
        Use a boolean position-attribute comparison as an indicator metric.

        Parameters
        ----------
        key : str
            Position attribute name to compare.
        op : str
            Comparison operator accepted by Rust, such as ``"eq"`` or ``"gt"``.
        text : str or None
            Optional string comparison value; supply when ``op`` compares text.
        number : float or None
            Optional numeric comparison value; supply when ``op`` compares numbers.

        Returns
        -------
        PerPositionMetric
            Binary metric returning ``1.0`` for a match and ``0.0`` otherwise.

        Raises
        ------
        ValueError
            If ``op`` is unknown, or exactly one of ``text`` and ``number`` is
            not supplied.

        Examples
        --------
        >>> from finstack_quant.portfolio import PerPositionMetric
        >>> PerPositionMetric.attribute_indicator("sector", "eq", text="tech").kind
        'attribute_indicator'
        """
        ...

    @classmethod
    def constant(cls, value: float) -> PerPositionMetric:
        """
        Use a constant metric value for every selected position.

        Parameters
        ----------
        value : float
            Numeric metric value assigned identically to each selected position.

        Returns
        -------
        PerPositionMetric
            Metric source returning ``value`` for every selected position.

        Notes
        -----
        This factory does not raise; it returns a new instance with the documented defaults.

        Examples
        --------
        >>> from finstack_quant.portfolio import PerPositionMetric
        >>> PerPositionMetric.constant(1.5).kind
        'constant'
        """
        ...

    @classmethod
    def from_json(cls, json_str: str) -> PerPositionMetric:
        """
        Deserialize a per-position metric expression from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized metric-source expression, normally produced by
            ``PerPositionMetric.to_json``.

        Returns
        -------
        PerPositionMetric
            Validated `PerPositionMetric` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or cannot be deserialized into the documented portfolio type.

        Examples
        --------
        >>> from finstack_quant.portfolio import PerPositionMetric
        >>> metric = PerPositionMetric.from_json(PerPositionMetric.pv_base().to_json())
        >>> metric.kind
        'pv_base'
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this per-position metric expression to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `PerPositionMetric`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def kind(self) -> str:
        """
        Metric-source variant label.

        Returns
        -------
        str
            Metric-source variant label.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class PositionFilter:
    """
    Declarative filter for selecting which positions a rule applies to.

    Examples
    --------
    >>> from finstack_quant.portfolio import PositionFilter
    >>> PositionFilter.all().kind
    'all'
    """

    @classmethod
    def all(cls) -> PositionFilter:
        """
        Filter that matches every position in the portfolio.

        Returns
        -------
        PositionFilter
            Filter matching every portfolio position.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.portfolio import PositionFilter
        >>> PositionFilter.all().kind
        'all'
        """
        ...

    @classmethod
    def by_entity_id(cls, entity_id: str) -> PositionFilter:
        """
        Select positions for one entity ID.

        Parameters
        ----------
        entity_id : str
            Entity identifier assigned to positions that should be selected.

        Returns
        -------
        PositionFilter
            Filter matching positions assigned to ``entity_id``.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.portfolio import PositionFilter
        >>> PositionFilter.by_entity_id("issuer-1").kind
        'by_entity_id'
        """
        ...

    @classmethod
    def by_attribute(
        cls,
        key: str,
        op: str,
        text: str | None = None,
        number: float | None = None,
    ) -> PositionFilter:
        """
        Select positions by attribute comparison.

        Parameters
        ----------
        key : str
            Position attribute name to compare.
        op : str
            Comparison operator accepted by Rust, such as ``"eq"`` or ``"gte"``.
        text : str or None
            Optional string comparison value for text-valued attributes.
        number : float or None
            Optional numeric comparison value for numeric attributes.

        Returns
        -------
        PositionFilter
            Filter matching positions whose attribute satisfies the comparison.

        Raises
        ------
        ValueError
            If ``op`` is unknown, or exactly one of ``text`` and ``number`` is
            not supplied.

        Examples
        --------
        >>> from finstack_quant.portfolio import PositionFilter
        >>> PositionFilter.by_attribute("sector", "eq", text="tech").kind
        'by_attribute'
        """
        ...

    @classmethod
    def by_position_ids(cls, position_ids: list[str]) -> PositionFilter:
        """
        Select positions by explicit position IDs.

        Parameters
        ----------
        position_ids : list[str]
            Explicit portfolio position identifiers to include in the filter.

        Returns
        -------
        PositionFilter
            Filter matching IDs contained in ``position_ids``.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.portfolio import PositionFilter
        >>> PositionFilter.by_position_ids(["P1", "P2"]).kind
        'by_position_ids'
        """
        ...

    @classmethod
    def not_(cls, inner: PositionFilter) -> PositionFilter:
        """
        Filter that matches positions excluded by another filter.

        Parameters
        ----------
        inner : PositionFilter
            Existing filter whose matching positions should be excluded.

        Returns
        -------
        PositionFilter
            Filter matching positions not matched by ``inner``.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.portfolio import PositionFilter
        >>> PositionFilter.not_(PositionFilter.all()).kind
        'not'
        """
        ...

    @classmethod
    def and_(cls, filters: list[PositionFilter]) -> PositionFilter:
        """
        Select positions matching all child filters.

        Parameters
        ----------
        filters : list[PositionFilter]
            Child filters that every selected position must satisfy.

        Returns
        -------
        PositionFilter
            Filter matching positions that satisfy every child filter.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.portfolio import PositionFilter
        >>> PositionFilter.and_([PositionFilter.all()]).kind
        'and'
        """
        ...

    @classmethod
    def or_(cls, filters: list[PositionFilter]) -> PositionFilter:
        """
        Select positions matching any child filter.

        Parameters
        ----------
        filters : list[PositionFilter]
            Child filters of which at least one must match each selected position.

        Returns
        -------
        PositionFilter
            Filter matching positions that satisfy at least one child filter.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.portfolio import PositionFilter
        >>> PositionFilter.or_([PositionFilter.all()]).kind
        'or'
        """
        ...

    @classmethod
    def from_json(cls, json_str: str) -> PositionFilter:
        """
        Deserialize a position filter from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized filter tree, normally produced by
            ``PositionFilter.to_json``.

        Returns
        -------
        PositionFilter
            Validated `PositionFilter` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or cannot be deserialized into the documented portfolio type.

        Examples
        --------
        >>> from finstack_quant.portfolio import PositionFilter
        >>> filter_ = PositionFilter.from_json(PositionFilter.all().to_json())
        >>> filter_.kind
        'all'
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this position filter to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `PositionFilter`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def kind(self) -> str:
        """
        Discriminator for the position-filter variant.

        Returns
        -------
        str
            Discriminator for the position-filter variant.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class MetricExpr:
    """
    Portfolio-level metric expression.

    Examples
    --------
    >>> from finstack_quant.portfolio import MetricExpr, PerPositionMetric
    >>> MetricExpr.weighted_sum(PerPositionMetric.pv_base()).kind
    'weighted_sum'
    """

    @classmethod
    def weighted_sum(
        cls,
        metric: PerPositionMetric,
        filter: PositionFilter | None = None,
    ) -> MetricExpr:
        """
        Build a weighted-sum portfolio metric expression.

        Parameters
        ----------
        metric : PerPositionMetric
            Per-position value to multiply by each selected position weight.
        filter : PositionFilter or None
            Optional selector limiting the expression to matching positions.

        Returns
        -------
        MetricExpr
            Expression for ``sum_i(w_i * m_i)``, restricted by ``filter`` when supplied.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.portfolio import MetricExpr, PerPositionMetric
        >>> MetricExpr.weighted_sum(PerPositionMetric.pv_base()).kind
        'weighted_sum'
        """
        ...

    @classmethod
    def value_weighted_average(
        cls,
        metric: PerPositionMetric,
        filter: PositionFilter | None = None,
    ) -> MetricExpr:
        """
        Build a value-weighted-average portfolio metric expression.

        Parameters
        ----------
        metric : PerPositionMetric
            Per-position value to average using portfolio market-value weights.
        filter : PositionFilter or None
            Optional selector limiting the expression to matching positions.

        Returns
        -------
        MetricExpr
            Value-weighted average of the metric, restricted by ``filter`` when supplied.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.portfolio import MetricExpr, PerPositionMetric
        >>> MetricExpr.value_weighted_average(PerPositionMetric.pv_base()).kind
        'value_weighted_average'
        """
        ...

    @classmethod
    def from_json(cls, json_str: str) -> MetricExpr:
        """
        Deserialize a metric expression from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized portfolio metric expression, normally produced
            by ``MetricExpr.to_json``.

        Returns
        -------
        MetricExpr
            Validated `MetricExpr` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or cannot be deserialized into the documented portfolio type.

        Examples
        --------
        >>> from finstack_quant.portfolio import MetricExpr, PerPositionMetric
        >>> original = MetricExpr.weighted_sum(PerPositionMetric.pv_base())
        >>> MetricExpr.from_json(original.to_json()).kind
        'weighted_sum'
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this metric expression to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `MetricExpr`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def kind(self) -> str:
        """
        Metric-expression variant label.

        Returns
        -------
        str
            Metric-expression variant label.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class Objective:
    """
    Optimization direction and target.

    Examples
    --------
    >>> from finstack_quant.portfolio import MetricExpr, Objective, PerPositionMetric
    >>> Objective.maximize(MetricExpr.weighted_sum(PerPositionMetric.pv_base())).direction
    'maximize'
    """

    @classmethod
    def maximize(cls, expr: MetricExpr) -> Objective:
        """
        Maximize the supplied metric expression.

        Parameters
        ----------
        expr : MetricExpr
            Portfolio-level metric expression the optimizer should maximize.

        Returns
        -------
        Objective
            Objective directing the optimizer to maximize ``expr``.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.portfolio import MetricExpr, Objective, PerPositionMetric
        >>> expr = MetricExpr.weighted_sum(PerPositionMetric.pv_base())
        >>> Objective.maximize(expr).direction
        'maximize'
        """
        ...

    @classmethod
    def minimize(cls, expr: MetricExpr) -> Objective:
        """
        Minimize the supplied metric expression.

        Parameters
        ----------
        expr : MetricExpr
            Portfolio-level metric expression the optimizer should minimize.

        Returns
        -------
        Objective
            Objective directing the optimizer to minimize ``expr``.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.portfolio import MetricExpr, Objective, PerPositionMetric
        >>> expr = MetricExpr.weighted_sum(PerPositionMetric.pv_base())
        >>> Objective.minimize(expr).direction
        'minimize'
        """
        ...

    @classmethod
    def from_json(cls, json_str: str) -> Objective:
        """
        Deserialize an optimization objective from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized objective direction and metric expression,
            normally produced by ``Objective.to_json``.

        Returns
        -------
        Objective
            Validated `Objective` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or cannot be deserialized into the documented portfolio type.

        Examples
        --------
        >>> from finstack_quant.portfolio import MetricExpr, Objective, PerPositionMetric
        >>> original = Objective.maximize(MetricExpr.weighted_sum(PerPositionMetric.pv_base()))
        >>> Objective.from_json(original.to_json()).direction
        'maximize'
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this optimization objective to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `Objective`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def direction(self) -> str:
        """
        Optimization direction label.

        Returns
        -------
        str
            Optimization direction label.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def expr(self) -> MetricExpr:
        """
        Metric expression being optimized.

        Returns
        -------
        MetricExpr
            Metric expression being optimized.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class Constraint:
    """
    Declarative constraint specification.

    Examples
    --------
    >>> from finstack_quant.portfolio import Constraint
    >>> Constraint.budget(1.0).kind
    'budget'
    """

    @classmethod
    def metric_bound(
        cls,
        metric: MetricExpr,
        op: Inequality,
        rhs: float,
        label: str | None = None,
    ) -> Constraint:
        """
        Constrain a metric expression against a right-hand side.

        Parameters
        ----------
        metric : MetricExpr
            Portfolio expression whose evaluated value is constrained.
        op : Inequality
            Less-than, greater-than, or equality operator for the bound.
        rhs : float
            Numeric right-hand-side bound in the metric expression's units.
        label : str or None
            Optional stable label for diagnostics, reporting, and dual values.

        Returns
        -------
        Constraint
            Constraint encoding ``metric op rhs`` with the optional diagnostic label.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.portfolio import Constraint, Inequality, MetricExpr, PerPositionMetric
        >>> expr = MetricExpr.weighted_sum(PerPositionMetric.pv_base())
        >>> Constraint.metric_bound(expr, Inequality.le(), 1.0).kind
        'metric_bound'
        """
        ...

    @classmethod
    def weight_bounds(
        cls,
        filter: PositionFilter,
        min: float,
        max: float,
        label: str | None = None,
    ) -> Constraint:
        """
        Constrain selected position weights to a min/max interval.

        Parameters
        ----------
        filter : PositionFilter
            Selector identifying the positions subject to the weight interval.
        min : float
            Inclusive lower bound on each selected position's decimal weight.
        max : float
            Inclusive upper bound on each selected position's decimal weight.
        label : str or None
            Optional stable label for diagnostics, reporting, and dual values.

        Returns
        -------
        Constraint
            Inclusive ``[min, max]`` weight interval for positions matching ``filter``.

        Raises
        ------
        ValueError
            If ``min`` is greater than ``max``.

        Examples
        --------
        >>> from finstack_quant.portfolio import Constraint, PositionFilter
        >>> Constraint.weight_bounds(PositionFilter.all(), 0.0, 1.0).kind
        'weight_bounds'
        """
        ...

    @classmethod
    def max_turnover(
        cls,
        max_turnover: float,
        label: str | None = None,
    ) -> Constraint:
        """
        Constrain total portfolio turnover.

        Parameters
        ----------
        max_turnover : float
            Maximum permitted aggregate turnover as a decimal weight fraction.
        label : str or None
            Optional stable label for diagnostics, reporting, and dual values.

        Returns
        -------
        Constraint
            Bound enforcing ``sum(abs(w_new - w_current)) <= max_turnover``.

        Raises
        ------
        ValueError
            If ``max_turnover`` is negative.

        Examples
        --------
        >>> from finstack_quant.portfolio import Constraint
        >>> Constraint.max_turnover(0.10).kind
        'max_turnover'
        """
        ...

    @classmethod
    def budget(cls, rhs: float) -> Constraint:
        """
        Constrain total portfolio budget/weight to ``rhs``.

        Parameters
        ----------
        rhs : float
            Required total portfolio weight, normally ``1.0`` for a fully
            invested long-only budget.

        Returns
        -------
        Constraint
            Normalization constraint enforcing ``sum(w_i) == rhs``.

        Raises
        ------
        ValueError
            If ``rhs`` is non-finite or negative.

        Examples
        --------
        >>> from finstack_quant.portfolio import Constraint
        >>> (Constraint.budget(1.0).kind, Constraint.budget(1.0).label)
        ('budget', 'budget')
        """
        ...

    @classmethod
    def exposure_limit(
        cls,
        key: str,
        value: str,
        max_share: float,
        label: str | None = None,
    ) -> Constraint:
        """
        Constrain maximum exposure share for an attribute key/value.

        Parameters
        ----------
        key : str
            Position attribute name used to define the exposure bucket.
        value : str
            Attribute value identifying the exposure bucket to cap.
        max_share : float
            Maximum decimal portfolio-weight share permitted in the bucket.
        label : str or None
            Optional stable label for diagnostics, reporting, and dual values.

        Returns
        -------
        Constraint
            Attribute-bucket constraint enforcing weighted exposure ``<= max_share``.

        Raises
        ------
        ValueError
            If ``max_share`` is outside ``[0, 1]`` or is non-finite.

        Examples
        --------
        >>> from finstack_quant.portfolio import Constraint
        >>> Constraint.exposure_limit("sector", "tech", 0.20).kind
        'metric_bound'
        """
        ...

    @classmethod
    def exposure_minimum(
        cls,
        key: str,
        value: str,
        min_share: float,
        label: str | None = None,
    ) -> Constraint:
        """
        Constrain minimum exposure share for an attribute key/value.

        Parameters
        ----------
        key : str
            Position attribute name used to define the exposure bucket.
        value : str
            Attribute value identifying the exposure bucket to floor.
        min_share : float
            Minimum decimal portfolio-weight share required in the bucket.
        label : str or None
            Optional stable label for diagnostics, reporting, and dual values.

        Returns
        -------
        Constraint
            Attribute-bucket constraint enforcing weighted exposure ``>= min_share``.

        Raises
        ------
        ValueError
            If ``min_share`` is outside ``[0, 1]`` or is non-finite.

        Examples
        --------
        >>> from finstack_quant.portfolio import Constraint
        >>> Constraint.exposure_minimum("sector", "tech", 0.10).kind
        'metric_bound'
        """
        ...

    def with_label(self, label: str) -> Constraint:
        """
        Return a copy with a human-readable label.

        Parameters
        ----------
        label : str
            Stable reader-facing name used in diagnostics and optimizer reports.

        Returns
        -------
        Constraint
            Copy carrying ``label``; budget constraints retain their fixed label.

        Notes
        -----
        This builder returns a copy with the field set and does not raise.

        """
        ...

    @classmethod
    def from_json(cls, json_str: str) -> Constraint:
        """
        Deserialize an optimization constraint from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized constraint, normally produced by
            ``Constraint.to_json``.

        Returns
        -------
        Constraint
            Validated `Constraint` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or cannot be deserialized into the documented portfolio type.

        Examples
        --------
        >>> from finstack_quant.portfolio import Constraint
        >>> original = Constraint.budget(1.0)
        >>> Constraint.from_json(original.to_json()).kind
        'budget'
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this optimization constraint to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `Constraint`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def kind(self) -> str:
        """
        Constraint variant label.

        Returns
        -------
        str
            Constraint variant label.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def label(self) -> str | None:
        """
        Optional human-readable label.

        Returns
        -------
            Optional human-readable label.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class CandidatePosition:
    """
    Candidate instrument the optimizer may add to the portfolio.

    Starts at weight zero and is bounded by ``min_weight`` / ``max_weight``.
    Attach candidates to a :class:`TradeUniverse` via ``with_candidate``.

    Examples
    --------
    >>> from finstack_quant.portfolio import CandidatePosition
    >>> try:
    ...     CandidatePosition("C1", "E1", "not-an-instrument")
    ... except ValueError:
    ...     print("invalid instrument payload")
    invalid instrument payload
    """

    def __init__(
        self,
        id: str,
        entity_id: str,
        instrument: Any,
        unit: str | dict[str, Any] | None = None,
        max_weight: float = 1.0,
        min_weight: float = 0.0,
        attributes: dict[str, str | float] | None = None,
    ) -> None:
        """
        Create a candidate.

        Parameters
        ----------
        id : str
            Identifier that becomes the position id if the optimizer trades it.
        entity_id : str
            Owning entity for the candidate.
        instrument : Bond | InterestRateSwap | ... | str
            Typed instrument wrapper or canonical instrument-envelope JSON.
        unit : str | dict | None
            Position unit: ``"units"`` (default), ``"face_value"``,
            ``"percentage"``, ``"notional"`` or ``{"notional": "USD"}``.
        max_weight : float
            Maximum weight (fraction) the candidate may receive.
        min_weight : float
            Minimum weight when included; ``0.0`` lets the optimizer skip it.
        attributes : dict[str, str | float] | None
            Attributes used by filters and exposure constraints.

        Raises
        ------
        TypeError
            If ``instrument`` is neither a typed instrument nor a string.
        ValueError
            If the instrument payload is invalid or ``unit`` is unknown.

        Examples
        --------
        >>> from finstack_quant.portfolio import CandidatePosition
        >>> try:
        ...     CandidatePosition("C1", "E1", "{}")
        ... except ValueError:
        ...     print("invalid instrument payload")
        invalid instrument payload
        """
        ...

    @staticmethod
    def from_json(json_str: str) -> CandidatePosition:
        """
        Parse from JSON (the instrument travels as its canonical tagged payload).

        Parameters
        ----------
        json_str : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        CandidatePosition
            Reconstructed candidate.

        Raises
        ------
        ValueError
            If the payload or embedded instrument is invalid.

        Examples
        --------
        >>> from finstack_quant.portfolio import CandidatePosition
        >>> try:
        ...     CandidatePosition.from_json("{}")
        ... except ValueError:
        ...     print("missing fields")
        missing fields
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to JSON.

        Returns
        -------
        str
            JSON accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If the instrument has no JSON representation.
        """
        ...

    @property
    def attributes(self) -> dict[str, str | float]:
        """
        Candidate attributes.

        Returns
        -------
        dict[str, str | float]
            Attribute values keyed by name.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def id(self) -> str:
        """
        Candidate position identifier.

        Returns
        -------
        str
            Candidate position identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def entity_id(self) -> str:
        """
        Candidate entity identifier.

        Returns
        -------
        str
            Candidate entity identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def max_weight(self) -> float:
        """
        Maximum allowed candidate weight.

        Returns
        -------
        float
            Maximum allowed candidate weight.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def min_weight(self) -> float:
        """
        Minimum allowed candidate weight.

        Returns
        -------
        float
            Minimum allowed candidate weight.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def instrument_id(self) -> str:
        """
        Underlying instrument identifier.

        Returns
        -------
        str
            Underlying instrument identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class TradeUniverse:
    """
    Universe of tradeable existing positions and candidate additions.

    Build with :meth:`all_positions` or :meth:`filtered`, chain
    :meth:`with_candidate` / :meth:`allow_shorting_candidates`, and attach
    it to a :class:`PortfolioOptimizationSpec` via ``with_trade_universe``.

    Examples
    --------
    >>> from finstack_quant.portfolio import TradeUniverse
    >>> TradeUniverse.all_positions().tradeable_filter.kind
    'all'
    """

    @classmethod
    def all_positions(cls) -> TradeUniverse:
        """
        Universe where every existing position is tradeable and no candidates exist.

        Returns
        -------
        TradeUniverse
            Default universe.

        Notes
        -----
        This constructor does not raise.

        Examples
        --------
        >>> from finstack_quant.portfolio import TradeUniverse
        >>> TradeUniverse.all_positions().allow_short_candidates
        False
        """
        ...

    @classmethod
    def filtered(cls, filter: PositionFilter) -> TradeUniverse:
        """
        Universe where only positions matching ``filter`` may trade.

        Parameters
        ----------
        filter : PositionFilter
            Filter selecting the tradeable existing positions.

        Returns
        -------
        TradeUniverse
            Universe with the supplied tradeable filter and no candidates.

        Notes
        -----
        This constructor does not raise.

        Examples
        --------
        >>> from finstack_quant.portfolio import PositionFilter, TradeUniverse
        >>> TradeUniverse.filtered(PositionFilter.by_entity_id("E1")).tradeable_filter.kind
        'by_entity_id'
        """
        ...

    def with_candidate(self, candidate: CandidatePosition) -> TradeUniverse:
        """
        Return a copy with ``candidate`` appended.

        Parameters
        ----------
        candidate : CandidatePosition
            Instrument the optimizer may add.

        Returns
        -------
        TradeUniverse
            New universe; the receiver is unchanged.

        Notes
        -----
        This method does not raise.
        """
        ...

    def allow_shorting_candidates(self) -> TradeUniverse:
        """
        Return a copy that lets candidates take negative weights.

        Returns
        -------
        TradeUniverse
            New universe with ``allow_short_candidates`` set.

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.portfolio import TradeUniverse
        >>> TradeUniverse.all_positions().allow_shorting_candidates().allow_short_candidates
        True
        """
        ...

    @staticmethod
    def from_json(json_str: str) -> TradeUniverse:
        """
        Parse from JSON.

        Parameters
        ----------
        json_str : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        TradeUniverse
            Reconstructed universe.

        Raises
        ------
        ValueError
            If the payload or an embedded candidate instrument is invalid.

        Examples
        --------
        >>> from finstack_quant.portfolio import TradeUniverse
        >>> TradeUniverse.from_json("{}").candidates
        []
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to JSON.

        Returns
        -------
        str
            JSON accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If a candidate instrument has no JSON representation.
        """
        ...

    @property
    def tradeable_filter(self) -> PositionFilter:
        """
        Filter selecting tradeable positions.

        Returns
        -------
        PositionFilter
            Filter selecting tradeable positions.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def held_filter(self) -> PositionFilter | None:
        """
        Optional filter selecting held positions.
        Returns
        -------
        PositionFilter or None

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def candidates(self) -> list[CandidatePosition]:
        """
        Candidate new positions.
        Returns
        -------
        list[CandidatePosition]

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def allow_short_candidates(self) -> bool:
        """
        Whether candidates may receive negative weights.

        Returns
        -------
        bool
            Whether candidates may receive negative weights.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class OptimizationStatus:
    """
    Status of an optimization run.

    Examples
    --------
    >>> from finstack_quant.portfolio import OptimizationStatus
    >>> (OptimizationStatus.optimal().kind, OptimizationStatus.optimal().is_feasible)
    ('optimal', True)
    """

    @classmethod
    def optimal(cls) -> OptimizationStatus:
        """
        Successful optimal solve.

        Returns
        -------
        OptimizationStatus
            Feasible status indicating that the solver proved optimality.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.portfolio import OptimizationStatus
        >>> (OptimizationStatus.optimal().kind, OptimizationStatus.optimal().is_feasible)
        ('optimal', True)
        """
        ...

    @classmethod
    def feasible_but_suboptimal(cls) -> OptimizationStatus:
        """
        Feasible solution that did not prove optimality.

        Returns
        -------
        OptimizationStatus
            Usable status indicating feasibility without proven optimality.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.portfolio import OptimizationStatus
        >>> OptimizationStatus.feasible_but_suboptimal().kind
        'feasible_but_suboptimal'
        """
        ...

    @classmethod
    def unbounded(cls) -> OptimizationStatus:
        """
        Optimization problem is unbounded.

        Returns
        -------
        OptimizationStatus
            Non-feasible status indicating an unbounded objective.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.portfolio import OptimizationStatus
        >>> OptimizationStatus.unbounded().is_feasible
        False
        """
        ...

    @classmethod
    def infeasible(cls, conflicting_constraints: list[str]) -> OptimizationStatus:
        """
        Create an infeasible status with the listed conflicting constraints.

        Parameters
        ----------
        conflicting_constraints : list[str]
            Constraint labels implicated in the infeasibility diagnosis.

        Returns
        -------
        OptimizationStatus
            Non-feasible status retaining the conflicting constraint labels.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.portfolio import OptimizationStatus
        >>> status = OptimizationStatus.infeasible(["budget"])
        >>> status.conflicting_constraints
        ['budget']
        """
        ...

    @classmethod
    def error(cls, message: str) -> OptimizationStatus:
        """
        Create a solver or model-building error status.

        Parameters
        ----------
        message : str
            Reader-facing error or diagnostic message returned by the optimizer.

        Returns
        -------
        OptimizationStatus
            Non-feasible solver-error status retaining ``message`` for diagnostics.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.portfolio import OptimizationStatus
        >>> OptimizationStatus.error("solver failed").message
        'solver failed'
        """
        ...

    @classmethod
    def from_json(cls, json_str: str) -> OptimizationStatus:
        """
        Deserialize an optimization status from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized solver status, normally produced by
            ``OptimizationStatus.to_json``.

        Returns
        -------
        OptimizationStatus
            Validated `OptimizationStatus` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or cannot be deserialized into the documented portfolio type.

        Examples
        --------
        >>> from finstack_quant.portfolio import OptimizationStatus
        >>> original = OptimizationStatus.infeasible(["budget"])
        >>> OptimizationStatus.from_json(original.to_json()).conflicting_constraints
        ['budget']
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this optimization status to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `OptimizationStatus`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def kind(self) -> str:
        """
        Discriminator for the optimization status variant.

        Returns
        -------
        str
            Discriminator for the optimization status variant.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def is_feasible(self) -> bool:
        """
        Whether the status includes a feasible solution.
        Returns
        -------
        bool
            Whether feasible holds for this `OptimizationStatus`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def conflicting_constraints(self) -> list[str]:
        """
        Constraint labels implicated in infeasibility.

        Returns
        -------
        list[str]
            Constraint labels implicated in infeasibility.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def message(self) -> str | None:
        """
        Error or diagnostic message, if present.

        Returns
        -------
            Error or diagnostic message, if present.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __eq__(self, other: object) -> bool:
        """Structural equality on the underlying Rust ``OptimizationStatus``.

        Two statuses are equal when they are the same variant with the same
        payload, e.g. ``OptimizationStatus.optimal() ==
        OptimizationStatus.optimal()``.

        Returns
        -------
        bool
        """
        ...

    def __hash__(self) -> int:
        """Hash consistent with :meth:`__eq__` (usable as dict/set keys).
        Returns
        -------
        int
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class TradeSpec:
    """
    Trade specification for a single position.

    Examples
    --------
    >>> from finstack_quant.portfolio import TradeSpec
    >>> doc = (
    ...     '{"position_id":"P1","instrument_id":"I1","trade_type":"existing",'
    ...     '"direction":"buy","current_quantity":1.0,"target_quantity":2.0,'
    ...     '"delta_quantity":1.0,"current_weight":0.1,"target_weight":0.2}'
    ... )
    >>> TradeSpec.from_json(doc).delta_quantity
    1.0
    """

    @classmethod
    def from_json(cls, json_str: str) -> TradeSpec:
        """
        Deserialize a trade specification from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized trade recommendation, normally produced by
            ``TradeSpec.to_json``.

        Returns
        -------
        TradeSpec
            Validated `TradeSpec` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or cannot be deserialized into the documented portfolio type.

        Examples
        --------
        >>> from finstack_quant.portfolio import TradeSpec
        >>> doc = (
        ...     '{"position_id":"P1","instrument_id":"I1","trade_type":"existing",'
        ...     '"direction":"buy","current_quantity":1.0,"target_quantity":2.0,'
        ...     '"delta_quantity":1.0,"current_weight":0.1,"target_weight":0.2}'
        ... )
        >>> TradeSpec.from_json(doc).direction.label
        'buy'
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this trade specification to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `TradeSpec`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def position_id(self) -> str:
        """
        Portfolio position identifier.

        Returns
        -------
        str
            Portfolio position identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def instrument_id(self) -> str:
        """
        Underlying instrument identifier.

        Returns
        -------
        str
            Underlying instrument identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def trade_type(self) -> TradeType:
        """
        Whether the trade adjusts an existing position, opens a new one, or

        Returns
        -------
        TradeType
            Whether the trade adjusts an existing position, opens a new one, or

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def direction(self) -> TradeDirection:
        """
        Buy, sell, or short direction for this trade spec.

        Returns
        -------
        TradeDirection
            Buy, sell, or short direction for this trade spec.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def current_quantity(self) -> float:
        """
        Quantity held before this trade is applied.

        Returns
        -------
        float
            Quantity held before this trade is applied.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def target_quantity(self) -> float:
        """
        Desired post-trade quantity in instrument units.

        Returns
        -------
        float
            Desired post-trade quantity in instrument units.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def delta_quantity(self) -> float:
        """
        Target quantity less current quantity.

        Returns
        -------
        float
            Target quantity less current quantity.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def current_weight(self) -> float:
        """
        Current portfolio weight.

        Returns
        -------
        float
            Current portfolio weight.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def target_weight(self) -> float:
        """
        Target portfolio weight, as a **fraction** rather than a percentage.

        Returns
        -------
        float
            Target portfolio weight, as a **fraction** rather than a percentage.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export this trade as a single-row pandas DataFrame.

        Columns: ``position_id``, ``instrument_id``, ``trade_type``,
        ``current_quantity``, ``target_quantity``, ``delta_quantity``,
        ``direction``, ``current_weight``, ``target_weight``.

        Returns
        -------
        pd.DataFrame
            Exactly one row, with the same schema
            :meth:`PortfolioOptimizationResult.to_trade_dataframe` emits, so
            single-trade frames concatenate cleanly onto a full trade list.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class PortfolioOptimizationSpec:
    """
    JSON-serializable portfolio optimization specification.

    Examples
    --------
    >>> from finstack_quant.portfolio import MetricExpr, Objective, PerPositionMetric, PortfolioOptimizationSpec
    >>> objective = Objective.maximize(MetricExpr.weighted_sum(PerPositionMetric.pv_base()))
    >>> spec_json = '{"id":"empty","base_currency":"USD","as_of":"2025-01-01","entities":{},"positions":[]}'
    >>> PortfolioOptimizationSpec.new(spec_json, objective).objective.direction
    'maximize'
    """

    @classmethod
    def new(
        cls,
        portfolio: Portfolio | str,
        objective: Objective,
    ) -> PortfolioOptimizationSpec:
        """
        Create an optimization specification from a portfolio and an objective.

        Parameters
        ----------
        portfolio : Portfolio | str
            Built :class:`Portfolio` (its canonical spec is captured) or a
            ``PortfolioSpec`` JSON string defining positions and starting
            weights.
        objective : Objective
            Direction and metric expression the optimizer should solve for.

        Returns
        -------
        PortfolioOptimizationSpec
            Spec with no constraints, value weighting, zero-fill missing
            metrics, no label and the default trade universe.

        Raises
        ------
        TypeError
            If ``portfolio`` is neither a ``Portfolio`` nor a string.
        ValueError
            If the JSON is malformed or does not match the ``PortfolioSpec``
            schema.

        Examples
        --------
        >>> from finstack_quant.portfolio import MetricExpr, Objective, PerPositionMetric, PortfolioOptimizationSpec
        >>> objective = Objective.maximize(MetricExpr.weighted_sum(PerPositionMetric.pv_base()))
        >>> spec_json = '{"id":"empty","base_currency":"USD","as_of":"2025-01-01","entities":{},"positions":[]}'
        >>> PortfolioOptimizationSpec.new(spec_json, objective).weighting.label
        'value_weight'
        """
        ...

    def with_trade_universe(self, universe: TradeUniverse) -> PortfolioOptimizationSpec:
        """
        Return a copy restricted to ``universe``.

        Parameters
        ----------
        universe : TradeUniverse
            Tradeable/held filters and candidate additions the optimizer
            honours.

        Returns
        -------
        PortfolioOptimizationSpec
            New spec; the receiver is unchanged.

        Notes
        -----
        This method does not raise.
        """
        ...

    @property
    def trade_universe(self) -> TradeUniverse | None:
        """
        Trade universe restricting the optimizer.

        Returns
        -------
        TradeUniverse | None
            ``None`` means every position is tradeable and no candidates.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def with_constraint(self, constraint: Constraint) -> PortfolioOptimizationSpec:
        """
        Return a copy with an additional constraint.

        Parameters
        ----------
        constraint : Constraint
            Constraint to append to the current optimization specification. An explicit
            budget replaces the implicit sum-of-weights budget of one when solving.

        Returns
        -------
        PortfolioOptimizationSpec
            Copy with ``constraint`` appended after the existing constraints.

        Notes
        -----
        This builder returns a copy with the field set and does not raise.

        """
        ...

    def with_objective(self, objective: Objective) -> PortfolioOptimizationSpec:
        """
        Return a copy with a replacement objective.

        Parameters
        ----------
        objective : Objective
            New direction and metric expression that replaces the existing one.

        Returns
        -------
        PortfolioOptimizationSpec
            Copy with the existing objective replaced by ``objective``.

        Notes
        -----
        This builder returns a copy with the field set and does not raise.

        """
        ...

    def with_weighting(self, weighting: WeightingScheme) -> PortfolioOptimizationSpec:
        """
        Return a copy with a replacement weighting scheme.

        Parameters
        ----------
        weighting : WeightingScheme
            Market-value or notional convention used to calculate portfolio weights.

        Returns
        -------
        PortfolioOptimizationSpec
            Copy with the existing weighting scheme replaced by ``weighting``.

        Notes
        -----
        This builder returns a copy with the field set and does not raise.

        """
        ...

    def with_missing_metric_policy(self, policy: MissingMetricPolicy) -> PortfolioOptimizationSpec:
        """
        Return a copy with a replacement missing-metric policy.

        Parameters
        ----------
        policy : MissingMetricPolicy
            Policy defining how positions without required metric data are handled.

        Returns
        -------
        PortfolioOptimizationSpec
            Copy with the existing missing-metric policy replaced by ``policy``.

        Notes
        -----
        This builder returns a copy with the field set and does not raise.

        """
        ...

    def with_label(self, label: str) -> PortfolioOptimizationSpec:
        """
        Return a copy with a human-readable label.

        Parameters
        ----------
        label : str
            Stable reader-facing name for reports, diagnostics, and persistence.

        Returns
        -------
        PortfolioOptimizationSpec
            Copy with its audit and reporting label set to ``label``.

        Notes
        -----
        This builder returns a copy with the field set and does not raise.

        """
        ...

    @classmethod
    def from_json(cls, json_str: str) -> PortfolioOptimizationSpec:
        """
        Deserialize an optimization specification from canonical JSON.

        Parameters
        ----------
        json_str : str
            Canonical serialized portfolio optimization specification, normally
            produced by ``PortfolioOptimizationSpec.to_json``.

        Returns
        -------
        PortfolioOptimizationSpec
            Validated `PortfolioOptimizationSpec` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or cannot be deserialized into the documented portfolio type.

        Examples
        --------
        >>> from finstack_quant.portfolio import MetricExpr, Objective, PerPositionMetric, PortfolioOptimizationSpec
        >>> objective = Objective.maximize(MetricExpr.weighted_sum(PerPositionMetric.pv_base()))
        >>> spec_json = '{"id":"empty","base_currency":"USD","as_of":"2025-01-01","entities":{},"positions":[]}'
        >>> original = PortfolioOptimizationSpec.new(spec_json, objective)
        >>> PortfolioOptimizationSpec.from_json(original.to_json()).objective.direction
        'maximize'
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this optimization specification to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `PortfolioOptimizationSpec`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def objective(self) -> Objective:
        """
        Objective maximized or minimized by this optimization spec.

        Returns
        -------
        Objective
            Objective maximized or minimized by this optimization spec.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def constraints(self) -> list[Constraint]:
        """
        Optimization constraints.

        Returns
        -------
        list[Constraint]
            Optimization constraints.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def weighting(self) -> WeightingScheme:
        """
        Weighting scheme used by the optimization.

        Returns
        -------
        WeightingScheme
            Weighting scheme used by the optimization.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def missing_metric_policy(self) -> MissingMetricPolicy:
        """
        Policy for missing per-position metrics.
        Returns
        -------
        MissingMetricPolicy

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def label(self) -> str | None:
        """
        Optional human-readable label.

        Returns
        -------
            Optional human-readable label.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def portfolio_spec_json(self) -> str:
        """
        Return the embedded portfolio specification JSON.
        Returns
        -------
        str
            Compact canonical JSON for the embedded ``PortfolioSpec``.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class PortfolioOptimizationResult:
    """
    Result of an optimization run.

    Round-trips through ``to_json`` / ``from_json`` and therefore supports
    ``pickle``, ``copy.deepcopy``, and ``multiprocessing``.

    Examples
    --------
    >>> from finstack_quant.portfolio import PortfolioOptimizationResult
    >>> try:
    ...     PortfolioOptimizationResult()
    ... except TypeError as exc:
    ...     print(exc)
    cannot create 'finstack_quant.portfolio.PortfolioOptimizationResult' instances
    """

    @staticmethod
    def from_json(json_str: str) -> PortfolioOptimizationResult:
        """
        Rebuild a `PortfolioOptimizationResult` from ``to_json`` output.

        Parameters
        ----------
        json_str : str
            Canonical JSON produced by :meth:`to_json`.

        Returns
        -------
        PortfolioOptimizationResult
            The reconstructed optimization result, field for field.

        Raises
        ------
        ValueError
            If ``json_str`` is malformed or does not match the wire schema.

        Examples
        --------
        >>> from finstack_quant.portfolio import PortfolioOptimizationResult
        >>> restored = PortfolioOptimizationResult.from_json(result.to_json())  # doctest: +SKIP
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this optimization result to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `PortfolioOptimizationResult`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def status(self) -> OptimizationStatus:
        """
        Solver status after the portfolio optimization run.
        Returns
        -------
        OptimizationStatus

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def is_feasible(self) -> bool:
        """
        Whether the solver returned a feasible portfolio.
        Returns
        -------
        bool
            Whether feasible holds for this `PortfolioOptimizationResult`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def objective_value(self) -> float:
        """
        Objective value at the solution.

        Returns
        -------
        float
            Objective value at the solution.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def current_weights(self) -> dict[str, float]:
        """
        Current weights by position ID.

        Returns
        -------
        dict[str, float]
            Current weights by position ID.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def optimal_weights(self) -> dict[str, float]:
        """
        Optimized target weights by position ID.

        Returns
        -------
        dict[str, float]
            Optimized target weights by position ID.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def weight_deltas(self) -> dict[str, float]:
        """
        Target less current weight by position ID.

        Returns
        -------
        dict[str, float]
            Target less current weight by position ID.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def implied_quantities(self) -> dict[str, float]:
        """
        Implied target quantities by position ID.

        Returns
        -------
        dict[str, float]
            Implied target quantities by position ID.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def metric_values(self) -> dict[str, float]:
        """
        Portfolio metric values at the solution.

        Returns
        -------
        dict[str, float]
            Portfolio metric values at the solution.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def constraint_slacks(self) -> dict[str, float]:
        """
        Constraint slack values by constraint label.

        Returns
        -------
        dict[str, float]
            Constraint slack values by constraint label.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def turnover(self) -> float:
        """
        Total turnover implied by the solution.

        Returns
        -------
        float
            Total turnover implied by the solution.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_rebalanced_portfolio(self) -> Portfolio:
        """
        Rebuild the portfolio with the implied post-trade quantities.

        Existing positions take their ``implied_quantities`` entry (positions
        outside the trade universe keep their quantity) and traded candidates
        become new positions.

        Returns
        -------
        Portfolio
            Built portfolio ready for ``value_portfolio``.

        Raises
        ------
        RuntimeError
            If the solution is infeasible, or this result was rebuilt from
            JSON / unpickled (the live problem is not part of the wire form).
        """
        ...

    def to_trade_list(self) -> list[TradeSpec]:
        """
        Convert weight deltas into trade specifications.
        Returns
        -------
        list[TradeSpec]
            Material trades sorted by descending absolute quantity change.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """
        ...

    def new_position_trades(self) -> list[TradeSpec]:
        """
        Return trade specs for new candidate positions only.
        Returns
        -------
        list[TradeSpec]
            New-candidate subset of ``to_trade_list()``, preserving its order.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the per-position weight and quantity solution, indexed by position.

        ``current_weights``, ``optimal_weights``, ``weight_deltas`` and
        ``implied_quantities`` share one position key space, so they are
        joined into a single frame. The row axis is the union of their keys,
        ordered by first appearance (``current_weights`` first); a value
        missing from one map becomes ``None`` in that column — this is how
        candidate positions with no current weight show up.

        Columns: ``current_weight``, ``optimal_weight``, ``weight_delta`` (all
        fractions of the portfolio, not percentages), ``implied_quantity``
        (units / face / notional per the weighting scheme).

        Returns
        -------
        pd.DataFrame
            One row per position; the index holds the position ids.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_trade_dataframe(self) -> pd.DataFrame:
        """
        Export :meth:`to_trade_list` as a pandas DataFrame.

        One row per trade, in the same order as :meth:`to_trade_list` (sorted
        by absolute quantity delta, largest first).

        Columns: ``position_id``, ``instrument_id``, ``trade_type``
        (``"existing"``, ``"new_position"``, ``"close_out"``),
        ``current_quantity``, ``target_quantity``, ``delta_quantity``,
        ``direction`` (``"buy"``, ``"sell"``, ``"hold"``), ``current_weight``,
        ``target_weight`` (weights are fractions, not percentages).

        Returns
        -------
        pd.DataFrame
            One row per trade; a solution that requires no trades yields a
            zero-row frame that still carries the column schema.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def binding_constraints(self) -> list[tuple[str, float]]:
        """
        Return constraints with near-zero slack as ``(label, slack)`` pairs.
        Returns
        -------
        list[tuple[str, float]]

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

def optimize_portfolio(
    spec: PortfolioOptimizationSpec,
    market: MarketContext | str,
) -> PortfolioOptimizationResult:
    """
    Optimize portfolio weights using the LP-based optimizer.

    Parameters
    ----------
    spec : PortfolioOptimizationSpec
        Typed portfolio definition, objective, constraints, and solver policy.
    market : MarketContext or str
        Market context object or JSON used to value positions and calculate metrics.

    Returns
    -------
    PortfolioOptimizationResult
        Typed optimal weights, trades, constraint values, and diagnostics.

    Raises
    ------
    TypeError
        If ``market`` is neither a ``MarketContext`` nor a JSON string.
    ValueError
        If supplied market JSON is malformed or schema-incompatible.
    PortfolioError
        If the embedded portfolio specification cannot be constructed or an
        operational optimization failure occurs before a result is available.
    ValuationError
        If a required position metric cannot be valued.
    FxError
        If a required base-currency conversion is unavailable.

    Examples
    --------
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.portfolio import (
    ...     MetricExpr,
    ...     Objective,
    ...     PerPositionMetric,
    ...     PortfolioError,
    ...     PortfolioOptimizationSpec,
    ...     optimize_portfolio,
    ... )
    >>> objective = Objective.maximize(MetricExpr.weighted_sum(PerPositionMetric.pv_base()))
    >>> portfolio = '{"id":"empty","base_currency":"USD","as_of":"2025-01-01","entities":{},"positions":[]}'
    >>> spec = PortfolioOptimizationSpec.new(portfolio, objective)
    >>> try:
    ...     optimize_portfolio(spec, MarketContext())
    ... except PortfolioError as exc:
    ...     print("no decision variables" in str(exc))
    True
    """
    ...

# Factor Sensitivity

class SensitivityMatrix:
    """
    Positions-by-factors sensitivity matrix.

    Each element ``(i, j)`` is the first-order sensitivity of position *i* to
    factor *j*, denominated in the factor's bump units (e.g. PV change per 1 bp
    for a rates factor).

    Construct via :func:`compute_factor_sensitivities`.

    Examples
    --------
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.portfolio import compute_factor_sensitivities
    >>> matrix = compute_factor_sensitivities("[]", "[]", MarketContext(), "2025-01-15", "USD")
    >>> (matrix.n_positions, matrix.n_factors)
    (0, 0)
    """

    @property
    def base_currency(self) -> str:
        """
        ISO reporting currency of monetary entries.

        Returns
        -------
        str
            Three-letter currency code used for the calculation.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @staticmethod
    def from_json(json: str) -> SensitivityMatrix:
        """
        Parse from canonical JSON produced by :meth:`to_json`.

        Parameters
        ----------
        json : str
            Canonical payload.

        Returns
        -------
        SensitivityMatrix
            Reconstructed value.

        Raises
        ------
        ValueError
            If the payload is malformed.

        Examples
        --------
        >>> from finstack_quant.portfolio import SensitivityMatrix
        >>> try:
        ...     SensitivityMatrix.from_json("{}")
        ... except ValueError:
        ...     print("missing fields")
        missing fields
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            JSON accepted by :meth:`from_json`; also backs ``pickle``.

        Notes
        -----
        This method does not raise for a well-formed value.
        """
        ...

    @property
    def position_ids(self) -> list[str]:
        """
        Ordered position identifiers (row axis).

        Returns
        -------
        list[str]
            Ordered position identifiers (row axis).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def factor_ids(self) -> list[str]:
        """
        Ordered factor identifiers (column axis).

        Returns
        -------
        list[str]
            Ordered factor identifiers (column axis).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def n_positions(self) -> int:
        """
        Number of positions (rows).

        Returns
        -------
        int
            Number of positions (rows).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def n_factors(self) -> int:
        """
        Number of factors (columns).

        Returns
        -------
        int
            Number of factors (columns).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def delta(self, position_idx: int, factor_idx: int) -> float:
        """
        Read a single sensitivity element.

        Parameters
        ----------
        position_idx : int
            Zero-based row index into ``position_ids`` for the requested position.
        factor_idx : int
            Column index.

        Returns
        -------
        float
            Sensitivity value.

        Raises
        ------
        ValueError
            If ``position_idx`` or ``factor_idx`` is outside the matrix bounds.
        """
        ...

    def position_deltas(self, position_idx: int) -> list[float]:
        """
        Sensitivity row for a single position across all factors.

        Parameters
        ----------
        position_idx : int
            Zero-based row index into ``position_ids`` for the requested position.

        Returns
        -------
        list[float]
            List of delta values, one per factor.

        Raises
        ------
        ValueError
            If ``position_idx`` is outside the matrix bounds.
        """
        ...

    def factor_deltas(self, factor_idx: int) -> list[float]:
        """
        Sensitivity column for a single factor across all positions.

        Parameters
        ----------
        factor_idx : int
            Column index.

        Returns
        -------
        list[float]
            List of delta values, one per position.

        Raises
        ------
        ValueError
            If ``factor_idx`` is outside the matrix bounds.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export as a pandas DataFrame with positions as rows and factors as columns.

        Returns
        -------
        pd.DataFrame
            DataFrame indexed by position IDs with factor IDs as column names.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class FactorPnlProfile:
    """
    P&L profile for one factor across a scenario grid.

    Each profile captures the hypothetical P&L for every position at each
    scenario shift, enabling non-linear (gamma, convexity) analysis.

    Construct via :func:`compute_pnl_profiles`.

    Examples
    --------
    >>> from finstack_quant.portfolio import FactorPnlProfile
    >>> profile = FactorPnlProfile.from_json(
    ...     '{"base_currency":"USD","factor_id":"USD_10Y","position_ids":["P1"],"shifts":[-1.0,0.0,1.0],"position_pnls":[[-1.0],[0.0],[1.0]]}'
    ... )
    >>> profile.to_dataframe()["P1"].tolist()
    [-1.0, 0.0, 1.0]
    """

    @property
    def base_currency(self) -> str:
        """
        ISO reporting currency of monetary entries.

        Returns
        -------
        str
            Three-letter currency code used for the calculation.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @staticmethod
    def from_json(json: str) -> FactorPnlProfile:
        """
        Parse from canonical JSON produced by :meth:`to_json`.

        Parameters
        ----------
        json : str
            Canonical payload.

        Returns
        -------
        FactorPnlProfile
            Reconstructed value.

        Raises
        ------
        ValueError
            If the payload is malformed.

        Examples
        --------
        >>> from finstack_quant.portfolio import FactorPnlProfile
        >>> try:
        ...     FactorPnlProfile.from_json("{}")
        ... except ValueError:
        ...     print("missing fields")
        missing fields
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            JSON accepted by :meth:`from_json`; also backs ``pickle``.

        Notes
        -----
        This method does not raise for a well-formed value.
        """
        ...

    @property
    def factor_id(self) -> str:
        """
        Identifier of the contributing risk factor in the model.

        Returns
        -------
        str
            Identifier of the contributing risk factor in the model.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def position_ids(self) -> list[str]:
        """
        Ordered position identifiers indexing the inner ``position_pnls`` axis.

        Returns
        -------
        list[str]
            Column labels used by :meth:`to_dataframe`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def shifts(self) -> list[float]:
        """
        Scenario shift coordinates (bump-size multiples).

        Returns
        -------
        list[float]
            Scenario shift coordinates (bump-size multiples).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def position_pnls(self) -> list[list[float]]:
        """
        Per-shift P&L vectors indexed as ``[shift_idx][position_idx]``.

        Returns
        -------
        list[list[float]]
            Per-shift P&L vectors indexed as ``[shift_idx][position_idx]``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export as a pandas DataFrame with shifts as rows and positions as columns.

        Returns
        -------
        pd.DataFrame
            Indexed by shift value; one column per entry of ``position_ids``.

        Raises
        ------
        ValueError
            If the frame cannot be built.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

def compute_factor_sensitivities(
    positions_json: str | dict[str, Any] | list[Any] | pd.DataFrame,
    factors_json: str | dict[str, Any] | list[Any] | pd.DataFrame,
    market: MarketContext | str,
    as_of: datetime.date | str,
    base_currency: str,
    bump_config_json: str | None = None,
) -> SensitivityMatrix:
    """
        Compute first-order factor sensitivities using central finite differences.

        Parameters
        ----------
        positions_json : str
            JSON array of position objects, each with ``id`` (str),
    ``instrument`` (canonical v1 instrument envelope), and ``weight`` (float).
        factors_json : str
            JSON array of ``FactorDefinition`` objects.
        market : MarketContext or str
            ``MarketContext`` instance or JSON string.
        as_of : datetime.date | str
            Valuation date, either a date-like object or an ISO 8601 string.
        base_currency : str
            ISO reporting currency for every sensitivity or P&L amount. Missing FX raises KeyError.
        bump_config_json : str, optional
            Optional JSON-serialized ``BumpSizeConfig``.
            Defaults to 1 bp / 1 % per factor type.

        Returns
        -------
        SensitivityMatrix
            Positions-by-factors delta matrix.

        Raises
        ------
        TypeError
            If ``market`` is neither a ``MarketContext`` nor a JSON string.
        ValueError
            If position, factor, market, bump-configuration, or date input is
            malformed or invalid, or a factor cannot be bumped or priced.
        KeyError
            If repricing requires market data that is absent from ``market``.

        Examples
        --------
        >>> from finstack_quant.core.market_data import MarketContext
        >>> from finstack_quant.portfolio import compute_factor_sensitivities
        >>> matrix = compute_factor_sensitivities("[]", "[]", MarketContext(), "2025-01-15", "USD")
        >>> matrix.to_dataframe().shape
        (0, 0)
    """
    ...

def compute_pnl_profiles(
    positions_json: str | dict[str, Any] | list[Any] | pd.DataFrame,
    factors_json: str | dict[str, Any] | list[Any] | pd.DataFrame,
    market: MarketContext | str,
    as_of: datetime.date | str,
    base_currency: str,
    bump_config_json: str | None = None,
    n_scenario_points: int = 5,
) -> list[FactorPnlProfile]:
    """
    Compute scenario P&L profiles via full repricing across a factor grid.

    Parameters
    ----------
    positions_json : str | dict | list | pandas.DataFrame
        JSON array of position objects (same schema as
        :func:`compute_factor_sensitivities`).
    factors_json : str | dict | list | pandas.DataFrame
        JSON array of ``FactorDefinition`` objects.
    market : MarketContext or str
        ``MarketContext`` instance or JSON string.
    as_of : datetime.date | str
        Valuation date, either a date-like object or an ISO 8601 string.
    base_currency : str
        ISO reporting currency for every P&L amount; missing FX raises KeyError.
    bump_config_json : str, optional
        Optional JSON-serialized ``BumpSizeConfig``.
    n_scenario_points : int, default 5
        Number of scenario grid points
        (default 5 produces shifts ``[-2, -1, 0, 1, 2]``).

    Returns
    -------
    list[FactorPnlProfile]
        One profile per factor, each containing scenario P&L for every position.

    Raises
    ------
    TypeError
        If ``market`` is neither a ``MarketContext`` nor a JSON string.
    ValueError
        If position, factor, market, bump-configuration, or date input is
        malformed or invalid; if ``n_scenario_points`` is less than ``3`` or
        even; or if a factor cannot be bumped or priced.
    KeyError
        If repricing requires market data that is absent from ``market``.

    Examples
    --------
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.portfolio import compute_pnl_profiles
    >>> compute_pnl_profiles("[]", "[]", MarketContext(), "2025-01-15", "USD")
    []
    """
    ...

# Risk Decomposition

class FactorRiskDecomposition:
    """
    Portfolio-level decomposition of total risk across factors and positions.

    Obtain via :func:`decompose_factor_risk`.  The decomposition expresses
    forecasted portfolio risk (variance, volatility, VaR, or ES) as a sum of
    Euler-allocated factor-level contributions, each drillable to per-position
    detail.

    Examples
    --------
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.portfolio import compute_factor_sensitivities, decompose_factor_risk
    >>> matrix = compute_factor_sensitivities("[]", "[]", MarketContext(), "2025-01-15", "USD")
    >>> decompose_factor_risk(matrix, '{"factor_ids":[],"n":0,"data":[]}').total_risk
    0.0
    """

    @staticmethod
    def from_json(json: str) -> FactorRiskDecomposition:
        """
        Parse from canonical JSON produced by :meth:`to_json`.

        Parameters
        ----------
        json : str
            Canonical payload.

        Returns
        -------
        FactorRiskDecomposition
            Reconstructed value.

        Raises
        ------
        ValueError
            If the payload is malformed.

        Examples
        --------
        >>> from finstack_quant.portfolio import FactorRiskDecomposition
        >>> try:
        ...     FactorRiskDecomposition.from_json("{}")
        ... except ValueError:
        ...     print("missing fields")
        missing fields
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            JSON accepted by :meth:`from_json`; also backs ``pickle``.

        Notes
        -----
        This method does not raise for a well-formed value.
        """
        ...

    @property
    def total_risk(self) -> float:
        """
        Total portfolio risk under the selected measure.

        Returns
        -------
        float
            Total portfolio risk under the selected measure.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def measure(self) -> str:
        """
        Risk-measure tag in canonical snake_case serde form: ``"variance"``,
        ``"volatility"``, ``"var"``, or ``"expected_shortfall"``. Matches the
        tag reported by the WASM ``decomposeFactorRisk`` output.

        Returns
        -------
        str
            Risk-measure tag in canonical snake_case serde form: ``"variance"``,

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def residual_risk(self) -> float:
        """
        Residual (idiosyncratic) risk not attributed to any factor.

        Returns
        -------
        float
            Residual (idiosyncratic) risk not attributed to any factor.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def factor_contributions(self) -> list[dict[str, object]]:
        """
        Factor-level contributions as a list of dicts.

        Each dict contains ``factor_id``, ``absolute_risk``, ``relative_risk``,
        and ``marginal_risk``.

        Returns
        -------
        list[dict[str, object]]
            List of per-factor contribution dicts.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def position_factor_contributions(self) -> list[dict[str, object]]:
        """
        Position x factor contributions as a list of dicts.

        Each dict contains ``position_id``, ``factor_id``, and
        ``risk_contribution``.

        Returns
        -------
        list[dict[str, object]]
            List of per-position, per-factor contribution dicts.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def position_residual_contributions(self) -> list[dict[str, Any]]:
        """
        Per-position residual (idiosyncratic) variance contributions.

        Each dict contains ``position_id``, ``residual_variance`` (annualized
        variance units), and a ``source`` object tagged by ``kind``. Empty for
        the parametric decomposer used by :func:`decompose_factor_risk` —
        populated only by credit-aware position decomposers.

        Returns
        -------
        list[dict[str, Any]]
            List of per-position residual contribution dicts.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Primary table: the factor-level risk decomposition.

        Alias of :meth:`to_factor_dataframe`. Every tabular result type in the library
        answers ``to_dataframe()``; the position-level views stay on :meth:`to_position_factor_dataframe`
        and :meth:`to_position_residual_dataframe`.

        Returns
        -------
        pd.DataFrame
            The same frame :meth:`to_factor_dataframe` returns.

        Examples
        --------
        >>> frame = result.to_dataframe()  # doctest: +SKIP

        Notes
        -----
        This alias does not raise; it delegates to the method named above.
        """
        ...

    def to_factor_dataframe(self) -> pd.DataFrame:
        """
        Export factor contributions as a pandas DataFrame.

        Columns: ``factor_id``, ``absolute_risk``, ``relative_risk``,
        ``marginal_risk``.

        Returns
        -------
        pd.DataFrame
            DataFrame with one row per factor.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_position_factor_dataframe(self) -> pd.DataFrame:
        """
        Export position x factor contributions as a pandas DataFrame.

        Columns: ``position_id``, ``factor_id``, ``risk_contribution``.

        Returns
        -------
        pd.DataFrame
            DataFrame with one row per position-factor pair.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

def decompose_factor_risk(
    sensitivities: SensitivityMatrix,
    covariance_json: str,
    risk_measure: str | dict[str, Any] | None = None,
) -> FactorRiskDecomposition:
    """
    Decompose portfolio risk into factor and position contributions.

    Uses the parametric (covariance-based) Euler decomposition to attribute
    forecasted portfolio risk across factors and individual positions.

    Parameters
    ----------
    sensitivities : SensitivityMatrix
        Weighted position x factor sensitivity matrix, as
        returned by :func:`compute_factor_sensitivities`.
    covariance_json : str
        JSON-serialized ``FactorCovarianceMatrix``.  Must use
        the same factor IDs and ordering as the sensitivity matrix.
    risk_measure : str or dict[str, Any] or None, optional
        Risk measure.  Defaults to ``"variance"``.
        Accepts Python strings (``"variance"``, ``"volatility"``) or dicts
        (``{"var": {"confidence": 0.99}}``,
        ``{"expected_shortfall": {"confidence": 0.975}}``).

    Returns
    -------
    FactorRiskDecomposition
        Portfolio-level risk decomposition with factor and position detail.

    Raises
    ------
    ValueError
        If factor axes do not match or the covariance matrix is
        invalid.

    Examples
    --------
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.portfolio import compute_factor_sensitivities, decompose_factor_risk
    >>> matrix = compute_factor_sensitivities("[]", "[]", MarketContext(), "2025-01-15", "USD")
    >>> result = decompose_factor_risk(matrix, '{"factor_ids":[],"n":0,"data":[]}', "volatility")
    >>> (result.total_risk, result.to_factor_dataframe().shape)
    (0.0, (0, 4))
    """
    ...
