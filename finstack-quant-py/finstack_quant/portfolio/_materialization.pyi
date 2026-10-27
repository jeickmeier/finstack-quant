"""Stub declarations mirroring ``finstack-quant-py/src/bindings/portfolio/materialization.rs``."""

from __future__ import annotations

from typing import Any
import pandas as pd

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
