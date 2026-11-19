"""Stub declarations mirroring ``finstack-quant-py/src/bindings/statements/adjustments.rs``."""

from __future__ import annotations

from collections.abc import Mapping
from typing import Any

from finstack_quant.core.money import Money
from finstack_quant.statements._evaluator import StatementResult

class AppliedAdjustment:
    """One adjustment applied to a normalized statement metric.

    Examples
    --------
    >>> from finstack_quant.statements import NormalizationResult
    >>> payload = '{"value_type":{"type":"scalar"},"period":"2025Q1","base_value":10.0,"adjustments":[{"adjustment_id":"a1","name":"Add-back","raw_amount":2.0,"capped_amount":1.5,"is_capped":true,"adjusted_value":11.5}],"final_value":11.5}'
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
        """Return the signed adjustment amount before any cap.

        Returns
        -------
        float
            Amount in the target's units; money as the nearest float (the
            exact amount is in :meth:`to_json`).

        Notes
        -----
        This accessor does not raise; it returns stored result data.
        """
        ...
    @property
    def cap_base(self) -> float | None:
        """Return the value a reference cap was sized against.

        This property does not raise.

        Returns
        -------
        float | None
            The base node's value in the period (for a progressive cap on the
            target, the reported value plus the earlier applied adjustments),
            or ``None`` for an absolute cap or no cap.
        """
        ...
    @property
    def cap_limit(self) -> float | None:
        """Return the non-negative limit on this adjustment's magnitude.

        This property does not raise.

        Returns
        -------
        float | None
            The absolute cap, or the fraction times the positive part of
            ``cap_base``; ``None`` when the adjustment has no cap.
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
    @property
    def adjusted_value(self) -> float:
        """Return the target value through this adjustment.

        Returns
        -------
        float
            The period's ``base_value`` plus the applied amounts of this and every
            earlier adjustment, in declaration order and the target's units. The last adjustment's value equals the period's
            ``final_value``, so each step of a base-to-adjusted bridge spans from
            the previous step's value (or ``base_value``) to this one.

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
            Serialized adjustment_id, name, raw_amount, optional cap_base and cap_limit, capped_amount, is_capped, and adjusted_value fields. An amount is a number for a scalar target and ``{"amount": ..., "currency": ...}`` for a monetary one.

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
        ...     '{"adjustment_id":"a","name":"A","raw_amount":2.0,"capped_amount":1.0,"is_capped":true,"adjusted_value":11.0}'
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
    An ordered bridge from a reported metric to a separately named adjusted metric.

    The reported ``target_node`` is never modified. The adjusted metric is
    published under the new ``output_node`` by :func:`merge_into_results`.

    Examples
    --------
    >>> from finstack_quant.statements import NormalizationConfig
    >>> config = NormalizationConfig("ebitda", "adjusted_ebitda")
    >>> (config.target_node, config.output_node, config.adjustment_count)
    ('ebitda', 'adjusted_ebitda', 0)

    """

    def __init__(self, target_node: str, output_node: str) -> None:
        """
        Create a bridge from ``target_node`` to ``output_node`` with no adjustments.

        Parameters
        ----------
        target_node:
            Reported node the bridge starts from, e.g. ``"ebitda"``.
        output_node:
            New node id the adjusted metric is published under, e.g.
            ``"adjusted_ebitda"``. It must differ from ``target_node`` and
            from every node of the results; :meth:`validate` and
            :func:`merge_into_results` check that.

        Notes
        -----
        Construction does not raise; arguments are stored as supplied.

        Examples
        --------
        >>> from finstack_quant.statements import NormalizationConfig
        >>> NormalizationConfig("ebitda", "adjusted_ebitda").adjustment_count
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
        >>> config = NormalizationConfig("ebitda", "adjusted_ebitda")
        >>> NormalizationConfig.from_json(config.to_json()).output_node
        'adjusted_ebitda'

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
        >>> json.loads(NormalizationConfig("ebitda", "adjusted_ebitda").to_json())["target_node"]
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
    def output_node(self) -> str:
        """
        Node id the adjusted metric is published under.

        Returns
        -------
        str
            New node identifier (e.g. ``"adjusted_ebitda"``).

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
        """Check the configuration on its own.

        References, units, period keys and numbers are checked against the
        results by :func:`normalize`.

        Raises
        ------
        ValueError
            If the output node is empty or equals the target node, or an
            adjustment identifier repeats.
        """
        ...

