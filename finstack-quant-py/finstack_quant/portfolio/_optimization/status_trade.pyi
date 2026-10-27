"""Stub declarations mirroring ``finstack-quant-py/src/bindings/portfolio/optimization/status_trade.rs``."""

from __future__ import annotations

import pandas as pd
from finstack_quant.portfolio._optimization.enums import TradeDirection, TradeType

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
    def infeasible(cls) -> OptimizationStatus:
        """
        No feasible solution exists.

        The LP backend does not expose an irreducible infeasible subset, so the
        status names no conflicting constraints.

        Returns
        -------
        OptimizationStatus
            Non-feasible status for an infeasible problem.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.portfolio import OptimizationStatus
        >>> status = OptimizationStatus.infeasible()
        >>> (status.kind, status.is_feasible)
        ('infeasible', False)
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
        >>> original = OptimizationStatus.error("solver failed")
        >>> OptimizationStatus.from_json(original.to_json()).message
        'solver failed'
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
            ``True`` only for ``optimal``.

        Notes
        -----
        This accessor does not raise; it is derived from the variant.
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
        Buy, sell, or hold classification from the signed quantity change.

        Returns
        -------
        TradeDirection
            ``buy`` for positive ``delta_quantity``, ``sell`` for negative, and
            ``hold`` for zero. This convention also applies to short holdings.

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
