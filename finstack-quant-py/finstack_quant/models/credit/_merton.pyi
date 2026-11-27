"""Stub declarations mirroring ``finstack-quant-py/src/bindings/models/credit/merton.rs``."""

from __future__ import annotations

from finstack_quant.core.types import CreditRating
import datetime
import numpy as np
from numpy.typing import NDArray
import pandas
import polars as pl
from finstack_quant.core.dates import DayCount
from finstack_quant.core.market_data import HazardCurve
from finstack_quant.core.table import ArrowTable, LabeledSeries
from typing import Any
from finstack_quant.models.credit import pd
from finstack_quant.models.credit._paths import SimulatedPaths

class MertonBarrierType:
    """
    Default barrier monitoring convention for structural credit models.

    Examples
    --------
    >>> from finstack_quant.models.credit import MertonBarrierType
    >>> MertonBarrierType.terminal() is not None
    True

    """

    @staticmethod
    def terminal() -> MertonBarrierType:
        """
        Classic Merton barrier tested only at maturity.

        Returns
        -------
        MertonBarrierType
            Terminal-barrier specification.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.models.credit import MertonBarrierType
        >>> bt = MertonBarrierType.terminal()
        >>> bt.to_json()
        '"terminal"'
        """
        ...

    @staticmethod
    def first_passage(barrier_growth_rate: float) -> MertonBarrierType:
        """
        Black-Cox first-passage barrier with optional growth rate.

        Parameters
        ----------
        barrier_growth_rate : float
            Continuous growth rate of the default barrier over time, as a
            decimal (e.g. ``0.02`` for 2% annual growth).

        Returns
        -------
        MertonBarrierType
            First-passage barrier specification.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.models.credit import MertonBarrierType
        >>> bt = MertonBarrierType.first_passage(0.02)
        >>> "first_passage" in bt.to_json()
        True

        Sources
        -------
        - Merton (1974): see docs/REFERENCES.md#merton-1974
        """
        ...

    @staticmethod
    def from_json(json: str) -> MertonBarrierType:
        """
        Deserialize a barrier type from canonical JSON.

        Parameters
        ----------
        json : str
            Canonical JSON payload.

        Returns
        -------
        MertonBarrierType
            Parsed barrier type.

        Raises
        ------
        ValueError
            If JSON is malformed or fails validation.

        Examples
        --------
        >>> from finstack_quant.models.credit import MertonBarrierType
        >>> restored = MertonBarrierType.from_json(MertonBarrierType.terminal().to_json())
        >>> restored.to_json()
        '"terminal"'

        """
        ...

    def to_json(self) -> str:
        """
        Serialize this barrier type to compact JSON.

        Returns
        -------
        str
            JSON string.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Reconstruct this value for ``pickle`` through its canonical JSON form.

        Returns
        -------
        tuple[Any, tuple[str]]
            The bound ``from_json`` reconstruction callable and its JSON payload.

        Raises
        ------
        ValueError
            If serialization fails or the JSON payload cannot reconstruct the
            stored value, including non-finite floats unsupported by its schema.
        """
        ...

class AssetDynamics:
    """
    Asset return dynamics specification for structural credit models.

    Examples
    --------
    >>> from finstack_quant.models.credit import AssetDynamics
    >>> AssetDynamics.geometric_brownian() is not None
    True

    """

    @staticmethod
    def geometric_brownian() -> AssetDynamics:
        """
        Standard geometric Brownian motion (lognormal diffusion).

        Returns
        -------
        AssetDynamics
            GBM dynamics specification.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.models.credit import AssetDynamics
        >>> dyn = AssetDynamics.geometric_brownian()
        >>> dyn.to_json()
        '"geometric_brownian"'
        """
        ...

    @staticmethod
    def jump_diffusion(
        jump_intensity: float,
        jump_mean: float,
        jump_vol: float,
    ) -> AssetDynamics:
        """
        Merton jump-diffusion asset dynamics.

        Parameters
        ----------
        jump_intensity : float
            Poisson jump arrival intensity (jumps per year).
        jump_mean : float
            Mean log-jump size.
        jump_vol : float
            Volatility of log-jump size.

        Returns
        -------
        AssetDynamics
            Jump-diffusion specification.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.models.credit import AssetDynamics
        >>> dyn = AssetDynamics.jump_diffusion(0.5, -0.1, 0.2)
        >>> "jump_diffusion" in dyn.to_json()
        True
        """
        ...

    @staticmethod
    def credit_grades(
        barrier_uncertainty: float,
        mean_recovery: float,
    ) -> AssetDynamics:
        """
        CreditGrades stochastic-barrier dynamics.

        Parameters
        ----------
        barrier_uncertainty : float
            Log-barrier volatility ``lambda`` (lognormal standard deviation of
            the default barrier).
        mean_recovery : float
            Mean recovery rate at default, as a decimal in ``[0, 1]``.

        Returns
        -------
        AssetDynamics
            CreditGrades dynamics specification.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.models.credit import AssetDynamics
        >>> dyn = AssetDynamics.credit_grades(0.3, 0.4)
        >>> "credit_grades" in dyn.to_json()
        True
        """
        ...

    @staticmethod
    def from_json(json: str) -> AssetDynamics:
        """
        Deserialize asset dynamics from canonical JSON.

        Parameters
        ----------
        json : str
            Canonical JSON payload.

        Returns
        -------
        AssetDynamics
            Parsed dynamics specification.

        Raises
        ------
        ValueError
            If JSON is malformed or fails validation.

        Examples
        --------
        >>> from finstack_quant.models.credit import AssetDynamics
        >>> restored = AssetDynamics.from_json(AssetDynamics.geometric_brownian().to_json())
        >>> restored.to_json()
        '"geometric_brownian"'

        """
        ...

    def to_json(self) -> str:
        """
        Serialize these asset dynamics to compact JSON.

        Returns
        -------
        str
            JSON string.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Reconstruct this value for ``pickle`` through its canonical JSON form.

        Returns
        -------
        tuple[Any, tuple[str]]
            The bound ``from_json`` reconstruction callable and its JSON payload.

        Raises
        ------
        ValueError
            If serialization fails or the JSON payload cannot reconstruct the
            stored value, including non-finite floats unsupported by its schema.
        """
        ...