class NormalizationResult:
    """One period of a normalization bridge.

    A period whose target, percentage reference or cap base is missing,
    masked or unavailable has no adjusted value: ``final_value`` is ``None``,
    ``unavailable`` names the reason and there are no rows.

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
    def base_value(self) -> float | None:
        """Return the reported value of the target node.

        Returns
        -------
        float | None
            Reported amount in the target's units (money as the nearest
            float), or ``None`` when the target has no value in the period.

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
    def final_value(self) -> float | None:
        """Return the adjusted value after every applied adjustment.

        Returns
        -------
        float | None
            Adjusted amount in the target's units (money as the nearest
            float), or ``None`` when the period is unavailable.

        Notes
        -----
        This accessor does not raise; it returns stored result data.
        """
        ...
    @property
    def unavailable(self) -> str | None:
        """Return why the period has no adjusted value.

        This property does not raise.

        Returns
        -------
        str | None
            The reason label of the missing input, such as
            ``"missing_input"`` or ``"hidden_by_cutoff"``; ``None`` when the
            period has a value.
        """
        ...
    def __repr__(self) -> str: ...

def normalize(results: StatementResult, config: NormalizationConfig) -> list[NormalizationResult]:
    """
    Build the ordered bridge from a reported node to its adjusted value.

    The whole configuration is validated before any period is calculated.
    Each period starts from the reported target value and applies the
    adjustments in declaration order: a signed fixed amount (zero for a period
    not listed) or a signed decimal percentage of a reference node's reported
    value in the same period. Each cap limits its own adjustment's magnitude
    and keeps its sign. Money is added and scaled exactly and scalars with a
    compensated sum; nothing is converted between currencies. Nothing is
    written to ``results``; use :func:`merge_into_results` to publish.

    Parameters
    ----------
    results:
        Evaluated or rolled-up statement results with their period plan.
    config:
        The reported node, the new output node and the ordered adjustments.

    Returns
    -------
    list[NormalizationResult]
        One result per period of the plan, in timeline order. A period whose
        target, percentage reference or cap base has no value carries
        ``unavailable`` and no rows.

    Raises
    ------
    ValueError
        If the results carry no period plan, the output node is empty or
        equals the target, an adjustment id repeats, the target or a
        referenced node is absent or has no recorded units, units or
        currencies differ, a fixed amount names a period outside the plan, a
        number is non-finite or a cap negative, or any row's arithmetic cannot
        be represented. No partial bridge is returned.

    Examples
    --------
    >>> from finstack_quant.statements import (
    ...     Adjustment,
    ...     Evaluator,
    ...     ModelBuilder,
    ...     NormalizationConfig,
    ...     normalize,
    ... )
    >>> builder = ModelBuilder("demo")
    >>> _ = builder.periods("2025Q1..Q1")
    >>> _ = builder.value("ebitda", [("2025Q1", 100.0)])
    >>> result = Evaluator().evaluate(builder.build())
    >>> cap = {"type": "reference", "base_node": "ebitda", "fraction": 0.30}
    >>> config = NormalizationConfig("ebitda", "adjusted_ebitda")
    >>> _ = config.add_adjustment(Adjustment.fixed("a", "First", {"2025Q1": 20.0}))
    >>> _ = config.add_adjustment(Adjustment.fixed("b", "Second", {"2025Q1": 40.0}).with_cap(cap))
    >>> row = normalize(result, config)[0]
    >>> (row.adjustments[1].cap_limit, row.final_value)
    (30.0, 150.0)

    """
    ...

