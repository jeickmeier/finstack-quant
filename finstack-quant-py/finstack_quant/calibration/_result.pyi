"""Stub declarations mirroring ``finstack-quant-py/src/bindings/calibration/result.rs``."""

from typing import Any
import pandas as pd
from finstack_quant.core.market_data import MarketContext
from finstack_quant.calibration._report import CalibrationReport

class CalibrationResultEnvelope:
    """Calibrated market plus the plan-level and per-step calibration reports.

    Examples:
    --------
    >>> from finstack_quant.calibration import CalibrationEnvelope, CalibrationPlan, calibrate
    >>> calibrate(CalibrationEnvelope(CalibrationPlan([], id="smoke"))).step_ids
    []

    """

    @property
    def success(self) -> bool:
        """
        Whether every step repriced its quotes inside tolerance.

        This property does not raise.

        Returns
        -------
        bool
            Whether every step repriced its quotes inside tolerance.
        """

    @property
    def market(self) -> MarketContext:
        """
        The calibrated market context.

        Returns
        -------
        MarketContext
            The calibrated market context.

        Raises
        ------
        ValueError
            If the calibrated market payload cannot be rebuilt.
        """

    @property
    def market_json(self) -> str:
        """
        The calibrated market as canonical ``MarketContext`` JSON.

        Returns
        -------
        str
            The calibrated market as canonical ``MarketContext`` JSON.

        Raises
        ------
        ValueError
            If the underlying value cannot be serialized to JSON.
        """

    @property
    def report(self) -> CalibrationReport:
        """
        Plan-level calibration report aggregated across steps.

        This property does not raise.

        Returns
        -------
        CalibrationReport
            Plan-level calibration report aggregated across steps.
        """

    @property
    def step_ids(self) -> list[str]:
        """
        Identifiers of the steps executed by the plan.

        This property does not raise.

        Returns
        -------
        list[str]
            Identifiers of the steps executed by the plan.
        """

    @property
    def iterations(self) -> int:
        """
        Total solver iterations across every step.

        This property does not raise.

        Returns
        -------
        int
            Total solver iterations across every step.
        """

    def step_report(self, step_id: str) -> CalibrationReport:
        """Return the calibration report for one step.

        Parameters
        ----------
        step_id : str
            Identifier of the calibration step, as listed in :attr:`step_ids`.

        Returns
        -------
        CalibrationReport
            Typed step report with raw residuals keyed by quote id.

        Raises
        ------
        KeyError
            If no step with that identifier exists.

        """

    def residuals(self, step_id: str) -> pd.DataFrame:
        """Return one step's per-quote residuals as a DataFrame.

        Parameters
        ----------
        step_id : str
            Identifier of the calibration step.

        Returns
        -------
        pandas.DataFrame
            Columns ``quote_id``, ``quote_value`` (native quote units),
            ``residual`` (in the report's declared units, target zero) and
            ``sensitivity``.

        Raises
        ------
        KeyError
            If no step with that identifier exists.
        RuntimeError
            If pandas is not installed.

        """

    def to_dataframe(self) -> pd.DataFrame:
        """Return one row per step summarizing its fit.

        Returns
        -------
        pandas.DataFrame
            Columns ``step_id``, ``success``, ``iterations``, ``max_residual``,
            ``rmse`` and ``convergence_reason``. Plan-level aggregates
            (``max_residual_ratio``, ``rmse_ratio``) are on ``result.report``.

        Raises
        ------
        RuntimeError
            If pandas is not installed.

        """

    def content_hash(self) -> str:
        """Stable content hash of the canonical result JSON.

        Returns
        -------
        str
            Hex digest identifying this result's exact content.

        Raises
        ------
        RuntimeError
            If the result cannot be canonicalized.

        """

    def to_json(self) -> str:
        """
        Serialize the result to compact canonical JSON.

        Returns
        -------
        str
            Compact JSON encoding of the calibrated market and reports.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """

    @staticmethod
    def from_json(json: str) -> CalibrationResultEnvelope:
        """
        Strictly load a result from JSON.

        Parameters
        ----------
        json : str
            Result JSON using schema ``finstack_quant.calibration/1``. The
            canonical loader limits input to 64 MiB, JSON nesting to 96
            containers, and retained diagnostics to 256 findings.

        Returns
        -------
        CalibrationResultEnvelope
            The loaded result.

        Raises
        ------
        ContractValidationError
            If the JSON or envelope shape is invalid, unknown fields are
            present, the schema marker is missing, malformed, or unsupported,
            or nested market restoration fails. Structured market
            failures carry pointer-level findings in the ``report`` attribute.
        ContractLimitExceededError
            If the input exceeds the canonical byte or nesting-depth limits.
        ValueError
            If an invalid nested value cannot be restored.

        Examples
        --------
        >>> from finstack_quant.calibration import CalibrationResultEnvelope
        >>> CalibrationResultEnvelope.from_json("{")
        Traceback (most recent call last):
        finstack_quant.portfolio.ContractValidationError: ...
        """

    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...
