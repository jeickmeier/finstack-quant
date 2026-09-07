"""
Structural credit models and path-dependent credit specifications.

Bindings for Merton-style structural models, dynamic recovery, endogenous hazard
rates, credit-state snapshots, and toggle exercise rules used by PIK/toggle
bonds and similar instruments.

Examples
--------
>>> from finstack_quant.models.credit import MertonModel
>>> round(MertonModel(100.0, 0.25, 80.0, 0.05).default_probability(1.0), 6)
0.166629

"""

from __future__ import annotations

import datetime
import pandas as pd

from finstack_quant.core.market_data.curves import HazardCurve
from finstack_quant.core.types import CreditRating
from typing import Any

__all__ = [
    "AssetDynamics",
    "BarrierType",
    "CreditState",
    "DynamicRecoverySpec",
    "EndogenousHazardSpec",
    "MertonModel",
    "RatingFactorTable",
    "SimulatedPaths",
    "ToggleExerciseModel",
]

class RatingFactorTable:
    """
    Rating-factor table (Moody's WARF methodology) from the embedded credit registry.

    Examples
    --------
    >>> from finstack_quant.models.credit import RatingFactorTable
    >>> table = RatingFactorTable.moodys_standard()
    >>> (table.get_factor("B"), table.agency)
    (2720.0, "Moody's")

    """

    @staticmethod
    def moodys_standard() -> RatingFactorTable:
        """
        Load the embedded Moody's standard WARF table.

        Returns
        -------
        RatingFactorTable
            Default rating-factor table.

        Raises
        ------
        ValueError
            If the embedded registry is invalid or its default table is missing.

        Examples
        --------
        >>> from finstack_quant.models.credit import RatingFactorTable
        >>> RatingFactorTable.moodys_standard().default_factor > 0.0
        True
        """
        ...

    @staticmethod
    def from_registry_id(id: str) -> RatingFactorTable:
        """
        Load a named rating-factor table from the embedded registry.

        Parameters
        ----------
        id : str
            Exact registry identifier of the methodology; an unknown id never
            falls back to the default table.

        Returns
        -------
        RatingFactorTable
            The named table.

        Raises
        ------
        KeyError
            If ``id`` is unknown.
        ValueError
            If the embedded registry is invalid.

        Examples
        --------
        >>> from finstack_quant.models.credit import RatingFactorTable
        >>> RatingFactorTable.from_registry_id("moodys_standard").agency
        "Moody's"
        """
        ...

    def get_factor(self, rating: str | CreditRating) -> float:
        """
        Rating factor for an exact rating notch.

        Parameters
        ----------
        rating : str | CreditRating
            Rating notch as a ``core.types.CreditRating`` or a rating string
            (``"BBB-"``, ``"Baa3"``).

        Returns
        -------
        float
            Rating factor.

        Raises
        ------
        ValueError
            If the notch has no factor in this table or the string is not a
            recognised rating.
        TypeError
            If ``rating`` is neither a string nor a ``CreditRating``.
        """
        ...

    @property
    def agency(self) -> str:
        """
        Agency the table is sourced from (e.g. ``"moodys"``).

        Returns
        -------
        str
            Agency label.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def methodology(self) -> str:
        """
        Methodology label recorded in the registry for this table.

        Returns
        -------
        str
            Methodology label.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def default_factor(self) -> float:
        """
        Factor assigned to unrated or defaulted names.

        Returns
        -------
        float
            Default rating factor.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @staticmethod
    def from_json(json: str) -> RatingFactorTable:
        """
        Deserialize a ``RatingFactorTable`` from its canonical JSON form.

        Parameters
        ----------
        json : str
            JSON text produced by :meth:`to_json` (strict serde; unknown or
            invalid fields are rejected).

        Returns
        -------
        RatingFactorTable
            Reconstructed value.

        Raises
        ------
        ValueError
            If the payload is not valid ``RatingFactorTable`` JSON.

        Examples
        --------
        >>> from finstack_quant.models.credit import RatingFactorTable
        >>> value = RatingFactorTable.moodys_standard()
        >>> RatingFactorTable.from_json(value.to_json()).to_json() == value.to_json()
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to compact canonical JSON.

        Returns
        -------
        str
            JSON text accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the canonical JSON representation.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(from_json, (json,))`` so unpickling rebuilds the value.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style representation rendered from the canonical fields.

        Returns
        -------
        str
            ``Name(field=value, ...)`` with Python literals.

        Notes
        -----
        This method does not raise.
        """
        ...

class BarrierType:
    """
    Default barrier monitoring convention for structural credit models.

    Examples
    --------
    >>> from finstack_quant.models.credit import BarrierType
    >>> BarrierType.terminal() is not None
    True

    """

    @staticmethod
    def terminal() -> BarrierType:
        """
        Classic Merton barrier tested only at maturity.

        Returns
        -------
        BarrierType
            Terminal-barrier specification.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.models.credit import BarrierType
        >>> bt = BarrierType.terminal()
        >>> bt.to_json()
        '"terminal"'
        """
        ...

    @staticmethod
    def first_passage(barrier_growth_rate: float) -> BarrierType:
        """
        Black-Cox first-passage barrier with optional growth rate.

        Parameters
        ----------
        barrier_growth_rate : float
            Continuous growth rate of the default barrier over time, as a
            decimal (e.g. ``0.02`` for 2% annual growth).

        Returns
        -------
        BarrierType
            First-passage barrier specification.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.models.credit import BarrierType
        >>> bt = BarrierType.first_passage(0.02)
        >>> "first_passage" in bt.to_json()
        True

        Sources
        -------
        - Merton (1974): see docs/REFERENCES.md#merton-1974
        """
        ...

    @staticmethod
    def from_json(json: str) -> BarrierType:
        """
        Deserialize a barrier type from canonical JSON.

        Parameters
        ----------
        json : str
            Canonical JSON payload.

        Returns
        -------
        BarrierType
            Parsed barrier type.

        Raises
        ------
        ValueError
            If JSON is malformed or fails validation.

        Examples
        --------
        >>> from finstack_quant.models.credit import BarrierType
        >>> restored = BarrierType.from_json(BarrierType.terminal().to_json())
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

