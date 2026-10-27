"""Stub declarations mirroring ``finstack-quant-py/src/bindings/scenarios/operation_spec/hierarchy.rs``."""

from __future__ import annotations

from typing import Any

class HierarchyTarget:
    """
    Path into the market-data hierarchy, with an optional tag filter, that a
    hierarchy-targeted operation resolves against the execution context's
    ``MarketDataHierarchy``.

    Parameters
    ----------
    path : list[str]
        Hierarchy path from the root, e.g. ``["Credit", "US", "IG"]``; every
        curve in that subtree is targeted.
    tag_filter : dict[str, Any] | None, default None
        Canonical ``{"predicates": [...]}`` filter with equals, in and exists
        predicates combined with AND semantics. ``None`` matches the subtree.

    Raises
    ------
    TypeError
        If ``path`` is not a list of strings or ``tag_filter`` is not JSON-serializable.
    ValueError
        If predicates do not match the canonical TagFilter shape.

    Examples
    --------
    >>> from finstack_quant.scenarios import HierarchyTarget
    >>> target = HierarchyTarget(
    ...     ["Credit", "US"], {"predicates": [{"equals": {"key": "sector", "value": "financials"}}]}
    ... )
    >>> target.path
    ['Credit', 'US']
    >>> HierarchyTarget.from_json(target.to_json()) == target
    True
    """

    def __init__(self, path: list[str], tag_filter: dict[str, Any] | None = None) -> None: ...
    @property
    def path(self) -> list[str]:
        """
        Hierarchy path from the root.

        Returns
        -------
        list[str]
            Node labels from the root to the targeted subtree.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def tag_filter(self) -> dict[str, Any] | None:
        """
        Complete canonical tag predicates, or ``None`` when no filter is set.

        Returns
        -------
        dict[str, Any] or None
            Canonical ``{"predicates": [...]}`` data, including equals, in
            and exists predicates in their original order.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to canonical JSON (``{"path": [...], "tag_filter": {...}}``).

        Returns
        -------
        str
            Canonical ``HierarchyTarget`` JSON.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @staticmethod
    def from_json(json: str) -> HierarchyTarget:
        """
        Deserialize from canonical JSON, including ``in`` / ``exists`` tag
        predicates accepted by the constructor.

        Parameters
        ----------
        json : str
            Canonical ``HierarchyTarget`` JSON.

        Returns
        -------
        HierarchyTarget
            Parsed target.

        Raises
        ------
        ValueError
            If the JSON does not match the ``HierarchyTarget`` contract.

        Examples
        --------
        >>> from finstack_quant.scenarios import HierarchyTarget
        >>> HierarchyTarget.from_json('{"path":["Credit"]}').path
        ['Credit']
        """
        ...

    def __eq__(self, other: object) -> bool:
        """
        Structural equality on path and tag filter.

        Parameters
        ----------
        other : object
            Value to compare; non-``HierarchyTarget`` values compare unequal.

        Returns
        -------
        bool
            ``True`` when both targets serialize identically.

        Raises
        ------
        None
            This method does not raise.
        """
        ...
