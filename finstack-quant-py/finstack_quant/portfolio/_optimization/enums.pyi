"""Stub declarations mirroring ``finstack-quant-py/src/bindings/portfolio/optimization/enums.rs``."""

from __future__ import annotations

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
            Policy that preserves existing positions' exact quantities and
            weights when an in-scope standard metric, custom key, or numeric
            attribute is missing. Drops each missing input from ``WeightedSum``
            and ``ValueWeightedAverage`` coefficients and omits it from a
            value-weighted-average denominator. Only expressions whose filters
            match the position can cause it to freeze.

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
    Trade direction from the signed quantity change (buy/sell/hold).

    Examples
    --------
    >>> from finstack_quant.portfolio import TradeDirection
    >>> TradeDirection.buy().label
    'buy'
    """

    @classmethod
    def buy(cls) -> TradeDirection:
        """
        Direction for increasing the signed instrument quantity.

        Returns
        -------
        TradeDirection
            Buy direction for a positive quantity change, including covering a short.

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
        Direction for decreasing the signed instrument quantity.

        Returns
        -------
        TradeDirection
            Sell direction for a negative quantity change, including increasing a short.

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
            Direction representing no change in instrument quantity.

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
        Buy/sell/hold label for the trade direction.

        Returns
        -------
        str
            Canonical ``buy``, ``sell``, or ``hold`` label.

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
