"""Stub declarations mirroring ``finstack-quant-py/src/bindings/statements_analytics/relative_value.rs``."""

from __future__ import annotations

from collections.abc import Sequence
from typing import Any
import pandas as pd
from finstack_quant.statements_analytics._comps import (
    CompanyMetrics,
    PeerStats,
    RegressionResult,
    RelativeValueResult,
)

class RelativeValueCompany:
    """
    One company's row in the comps table of a :class:`RelativeValueReport`.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import RelativeValueCompany
    >>> company = RelativeValueCompany.from_json('{"id": "P1", "is_subject": false, "values": [8.0, null]}')
    >>> company.id, company.is_subject, company.values
    ('P1', False, [8.0, None])
    """

    @staticmethod
    def from_json(json: str) -> RelativeValueCompany:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``RelativeValueCompany`` (the shape ``to_json`` writes).

        Returns
        -------
        RelativeValueCompany
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import RelativeValueCompany
        >>> try:
        ...     RelativeValueCompany.from_json("{}")
        ... except ValueError:
        ...     print("rejected")
        rejected
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            The row as canonical JSON.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return the row's wire form as a dictionary.

        Returns
        -------
        dict[str, Any]
            The parsed form of :meth:`to_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            :meth:`from_json` and the ``to_json`` text.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __repr__(self) -> str:
        """
        Return ``repr(self)``.

        Returns
        -------
        str
            A one-line summary of the row.
        """
        ...

    @property
    def id(self) -> str:
        """
        Company identifier.

        Returns
        -------
        str
            The identifier.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def is_subject(self) -> bool:
        """
        Whether this row is the subject.

        Returns
        -------
        bool
            ``True`` on the subject's row only.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def values(self) -> list[float | None]:
        """
        One value per configured multiple, in config order.

        Returns
        -------
        list[float | None]
            The values, ``None`` where an input is missing, the denominator is not positive or the value is non-finite.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class RelativeValueMultiple:
    """
    One column of the comps table, summarised over the peers.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import RelativeValueMultiple
    >>> multiple = RelativeValueMultiple.from_json(
    ...     '{"multiple": "ev_ebitda", "subject_value": 10.0, "peer_stats": null, "percentile": null, "z_score": null}'
    ... )
    >>> multiple.multiple, multiple.subject_value, multiple.peer_stats, multiple.percentile, multiple.z_score
    ('ev_ebitda', 10.0, None, None, None)
    """

    @staticmethod
    def from_json(json: str) -> RelativeValueMultiple:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``RelativeValueMultiple`` (the shape ``to_json`` writes).

        Returns
        -------
        RelativeValueMultiple
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import RelativeValueMultiple
        >>> try:
        ...     RelativeValueMultiple.from_json("{}")
        ... except ValueError:
        ...     print("rejected")
        rejected
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            The column as canonical JSON.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return the column's wire form as a dictionary.

        Returns
        -------
        dict[str, Any]
            The parsed form of :meth:`to_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            :meth:`from_json` and the ``to_json`` text.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __repr__(self) -> str:
        """
        Return ``repr(self)``.

        Returns
        -------
        str
            A one-line summary of the column.
        """
        ...

    @property
    def multiple(self) -> str:
        """
        Serde name of the multiple, such as ``"ev_ebitda"``.

        Returns
        -------
        str
            The serde name of the column's multiple, as configured.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def subject_value(self) -> float | None:
        """
        Subject's value of the multiple.

        Returns
        -------
        float | None
            The value, or ``None`` when it cannot be computed.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def peer_stats(self) -> PeerStats | None:
        """
        Statistics of the peers' computable values.

        Returns
        -------
        PeerStats | None
            The statistics, or ``None`` when no peer has a value.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def percentile(self) -> float | None:
        """
        Share of the peer values at or below the subject's, from 0 to 1.

        Returns
        -------
        float | None
            The share, or ``None`` without a subject value or peer values.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def z_score(self) -> float | None:
        """
        Subject's z-score within the peer values.

        Returns
        -------
        float | None
            The score, or ``None`` without a subject value, with fewer than two peer values or when they do not differ.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class RelativeValuePoint:
    """
    One company on a dimension's regression scatter.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import RelativeValuePoint
    >>> point = RelativeValuePoint.from_json(
    ...     '{"company_id": "P1", "is_subject": false, "x": 1.0, "y": 100.0, "fitted_y": 105.0}'
    ... )
    >>> point.company_id, point.is_subject, point.x, point.y, point.fitted_y
    ('P1', False, 1.0, 100.0, 105.0)
    """

    @staticmethod
    def from_json(json: str) -> RelativeValuePoint:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``RelativeValuePoint`` (the shape ``to_json`` writes).

        Returns
        -------
        RelativeValuePoint
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import RelativeValuePoint
        >>> try:
        ...     RelativeValuePoint.from_json("{}")
        ... except ValueError:
        ...     print("rejected")
        rejected
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            The point as canonical JSON.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return the point's wire form as a dictionary.

        Returns
        -------
        dict[str, Any]
            The parsed form of :meth:`to_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            :meth:`from_json` and the ``to_json`` text.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __repr__(self) -> str:
        """
        Return ``repr(self)``.

        Returns
        -------
        str
            A one-line summary of the point.
        """
        ...

    @property
    def company_id(self) -> str:
        """
        Company identifier.

        Returns
        -------
        str
            The identifier.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def is_subject(self) -> bool:
        """
        Whether this point is the subject's.

        Returns
        -------
        bool
            ``True`` on the subject's point only, which is the first point when
            the subject has both metrics.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def x(self) -> float:
        """
        X metric of the company.

        Returns
        -------
        float
            The value, in the X metric's own units.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def y(self) -> float:
        """
        Y metric of the company.

        Returns
        -------
        float
            The value, in the Y metric's own units.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def fitted_y(self) -> float | None:
        """
        The dimension's regression line at this point's ``x``.

        Returns
        -------
        float | None
            ``intercept + slope * x``, or ``None`` without a regression.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class RelativeValueDimension:
    """
    One configured scoring dimension of a :class:`RelativeValueReport`, used by the composite or not.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import RelativeValueDimension
    >>> dimension = RelativeValueDimension.from_json(
    ...     '{"label": "Spread vs Leverage", "weight": 0.0, "direction": "higher_is_cheap",'
    ...     ' "y": {"named": "oas_bp"}, "x": {"multiple": "ev_ebitda"}, "subject_y": 300.0, "subject_x": null,'
    ...     ' "peer_stats": null, "percentile": null, "z_score": null, "regression": null, "signal": null,'
    ...     ' "excluded_reason": "zero_weight", "points": []}'
    ... )
    >>> dimension.label, dimension.weight, dimension.direction, dimension.y, dimension.x
    ('Spread vs Leverage', 0.0, 'higher_is_cheap', {'named': 'oas_bp'}, {'multiple': 'ev_ebitda'})
    >>> dimension.subject_y, dimension.subject_x, dimension.peer_stats, dimension.percentile, dimension.z_score
    (300.0, None, None, None, None)
    >>> dimension.regression, dimension.signal, dimension.excluded_reason, dimension.points
    (None, None, 'zero_weight', [])
    """

    @staticmethod
    def from_json(json: str) -> RelativeValueDimension:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``RelativeValueDimension`` (the shape ``to_json`` writes).

        Returns
        -------
        RelativeValueDimension
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import RelativeValueDimension
        >>> try:
        ...     RelativeValueDimension.from_json("{}")
        ... except ValueError:
        ...     print("rejected")
        rejected
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            The dimension as canonical JSON.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return the dimension's wire form as a dictionary.

        Returns
        -------
        dict[str, Any]
            The parsed form of :meth:`to_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            :meth:`from_json` and the ``to_json`` text.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __repr__(self) -> str:
        """
        Return ``repr(self)``.

        Returns
        -------
        str
            A one-line summary of the dimension.
        """
        ...

    @property
    def label(self) -> str:
        """
        Dimension label.

        Returns
        -------
        str
            The label as configured, not interpreted.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def weight(self) -> float:
        """
        Configured weight, not normalized.

        Returns
        -------
        float
            The weight; zero disables the dimension.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def direction(self) -> str:
        """
        Rich/cheap sign convention of the Y metric.

        Returns
        -------
        str
            ``"higher_is_cheap"`` or ``"higher_is_rich"``.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def y(self) -> dict[str, str]:
        """
        Y metric extractor, as the ``MetricExtractor`` serde document.

        Returns
        -------
        dict[str, str]
            One-key dict naming the extractor kind and its metric:
            ``{"named": "oas_bp"}`` for a canonical metric, ``{"custom": "<key>"}``
            for a ``custom`` metric and ``{"multiple": "ev_ebitda"}`` for a
            valuation multiple. It is the ``y_extractor`` the dimension was
            configured with, and matches the WASM report's ``y``.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def x(self) -> dict[str, str] | None:
        """
        X metric extractor, as the ``MetricExtractor`` serde document.

        Returns
        -------
        dict[str, str] | None
            One-key dict in the form :attr:`y` uses, the dimension's configured
            ``x_extractor``; ``None`` for a dimension scored on its peer distribution.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def subject_y(self) -> float | None:
        """
        Subject's Y metric.

        Returns
        -------
        float | None
            The value, or ``None`` when missing or non-finite.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def subject_x(self) -> float | None:
        """
        Subject's X metric.

        Returns
        -------
        float | None
            The value, or ``None`` without an X metric or when missing or non-finite.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def peer_stats(self) -> PeerStats | None:
        """
        Statistics of Y over every peer that has it, including peers missing X.

        Returns
        -------
        PeerStats | None
            The statistics, or ``None`` when no peer has Y.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def percentile(self) -> float | None:
        """
        Share of those peer Y values at or below the subject's, from 0 to 1.

        Returns
        -------
        float | None
            The share, or ``None`` without a subject Y or peer values.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def z_score(self) -> float | None:
        """
        Subject's raw z-score within those peer Y values, before the direction convention.

        Returns
        -------
        float | None
            The score, or ``None`` without a subject Y, with fewer than two peer values or when they do not differ.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def regression(self) -> RegressionResult | None:
        """
        Fit of Y on X over the peers that have both, evaluated at the subject.

        Returns
        -------
        RegressionResult | None
            The fit, whose ``fitted_value`` is the subject's fitted Y and ``residual`` its actual less fitted in Y units; ``None`` without X, without both subject metrics or when it cannot be fitted. It can be present on an excluded dimension.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def signal(self) -> float | None:
        """
        What this dimension contributes to the composite, positive meaning cheap.

        Returns
        -------
        float | None
            The standardized regression residual, or the z-score without X, with the sign set by ``direction``; ``None`` when excluded.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def excluded_reason(self) -> str | None:
        """
        Why the composite leaves this dimension out.

        Returns
        -------
        str | None
            ``"zero_weight"``, ``"no_subject_value"``, ``"no_peer_values"``, ``"no_dispersion"``, ``"no_subject_x"``, ``"regression_unusable"`` or ``"no_residual_dispersion"``; ``None`` when it is used.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def points(self) -> list[RelativeValuePoint]:
        """
        Regression scatter.

        Returns
        -------
        list[RelativeValuePoint]
            The subject first when it has both metrics, then every peer with both metrics in universe order; empty without X.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class RelativeValueReport:
    """
    Comps table, peer statistics, regression scatter and composite score of one subject.

    Returned by :func:`relative_value_report`.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import CompanyMetrics, relative_value_report
    >>> oas = {1: 100.0, 2: 200.0, 3: 310.0, 4: 380.0}
    >>> peers = [CompanyMetrics(f"P{i}", {"leverage": float(i), "oas_bp": oas[i]}) for i in oas]
    >>> subject = CompanyMetrics("SUBJ", {"leverage": 2.5, "oas_bp": 300.0})
    >>> dimension = {
    ...     "label": "Spread vs Leverage",
    ...     "y_extractor": {"named": "oas_bp"},
    ...     "x_extractor": {"named": "leverage"},
    ...     "weight": 1.0,
    ... }
    >>> report = relative_value_report(
    ...     subject, peers, {"period_basis": "ltm", "dimensions": [dimension], "multiples": ["spread_per_turn"]}
    ... )
    >>> report.subject_id, report.period_basis, report.peer_count, report.excluded_count
    ('SUBJ', 'ltm', 4, 0)
    >>> [row.id for row in report.companies][:2], report.multiples[0].multiple, report.dimensions[0].label
    (['SUBJ', 'P1'], 'spread_per_turn', 'Spread vs Leverage')
    >>> report.composite.composite_score > 0, report.composite_excluded_reason
    (True, None)
    """

    @staticmethod
    def from_json(json: str) -> RelativeValueReport:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``RelativeValueReport`` (the shape ``to_json`` writes).

        Returns
        -------
        RelativeValueReport
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import RelativeValueReport
        >>> try:
        ...     RelativeValueReport.from_json("{}")
        ... except ValueError:
        ...     print("rejected")
        rejected
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            The report as canonical JSON.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return the report's wire form as a dictionary.

        Returns
        -------
        dict[str, Any]
            The parsed form of :meth:`to_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            :meth:`from_json` and the ``to_json`` text.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __repr__(self) -> str:
        """
        Return ``repr(self)``.

        Returns
        -------
        str
            A one-line summary of the report.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Return the comps table as a DataFrame with one row per company.

        Columns are ``id``, ``is_subject`` and then one float column per configured
        multiple, named by its serde name, in config order. A value Rust cannot compute
        is missing.

        Returns
        -------
        pandas.DataFrame
            The subject's row first, then the peers in universe order.

        Raises
        ------
        ValueError
            If the report cannot be serialized.
        ImportError
            If pandas is not installed.
        """
        ...

    @property
    def subject_id(self) -> str:
        """
        Subject company identifier.

        Returns
        -------
        str
            The identifier.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def period_basis(self) -> str:
        """
        Period basis from the config.

        Returns
        -------
        str
            ``"ltm"``, ``"ntm"`` or the custom label.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def peer_count(self) -> int:
        """
        Number of peers, excluding the subject.

        Returns
        -------
        int
            The companies that passed the filter, other than the subject.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def excluded_count(self) -> int:
        """
        Universe members the filter dropped, not counting the subject.

        Returns
        -------
        int
            The members the filter dropped; the subject's own row does not count.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def companies(self) -> list[RelativeValueCompany]:
        """
        Rows of the comps table, one per company.

        Returns
        -------
        list[RelativeValueCompany]
            The subject first, then the peers in universe order.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def multiples(self) -> list[RelativeValueMultiple]:
        """
        One entry per configured multiple.

        Returns
        -------
        list[RelativeValueMultiple]
            The entries, in config order.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def dimensions(self) -> list[RelativeValueDimension]:
        """
        One entry per configured dimension, including excluded ones.

        Returns
        -------
        list[RelativeValueDimension]
            The entries, in config order.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def composite(self) -> RelativeValueResult | None:
        """
        Result of :func:`score_relative_value` over the same peers and dimensions.

        Returns
        -------
        RelativeValueResult | None
            The score, or ``None`` when ``composite_excluded_reason`` is set.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def composite_excluded_reason(self) -> str | None:
        """
        Why there is no composite.

        Returns
        -------
        str | None
            ``"no_dimensions"``, ``"no_usable_dimension"`` or ``"non_finite_score"``; ``None`` when there is a composite.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

def relative_value_report(
    subject: CompanyMetrics | dict[str, Any] | str,
    universe: Sequence[CompanyMetrics | dict[str, Any] | str] | str,
    config: dict[str, Any] | str,
) -> RelativeValueReport:
    """
    Compare one subject with its filtered peers.

    Returns the comps table, peer statistics, regression scatter and composite
    rich/cheap score in one result, so a caller has nothing to compute. A used
    dimension and the composite are exactly what :func:`score_relative_value` returns
    on the same peers and dimensions. Where that function raises because no dimension
    is usable, this one still returns the table, with ``composite`` set to ``None``
    and ``composite_excluded_reason`` saying why.

    Monetary metrics carry no currency, so every company's must already be in one
    currency: normalization is the caller's. Rows are reported as supplied, so a
    duplicated peer id gives two rows.

    Parameters
    ----------
    subject : CompanyMetrics or dict or str
        Company being evaluated, typed or as its serde document. It need not be in
        ``universe``.
    universe : sequence of CompanyMetrics or dict or str, or str
        Candidate peers in the order the report returns them: a sequence whose
        members are typed, serde documents or JSON text, or the whole list as one
        JSON array text, as WASM accepts. A member with the subject's id is never a
        peer and does not count as excluded.
    config : dict or str
        ``RelativeValueReportConfig`` document, a dict or JSON text: ``filter``
        (optional; a ``PeerFilter`` document in which every field is optional, an
        omitted field meaning no constraint and an unknown one rejected; an omitted
        filter makes every other company a peer), ``period_basis`` (required; ``"ltm"``, ``"ntm"`` or
        ``{"custom": "<label>"}``; it labels the report and the metrics are used as
        given), ``dimensions`` (required; ``ScoringDimension`` documents with finite
        non-negative weights, zero disabling one; may be empty when ``multiples`` is
        not) and ``multiples`` (required; serde names of the table's columns such as
        ``"ev_ebitda"``, in order; may be empty when ``dimensions`` is not).

    Returns
    -------
    RelativeValueReport
        The table with the subject first, multiples and dimensions in config order, and
        each dimension's exclusion reason when it has one.

    Raises
    ------
    ValueError
        If an argument is malformed or carries an unknown field (a misspelt
        ``CompanyMetrics`` or ``PeerFilter`` key included), both ``dimensions`` and
        ``multiples`` are empty, or a weight is negative or non-finite.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import CompanyMetrics, relative_value_report
    >>> oas = {1: 100.0, 2: 200.0, 3: 310.0, 4: 380.0}
    >>> peers = [CompanyMetrics(f"P{i}", {"leverage": float(i), "oas_bp": oas[i]}) for i in oas]
    >>> subject = CompanyMetrics("SUBJ", {"leverage": 2.5, "oas_bp": 300.0})
    >>> dimension = {
    ...     "label": "Spread vs Leverage",
    ...     "y_extractor": {"named": "oas_bp"},
    ...     "x_extractor": {"named": "leverage"},
    ...     "weight": 1.0,
    ... }
    >>> report = relative_value_report(
    ...     subject, peers, {"period_basis": "ltm", "dimensions": [dimension], "multiples": ["spread_per_turn"]}
    ... )
    >>> report.companies[0].values
    [120.0]
    >>> report.dimensions[0].y, report.dimensions[0].x
    ({'named': 'oas_bp'}, {'named': 'leverage'})
    >>> round(report.dimensions[0].points[1].fitted_y, 6)
    105.0
    >>> report.composite.composite_score > 0
    True
    """
    ...
