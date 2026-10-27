"""Stub declarations mirroring ``finstack-quant-py/src/bindings/statements/adjustments.rs``."""

from __future__ import annotations

from collections.abc import Mapping
from finstack_quant.statements._evaluator import StatementResult

class AppliedAdjustment:
    """One adjustment applied to a normalized statement metric.

    Examples
    --------
    >>> from finstack_quant.statements import NormalizationResult
    >>> payload = '{"value_type":{"type":"scalar"},"period":"2025Q1","base_value":10.0,"adjustments":[{"adjustment_id":"a1","name":"Add-back","raw_amount":2.0,"capped_amount":1.5,"is_capped":true}],"final_value":11.5}'
    >>> NormalizationResult.from_json(payload).adjustments[0].capped_amount
    1.5
    """

    @property
    def adjustment_id(self) -> str:
        """Return the stable adjustment identifier.

        Returns
        -------
        str
            Identifier supplied by the normalization configuration.

        Notes
        -----
        This accessor does not raise; it returns stored result data.
        """
        ...
    @property
    def name(self) -> str:
        """Return the human-readable adjustment name.

        Returns
        -------
        str
            Display name stored with the applied adjustment.

        Notes
        -----
        This accessor does not raise; it returns stored result data.
        """
        ...
    @property
    def raw_amount(self) -> float:
        """Return the uncapped signed adjustment amount.

        Returns
        -------
        float
            Amount in the normalized metric's reporting units.

        Notes
        -----
        This accessor does not raise; it returns stored result data.
        """
        ...
    @property
    def capped_amount(self) -> float:
        """Return the signed adjustment amount after cap enforcement.

        Returns
        -------
        float
            Applied amount in the normalized metric's reporting units.

        Notes
        -----
        This accessor does not raise; it returns stored result data.
        """
        ...
    @property
    def is_capped(self) -> bool:
        """Report whether a configured cap changed the raw amount.

        Returns
        -------
        bool
            ``True`` when ``capped_amount`` differs from ``raw_amount``.

        Notes
        -----
        This accessor does not raise; it returns stored result data.
        """
        ...
    def __repr__(self) -> str: ...
    @staticmethod
    def from_json(json: str, /) -> AppliedAdjustment:
        """Reconstruct an applied adjustment from its canonical JSON.

        Parameters
        ----------
        json:
            Serialized adjustment_id, name, raw_amount, capped_amount, and is_capped fields; amounts use the normalized metric's units.

        Returns
        -------
        AppliedAdjustment
            Reconstructed applied adjustment.

        Raises
        ------
        ValueError
            If JSON or the canonical adjustment shape is malformed.

        Examples
        --------
        >>> from finstack_quant.statements import AppliedAdjustment
        >>> AppliedAdjustment.from_json(
        ...     '{"adjustment_id":"a","name":"A","raw_amount":2.0,"capped_amount":1.0,"is_capped":true}'
        ... ).capped_amount
        1.0
        """
        ...

    def to_json(self) -> str:
        """Serialize this applied adjustment to canonical JSON.

        Returns
        -------
        str
            JSON accepted by AppliedAdjustment.from_json, with amounts in the normalized metric's units.

        Raises
        ------
        ValueError
            If a numeric result is non-finite or serialization fails.
        """
        ...