def merge_into_results(results: StatementResult, config: NormalizationConfig) -> StatementResult:
    """Publish the adjusted metric of ``config`` as the new node ``config.output_node``.

    Wraps Rust ``adjustments::merge_into_results``. The bridge is calculated
    from ``config`` by :func:`normalize`, so the published values always agree
    with the configuration recorded for them in
    :attr:`StatementResult.normalizations`. A period without an adjusted value
    is published as an unavailable cell. The reported target, every other node
    and the check report are left as they are: a ratio or covenant on the
    adjusted basis must reference the output node explicitly.

    Parameters
    ----------
    results:
        Result of an evaluation run, or a rollup of one. Left unchanged; the
        updated copy is returned (Rust updates its argument in place).
    config:
        The bridge to calculate and publish.

    Returns
    -------
    StatementResult
        Copy of ``results`` holding the new node, its units and the
        configuration.

    Raises
    ------
    ValueError
        If ``results`` record no source run, already hold a node named
        ``config.output_node`` (including one published earlier), or
        :func:`normalize` rejects the configuration. Nothing is published on
        error.

    Examples
    --------
    >>> from finstack_quant.statements import (
    ...     Adjustment,
    ...     Evaluator,
    ...     ModelBuilder,
    ...     NormalizationConfig,
    ...     merge_into_results,
    ... )
    >>> builder = ModelBuilder("m")
    >>> _ = builder.periods("2025Q1..Q2")
    >>> _ = builder.value("ebitda", [("2025Q1", 100.0), ("2025Q2", 110.0)])
    >>> results = Evaluator().evaluate(builder.build())
    >>> config = NormalizationConfig("ebitda", "adjusted_ebitda")
    >>> _ = config.add_adjustment(Adjustment.fixed("a", "Add-back", {"2025Q2": 5.0}))
    >>> merged = merge_into_results(results, config)
    >>> (merged.get("ebitda", "2025Q2"), merged.get("adjusted_ebitda", "2025Q2"))
    (110.0, 115.0)
    >>> list(merged.normalizations)
    ['adjusted_ebitda']
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
    def fixed(id: str, name: str, amounts: Mapping[str, float | Money]) -> Adjustment:
        """Build an adjustment of explicit signed per-period amounts.

        Parameters
        ----------
        id : str
            Adjustment identifier, unique within a configuration.
        name : str
            Human-readable label carried into the result.
        amounts : Mapping[str, float | Money]
            Signed amounts keyed by period id, in the target node's units:
            floats for a scalar target, :class:`Money` in its currency for a
            monetary one (positive add-back, negative deduction). A period of
            the results that is not listed adjusts by zero; a period outside
            them is rejected by :func:`normalize`.

        Returns
        -------
        Adjustment
            Fixed-amount adjustment.

        Raises
        ------
        ValueError
            If a period id cannot be parsed or an amount is neither a number
            nor ``Money``.

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
            Node whose reported value in the same period the percentage is
            applied to. It must share the target's units.
        percentage : float
            Signed decimal fraction (``0.05`` = 5%), not percentage points.

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

    def with_cap(self, cap: Mapping[str, Any]) -> Adjustment:
        """Return a copy whose magnitude is capped; the adjustment keeps its sign.

        Each cap applies to its own adjustment only; it is not a shared basket.

        Parameters
        ----------
        cap : Mapping[str, Any]
            The Rust ``AdjustmentCap`` serde form, one of
            ``{"type": "absolute", "amount": amount}``, a non-negative limit
            in the target's units (a number, or
            ``{"amount": "25", "currency": "USD"}`` for a monetary target),
            and ``{"type": "reference", "base_node": node, "fraction": f,
            "base_mode": mode}``, a limit of the non-negative decimal
            ``fraction`` (which may exceed 1) times the positive part of the
            base node's value in the same period. ``base_mode`` is optional
            and matters only when the base node is the target itself:
            ``"reported"`` (the default) uses its reported value and
            ``"progressive"`` adds the earlier applied adjustments.

        Returns
        -------
        Adjustment
            Capped copy; the receiver is unchanged.

        Raises
        ------
        ValueError
            If ``cap`` is not one of the two forms or holds an unknown key.
            Units, signs and references are checked by :func:`normalize`.

        Examples
        --------
        >>> from finstack_quant.statements import Adjustment
        >>> cap = {"type": "reference", "base_node": "ebitda", "fraction": 0.3}
        >>> Adjustment.fixed("a", "A", {"2025Q1": 40.0}).with_cap(cap).cap
        {'type': 'reference', 'base_node': 'ebitda', 'fraction': 0.3}

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
        ``"fixed"`` or ``"percentage_of_node"``.

        This property does not raise.

        Returns
        -------
        str
            ``"fixed"`` or ``"percentage_of_node"``.
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
    def cap(self) -> dict[str, Any] | None:
        """
        The configured cap in its serde form, or ``None`` when uncapped.

        Returns
        -------
        dict[str, Any] | None
            ``{"type": "absolute", "amount": ...}`` or ``{"type":
            "reference", "base_node": ..., "fraction": ...}`` with
            ``"base_mode": "progressive"`` when it is not the default.

        Raises
        ------
        ValueError
            If the stored cap cannot be serialized.
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
