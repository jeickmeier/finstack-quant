"""Stub declarations mirroring ``finstack-quant-py/src/bindings/portfolio/optimization/expressions.rs``."""

from __future__ import annotations
from finstack_quant.portfolio._optimization.enums import Inequality

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

        Notes
        -----
        This method does not raise; it builds the constraint as given.
        The interval is checked when the problem is optimized:
        :func:`optimize_portfolio` raises ``PortfolioError`` for a non-finite bound or ``min > max``.

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

        Notes
        -----
        This method does not raise; it builds the constraint as given.
        The cap is checked when the problem is optimized:
        :func:`optimize_portfolio` raises ``PortfolioError`` for a negative or non-finite ``max_turnover``.

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

        Notes
        -----
        This method does not raise; it builds the constraint as given.
        ``rhs`` is checked when the problem is optimized:
        :func:`optimize_portfolio` raises ``PortfolioError`` for a negative or non-finite ``rhs``.

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
        PortfolioError
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
        PortfolioError
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