class NormalizationConfig:
    """
    Configuration for normalizing a target metric (for example EBITDA).

    Examples
    --------
    >>> from finstack_quant.statements import NormalizationConfig
    >>> config = NormalizationConfig("ebitda")
    >>> (config.target_node, config.adjustment_count)
    ('ebitda', 0)

    """

    def __init__(self, target_node: str) -> None:
        """
        Create an empty configuration for ``target_node``.

        Parameters
        ----------
        target_node:
            Node id whose values will be adjusted.

        Notes
        -----
        Construction does not raise; arguments are stored as supplied.

        Examples
        --------
        >>> from finstack_quant.statements import NormalizationConfig
        >>> config = NormalizationConfig("adjusted_ebitda")
        >>> config.adjustment_count
        0
        """
        ...

    @staticmethod
    def from_json(json: str) -> NormalizationConfig:
        """
        Load normalization rules from JSON.

        Parameters
        ----------
        json:
            JSON document for ``NormalizationConfig``.

        Returns
        -------
        NormalizationConfig
            Parsed configuration.

        Raises
        ------
        ValueError
            If JSON is invalid.

        Examples
        --------
        >>> from finstack_quant.statements import NormalizationConfig
        >>> config = NormalizationConfig("ebitda")
        >>> NormalizationConfig.from_json(config.to_json()).target_node
        'ebitda'

        """
        ...

    def to_json(self) -> str:
        """
        Serialize this configuration to pretty-printed JSON.

        Returns
        -------
        str
            JSON text.

            Canonical JSON representation of this `NormalizationConfig`, suitable for a matching `from_json` call.
        Raises
        ------
        ValueError
            If serialization fails.

        Examples
        --------
        >>> import json
        >>> from finstack_quant.statements import NormalizationConfig
        >>> json.loads(NormalizationConfig("ebitda").to_json())["target_node"]
        'ebitda'

        """
        ...

    @property
    def target_node(self) -> str:
        """
        Node id being normalized.
        Returns
        -------
        str
            Node identifier of the metric being normalized (e.g. ``"ebitda"``).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def adjustment_count(self) -> int:
        """
        Number of adjustment line items configured.
        Returns
        -------
        int
            Number of add-back / deduction adjustments configured.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise summary including target node and adjustment count.
        Returns
        -------
        str
        """
        ...

    @property
    def adjustments(self) -> list[Adjustment]:
        """Return configured add-backs and deductions in application order.

        Returns
        -------
        list[Adjustment]
            Copies of the adjustment specifications; caps may depend on earlier adjustments.

        Notes
        -----
        This accessor does not raise; it returns stored or derived model data.
        """
        ...

    def add_adjustment(self, adjustment: Adjustment) -> NormalizationConfig:
        """Append an add-back or deduction to this normalization policy.

        Parameters
        ----------
        adjustment:
            Adjustment with an identifier unique within this configuration; values use the target metric's units.

        Returns
        -------
        NormalizationConfig
            This configuration, for chaining.

        Raises
        ------
        ValueError
            If another configured adjustment has the same identifier.
        """
        ...

    def validate(self) -> None:
        """Validate adjustment identifiers, finite values, and cap configuration.

        Raises
        ------
        ValueError
            If adjustment identifiers repeat, a numeric assumption is invalid, or a cap is misconfigured.
        """
        ...

class NormalizationResult:
    """Normalized value and adjustment detail for one reporting period.

    Examples
    --------
    >>> from finstack_quant.statements import NormalizationResult
    >>> result = NormalizationResult.from_json(
    ...     '{"value_type":{"type":"scalar"},"period":"2025Q1","base_value":10.0,"adjustments":[],"final_value":10.0}'
    ... )
    >>> (result.period, result.final_value)
    ('2025Q1', 10.0)
    """

    @staticmethod
    def from_json(json: str) -> NormalizationResult:
        """Deserialize one result from canonical JSON.

        Parameters
        ----------
        json:
            Canonical normalization-result JSON object.

        Returns
        -------
        NormalizationResult
            Parsed typed result for one statement period.

        Raises
        ------
        ValueError
            If ``json`` is malformed or has the wrong shape.

        Examples
        --------
        >>> NormalizationResult.from_json(
        ...     '{"value_type":{"type":"scalar"},"period":"2025Q1","base_value":10.0,"adjustments":[],"final_value":10.0}'
        ... ).final_value
        10.0
        """
        ...

    def to_json(self) -> str:
        """Serialize this result to compact canonical JSON.

        Returns
        -------
        str
            Canonical compact JSON for this period result.

        Raises
        ------
        ValueError
            If a numeric field is non-finite or serialization fails.
        """
        ...

    @property
    def period(self) -> str:
        """Return the canonical statement-period identifier.

        Returns
        -------
        str
            Period such as ``"2025Q1"``.

        Notes
        -----
        This accessor does not raise; it returns stored result data.
        """
        ...
    @property
    def value_type(self) -> str:
        """Return the common unit classification of this result's amounts.

        Returns
        -------
        str
            ``"scalar"`` or ``"monetary"``; monetary amounts share ``currency``.

        Notes
        -----
        This accessor does not raise.
        """
        ...
    @property
    def currency(self) -> str | None:
        """Return the currency shared by the base, adjustments, and final value.

        Returns
        -------
        str | None
            ISO-4217 currency for monetary values, or ``None`` for scalars.

        Notes
        -----
        This accessor does not raise.
        """
        ...
    @property
    def base_value(self) -> float:
        """Return the metric value before normalization adjustments.

        Returns
        -------
        float
            Base amount in the statement metric's reporting units.

        Notes
        -----
        This accessor does not raise; it returns stored result data.
        """
        ...
    @property
    def adjustments(self) -> list[AppliedAdjustment]:
        """Return the ordered adjustments applied to the base value.

        Returns
        -------
        list[AppliedAdjustment]
            Adjustment detail in engine application order.

        Notes
        -----
        This accessor does not raise; it returns stored result data.
        """
        ...
    @property
    def final_value(self) -> float:
        """Return the normalized value after all capped adjustments.

        Returns
        -------
        float
            Final amount in the statement metric's reporting units.

        Notes
        -----
        This accessor does not raise; it returns stored result data.
        """
        ...
    def __repr__(self) -> str: ...

def normalize(results: StatementResult, config: NormalizationConfig) -> list[NormalizationResult]:
    """
    Run normalization and return typed period results.

    Parameters
    ----------
    results:
        Evaluated statement output.
    config:
        Target node and adjustment definitions.

    Returns
    -------
    list[NormalizationResult]
        Chronologically ordered period results.

    Raises
    ------
    ValueError
        If the engine fails.

    Examples
    --------
    >>> from finstack_quant.statements import Evaluator, ModelBuilder, NormalizationConfig, normalize
    >>> builder = ModelBuilder("demo")
    >>> _ = builder.periods("2025Q1..Q1")
    >>> _ = builder.value("ebitda", [("2025Q1", 25.0)])
    >>> result = Evaluator().evaluate(builder.build())
    >>> normalize(result, NormalizationConfig("ebitda"))[0].final_value
    25.0

    """
    ...

def merge_into_results(
    results: StatementResult,
    normalization_results: list[NormalizationResult],
    output_node_id: str,
) -> StatementResult:
    """Write normalized values back into statement results as a node.

    Wraps Rust ``adjustments::merge_into_results``: each period's
    ``final_value`` becomes node ``output_node_id`` with the shared unit (and
    currency for monetary values), replacing any existing node of that name
    across all periods. The attached check report is cleared because it
    describes the values before the merge; rerun the checks afterwards.

    Parameters
    ----------
    results:
        Evaluated statement results. Left unchanged; the merged copy is
        returned (Rust updates its argument in place).
    normalization_results:
        Per-period results from :func:`normalize`, all with the same unit and
        currency. An empty list returns an unchanged copy.
    output_node_id:
        Node to create or replace, e.g. ``"adjusted_ebitda"``.

    Returns
    -------
    StatementResult
        Copy of ``results`` holding the merged node.

    Raises
    ------
    ValueError
        If the results mix units or currencies, repeat a period, or hold a
        non-finite value.

    Examples
    --------
    >>> from finstack_quant.statements import (
    ...     Evaluator,
    ...     ModelBuilder,
    ...     NormalizationConfig,
    ...     merge_into_results,
    ...     normalize,
    ... )
    >>> builder = ModelBuilder("m")
    >>> _ = builder.periods("2025Q1..Q2")
    >>> _ = builder.value("ebitda", [("2025Q1", 100.0), ("2025Q2", 110.0)])
    >>> results = Evaluator().evaluate(builder.build())
    >>> merged = merge_into_results(results, normalize(results, NormalizationConfig("ebitda")), "adj")
    >>> merged.get("adj", "2025Q2")
    110.0
    """
    ...

class Adjustment:
    """A normalization adjustment: a fixed add-back or a percentage of a node.

    Examples
    --------
    >>> from finstack_quant.statements import Adjustment
    >>> Adjustment.fixed("a1", "Add-back", {"2025Q1": 2.0}).value_type
    'fixed'

    """

    @staticmethod
    def fixed(id: str, name: str, amounts: Mapping[str, float]) -> Adjustment:
        """Build an adjustment of explicit per-period amounts.

        Parameters
        ----------
        id : str
            Adjustment identifier, unique within a configuration.
        name : str
            Human-readable label carried into the result.
        amounts : Mapping[str, float]
            Amounts keyed by period id, in the target node's units.

        Returns
        -------
        Adjustment
            Fixed-amount adjustment.

        Raises
        ------
        ValueError
            If a period id cannot be parsed.

        Examples
        --------
        >>> from finstack_quant.statements import Adjustment
        >>> Adjustment.fixed("a1", "Add-back", {"2025Q1": 2.0}).value_type
        'fixed'

        """
        ...

    @staticmethod
    def percentage(id: str, name: str, node_id: str, percentage: float) -> Adjustment:
        """Build an adjustment equal to a percentage of another node.

        Parameters
        ----------
        id : str
            Adjustment identifier, unique within a configuration.
        name : str
            Human-readable label carried into the result.
        node_id : str
            Node whose per-period value the percentage is applied to.
        percentage : float
            Decimal fraction (``0.05`` = 5%), not percentage points.

        Returns
        -------
        Adjustment
            Percentage-of-node adjustment.

        This constructor stores the inputs verbatim and does not raise.

        Examples
        --------
        >>> from finstack_quant.statements import Adjustment
        >>> Adjustment.percentage("a2", "Rent add-back", "revenue", 0.05).value_type
        'percentage_of_node'

        """
        ...

    def with_cap(self, base_node: str | None, value: float, base_mode: str) -> Adjustment:
        """Return a capped copy using an explicit self-referential base mode.

        Parameters
        ----------
        base_node : str or None
            Node whose value sets the cap base, or ``None`` for an absolute cap.
        value : float
            Maximum magnitude: a decimal fraction of the base node (0.20 means
            20%), or an absolute amount in the target metric's units.
        base_mode : str
            ``"reported"`` uses the pre-adjustment target value;
            ``"progressive"`` includes earlier adjustments. Only affects a
            cap whose base node is the normalization target itself.

        Returns
        -------
        Adjustment
            Capped copy; the receiver is unchanged.

        Raises
        ------
        ValueError
            If ``base_mode`` is neither ``"reported"`` nor ``"progressive"``.

        """
        ...

    def with_category(self, category: str) -> Adjustment:
        """Return a copy tagged with ``category``.

        Parameters
        ----------
        category : str
            Free-form classification label.

        Returns
        -------
        Adjustment
            Tagged copy; the receiver is unchanged.

        This method rewrites an in-memory field and does not raise.

        """
        ...

    @property
    def id(self) -> str:
        """
        Adjustment identifier.

        This property does not raise.

        Returns
        -------
        str
            Adjustment identifier.
        """
        ...

    @property
    def name(self) -> str:
        """
        Human-readable label.

        This property does not raise.

        Returns
        -------
        str
            Human-readable label.
        """
        ...

    @property
    def category(self) -> str | None:
        """
        Classification label, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            Classification label, or ``None``.
        """
        ...

    @property
    def value_type(self) -> str:
        """
        ``"fixed"`` or ``"percentage"``.

        This property does not raise.

        Returns
        -------
        str
            ``"fixed"`` or ``"percentage"``.
        """
        ...

    @property
    def has_cap(self) -> bool:
        """
        Whether a cap is configured.

        This property does not raise.

        Returns
        -------
        bool
            Whether a cap is configured.
        """
        ...

    @property
    def cap_base_node(self) -> str | None:
        """
        Node the cap is measured against, or ``None`` for an absolute cap.

        This property does not raise.

        Returns
        -------
        str | None
            Node the cap is measured against, or ``None`` for an absolute cap.
        """
        ...

    @property
    def cap_value(self) -> float | None:
        """
        Cap magnitude, or ``None`` when uncapped.

        This property does not raise.

        Returns
        -------
        float | None
            Cap magnitude, or ``None`` when uncapped.
        """
        ...

    @property
    def cap_base_mode(self) -> str | None:
        """
        Cap base mode, or ``None`` when uncapped.

        Returns
        -------
        str | None
            Cap base mode, or ``None`` when uncapped.

        Raises
        ------
        RuntimeError
            If the stored enum cannot be rendered as its canonical Rust wire label.
        """
        ...

    def to_json(self) -> str:
        """Serialize to canonical JSON.

        Returns
        -------
        str
            Payload accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If serialization fails.

        """
        ...

    @staticmethod
    def from_json(json: str, /) -> Adjustment:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            Payload produced by :meth:`to_json`.

        Returns
        -------
        Adjustment
            Reconstructed adjustment.

        Raises
        ------
        ValueError
            If the payload is malformed.

        Examples
        --------
        >>> from finstack_quant.statements import Adjustment
        >>> Adjustment.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """
        ...

    def __repr__(self) -> str: ...
