"""Stub declarations mirroring ``finstack-quant-py/src/bindings/portfolio/factor_model/assignment.rs``."""

from __future__ import annotations

import pandas as pd

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
    def from_json(cls, json: str) -> PositionAssignment:
        """
        Deserialize a position assignment from canonical JSON.

        Parameters
        ----------
        json : str
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
    def from_json(cls, json: str) -> UnmatchedEntry:
        """
        Deserialize an unmatched dependency entry from canonical JSON.

        Parameters
        ----------
        json : str
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
    def from_json(cls, json: str) -> FactorAssignmentReport:
        """
        Deserialize a factor assignment report from canonical JSON.

        Parameters
        ----------
        json : str
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