class MertonReport:
    """
    One Merton report: the model, its term structure, grades and CDS terms.

    Returned by `merton_report`. Per-horizon sequences align with ``horizons``.
    A spread or implied-equity measure Rust rejects for this model reads as
    ``None`` and carries its message in ``errors``.

    Examples
    --------
    >>> from finstack_quant.models.credit import merton_report
    >>> report = merton_report({
    ...     "source": {
    ...         "kind": "equity",
    ...         "equity_value": 50.0,
    ...         "equity_vol": 0.4,
    ...         "short_term_debt": 30.0,
    ...         "long_term_debt": 40.0,
    ...         "risk_free_rate": 0.03,
    ...         "payout_rate": 0.0,
    ...         "maturity": 1.0,
    ...     },
    ...     "horizons": [1.0, 5.0],
    ...     "maturity": 5.0,
    ...     "recovery": 0.4,
    ... })
    >>> report.default_point
    50.0

    """

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` via the canonical JSON round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(MertonReport.from_json, (json,))`` so unpickling rebuilds the report.

        Notes
        -----
        This accessor does not raise; it serializes the stored value.
        """
        ...

    @staticmethod
    def from_json(json: str) -> MertonReport:
        """
        Deserialize a report from its canonical JSON form.

        Parameters
        ----------
        json : str
            Canonical JSON as produced by `to_json`; unknown fields are rejected.

        Returns
        -------
        MertonReport
            The reconstructed report; nothing is recomputed.

        Raises
        ------
        ValueError
            If ``json`` is malformed or has an unknown field.

        Examples
        --------
        >>> from finstack_quant.models.credit import MertonReport, merton_report
        >>> firm = {"asset_value": 100.0, "asset_vol": 0.25, "debt_barrier": 60.0, "risk_free_rate": 0.03}
        >>> firm |= {"payout_rate": 0.0, "barrier_type": "terminal", "dynamics": "geometric_brownian"}
        >>> source = {"kind": "model", "model": firm}
        >>> report = merton_report({"source": source, "horizons": [1.0], "maturity": 5.0, "recovery": 0.4})
        >>> MertonReport.from_json(report.to_json()).horizons
        [1.0]
        """
        ...

    def to_json(self) -> str:
        """
        Serialize the report to canonical JSON.

        Returns
        -------
        str
            Canonical JSON with a rejected measure as ``null`` and its message
            in ``errors``.

        Raises
        ------
        ValueError
            If the report holds a non-finite number that JSON cannot carry.
        """
        ...

    @property
    def model(self) -> MertonModel:
        """
        The model the report was computed from.

        Returns
        -------
        MertonModel
            The supplied model, or the model calibrated from equity.

        Notes
        -----
        This accessor does not raise; it reads the stored value.
        """
        ...

    @property
    def default_point(self) -> float | None:
        """
        KMV default point of an equity source.

        Returns
        -------
        float | None
            Short-term debt plus half of long-term debt, in the firm's monetary
            unit, used as the barrier; ``None`` for a supplied model.

        Notes
        -----
        This accessor does not raise; it reads the stored value.
        """
        ...

    @property
    def horizons(self) -> list[float]:
        """
        The requested horizons in years, in request order.

        Returns
        -------
        list[float]
            Horizons in years; every per-horizon sequence aligns with them.

        Notes
        -----
        This accessor does not raise; it reads the stored value.
        """
        ...

    @property
    def default_probabilities(self) -> list[float]:
        """
        Default probability per horizon.

        Returns
        -------
        list[float]
            Decimal probabilities aligned with ``horizons``.

        Notes
        -----
        This accessor does not raise; it reads the stored value.
        """
        ...

    @property
    def distance_to_default(self) -> list[float]:
        """
        Distance to default per horizon.

        Returns
        -------
        list[float]
            Standard deviations of asset value, aligned with ``horizons``.

        Notes
        -----
        This accessor does not raise; it reads the stored value.
        """
        ...

    @property
    def grades(self) -> list[pd.MasterScaleResult]:
        """
        Default-calibration master-scale grade of each default probability.

        Returns
        -------
        list[pd.MasterScaleResult]
            One grade per horizon, from ``pd.MasterScale.default_calibration()``.

        Notes
        -----
        This accessor does not raise; it reads the stored value.
        """
        ...

    @property
    def implied_spread(self) -> list[float] | None:
        """
        Exogenous-recovery zero-coupon spread per horizon.

        Returns
        -------
        list[float] | None
            Decimal per year aligned with ``horizons``; ``None`` when rejected,
            with the message in ``errors``.

        Notes
        -----
        This accessor does not raise; it reads the stored value.
        """
        ...

    @property
    def debt_spread(self) -> list[float] | None:
        """
        Endogenous Merton (1974) debt spread per horizon.

        Returns
        -------
        list[float] | None
            Decimal per year aligned with ``horizons``; ``None`` for a model
            without a terminal barrier, with the message in ``errors``.

        Notes
        -----
        This accessor does not raise; it reads the stored value.
        """
        ...

    @property
    def maturity(self) -> float:
        """
        The requested CDS and implied-equity maturity.

        Returns
        -------
        float
            Maturity in years.

        Notes
        -----
        This accessor does not raise; it reads the stored value.
        """
        ...

    @property
    def recovery(self) -> float:
        """
        The requested recovery.

        Returns
        -------
        float
            Decimal fraction of par used by the implied and CDS spreads.

        Notes
        -----
        This accessor does not raise; it reads the stored value.
        """
        ...

    @property
    def cds_par_spread(self) -> float | None:
        """
        CDS par spread at ``maturity``.

        Returns
        -------
        float | None
            Decimal per year; ``None`` when rejected, with the message in ``errors``.

        Notes
        -----
        This accessor does not raise; it reads the stored value.
        """
        ...

    @property
    def implied_equity(self) -> tuple[float, float] | None:
        """
        Implied equity at ``maturity``.

        Returns
        -------
        tuple[float, float] | None
            ``(equity value, annualized equity volatility)``; ``None`` when
            rejected, with the message in ``errors``.

        Notes
        -----
        This accessor does not raise; it reads the stored value.
        """
        ...

    @property
    def errors(self) -> dict[str, str]:
        """
        Messages of the measures Rust rejected for this model.

        Returns
        -------
        dict[str, str]
            Message by measure name (``implied_spread``, ``debt_spread``,
            ``cds_par_spread``, ``implied_equity``); empty when all computed.

        Notes
        -----
        This accessor does not raise; it reads the stored value.
        """
        ...

    def to_arrow(self) -> ArrowTable:
        """Export the Merton report term structure as an :class:`~finstack_quant.core.table.ArrowTable`.

        Returns
        -------
        ArrowTable
            One row per horizon with ``horizon``, ``default_probability``,
            ``distance_to_default``, ``implied_spread``, ``debt_spread``,
            ``grade``, and ``central_pd``. Rejected spread columns hold nulls.

        Raises
        ------
        ValueError
            If the result cannot be represented as an Arrow table.
        """
        ...

    def to_pandas(self) -> pandas.DataFrame:
        """Export the Merton report term structure as a pandas ``DataFrame``.

        Returns
        -------
        pandas.DataFrame
            One row per horizon with ``horizon``, ``default_probability``,
            ``distance_to_default``, ``implied_spread``, ``debt_spread``,
            ``grade``, and ``central_pd``.

        Raises
        ------
        ImportError
            If ``pandas`` is not installed.
        ValueError
            If the result cannot be represented as a pandas DataFrame.
        """
        ...

    def to_polars(self) -> pl.DataFrame:
        """Export the Merton report term structure as a Polars ``DataFrame``.

        Returns
        -------
        polars.DataFrame
            One row per horizon with ``horizon``, ``default_probability``,
            ``distance_to_default``, ``implied_spread``, ``debt_spread``,
            ``grade``, and ``central_pd``.

        Raises
        ------
        ImportError
            If ``polars`` is not installed.
        ValueError
            If the result cannot be represented as a Polars DataFrame.
        """
        ...

    def to_pyarrow(self) -> Any:
        """Export the Merton report term structure as a ``pyarrow.Table``.

        Returns
        -------
        pyarrow.Table
            One row per horizon with ``horizon``, ``default_probability``,
            ``distance_to_default``, ``implied_spread``, ``debt_spread``,
            ``grade``, and ``central_pd``.

        Raises
        ------
        ImportError
            If ``pyarrow`` is not installed.
        ValueError
            If the result cannot be represented as a PyArrow table.
        """
        ...

    def __arrow_c_stream__(self, requested_schema: object | None = None) -> object: ...
    def __arrow_c_schema__(self) -> object: ...
    def _repr_html_(self) -> str | None: ...
    def __repr__(self) -> str:
        """
        Return a compact debug representation.

        Returns
        -------
        str
            Debug text naming the class and its serialized fields.

        Notes
        -----
        This accessor does not raise; it serializes the stored value.
        """
        ...

def merton_report(request: dict[str, Any]) -> MertonReport:
    """
    Compute a Merton report in one call.

    An equity source takes the KMV default point (short-term debt plus half of
    long-term debt) as its barrier, and grades come from the default PD master
    scale, so every host gets the same numbers.

    Parameters
    ----------
    request : dict[str, Any]
        ``MertonReportRequest`` as a dict: ``source`` (``{"kind":
        "model", "model": ...}`` with canonical `MertonModel` JSON, or
        ``{"kind": "equity", "equity_value", "equity_vol", "short_term_debt",
        "long_term_debt", "risk_free_rate", "payout_rate", "maturity"}`` in the
        firm's monetary unit, decimals and years), ``horizons`` (years, finite
        and positive, in display order), ``maturity`` (CDS and implied-equity
        years) and ``recovery`` (decimal fraction of par).

    Returns
    -------
    MertonReport
        The full report; a measure rejected for this model reads as ``None``.

    Raises
    ------
    ValueError
        If the request is malformed or has an unknown field, a horizon is
        non-finite or not positive, or the equity calibration rejects its inputs.

    Examples
    --------
    >>> from finstack_quant.models.credit import merton_report
    >>> firm = {"asset_value": 100.0, "asset_vol": 0.25, "debt_barrier": 60.0, "risk_free_rate": 0.03}
    >>> firm |= {"payout_rate": 0.0, "barrier_type": "terminal", "dynamics": "geometric_brownian"}
    >>> source = {"kind": "model", "model": firm}
    >>> report = merton_report({"source": source, "horizons": [1.0, 5.0], "maturity": 5.0, "recovery": 0.4})
    >>> (report.default_point, len(report.grades), report.errors)
    (None, 2, {})
    """
    ...

class MertonModel:
    """
    Merton (1974) structural credit model with optional CreditGrades calibration.

    Firm value follows geometric Brownian motion under the risk-neutral measure;
    default occurs when asset value crosses a debt barrier at horizon. Spreads
    and default probabilities are risk-neutral.

    Examples
    --------
    >>> from finstack_quant.models.credit import MertonModel
    >>> model = MertonModel(100.0, 0.25, 80.0, 0.05)
    >>> (round(model.distance_to_default(1.0), 6), round(model.default_probability(1.0), 6))
    (0.967574, 0.166629)

    """

    def __init__(
        self,
        asset_value: float,
        asset_vol: float,
        debt_barrier: float,
        risk_free_rate: float,
    ) -> None:
        """
        Construct a Merton structural model from firm asset inputs.

        Parameters
        ----------
        asset_value : float
            Firm asset value (positive, finite).
        asset_vol : float
            Annualized asset volatility as a decimal (e.g. ``0.30`` for 30%).
        debt_barrier : float
            Default barrier, typically total debt face value.
        risk_free_rate : float
            Continuously compounded risk-free rate as a decimal.

        Raises
        ------
        ValueError
            If inputs are non-finite or out of range.

        Sources
        -------
        See ``docs/REFERENCES.md#merton-1974``.
        """
        ...

    @staticmethod
    def from_equity(
        equity_value: float,
        equity_vol: float,
        total_debt: float,
        risk_free_rate: float,
        payout_rate: float,
        maturity: float,
    ) -> MertonModel:
        """
        KMV calibration from observed equity value and volatility.

        Parameters
        ----------
        equity_value : float
            Observed market equity value (positive, finite).
        equity_vol : float
            Equity volatility as a decimal.
        total_debt : float
            Face value of debt used as the default barrier.
        risk_free_rate : float
            Continuously compounded risk-free rate as a decimal.
        payout_rate : float
            Continuous dividend / payout yield on assets as a decimal.
        maturity : float
            Calibration horizon in years.

        Returns
        -------
        MertonModel
            Calibrated structural model.

        Raises
        ------
        ValueError
            If inputs are invalid or calibration fails to converge.

        Examples
        --------
        >>> from finstack_quant.models.credit import MertonModel
        >>> model = MertonModel.from_equity(25.0, 0.30, 80.0, 0.05, 0.0, 1.0)
        >>> model.asset_value > 0
        True

        """
        ...

    @staticmethod
    def from_cds_spread(
        cds_spread_bp: float,
        recovery: float,
        total_debt: float,
        risk_free_rate: float,
        maturity: float,
        asset_value: float,
        payout_rate: float,
    ) -> MertonModel:
        """
        Calibrate asset volatility to match a quoted CDS par spread.

        The objective is :meth:`cds_par_spread`, a full ISDA-style par spread
        built from the model's survival curve, not the zero-coupon
        approximation of :meth:`implied_spread`. Because the par spread is not
        monotonic in asset volatility, the objective is scanned across
        ``[0.01, 2.0]`` and a quote that no volatility reproduces, or one
        consistent with several, is rejected rather than resolved arbitrarily.

        Parameters
        ----------
        cds_spread_bp : float
            Quoted CDS par spread in basis points; must be finite and positive.
        recovery : float
            Assumed recovery rate as a decimal in ``[0, 1)``.
        total_debt : float
            Face value of debt acting as the default barrier, strictly
            positive and in the same currency as ``asset_value``.
        risk_free_rate : float
            Continuously compounded discount rate as a decimal, used for both
            CDS legs.
        maturity : float
            CDS maturity in years, strictly positive.
        asset_value : float
            Assumed initial firm asset value, held fixed during the solve.
        payout_rate : float
            Continuous payout rate on assets as a decimal.

        Returns
        -------
        MertonModel
            Calibrated structural model with terminal barrier and GBM dynamics.

        Raises
        ------
        ValueError
            If inputs are out of range, if no volatility in ``[0.01, 2.0]``
            reproduces the quote, or if the quote is consistent with more than
            one volatility.

        Examples
        --------
        >>> from finstack_quant.models.credit import MertonModel
        >>> model = MertonModel.from_cds_spread(150.0, 0.40, 80.0, 0.04, 5.0, 100.0, 0.0)
        >>> model.asset_vol > 0
        True

        """
        ...

    @staticmethod
    def from_target_pd(
        asset_value: float,
        asset_vol: float,
        risk_free_rate: float,
        payout_rate: float,
        target_pd: float,
        maturity: float,
    ) -> MertonModel:
        """
        Calibrate the debt barrier to match a target cumulative default probability.

        ``target_pd`` is interpreted under the risk-neutral measure. To
        calibrate against a physical default rate, pass the firm's expected
        physical asset return as ``risk_free_rate``; the resulting barrier then
        reproduces that probability through
        :meth:`default_probability_with_drift`.

        Parameters
        ----------
        asset_value : float
            Current firm asset value, strictly positive.
        asset_vol : float
            Annualized asset volatility as a decimal, strictly positive. A
            zero-volatility firm has a degenerate step-function default
            probability that cannot hit an interior target.
        risk_free_rate : float
            Continuously compounded risk-free rate as a decimal.
        payout_rate : float
            Continuous payout rate on assets as a decimal. It enters the
            calibration drift and is carried on the returned model, so omitting
            it shifts the barrier whenever the model is later evaluated with a
            non-zero payout.
        target_pd : float
            Target cumulative default probability in ``(0, 1)``.
        maturity : float
            Calibration horizon in years, strictly positive.

        Returns
        -------
        MertonModel
            Calibrated structural model.

        Raises
        ------
        ValueError
            If inputs are invalid or no barrier attains the target.

        Examples
        --------
        >>> from finstack_quant.models.credit import MertonModel
        >>> model = MertonModel.from_target_pd(100.0, 0.25, 0.05, 0.0, 0.05, 1.0)
        >>> round(model.default_probability(1.0), 6)
        0.05

        """
        ...

    @staticmethod
    def kmv_default_point(short_term_debt: float, long_term_debt: float) -> float:
        """
        Compute the Moody's KMV default point.

        The KMV framework does not use total liabilities as the default
        barrier. Firms empirically default when asset value falls to roughly
        current liabilities plus half of long-term liabilities, because
        long-dated debt does not have to be repaid immediately. Feed the result
        in as the debt barrier when building a model for KMV/EDF work.

        Parameters
        ----------
        short_term_debt : float
            Book value of debt and other liabilities due within one year, in
            the issuer's reporting currency. Must be finite and non-negative.
        long_term_debt : float
            Book value of debt maturing beyond one year, in the same currency.
            Must be finite and non-negative; exactly half of it enters the
            default point.

        Returns
        -------
        float
            ``short_term_debt + 0.5 * long_term_debt``, in the same currency as
            the inputs.

        Raises
        ------
        ValueError
            If either input is negative or non-finite, or if the resulting
            default point is zero.

        Examples
        --------
        >>> from finstack_quant.models.credit import MertonModel
        >>> MertonModel.kmv_default_point(40.0, 120.0)
        100.0

        """
        ...

    @staticmethod
    def new_with_dynamics(
        asset_value: float,
        asset_vol: float,
        debt_barrier: float,
        risk_free_rate: float,
        payout_rate: float,
        barrier_type: MertonBarrierType,
        dynamics: AssetDynamics,
    ) -> MertonModel:
        """
        Construct a Merton model with explicit barrier and dynamics specifications.

        Parameters
        ----------
        asset_value : float
            Current firm asset value.
        asset_vol : float
            Asset volatility as a decimal.
        debt_barrier : float
            Default barrier level.
        risk_free_rate : float
            Continuously compounded risk-free rate as a decimal.
        payout_rate : float
            Continuous payout rate on assets as a decimal.
        barrier_type : MertonBarrierType
            Terminal or first-passage barrier monitoring.
        dynamics : AssetDynamics
            Asset return dynamics specification.

        Returns
        -------
        MertonModel
            Fully specified structural model.

        Raises
        ------
        ValueError
            If inputs are non-finite or out of range.

        Examples
        --------
        >>> from finstack_quant.models.credit import (
        ...     AssetDynamics,
        ...     MertonBarrierType,
        ...     MertonModel,
        ... )
        >>> model = MertonModel.new_with_dynamics(
        ...     100.0,
        ...     0.25,
        ...     80.0,
        ...     0.05,
        ...     0.0,
        ...     MertonBarrierType.first_passage(0.02),
        ...     AssetDynamics.geometric_brownian(),
        ... )
        >>> round(model.default_probability(1.0), 6)
        0.373747

        """
        ...

    @staticmethod
    def credit_grades(
        equity_value: float,
        equity_vol: float,
        total_debt: float,
        risk_free_rate: float,
        barrier_uncertainty: float,
        mean_recovery: float,
    ) -> MertonModel:
        """
        Build a CreditGrades-style model calibrated from equity inputs.

        Inverts the structural mapping from observable equity value and volatility
        to implied firm asset value and asset volatility, with barrier uncertainty
        and mean recovery governing the default boundary.

        Parameters
        ----------
        equity_value : float
            Market equity value (positive, finite).
        equity_vol : float
            Equity volatility as a decimal.
        total_debt : float
            Total debt face used as the reference barrier scale.
        risk_free_rate : float
            Continuously compounded risk-free rate as a decimal.
        barrier_uncertainty : float
            Barrier uncertainty parameter (CreditGrades ``alpha`` scale).
        mean_recovery : float
            Expected recovery rate as a decimal in ``[0, 1]``.

        Returns
        -------
        MertonModel
            Calibrated structural model.

        Raises
        ------
        ValueError
            If inputs are non-finite or violate model constraints.

        Sources
        -------
        See ``docs/REFERENCES.md#merton-1974`` and ``docs/REFERENCES.md#o-kane-2008``.

        Examples
        --------
        >>> from finstack_quant.models.credit import MertonModel
        >>> model = MertonModel.credit_grades(100.0, 0.3, 80.0, 0.05, 0.3, 0.4)
        >>> round(model.default_probability(1.0), 6)
        0.00013

        """
        ...

    @staticmethod
    def from_json(json: str) -> MertonModel:
        """
        Deserialize a structural credit model from JSON.

        Parameters
        ----------
        json : str
            Canonical JSON payload for ``MertonModel``.

        Returns
        -------
        MertonModel
            Parsed model instance.

        Raises
        ------
        ValueError
            If JSON is malformed or fails validation.

        Examples
        --------
        >>> from finstack_quant.models.credit import MertonModel
        >>> model = MertonModel(100.0, 0.25, 80.0, 0.05)
        >>> round(MertonModel.from_json(model.to_json()).default_probability(1.0), 6)
        0.166629

        """
        ...

    def to_json(self) -> str:
        """
        Serialize this model to pretty-printed canonical JSON.

        Returns
        -------
        str
            JSON string.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def asset_value(self) -> float:
        """
        Current firm asset value ``V_0``.

        Returns
        -------
        float
            Asset value in the issuer's reporting currency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def asset_vol(self) -> float:
        """
        Annualized asset volatility ``sigma_V``.

        Returns
        -------
        float
            Volatility as a decimal (``0.25`` is 25%).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def debt_barrier(self) -> float:
        """
        Asset-value default barrier ``B`` in the Merton model.

        Returns
        -------
        float
            Barrier level, in the same currency as ``asset_value``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def risk_free_rate(self) -> float:
        """
        Continuously compounded risk-free rate ``r``.

        Returns
        -------
        float
            Rate as a decimal.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def payout_rate(self) -> float:
        """
        Continuous payout (dividend) rate ``q`` on assets.

        Returns
        -------
        float
            Payout rate as a decimal.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def barrier_type(self) -> MertonBarrierType:
        """
        Barrier monitoring convention.

        Returns
        -------
        MertonBarrierType
            Terminal or first-passage barrier specification.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def dynamics(self) -> AssetDynamics:
        """
        Asset return dynamics specification.

        Returns
        -------
        AssetDynamics
            GBM, jump-diffusion, or CreditGrades dynamics.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_arrow(self) -> ArrowTable:
        """Export the Merton model parameters as a single-row :class:`~finstack_quant.core.table.ArrowTable`.

        Returns
        -------
        ArrowTable
            Single-row table with the canonical model fields as columns.

        Raises
        ------
        ValueError
            If the result cannot be represented as an Arrow table.
        """
        ...

    def to_pandas(self) -> pandas.DataFrame:
        """Export the Merton model parameters as a single-row pandas ``DataFrame``.

        Returns
        -------
        pandas.DataFrame
            Single-row frame with the canonical model fields as columns.

        Raises
        ------
        ImportError
            If ``pandas`` is not installed.
        ValueError
            If the result cannot be represented as a pandas DataFrame.
        """
        ...

    def to_polars(self) -> pl.DataFrame:
        """Export the Merton model parameters as a single-row Polars ``DataFrame``.

        Returns
        -------
        polars.DataFrame
            Single-row frame with the canonical model fields as columns.

        Raises
        ------
        ImportError
            If ``polars`` is not installed.
        ValueError
            If the result cannot be represented as a Polars DataFrame.
        """
        ...

    def to_pyarrow(self) -> Any:
        """Export the Merton model parameters as a single-row ``pyarrow.Table``.

        Returns
        -------
        pyarrow.Table
            Single-row table with the canonical model fields as columns.

        Raises
        ------
        ImportError
            If ``pyarrow`` is not installed.
        ValueError
            If the result cannot be represented as a PyArrow table.
        """
        ...

    def __arrow_c_stream__(self, requested_schema: object | None = None) -> object: ...
    def __arrow_c_schema__(self) -> object: ...
    def _repr_html_(self) -> str | None: ...
    def distance_to_default(self, horizon: float) -> float:
        """
        Return risk-neutral distance to default at ``horizon`` years.

        This is the risk-neutral ``d2``, driven by the risk-free rate. It is
        not the Moody's KMV distance-to-default; use
        :meth:`distance_to_default_with_drift` with
        :meth:`kmv_default_point` for that. Under jump-diffusion or
        CreditGrades dynamics ``N(-dd)`` differs from
        :meth:`default_probability`, which remains the authoritative
        probability.

        Parameters
        ----------
        horizon : float
            Horizon in years (positive, finite). A non-positive horizon
            returns ``inf``.

        Returns
        -------
        float
            Distance-to-default statistic (standard-deviation units).

        Notes
        -----
        This method does not raise; out-of-domain or non-finite inputs yield ``NaN`` or ``inf`` rather than an exception.

        Examples
        --------
        >>> from finstack_quant.models.credit import MertonModel
        >>> MertonModel(100.0, 0.25, 80.0, 0.04).distance_to_default(1.0) > 0
        True

        """
        ...

    def distance_to_default_with_drift(self, asset_drift: float, horizon: float) -> float:
        """
        Return physical-measure (Moody's KMV) distance to default.

        Replaces the risk-free rate with the firm's expected physical asset
        return, giving the KMV/EDF construction. Pair it with
        :meth:`kmv_default_point` to reproduce the KMV default-point
        convention when building the model.

        Parameters
        ----------
        asset_drift : float
            Expected physical total return on the firm's assets, continuously
            compounded and expressed as a decimal (``0.09`` is 9% per annum).
            The model's ``payout_rate`` is still subtracted.
        horizon : float
            Horizon in years (positive, finite). A non-positive horizon
            returns ``inf``.

        Returns
        -------
        float
            Physical-measure distance-to-default (standard-deviation units).

        Raises
        ------
        ValueError
            If ``asset_drift`` is not finite, or the model uses CreditGrades
            dynamics, which are driftless by construction.

        Examples
        --------
        >>> from finstack_quant.models.credit import MertonModel
        >>> model = MertonModel(100.0, 0.25, 80.0, 0.04)
        >>> model.distance_to_default_with_drift(0.09, 1.0) > model.distance_to_default(1.0)
        True

        """
        ...

    def default_probability(self, horizon: float) -> float:
        """
        Return risk-neutral default probability over ``horizon`` years.

        This is the pricing (Q-measure) probability and materially overstates
        the real-world default rate whenever the market price of asset risk is
        positive. Use :meth:`default_probability_with_drift` for
        expected-loss, capital, or rating analytics.

        Parameters
        ----------
        horizon : float
            Horizon in years (positive, finite). A non-positive horizon
            returns ``0.0``.

        Returns
        -------
        float
            Default probability in ``[0, 1]``.

        Raises
        ------
        ValueError
            If a horizon is non-finite or a Gaussian variate is undefined.
        RuntimeError
            If the structural model cannot resolve a finite probability.

        Examples
        --------
        >>> from finstack_quant.models.credit import MertonModel
        >>> 0.0 <= MertonModel(100.0, 0.25, 80.0, 0.04).default_probability(1.0) <= 1.0
        True

        """
        ...

    def default_probabilities(self, horizons: list[float]) -> LabeledSeries:
        """
        Risk-neutral cumulative default probabilities over several horizons.

        Each horizon follows the same Rust rule, so a non-positive horizon gives ``0.0``.

        Parameters
        ----------
        horizons : list[float]
            Horizons in years.

        Returns
        -------
        LabeledSeries
            Series named ``default_probability`` indexed by ``horizon`` labels
            (``str(horizon)``), in input order.

        Raises
        ------
        ValueError
            If a horizon is non-finite or a Gaussian variate is undefined.
        RuntimeError
            If the structural model cannot resolve a finite probability.
        """
        ...

    def default_probability_with_drift(self, asset_drift: float, horizon: float) -> float:
        """
        Return physical-measure default probability (theoretical EDF).

        Identical dispatch to :meth:`default_probability`, with the firm's
        expected physical asset return substituted for the risk-free rate.
        Moody's published EDF applies a further proprietary empirical mapping
        from distance-to-default to observed default frequency, which is not
        reproduced here.

        Parameters
        ----------
        asset_drift : float
            Expected physical total return on the firm's assets, continuously
            compounded and expressed as a decimal (``0.09`` is 9% per annum).
        horizon : float
            Horizon in years (positive, finite). A non-positive horizon
            returns ``0.0``.

        Returns
        -------
        float
            Physical-measure default probability in ``[0, 1]``.

        Raises
        ------
        ValueError
            If drift or horizon is non-finite, a Gaussian variate is undefined,
            or the model uses driftless CreditGrades dynamics.
        RuntimeError
            If the structural model cannot resolve a finite probability.

        Examples
        --------
        >>> from finstack_quant.models.credit import MertonModel
        >>> model = MertonModel(100.0, 0.25, 80.0, 0.04)
        >>> model.default_probability_with_drift(0.11, 5.0) < model.default_probability(5.0)
        True

        """
        ...

    def implied_spread(self, horizon: float, recovery: float) -> float:
        """
        Return the zero-coupon bond credit spread with exogenous recovery.

        This is the continuously compounded spread of a risky discount bond
        whose recovery is a fixed fraction of face value paid at maturity. It
        is neither the Merton endogenous spread (:meth:`debt_spread`) nor a CDS
        par spread (:meth:`cds_par_spread`); the three differ materially at
        distressed levels.

        Parameters
        ----------
        horizon : float
            Bond maturity in years; must be finite and positive.
        recovery : float
            Assumed recovery rate as a decimal in ``[0, 1]`` (not basis
            points), treated as paid at maturity.

        Returns
        -------
        float
            Zero-coupon spread as a decimal (e.g. ``0.012`` for 120 bp).

        Raises
        ------
        ValueError
            If ``horizon`` or ``recovery`` are invalid.

        Sources
        -------
        See ``docs/REFERENCES.md#o-kane-2008`` for CDS spread conventions.

        Examples
        --------
        >>> from finstack_quant.models.credit import MertonModel
        >>> MertonModel(100.0, 0.25, 80.0, 0.04).implied_spread(5.0, 0.40) > 0
        True

        """
        ...

    def debt_spread(self, horizon: float) -> float:
        """
        Return the Merton (1974) endogenous credit spread on the firm's debt.

        Recovery is endogenous: debt holders receive ``min(V_T, B)``, so the
        recovery rate is the firm's own terminal asset value rather than an
        assumed constant. This is typically well below
        :meth:`implied_spread` at a 40% exogenous recovery.

        Parameters
        ----------
        horizon : float
            Maturity of the firm's debt in years; must be finite and positive.

        Returns
        -------
        float
            Endogenous debt spread as a decimal (e.g. ``0.004`` for 40 bp).

        Raises
        ------
        ValueError
            If ``horizon`` is not positive, the barrier type is not terminal,
            or the implied debt value is non-positive.

        Examples
        --------
        >>> from finstack_quant.models.credit import MertonModel
        >>> model = MertonModel(100.0, 0.25, 80.0, 0.04)
        >>> model.debt_spread(5.0) < model.implied_spread(5.0, 0.40)
        True

        """
        ...

    def cds_par_spread(self, maturity: float, recovery: float) -> float:
        """
        Return the ISDA-style CDS par spread implied by the model.

        The model's survival probabilities are exported to a hazard curve on
        the quarterly premium grid and both CDS legs are priced against it,
        including accrual on default and discounting. Prefer this over
        :meth:`implied_spread` whenever the target is a quoted CDS level.

        Parameters
        ----------
        maturity : float
            CDS maturity in years; must be finite and positive.
        recovery : float
            Recovery rate as a decimal in ``[0, 1]``. Under CreditGrades
            dynamics it must equal the model's own ``mean_recovery``.

        Returns
        -------
        float
            Par spread as a decimal per annum (multiply by 10,000 for basis
            points).

        Raises
        ------
        ValueError
            If ``maturity`` is not positive, if ``recovery`` is out of range or
            contradicts the model's ``mean_recovery``, or if the implied
            survival curve cannot be bootstrapped.

        Examples
        --------
        >>> from finstack_quant.models.credit import MertonModel
        >>> model = MertonModel(100.0, 0.25, 80.0, 0.04)
        >>> model.cds_par_spread(5.0, 0.40) > model.implied_spread(5.0, 0.40)
        True

        """
        ...

    def implied_equity(self, horizon: float) -> tuple[float, float]:
        """
        Return implied equity value and equity volatility at ``horizon`` years.

        Diffusion-only. Jump-diffusion dynamics are rejected, because the
        delta-scaled volatility would be the diffusive component alone and
        would misstate observed equity volatility.

        Parameters
        ----------
        horizon : float
            Time horizon in years (positive, finite).

        Returns
        -------
        tuple[float, float]
            ``(equity_value, equity_vol)`` implied by the structural model.

        Raises
        ------
        ValueError
            When ``horizon`` is not positive, the model uses jump-diffusion
            dynamics, or the firm is economically in default and the inversion
            is numerically ill-conditioned.
        """
        ...

    def to_hazard_curve(
        self,
        id: str,
        base_date: datetime.date,
        tenors: list[float],
        recovery: float,
        day_count: DayCount | str,
    ) -> HazardCurve:
        """
        Bootstrap a piecewise-constant hazard curve from structural default probabilities.

        The curve carries risk-neutral hazard rates, since it is built from
        :meth:`default_probability`.

        Parameters
        ----------
        id : str
            Curve identifier, used as the lookup key in a market context.
        base_date : datetime.date
            Valuation date the curve's year fractions are measured from.
        tenors : list[float]
            Tenor grid in years (non-empty, strictly positive, distinct). Need
            not be sorted.
        recovery : float
            Recovery rate assumption as a decimal in ``[0, 1]``. Under
            CreditGrades dynamics it must equal the model's own
            ``mean_recovery``, since that value already sets the barrier.
        day_count : DayCount | str
            Day-count convention the curve uses to turn dates into year
            fractions, e.g. ``"act_365f"``. Pass the convention of the discount
            curve the hazard curve will be paired with; there is no default,
            as in Rust and WASM.

        Returns
        -------
        HazardCurve
            Bootstrapped hazard curve compatible with pricing engines.

        Raises
        ------
        ValueError
            If ``tenors`` is empty, contains non-positive or duplicate values,
            if ``recovery`` is out of range or contradicts ``mean_recovery``,
            if ``day_count`` is not a recognized convention, if survival
            reaches zero at some tenor, or if the bootstrap otherwise fails.
        """
        ...

    def simulate_paths(
        self,
        num_paths: int,
        num_steps: int,
        horizon: float,
        seed: int,
        antithetic: bool,
    ) -> SimulatedPaths:
        """
        Simulate asset value paths using Monte Carlo.

        Parameters
        ----------
        num_paths : int
            Number of paths to simulate.
        num_steps : int
            Number of time steps per path (must be >= 1).
        horizon : float
            Simulation horizon in years (must be > 0).
        seed : int
            Seed for reproducible draws. The Rust
            ``MertonModel::simulate_paths`` owns the generator (PCG64),
            so equal seeds give equal paths in Python and WASM.
        antithetic : bool
            When ``True``, use antithetic variates for variance reduction.
            Required, as in Rust and WASM.

        Returns
        -------
        SimulatedPaths
            Time grid and simulated asset paths.

        Raises
        ------
        ValueError
            If ``num_steps`` is zero or ``horizon`` is non-positive.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Reconstruct this value for ``pickle`` through its canonical JSON form.

        Returns
        -------
        tuple[Any, tuple[str]]
            The bound ``from_json`` reconstruction callable and its JSON payload.

        Raises
        ------
        ValueError
            If serialization fails or the JSON payload cannot reconstruct the
            stored value, including non-finite floats unsupported by its schema.
        """
        ...