class SimulatedPaths:
    """
    Monte Carlo asset path simulation results from a Merton model.

    Examples
    --------
    >>> from finstack_quant.models.credit import MertonModel
    >>> model = MertonModel(100.0, 0.25, 80.0, 0.05)
    >>> paths = model.simulate_paths(4, 10, 1.0, seed=42)
    >>> (paths.num_paths, paths.num_steps, len(paths.times))
    (4, 10, 11)

    """

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` via the canonical JSON round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(SimulatedPaths.from_json, (json,))`` so unpickling rebuilds the paths.

        Notes
        -----
        This accessor does not raise; it serializes the stored value.
        """
        ...

    @staticmethod
    def from_json(json: str) -> SimulatedPaths:
        """
        Deserialize simulated paths from their canonical JSON form.

        Parameters
        ----------
        json : str
            Canonical JSON with ``times``, ``asset_values`` (row-major),
            ``num_paths`` and ``num_steps``.

        Returns
        -------
        SimulatedPaths
            Reconstructed path set.

        Raises
        ------
        ValueError
            If ``json`` is malformed or has an incompatible shape.

        Examples
        --------
        >>> from finstack_quant.models.credit import SimulatedPaths
        >>> p = SimulatedPaths.from_json('{"times":[0.0,1.0],"asset_values":[100.0,105.0],"num_paths":1,"num_steps":1}')
        >>> p.get(0, 1)
        105.0
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to canonical JSON (``times``, ``asset_values``, ``num_paths``, ``num_steps``).

        Returns
        -------
        str
            Canonical JSON representation of this `SimulatedPaths`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    @property
    def times(self) -> list[float]:
        """
        Time grid from 0 to the simulation horizon.

        Returns
        -------
        list[float]
            Time points in years.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def asset_values(self) -> list[float]:
        """
        Asset values in row-major order.

        Returns
        -------
        list[float]
            Flattened path values.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def num_paths(self) -> int:
        """
        Number of simulated paths.

        Returns
        -------
        int
            Exactly the count requested from ``simulate_paths``. Antithetic
            mirrors are included in this total rather than doubling it, so the
            row count of :attr:`asset_values` is always
            ``num_paths * (num_steps + 1)``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def num_steps(self) -> int:
        """
        Number of time steps between grid points.

        Returns
        -------
        int
            Count of simulation increments, at least ``1``. The grid in
            :attr:`times` holds one more point than this because it includes
            ``t = 0``, and each step spans ``horizon / num_steps`` years.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def values_per_path(self) -> int:
        """
        Number of stored values per path (``num_steps + 1``, including ``t = 0``).

        Returns
        -------
        int
            Number of stored values per path (``num_steps + 1``, including ``t = 0``).

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    def to_dataframe(self) -> pd.DataFrame:
        """
        Long frame with ``path`` (int), ``time`` (float, years), ``asset_value`` (float); one row per path per grid point, ordered by path then time.

        Returns
        -------
        pandas.DataFrame
            Long frame with ``path`` (int), ``time`` (float, years), ``asset_value`` (float); one row per path per grid point, ordered by path then time.

        Raises
        ------
        ValueError
            If the value cannot be serialized into a pandas object.
        """
        ...
    def get(self, path_idx: int, time_idx: int) -> float | None:
        """
        Return one asset value by path and time-grid index.

        Parameters
        ----------
        path_idx : int
            Zero-based path index.
        time_idx : int
            Zero-based time-grid index (includes ``t = 0``).

        Returns
        -------
        float or None
            Asset value at the requested coordinate, or ``None`` when indices
            are out of range.

        Notes
        -----
        This method does not raise; a missing result is ``None`` rather than an exception.
        """
        ...

    def path(self, path_idx: int) -> list[float] | None:
        """
        Return the contiguous asset-value row for one path.

        Parameters
        ----------
        path_idx : int
            Zero-based path index.

        Returns
        -------
        list[float] or None
            Asset values along the path, or ``None`` when ``path_idx`` is out
            of range.

        Notes
        -----
        This method does not raise; a missing result is ``None`` rather than an exception.
        """
        ...

    def to_nested(self) -> list[list[float]]:
        """
        Materialize nested path storage as a list of path rows.

        Returns
        -------
        list[list[float]]
            One inner list per simulated path.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
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
        barrier_type: BarrierType,
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
        barrier_type : BarrierType
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
        ...     BarrierType,
        ...     MertonModel,
        ... )
        >>> model = MertonModel.new_with_dynamics(
        ...     100.0,
        ...     0.25,
        ...     80.0,
        ...     0.05,
        ...     0.0,
        ...     BarrierType.first_passage(0.02),
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
    def barrier_type(self) -> BarrierType:
        """
        Barrier monitoring convention.

        Returns
        -------
        BarrierType
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

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the model parameters as a single-row pandas ``DataFrame``.

        Returns
        -------
        pandas.DataFrame
            One row with the canonical model fields as columns.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

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

        Notes
        -----
        This method does not raise; out-of-domain or non-finite inputs yield ``NaN`` or ``inf`` rather than an exception.

        Examples
        --------
        >>> from finstack_quant.models.credit import MertonModel
        >>> 0.0 <= MertonModel(100.0, 0.25, 80.0, 0.04).default_probability(1.0) <= 1.0
        True

        """
        ...

    def default_probabilities(self, horizons: list[float]) -> pd.Series:
        """
        Risk-neutral cumulative default probabilities over several horizons.

        Parameters
        ----------
        horizons : list[float]
            Horizons in years; each must be finite and strictly positive.

        Returns
        -------
        pandas.Series
            Float series named ``default_probability`` indexed by the horizon
            labels (``str(horizon)``), in input order.

        Raises
        ------
        ValueError
            If any horizon is non-finite or non-positive.
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
            If ``asset_drift`` is not finite, or the model uses CreditGrades
            dynamics, which are driftless by construction.

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

    def try_implied_equity(self, horizon: float) -> tuple[float, float]:
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
        day_count: str = "act_365f",
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
        day_count : str, optional
            Day-count convention the curve uses to turn dates into year
            fractions. Pass the convention of the discount curve the hazard
            curve will be paired with. Default ``"act_365f"``, which matches
            the year-fraction axis the model's horizons use.

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
        antithetic: bool = False,
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
            RNG seed for reproducible draws.
        antithetic : bool, optional
            When ``True``, use antithetic variates for variance reduction.
            Default ``False``.

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

class DynamicRecoverySpec:
    """
    Recovery specification with optional notional dependence.

    Examples
    --------
    >>> from finstack_quant.models.credit import DynamicRecoverySpec
    >>> spec = DynamicRecoverySpec.constant(0.4)
    >>> spec.recovery_at_notional(100.0)
    0.4

    """

    @staticmethod
    def constant(recovery: float) -> DynamicRecoverySpec:
        """
        Create a constant recovery-rate specification.

        Parameters
        ----------
        recovery : float
            Recovery rate as a decimal in ``[0, 1]``.

        Returns
        -------
        DynamicRecoverySpec
            Constant recovery spec.

        Raises
        ------
        ValueError
            If ``recovery`` is out of range or non-finite.

        Examples
        --------
        >>> from finstack_quant.models.credit import DynamicRecoverySpec
        >>> spec = DynamicRecoverySpec.constant(0.4)
        >>> spec.recovery_at_notional(100.0)
        0.4

        """
        ...

    @staticmethod
    def inverse_linear(base_recovery: float, base_notional: float) -> DynamicRecoverySpec:
        """
        Recovery inversely proportional to notional: ``R(N) = R_0 * N_0 / N``.

        Parameters
        ----------
        base_recovery : float
            Reference recovery ``R_0`` as a decimal in ``[0, 1]``.
        base_notional : float
            Reference notional ``N_0`` (strictly positive), in the instrument's currency.

        Returns
        -------
        DynamicRecoverySpec
            Specification with ``kind == "inverse_linear"``.

        Raises
        ------
        ValueError
            On non-finite or out-of-range inputs.

        Examples
        --------
        >>> from finstack_quant.models.credit import DynamicRecoverySpec
        >>> DynamicRecoverySpec.inverse_linear(0.4, 100.0).recovery_at_notional(200.0)
        0.2
        """
        ...

    @staticmethod
    def inverse_power(base_recovery: float, base_notional: float, exponent: float) -> DynamicRecoverySpec:
        """
        Recovery decaying as a power of notional: ``R(N) = R_0 * (N_0 / N)**exponent``.

        Parameters
        ----------
        base_recovery : float
            Reference recovery ``R_0`` as a decimal in ``[0, 1]``.
        base_notional : float
            Reference notional ``N_0`` (strictly positive), in the instrument's currency.
        exponent : float
            Non-negative decay exponent (``1.0`` reproduces ``inverse_linear``).

        Returns
        -------
        DynamicRecoverySpec
            Specification with ``kind == "inverse_power"``.

        Raises
        ------
        ValueError
            On non-finite or out-of-range inputs.

        Examples
        --------
        >>> from finstack_quant.models.credit import DynamicRecoverySpec
        >>> DynamicRecoverySpec.inverse_power(0.4, 100.0, 1.0).kind
        'inverse_power'
        """
        ...

    @staticmethod
    def floored_inverse(base_recovery: float, base_notional: float, floor: float) -> DynamicRecoverySpec:
        """
        Inverse-linear recovery floored at a minimum: ``max(R_0 * N_0 / N, floor)``.

        Parameters
        ----------
        base_recovery : float
            Reference recovery ``R_0`` as a decimal in ``[0, 1]``.
        base_notional : float
            Reference notional ``N_0`` (strictly positive), in the instrument's currency.
        floor : float
            Minimum recovery as a decimal in ``[0, base_recovery]``.

        Returns
        -------
        DynamicRecoverySpec
            Specification with ``kind == "floored_inverse"``.

        Raises
        ------
        ValueError
            On non-finite or out-of-range inputs.

        Examples
        --------
        >>> from finstack_quant.models.credit import DynamicRecoverySpec
        >>> DynamicRecoverySpec.floored_inverse(0.4, 100.0, 0.3).recovery_at_notional(400.0)
        0.3
        """
        ...

    @staticmethod
    def linear_decline(base_recovery: float, base_notional: float, slope: float, floor: float) -> DynamicRecoverySpec:
        """
        Recovery declining linearly with accretion: ``max(R_0 - slope * (N - N_0) / N_0, floor)``.

        Parameters
        ----------
        base_recovery : float
            Reference recovery ``R_0`` as a decimal in ``[0, 1]``.
        base_notional : float
            Reference notional ``N_0`` (strictly positive), in the instrument's currency.
        slope : float
            Non-negative recovery decline per unit of relative accretion.
        floor : float
            Minimum recovery as a decimal in ``[0, base_recovery]``.

        Returns
        -------
        DynamicRecoverySpec
            Specification with ``kind == "linear_decline"``.

        Raises
        ------
        ValueError
            On non-finite or out-of-range inputs.

        Examples
        --------
        >>> from finstack_quant.models.credit import DynamicRecoverySpec
        >>> DynamicRecoverySpec.linear_decline(0.4, 100.0, 0.1, 0.2).kind
        'linear_decline'
        """
        ...

    @property
    def kind(self) -> str:
        """
        Canonical name of the active recovery model: ``"constant"``, ``"inverse_linear"``, ``"inverse_power"``, ``"floored_inverse"`` or ``"linear_decline"``.

        Returns
        -------
        str
            Canonical name of the active recovery model: ``"constant"``, ``"inverse_linear"``, ``"inverse_power"``, ``"floored_inverse"`` or ``"linear_decline"``.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @staticmethod
    def from_json(json: str) -> DynamicRecoverySpec:
        """
        Deserialize a recovery specification from JSON.

        Parameters
        ----------
        json : str
            Canonical JSON payload.

        Returns
        -------
        DynamicRecoverySpec
            Parsed specification.

        Raises
        ------
        ValueError
            If JSON is invalid.

        Examples
        --------
        >>> from finstack_quant.models.credit import DynamicRecoverySpec
        >>> spec = DynamicRecoverySpec.constant(0.4)
        >>> DynamicRecoverySpec.from_json(spec.to_json()).recovery_at_notional(100.0)
        0.4

        """
        ...

    def to_json(self) -> str:
        """
        Serialize this recovery specification to canonical JSON.

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

    def recovery_at_notional(self, notional: float) -> float:
        """
        Return recovery rate for the supplied notional.

        Parameters
        ----------
        notional : float
            Outstanding notional (positive, finite).

        Returns
        -------
        float
            Recovery rate as a decimal.

        Notes
        -----
        This method does not raise; out-of-domain or non-finite inputs yield ``NaN`` or ``inf`` rather than an exception.
        """
        ...

    @property
    def base_recovery(self) -> float:
        """
        Base (reference) recovery rate ``R_0``.

        Returns
        -------
        float
            Recovery rate as a decimal in ``[0, 1]``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def base_notional(self) -> float:
        """
        Base (reference) notional ``N_0`` the recovery mapping is anchored to.

        Returns
        -------
        float
            Reference notional (positive).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def model(self) -> Any:
        """
        Notional-to-recovery mapping, in canonical JSON form.

        Returns
        -------
        Any
            ``"constant"`` / ``"inverse_linear"`` for the parameterless models,
            or a single-key mapping (``inverse_power``, ``floored_inverse``,
            ``linear_decline``) carrying that model's parameters.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the recovery specification as a single-row pandas ``DataFrame``.

        Returns
        -------
        pandas.DataFrame
            One row with the canonical specification fields as columns.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

class EndogenousHazardSpec:
    """
    Hazard-rate model driven by leverage or PIK-accreted notional.

    Examples
    --------
    >>> from finstack_quant.models.credit import EndogenousHazardSpec
    >>> spec = EndogenousHazardSpec.power_law(0.02, 4.0, 2.0)
    >>> (spec.hazard_at_leverage(4.0), spec.hazard_at_leverage(8.0))
    (0.02, 0.08)

    """

    @staticmethod
    def power_law(
        base_hazard: float,
        base_leverage: float,
        exponent: float,
    ) -> EndogenousHazardSpec:
        """
        Create a power-law hazard model around a base leverage point.

        Parameters
        ----------
        base_hazard : float
            Hazard rate at ``base_leverage`` (decimal annualized intensity).
        base_leverage : float
            Reference leverage ratio (e.g. debt / EBITDA).
        exponent : float
            Power-law sensitivity of hazard to leverage.

        Returns
        -------
        EndogenousHazardSpec
            Endogenous hazard specification.

        Raises
        ------
        ValueError
            If parameters are non-finite or violate constraints.

        Examples
        --------
        >>> from finstack_quant.models.credit import EndogenousHazardSpec
        >>> spec = EndogenousHazardSpec.power_law(0.02, 4.0, 2.0)
        >>> spec.hazard_at_leverage(8.0)
        0.08

        """
        ...

    @staticmethod
    def exponential(base_hazard: float, base_leverage: float, sensitivity: float) -> EndogenousHazardSpec:
        """
        Exponential feedback: ``lambda(L) = lambda_0 * exp(sensitivity * (L - L_0))``.

        Parameters
        ----------
        base_hazard : float
            Reference annualized hazard rate ``lambda_0`` (non-negative decimal).
        base_leverage : float
            Reference leverage ``L_0`` (strictly positive ratio).
        sensitivity : float
            Non-negative exponential sensitivity per unit of leverage.

        Returns
        -------
        EndogenousHazardSpec
            Specification with ``kind == "exponential"``.

        Raises
        ------
        ValueError
            On non-finite or out-of-range inputs.

        Examples
        --------
        >>> from finstack_quant.models.credit import EndogenousHazardSpec
        >>> EndogenousHazardSpec.exponential(0.02, 4.0, 0.5).hazard_at_leverage(4.0)
        0.02
        """
        ...

    @staticmethod
    def tabular(leverage_points: list[float], hazard_points: list[float]) -> EndogenousHazardSpec:
        """
        Piecewise-linear tabular feedback through ``(leverage, hazard)`` points.

        Parameters
        ----------
        leverage_points : list[float]
            Strictly increasing leverage ratios (at least two).
        hazard_points : list[float]
            Non-negative annualized hazard rates, one per leverage point; the
            base leverage and base hazard are taken from the first point.

        Returns
        -------
        EndogenousHazardSpec
            Specification with ``kind == "tabular"``.

        Raises
        ------
        ValueError
            If the lists differ in length, have fewer than two points, are not
            strictly increasing in leverage, or contain non-finite or negative
            values.

        Examples
        --------
        >>> from finstack_quant.models.credit import EndogenousHazardSpec
        >>> EndogenousHazardSpec.tabular([2.0, 6.0], [0.01, 0.05]).hazard_at_leverage(4.0)
        0.03
        """
        ...

    @property
    def kind(self) -> str:
        """
        Canonical name of the active leverage-to-hazard mapping: ``"power_law"``, ``"exponential"`` or ``"tabular"``.

        Returns
        -------
        str
            Canonical name of the active leverage-to-hazard mapping: ``"power_law"``, ``"exponential"`` or ``"tabular"``.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @staticmethod
    def from_json(json: str) -> EndogenousHazardSpec:
        """
        Deserialize an endogenous hazard specification from JSON.

        Parameters
        ----------
        json : str
            Canonical JSON payload.

        Returns
        -------
        EndogenousHazardSpec
            Parsed specification.

        Raises
        ------
        ValueError
            If JSON is invalid.

        Examples
        --------
        >>> from finstack_quant.models.credit import EndogenousHazardSpec
        >>> spec = EndogenousHazardSpec.power_law(0.02, 4.0, 2.0)
        >>> EndogenousHazardSpec.from_json(spec.to_json()).hazard_at_leverage(8.0)
        0.08

        """
        ...

    def to_json(self) -> str:
        """
        Serialize this hazard specification to canonical JSON.

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

    def hazard_at_leverage(self, leverage: float) -> float:
        """
        Return hazard rate at the supplied leverage.

        Parameters
        ----------
        leverage : float
            Leverage ratio (positive, finite).

        Returns
        -------
        float
            Annualized hazard rate as a decimal.

        Notes
        -----
        This method does not raise; out-of-domain or non-finite inputs yield ``NaN`` or ``inf`` rather than an exception.
        """
        ...

    def hazard_after_pik_accrual(
        self,
        accreted_notional: float,
        asset_value: float,
    ) -> float:
        """
        Return hazard rate after PIK accrual changes leverage.

        Parameters
        ----------
        accreted_notional : float
            PIK-accreted notional outstanding.
        asset_value : float
            Firm asset value used in the leverage mapping.

        Returns
        -------
        float
            Updated annualized hazard rate as a decimal.

        Notes
        -----
        This method does not raise; out-of-domain or non-finite inputs yield ``NaN`` or ``inf`` rather than an exception.
        """
        ...

    @property
    def base_hazard_rate(self) -> float:
        """
        Base (reference) hazard rate ``lambda_0``.

        Returns
        -------
        float
            Annualized hazard rate as a decimal.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def base_leverage(self) -> float:
        """
        Base (reference) leverage level ``L_0`` the hazard mapping is anchored to.

        Returns
        -------
        float
            Reference leverage ratio (positive).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def leverage_hazard_map(self) -> Any:
        """
        Leverage-to-hazard mapping, in canonical JSON form.

        Returns
        -------
        Any
            A single-key mapping (``power_law``, ``exponential``, ``tabular``)
            carrying that model's parameters.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the hazard specification as a single-row pandas ``DataFrame``.

        Returns
        -------
        pandas.DataFrame
            One row with the canonical specification fields as columns.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

class CreditState:
    """
    Point-in-time credit state for toggle and path-dependent credit logic.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.models.credit import CreditState
    >>> state = CreditState(hazard_rate=0.02, leverage=4.0)
    >>> (json.loads(state.to_json())["hazard_rate"], json.loads(state.to_json())["leverage"])
    (0.02, 4.0)

    """

    def __init__(
        self,
        hazard_rate: float = 0.0,
        distance_to_default: float | None = None,
        leverage: float = 0.0,
        accreted_notional: float = 0.0,
        coupon_due: float = 0.0,
        asset_value: float | None = None,
    ) -> None:
        """
        Create a credit-state snapshot.

        Parameters
        ----------
        hazard_rate : float, optional
            Instantaneous hazard rate as a decimal. Default ``0.0``.
        distance_to_default : float, optional
            Structural distance-to-default if available.
        leverage : float, optional
            Leverage ratio for endogenous hazard models. Default ``0.0``.
        accreted_notional : float, optional
            PIK-accreted notional. Default ``0.0``.
        coupon_due : float, optional
            Coupon amount due at the decision date. Default ``0.0``.
        asset_value : float, optional
            Firm asset value for structural/toggle models.

        Notes
        -----
        Construction does not raise; arguments are stored as supplied.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this state to canonical JSON.

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

    @staticmethod
    def from_json(json: str) -> CreditState:
        """
        Deserialize a `CreditState` from JSON produced by :meth:`to_json`.

        Completes the wire round-trip, which is also what makes this type
        picklable.

        Parameters
        ----------
        json : str
            Canonical JSON produced by :meth:`to_json`.

        Returns
        -------
        CreditState
            Validated state reconstructed from the canonical JSON payload.

        Examples
        --------
        >>> from finstack_quant.models.credit import CreditState
        >>> state = CreditState(hazard_rate=0.02, leverage=4.0)
        >>> restored = CreditState.from_json(state.to_json())
        >>> (restored.hazard_rate, restored.leverage)
        (0.02, 4.0)

        Raises
        ------
        ValueError
            If ``json`` is malformed or does not match the serialized schema.
        """
        ...

    @property
    def hazard_rate(self) -> float:
        """
        Instantaneous default intensity at this observation.

        Returns
        -------
        float
            Annualized hazard rate as a decimal.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def distance_to_default(self) -> float | None:
        """
        Structural distance-to-default.

        Returns
        -------
        float or None
            Distance in standard deviations, or ``None`` when unavailable.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def leverage(self) -> float:
        """
        Leverage ratio (debt / assets).

        Returns
        -------
        float
            Leverage ratio.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def accreted_notional(self) -> float:
        """
        Accreted (PIK-augmented) notional outstanding.

        Returns
        -------
        float
            Outstanding notional.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def coupon_due(self) -> float:
        """
        Cash coupon amount due at this decision date.

        Returns
        -------
        float
            Coupon amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def asset_value(self) -> float | None:
        """
        Fair value of the firm's assets.

        Returns
        -------
        float or None
            Asset value, or ``None`` when unavailable.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the observed credit state as a single-row pandas ``DataFrame``.

        Returns
        -------
        pandas.DataFrame
            One row with the canonical state fields as columns.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

class ToggleExerciseModel:
    """
    Exercise model for PIK/cash toggle and similar embedded options.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.models.credit import ToggleExerciseModel
    >>> model = ToggleExerciseModel.threshold("leverage", 5.0, "above")
    >>> json.loads(model.to_json())["threshold"]["state_variable"]
    'leverage'

    """

    @staticmethod
    def threshold(
        variable: str,
        threshold: float,
        direction: str,
    ) -> ToggleExerciseModel:
        """
        Create a threshold exercise rule on a credit-state variable.

        Parameters
        ----------
        variable : str
            State variable name (e.g. ``"leverage"``, ``"distance_to_default"``).
        threshold : float
            Threshold value triggering exercise.
        direction : str
            ``"above"`` or ``"below"`` — exercise when the variable is above or
            below the threshold.

        Returns
        -------
        ToggleExerciseModel
            Threshold exercise specification.

        Raises
        ------
        ValueError
            If ``variable`` or ``direction`` is not recognized.

        Examples
        --------
        >>> import json
        >>> from finstack_quant.models.credit import ToggleExerciseModel
        >>> model = ToggleExerciseModel.threshold("leverage", 5.0, "above")
        >>> json.loads(model.to_json())["threshold"]["direction"]
        'above'

        """
        ...

    @staticmethod
    def optimal(
        nested_paths: int,
        equity_discount_rate: float,
        asset_vol: float,
        risk_free_rate: float,
        horizon: float,
    ) -> ToggleExerciseModel:
        """
        Create an optimal exercise model from nested-path parameters.

        Parameters
        ----------
        nested_paths : int
            Number of nested Monte Carlo paths for the inner optimization.
        equity_discount_rate : float
            Equity-holder discount rate as a decimal.
        asset_vol : float
            Asset volatility as a decimal.
        risk_free_rate : float
            Risk-free rate as a decimal.
        horizon : float
            Exercise horizon in years.

        Returns
        -------
        ToggleExerciseModel
            Optimal exercise specification.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> import json
        >>> from finstack_quant.models.credit import ToggleExerciseModel
        >>> model = ToggleExerciseModel.optimal(100, 0.12, 0.3, 0.05, 1.0)
        >>> json.loads(model.to_json())["optimal_exercise"]["nested_paths"]
        100
        """
        ...

    @staticmethod
    def stochastic(variable: str, intercept: float, sensitivity: float) -> ToggleExerciseModel:
        """
        Stochastic rule: PIK with probability ``logistic(intercept + sensitivity * x)``.

        Parameters
        ----------
        variable : str
            Credit-state variable ``x`` observed: ``"hazard_rate"``,
            ``"distance_to_default"`` or ``"leverage"``.
        intercept : float
            Logit intercept.
        sensitivity : float
            Logit slope per unit of the variable.

        Returns
        -------
        ToggleExerciseModel
            Rule with ``kind == "stochastic"``.

        Raises
        ------
        ValueError
            For an unknown variable string.

        Examples
        --------
        >>> from finstack_quant.models.credit import ToggleExerciseModel
        >>> ToggleExerciseModel.stochastic("leverage", -2.0, 0.5).kind
        'stochastic'
        """
        ...

    def should_pik_with_uniform(self, state: CreditState, u: float) -> bool:
        """
        Whether the rule elects PIK for ``state`` given one uniform draw.

        Parameters
        ----------
        state : CreditState
            Observed credit state at the decision date.
        u : float
            Uniform draw in ``[0, 1)``; ignored by threshold rules, used as the
            Bernoulli draw by stochastic rules. Optimal-exercise rules need
            nested simulation and return ``False`` here.

        Returns
        -------
        bool
            ``True`` when the coupon is paid in kind.

        Notes
        -----
        This method does not raise.
        """
        ...

    @staticmethod
    def from_json(json: str) -> ToggleExerciseModel:
        """
        Deserialize a toggle exercise model from JSON.

        Parameters
        ----------
        json : str
            Canonical JSON payload.

        Returns
        -------
        ToggleExerciseModel
            Parsed model.

        Raises
        ------
        ValueError
            If JSON is invalid.

        Examples
        --------
        >>> import json
        >>> from finstack_quant.models.credit import ToggleExerciseModel
        >>> model = ToggleExerciseModel.threshold("leverage", 5.0, "above")
        >>> restored = ToggleExerciseModel.from_json(model.to_json())
        >>> json.loads(restored.to_json())["threshold"]["threshold"]
        5.0

        """
        ...

    def to_json(self) -> str:
        """
        Serialize this exercise model to canonical JSON.

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
    def kind(self) -> str:
        """
        Which exercise rule this model carries.

        Returns
        -------
        str
            One of ``"threshold"``, ``"stochastic"`` or ``"optimal_exercise"`` —
            the canonical serde tag, so it also names the single key in the
            ``to_json`` payload.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def params(self) -> Any:
        """
        Parameters of the active rule, in canonical JSON form.

        Returns
        -------
        Any
            Mapping whose keys depend on ``kind``: ``state_variable`` /
            ``threshold`` / ``direction`` for a threshold rule,
            ``state_variable`` / ``intercept`` / ``sensitivity`` for a
            stochastic one, and the nested-Monte-Carlo settings for optimal
            exercise.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
