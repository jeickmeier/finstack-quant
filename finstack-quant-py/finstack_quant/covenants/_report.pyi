"""Stub declarations mirroring ``finstack-quant-py/src/bindings/covenants/report.rs``."""

from __future__ import annotations

from typing import Any
import pandas as pd
import polars as pl
from finstack_quant.core.table import ArrowTable

class CovenantReport:
    """
    Result of a single covenant evaluation.

    Carries pass/fail status, the tested value against its threshold, the
    headroom (positive is cushion, negative is deficit), an optional
    human-readable explanation, and the audit stamp in force when the covenant
    was evaluated. Reports compare equal by value.

    Construct via :meth:`CovenantEngine.evaluate` or :meth:`from_json`.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.covenants import CovenantReport
    >>> report = CovenantReport.from_json(
    ...     json.dumps({
    ...         "covenant_type": "Debt/EBITDA <= 5.00x",
    ...         "passed": False,
    ...         "actual_value": 5.5,
    ...         "threshold": 5.0,
    ...         "details": "Exceeded",
    ...         "headroom": -0.5,
    ...     })
    ... )
    >>> report.passed
    False
    >>> report == CovenantReport.from_json(report.to_json())
    True
    """

    @staticmethod
    def from_json(json: str) -> CovenantReport:
        """
        Deserialize a ``CovenantReport`` from JSON.

        Parameters
        ----------
        json : str
            JSON string matching the ``CovenantReport`` wire schema.

        Returns
        -------
        CovenantReport
            Parsed report.

        Raises
        ------
        ValueError
            If ``json`` is malformed or omits required fields.

        Examples
        --------
        >>> import json
        >>> from finstack_quant.covenants import CovenantReport
        >>> report = CovenantReport.from_json(
        ...     json.dumps({
        ...         "covenant_type": "Debt/EBITDA <= 5.00x",
        ...         "passed": False,
        ...         "actual_value": 5.5,
        ...         "threshold": 5.0,
        ...         "details": "Exceeded",
        ...         "headroom": -0.5,
        ...     })
        ... )
        >>> CovenantReport.from_json(report.to_json()).headroom
        -0.5
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            Compact JSON string.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def covenant_type(self) -> str:
        """
        Human-readable description of the covenant being tested.

        Returns
        -------
        str
            For example ``"Debt/EBITDA <= 5.00x"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def covenant_id(self) -> str | None:
        """
        Stable machine-readable covenant instance identifier.

        Returns
        -------
        str | None
            ``None`` when the report was produced without an identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def passed(self) -> bool:
        """
        Whether the covenant passed.

        Inactive covenants, unmet springing conditions, and full waivers also
        report ``True``; :attr:`details` carries the reason.

        Returns
        -------
        bool
            ``True`` when the tested metric satisfied its threshold. Because an
            untested covenant also reports ``True``, read it together with
            :attr:`actual_value`, which is ``None`` exactly when no numeric
            test ran.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def actual_value(self) -> float | None:
        """
        Observed metric value that was tested against the covenant.

        Returns
        -------
        float | None
            ``None`` when the covenant was not evaluated numerically (inactive,
            waived, or springing condition unmet).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def threshold(self) -> float | None:
        """
        Threshold the metric was tested against.

        Returns
        -------
        float | None
            ``None`` when no numeric test was applied.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def details(self) -> str | None:
        """
        Explanation of the outcome.

        Returns
        -------
        str | None
            Free text such as ``"Covenant inactive"``, ``"Waived by lender
            agreement"``, or ``"In cure period"``. ``None`` for a plain numeric
            result whose evaluator supplied no commentary, so absence carries
            no meaning of its own.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def headroom_raw(self) -> float | None:
        """Get the signed cushion in the covenant metric's original units.

        Returns
        -------
        float or None
            Signed distance when the metric and threshold support a numerical comparison.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def headroom(self) -> float | None:
        """
        Cushion relative to the threshold.

        Returns
        -------
        float | None
            Signed distance from the threshold divided by ``|threshold|``:
            positive is a passing buffer, negative a deficit. ``None`` when no
            numeric test was applied.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def meta(self) -> dict[str, Any]:
        """
        Audit stamp: numeric mode, rounding context, and FX policy in force.

        Returns
        -------
        dict[str, Any]
            Keys ``numeric_mode``, ``rounding``, ``fx_policy_applied`` and
            ``version``; ``fx_policy_applied`` is ``None`` when the evaluation
            stayed in one currency. Reproducing a report requires re-running
            under the same ``rounding`` context.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_arrow(self) -> ArrowTable:
        """
        Export the report as a single-row :class:`~finstack_quant.core.table.ArrowTable`.

        Columns: ``covenant_type``, ``covenant_id``, ``passed``,
        ``actual_value``, ``threshold``, ``headroom``, ``headroom_raw``, ``details``.

        Returns
        -------
        ArrowTable
            One row describing this covenant evaluation.

        Raises
        ------
        ValueError
            If the underlying Arrow record batch cannot be built.
        """
        ...

    def to_pandas(self) -> pd.DataFrame:
        """
        Export the report as a single-row pandas DataFrame.

        Columns: ``covenant_type``, ``covenant_id``, ``passed``,
        ``actual_value``, ``threshold``, ``headroom``, ``headroom_raw``, ``details``.

        Returns
        -------
        pd.DataFrame
            One row describing this covenant evaluation.

        Raises
        ------
        ImportError
            If ``pandas`` is not installed.
        """
        ...

    def to_polars(self) -> pl.DataFrame:
        """
        Export the report as a single-row ``polars.DataFrame``.

        Returns
        -------
        pl.DataFrame
            One-row Polars DataFrame constructed via the Arrow PyCapsule interface.

        Raises
        ------
        ImportError
            If ``polars`` is not installed.
        """
        ...

    def to_pyarrow(self) -> Any:
        """
        Export the report as a single-row ``pyarrow.Table``.

        Returns
        -------
        pyarrow.Table
            Zero-copy table built through the Arrow PyCapsule interface.

        Raises
        ------
        ImportError
            If ``pyarrow`` is not installed.
        """
        ...

    def __arrow_c_stream__(self, requested_schema: Any | None = None) -> Any: ...
    def __arrow_c_schema__(self) -> Any: ...
    def _repr_html_(self) -> str | None: ...
    def __eq__(self, other: object) -> bool: ...
