"""
Loss-given-default: seniority Beta recovery, workout LGD, downturn
adjustments, EAD.

Collateral types accepted as strings: ``cash``, ``securities``, ``receivables``, ``inventory``, ``equipment``, ``real_estate``, ``intellectual_property``, ``other``.

Examples
--------
>>> from finstack_quant.models.credit import lgd
>>> lgd.EadCalculator(60.0, 40.0, 0.5).ead
80.0

"""

from __future__ import annotations

from typing import Any

import pandas

__all__ = [
    "BetaRecovery",
    "CollateralPiece",
    "DownturnLgd",
    "EadCalculator",
    "WorkoutCosts",
    "WorkoutLgd",
    "WorkoutLgdBuilder",
    "WorkoutLgdResult",
    "seniority_recovery_stats",
    "workout_lgd",
]

class BetaRecovery:
    """
    Beta-distributed recovery rate parameterised by mean and standard deviation.

    Examples
    --------
    >>> from finstack_quant.models.credit import lgd
    >>> recovery = lgd.BetaRecovery(0.4, 0.2)
    >>> (recovery.mean, round(recovery.mean_lgd, 2))
    (0.4, 0.6)

    """

    def __init__(self, mean: float, std_dev: float) -> None:
        """
        Build a Beta recovery distribution from its first two moments.

        Parameters
        ----------
        mean : float
            Mean recovery rate as a decimal in ``(0, 1)``.
        std_dev : float
            Standard deviation; must satisfy ``std_dev**2 < mean * (1 - mean)``.

        Raises
        ------
        ValueError
            If the moments cannot parameterise a Beta distribution.
        """
        ...

    @property
    def mean(self) -> float:
        """
        Mean recovery rate (decimal).

        Returns
        -------
        float
            Mean recovery rate (decimal).

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def std_dev(self) -> float:
        """
        Standard deviation of the recovery rate.

        Returns
        -------
        float
            Standard deviation of the recovery rate.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def alpha(self) -> float:
        """
        Beta shape parameter alpha.

        Returns
        -------
        float
            Beta shape parameter alpha.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def beta_param(self) -> float:
        """
        Beta shape parameter beta.

        Returns
        -------
        float
            Beta shape parameter beta.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def variance(self) -> float:
        """
        Variance of the recovery rate (``std_dev**2``).

        Returns
        -------
        float
            Variance of the recovery rate (``std_dev**2``).

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def mode(self) -> float | None:
        """
        Mode of the distribution, or ``None`` when a shape parameter is <= 1.

        Returns
        -------
        float | None
            Mode of the distribution, or ``None`` when a shape parameter is <= 1.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def mean_lgd(self) -> float:
        """
        Expected loss given default, ``1 - mean``.

        Returns
        -------
        float
            Expected loss given default, ``1 - mean``.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    def quantile(self, p: float) -> float:
        """
        Recovery rate at probability ``p``.

        Parameters
        ----------
        p : float
            Probability in ``(0, 1)``.

        Returns
        -------
        float
            Recovery rate as a decimal.

        Raises
        ------
        ValueError
            If ``p`` is non-finite or outside ``(0, 1)``.
        """
        ...

    def sample_seeded(self, n_samples: int, seed: int) -> list[float]:
        """
        Draw recovery rates with a deterministic PCG64 RNG.

        Parameters
        ----------
        n_samples : int
            Number of draws.
        seed : int
            RNG seed; the same seed yields the same sequence.

        Returns
        -------
        list[float]
            ``n_samples`` recovery rates as decimals.

        Raises
        ------
        ValueError
            If sampling fails.
        """
        ...
    @staticmethod
    def from_json(json: str) -> BetaRecovery:
        """
        Deserialize a ``BetaRecovery`` from its canonical JSON form.

        Parameters
        ----------
        json : str
            JSON text produced by :meth:`to_json` (strict serde; unknown or
            invalid fields are rejected).

        Returns
        -------
        BetaRecovery
            Reconstructed value.

        Raises
        ------
        ValueError
            If the payload is not valid ``BetaRecovery`` JSON.

        Examples
        --------
        >>> from finstack_quant.models.credit import lgd
        >>> value = lgd.BetaRecovery(0.4, 0.2)
        >>> lgd.BetaRecovery.from_json(value.to_json()).to_json() == value.to_json()
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
    def to_dataframe(self) -> pandas.DataFrame:
        """
        Single-row frame with ``mean``, ``std_dev``, ``alpha``, ``beta_param``.

        Returns
        -------
        pandas.DataFrame
            Single-row frame with ``mean``, ``std_dev``, ``alpha``, ``beta_param``.

        Raises
        ------
        ValueError
            If the value cannot be serialized into a pandas object.
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

class CollateralPiece:
    """
    One collateral piece in a workout waterfall.

    Examples
    --------
    >>> from finstack_quant.models.credit import lgd
    >>> lgd.CollateralPiece("real_estate", 80.0, 0.3).liquidation_value
    56.0

    """

    def __init__(self, collateral_type: str, book_value: float, haircut: float) -> None:
        """
        Build a collateral piece.

        Parameters
        ----------
        collateral_type : str
            One of ``cash``, ``securities``, ``receivables``, ``inventory``, ``equipment``, ``real_estate``, ``intellectual_property``, ``other``.
        book_value : float
            Pre-haircut book value (non-negative), in the exposure's currency.
        haircut : float
            Liquidation haircut as a decimal in ``[0, 1]``.

        Raises
        ------
        ValueError
            For an unknown type, a negative value, or a haircut outside
            ``[0, 1]``.
        """
        ...

    @property
    def collateral_type(self) -> str:
        """
        Canonical collateral-type label.

        Returns
        -------
        str
            Canonical collateral-type label.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def book_value(self) -> float:
        """
        Pre-haircut book value.

        Returns
        -------
        float
            Pre-haircut book value.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def haircut(self) -> float:
        """
        Liquidation haircut as a decimal in ``[0, 1]``.

        Returns
        -------
        float
            Liquidation haircut as a decimal in ``[0, 1]``.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def liquidation_value(self) -> float:
        """
        ``book_value * (1 - haircut)``.

        Returns
        -------
        float
            ``book_value * (1 - haircut)``.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @staticmethod
    def from_json(json: str) -> CollateralPiece:
        """
        Deserialize a ``CollateralPiece`` from its canonical JSON form.

        Parameters
        ----------
        json : str
            JSON text produced by :meth:`to_json` (strict serde; unknown or
            invalid fields are rejected).

        Returns
        -------
        CollateralPiece
            Reconstructed value.

        Raises
        ------
        ValueError
            If the payload is not valid ``CollateralPiece`` JSON.

        Examples
        --------
        >>> from finstack_quant.models.credit import lgd
        >>> value = lgd.CollateralPiece("cash", 10.0, 0.0)
        >>> lgd.CollateralPiece.from_json(value.to_json()).to_json() == value.to_json()
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

class WorkoutCosts:
    """
    Direct and indirect workout cost rates as decimal fractions of EAD.

    Examples
    --------
    >>> from finstack_quant.models.credit import lgd
    >>> lgd.WorkoutCosts(0.05, 0.03).total_rate
    0.08

    """

    def __init__(self, direct_cost_rate: float, indirect_cost_rate: float) -> None:
        """
        Build a cost specification.

        Parameters
        ----------
        direct_cost_rate : float
            Direct (legal, administrative) costs as a decimal fraction of
            EAD (>= 0).
        indirect_cost_rate : float
            Indirect (opportunity) costs as a decimal fraction of EAD (>= 0).

        Raises
        ------
        ValueError
            For negative or non-finite rates.
        """
        ...

    @staticmethod
    def zero() -> WorkoutCosts:
        """
        Zero workout costs.

        Returns
        -------
        WorkoutCosts
            Both rates ``0.0``.

        Notes
        -----
        This constructor does not raise.

        Examples
        --------
        >>> from finstack_quant.models.credit import lgd
        >>> lgd.WorkoutCosts.zero().total_rate
        0.0
        """
        ...

    @staticmethod
    def standard() -> WorkoutCosts:
        """
        Registry-default workout costs.

        Returns
        -------
        WorkoutCosts
            Default direct / indirect rates from the embedded registry.

        Raises
        ------
        ValueError
            If the embedded credit registry is invalid.

        Examples
        --------
        >>> from finstack_quant.models.credit import lgd
        >>> lgd.WorkoutCosts.standard().total_rate > 0.0
        True
        """
        ...

    @property
    def direct_cost_rate(self) -> float:
        """
        Direct cost rate (decimal fraction of EAD).

        Returns
        -------
        float
            Direct cost rate (decimal fraction of EAD).

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def indirect_cost_rate(self) -> float:
        """
        Indirect cost rate (decimal fraction of EAD).

        Returns
        -------
        float
            Indirect cost rate (decimal fraction of EAD).

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def total_rate(self) -> float:
        """
        ``direct_cost_rate + indirect_cost_rate``.

        Returns
        -------
        float
            ``direct_cost_rate + indirect_cost_rate``.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @staticmethod
    def from_json(json: str) -> WorkoutCosts:
        """
        Deserialize a ``WorkoutCosts`` from its canonical JSON form.

        Parameters
        ----------
        json : str
            JSON text produced by :meth:`to_json` (strict serde; unknown or
            invalid fields are rejected).

        Returns
        -------
        WorkoutCosts
            Reconstructed value.

        Raises
        ------
        ValueError
            If the payload is not valid ``WorkoutCosts`` JSON.

        Examples
        --------
        >>> from finstack_quant.models.credit import lgd
        >>> value = lgd.WorkoutCosts(0.05, 0.03)
        >>> lgd.WorkoutCosts.from_json(value.to_json()).to_json() == value.to_json()
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

class WorkoutLgdResult:
    """
    Net recovery, LGD, and recovery rate from a workout evaluation.

    Examples
    --------
    >>> from finstack_quant.models.credit import lgd
    >>> result = lgd.workout_lgd(100.0, [("real_estate", 80.0, 0.3)], 0.05, 0.03, 2.0, 0.05)
    >>> round(result.lgd + result.recovery_rate, 12)
    1.0

    """

    @property
    def net_recovery(self) -> float:
        """
        Post-cost, post-discount recovery amount (floored at zero), in EAD units.

        Returns
        -------
        float
            Post-cost, post-discount recovery amount (floored at zero), in EAD units.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def lgd(self) -> float:
        """
        Loss given default as a decimal in ``[0, 1]``.

        Returns
        -------
        float
            Loss given default as a decimal in ``[0, 1]``.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def recovery_rate(self) -> float:
        """
        Recovery rate ``1 - lgd`` as a decimal in ``[0, 1]``.

        Returns
        -------
        float
            Recovery rate ``1 - lgd`` as a decimal in ``[0, 1]``.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @staticmethod
    def from_json(json: str) -> WorkoutLgdResult:
        """
        Deserialize a ``WorkoutLgdResult`` from its canonical JSON form.

        Parameters
        ----------
        json : str
            JSON text produced by :meth:`to_json` (strict serde; unknown or
            invalid fields are rejected).

        Returns
        -------
        WorkoutLgdResult
            Reconstructed value.

        Raises
        ------
        ValueError
            If the payload is not valid ``WorkoutLgdResult`` JSON.

        Examples
        --------
        >>> from finstack_quant.models.credit import lgd
        >>> value = lgd.workout_lgd(100.0, [("cash", 50.0, 0.0)], 0.0, 0.0, 1.0, 0.0)
        >>> lgd.WorkoutLgdResult.from_json(value.to_json()) == value
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
    def to_dataframe(self) -> pandas.DataFrame:
        """
        Single-row frame with ``net_recovery``, ``lgd``, ``recovery_rate``.

        Returns
        -------
        pandas.DataFrame
            Single-row frame with ``net_recovery``, ``lgd``, ``recovery_rate``.

        Raises
        ------
        ValueError
            If the value cannot be serialized into a pandas object.
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

class WorkoutLgd:
    """
    Workout (collateral-waterfall) LGD model built via ``WorkoutLgd.builder()``.

    ``net_recovery = (min(sum liquidation values, EAD) - costs * EAD) * DF``
    and ``lgd = 1 - clamp(net_recovery / EAD, 0, 1)`` where ``DF``
    discounts over the workout horizon (Basel workout-LGD methodology).

    Examples
    --------
    >>> from finstack_quant.models.credit import lgd
    >>> model = (
    ...     lgd.WorkoutLgd
    ...     .builder()
    ...     .collateral(lgd.CollateralPiece("cash", 50.0, 0.0))
    ...     .workout_years(0.0)
    ...     .discount_rate(0.0)
    ...     .costs(lgd.WorkoutCosts.zero())
    ...     .build()
    ... )
    >>> model.lgd(100.0)
    0.5

    """

    @staticmethod
    def builder() -> WorkoutLgdBuilder:
        """
        Start a fluent builder (the only construction entry point).

        Returns
        -------
        WorkoutLgdBuilder
            Empty builder; unset fields fall back to registry defaults.

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.models.credit import lgd
        >>> isinstance(lgd.WorkoutLgd.builder().build(), lgd.WorkoutLgd)
        True
        """
        ...

    def evaluate(self, ead: float) -> WorkoutLgdResult:
        """
        Evaluate net recovery, LGD, and recovery rate at ``ead``.

        Parameters
        ----------
        ead : float
            Exposure at default (> 0), in the collateral's currency.

        Returns
        -------
        WorkoutLgdResult
            Consistent ``net_recovery`` / ``lgd`` / ``recovery_rate``.

        Raises
        ------
        ValueError
            If ``ead`` is non-finite or non-positive.
        """
        ...

    def lgd(self, ead: float) -> float:
        """
        Loss given default at ``ead``.

        Parameters
        ----------
        ead : float
            Exposure at default (> 0).

        Returns
        -------
        float
            LGD as a decimal in ``[0, 1]``.

        Raises
        ------
        ValueError
            If ``ead`` is non-finite or non-positive.
        """
        ...

    def net_recovery(self, ead: float) -> float:
        """
        Net recovery amount at ``ead``.

        Parameters
        ----------
        ead : float
            Exposure at default (> 0).

        Returns
        -------
        float
            Post-cost, discounted recovery amount (floored at zero).

        Raises
        ------
        ValueError
            If ``ead`` is non-finite or non-positive.
        """
        ...

    def recovery_rate(self, ead: float) -> float:
        """
        Recovery rate ``1 - lgd`` at ``ead``.

        Parameters
        ----------
        ead : float
            Exposure at default (> 0).

        Returns
        -------
        float
            Recovery rate as a decimal in ``[0, 1]``.

        Raises
        ------
        ValueError
            If ``ead`` is non-finite or non-positive.
        """
        ...

    @property
    def collateral(self) -> list[CollateralPiece]:
        """
        Ordered collateral waterfall, highest priority first.

        Returns
        -------
        list[lgd.CollateralPiece]
            Ordered collateral waterfall, highest priority first.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def workout_years(self) -> float:
        """
        Expected workout duration in years.

        Returns
        -------
        float
            Expected workout duration in years.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def discount_rate(self) -> float:
        """
        Annual decimal discount rate over the workout horizon.

        Returns
        -------
        float
            Annual decimal discount rate over the workout horizon.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def costs(self) -> WorkoutCosts:
        """
        Direct and indirect cost rates.

        Returns
        -------
        lgd.WorkoutCosts
            Direct and indirect cost rates.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    def to_dataframe(self) -> pandas.DataFrame:
        """
        Frame of the collateral waterfall with ``collateral_type``, ``book_value``, ``haircut``; one row per piece.

        Returns
        -------
        pandas.DataFrame
            Frame of the collateral waterfall with ``collateral_type``, ``book_value``, ``haircut``; one row per piece.

        Raises
        ------
        ValueError
            If the value cannot be serialized into a pandas object.
        """
        ...
    @staticmethod
    def from_json(json: str) -> WorkoutLgd:
        """
        Deserialize a ``WorkoutLgd`` from its canonical JSON form.

        Parameters
        ----------
        json : str
            JSON text produced by :meth:`to_json` (strict serde; unknown or
            invalid fields are rejected).

        Returns
        -------
        WorkoutLgd
            Reconstructed value.

        Raises
        ------
        ValueError
            If the payload is not valid ``WorkoutLgd`` JSON.

        Examples
        --------
        >>> from finstack_quant.models.credit import lgd
        >>> value = lgd.WorkoutLgd.builder().build()
        >>> lgd.WorkoutLgd.from_json(value.to_json()).to_json() == value.to_json()
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

class WorkoutLgdBuilder:
    """
    Fluent builder for :class:`WorkoutLgd`; obtain via ``WorkoutLgd.builder()``.

    Unset ``workout_years`` / ``discount_rate`` / ``costs`` fall back to
    the embedded registry defaults at :meth:`build`.

    Examples
    --------
    >>> from finstack_quant.models.credit import lgd
    >>> builder = lgd.WorkoutLgd.builder().workout_years(2.0).discount_rate(0.05)
    >>> builder.build().workout_years
    2.0

    """

    def collateral(self, piece: CollateralPiece) -> WorkoutLgdBuilder:
        """
        Append one collateral piece to the waterfall (highest priority first).

        Parameters
        ----------
        piece : CollateralPiece
            Collateral to append.

        Returns
        -------
        WorkoutLgdBuilder
            ``self`` for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by :meth:`build`.
        """
        ...

    def collateral_pieces(self, pieces: list[CollateralPiece]) -> WorkoutLgdBuilder:
        """
        Append several collateral pieces in order.

        Parameters
        ----------
        pieces : list[CollateralPiece]
            Collateral to append, highest priority first.

        Returns
        -------
        WorkoutLgdBuilder
            ``self`` for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by :meth:`build`.
        """
        ...

    def workout_years(self, years: float) -> WorkoutLgdBuilder:
        """
        Set the expected workout duration.

        Parameters
        ----------
        years : float
            Workout duration in years (>= 0).

        Returns
        -------
        WorkoutLgdBuilder
            ``self`` for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by :meth:`build`.
        """
        ...

    def discount_rate(self, rate: float) -> WorkoutLgdBuilder:
        """
        Set the discount rate over the workout horizon.

        Parameters
        ----------
        rate : float
            Annual decimal discount rate (>= 0).

        Returns
        -------
        WorkoutLgdBuilder
            ``self`` for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by :meth:`build`.
        """
        ...

    def costs(self, costs: WorkoutCosts) -> WorkoutLgdBuilder:
        """
        Set the workout cost rates.

        Parameters
        ----------
        costs : WorkoutCosts
            Direct and indirect cost rates.

        Returns
        -------
        WorkoutLgdBuilder
            ``self`` for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by :meth:`build`.
        """
        ...

    def build(self) -> WorkoutLgd:
        """
        Validate and build the model; the builder is consumed.

        Returns
        -------
        WorkoutLgd
            Validated workout model.

        Raises
        ------
        ValueError
            If ``workout_years`` or ``discount_rate`` is negative or
            non-finite, or the builder was already consumed.
        """
        ...

class DownturnLgd:
    """
    Downturn LGD adjuster (stressed approximation or regulatory floor).

    Examples
    --------
    >>> from finstack_quant.models.credit import lgd
    >>> lgd.DownturnLgd.regulatory_floor(0.05, 0.25).adjust(0.10)
    0.25

    """

    @staticmethod
    def stressed(asset_correlation: float, lgd_sensitivity: float, stress_quantile: float) -> DownturnLgd:
        """
        Stressed approximation:
        ``LGD_base + lgd_sensitivity * sqrt(rho) * Phi^-1(q) * sqrt(LGD_base * (1 - LGD_base))``.

        Parameters
        ----------
        asset_correlation : float
            Asset correlation ``rho`` in ``(0, 1)``; Basel 0.12-0.24.
        lgd_sensitivity : float
            LGD sensitivity to the systematic factor (>= 0); typical 0.3-0.5.
        stress_quantile : float
            Downturn quantile in ``(0, 1)``, e.g. ``0.999``.

        Returns
        -------
        DownturnLgd
            Adjuster with ``method == "stressed_approximation"``.

        Raises
        ------
        ValueError
            On out-of-range parameters.

        Examples
        --------
        >>> from finstack_quant.models.credit import lgd
        >>> lgd.DownturnLgd.stressed(0.15, 0.4, 0.999).adjust(0.4) > 0.4
        True
        """
        ...

    @staticmethod
    def regulatory_floor(add_on: float, floor: float) -> DownturnLgd:
        """
        Regulatory floor: ``max(LGD_base + add_on, floor)``.

        Parameters
        ----------
        add_on : float
            Flat add-on (>= 0); typical 0.05-0.10.
        floor : float
            Absolute floor in ``[0, 1]``; typical 0.10 secured / 0.25
            unsecured.

        Returns
        -------
        DownturnLgd
            Adjuster with ``method == "regulatory_floor"``.

        Raises
        ------
        ValueError
            On out-of-range parameters.

        Examples
        --------
        >>> from finstack_quant.models.credit import lgd
        >>> lgd.DownturnLgd.regulatory_floor(0.05, 0.25).adjust(0.30)
        0.35
        """
        ...

    @staticmethod
    def from_registry_id(id: str) -> DownturnLgd:
        """
        Load a regulatory-floor preset by id from the embedded registry.

        Parameters
        ----------
        id : str
            Registry preset identifier (e.g. ``"basel_unsecured"``).

        Returns
        -------
        DownturnLgd
            The preset adjuster.

        Raises
        ------
        KeyError
            If ``id`` is unknown.
        ValueError
            If the preset uses an unsupported method.

        Examples
        --------
        >>> from finstack_quant.models.credit import lgd
        >>> lgd.DownturnLgd.from_registry_id("basel_unsecured").method
        'regulatory_floor'
        """
        ...

    @staticmethod
    def basel_secured() -> DownturnLgd:
        """
        Registry default secured-exposure floor (Basel).

        Returns
        -------
        DownturnLgd
            Secured regulatory-floor preset.

        Raises
        ------
        ValueError
            If the embedded credit registry is invalid.

        Examples
        --------
        >>> from finstack_quant.models.credit import lgd
        >>> lgd.DownturnLgd.basel_secured().method
        'regulatory_floor'
        """
        ...

    @staticmethod
    def basel_unsecured() -> DownturnLgd:
        """
        Registry ``basel_unsecured`` floor preset.

        Returns
        -------
        DownturnLgd
            Unsecured regulatory-floor preset.

        Raises
        ------
        ValueError
            If the embedded credit registry is invalid.

        Examples
        --------
        >>> from finstack_quant.models.credit import lgd
        >>> lgd.DownturnLgd.basel_unsecured().adjust(0.0) > 0.0
        True
        """
        ...

    def adjust(self, base_lgd: float) -> float:
        """
        Downturn LGD for ``base_lgd``, clamped to ``[0, 1]``.

        Parameters
        ----------
        base_lgd : float
            Through-the-cycle LGD as a decimal in ``[0, 1]``.

        Returns
        -------
        float
            Downturn LGD as a decimal.

        Raises
        ------
        ValueError
            If ``base_lgd`` is non-finite or outside ``[0, 1]``.
        """
        ...

    @property
    def method(self) -> str:
        """
        Canonical method name: ``"stressed_approximation"`` or ``"regulatory_floor"``.

        Returns
        -------
        str
            Canonical method name: ``"stressed_approximation"`` or ``"regulatory_floor"``.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def params(self) -> Any:
        """
        Method parameters as a mapping in canonical JSON form.

        Returns
        -------
        Any
            Method parameters as a mapping in canonical JSON form.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @staticmethod
    def from_json(json: str) -> DownturnLgd:
        """
        Deserialize a ``DownturnLgd`` from its canonical JSON form.

        Parameters
        ----------
        json : str
            JSON text produced by :meth:`to_json` (strict serde; unknown or
            invalid fields are rejected).

        Returns
        -------
        DownturnLgd
            Reconstructed value.

        Raises
        ------
        ValueError
            If the payload is not valid ``DownturnLgd`` JSON.

        Examples
        --------
        >>> from finstack_quant.models.credit import lgd
        >>> value = lgd.DownturnLgd.regulatory_floor(0.05, 0.25)
        >>> lgd.DownturnLgd.from_json(value.to_json()).to_json() == value.to_json()
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

class EadCalculator:
    """
    Exposure-at-default calculator: ``EAD = drawn + undrawn * CCF``.

    Examples
    --------
    >>> from finstack_quant.models.credit import lgd
    >>> calc = lgd.EadCalculator.revolver(60.0, 40.0)
    >>> (calc.ead, calc.utilization)
    (90.0, 0.6)

    """

    def __init__(self, drawn: float, undrawn: float, ccf: float) -> None:
        """
        Build a calculator with an explicit credit conversion factor.

        Parameters
        ----------
        drawn : float
            Currently drawn amount (>= 0).
        undrawn : float
            Undrawn commitment (>= 0).
        ccf : float
            Credit conversion factor as a decimal in ``[0, 1]``.

        Raises
        ------
        ValueError
            For negative or non-finite amounts or a CCF outside ``[0, 1]``.
        """
        ...

    @staticmethod
    def term_loan(drawn: float) -> EadCalculator:
        """
        Fully drawn term loan (no undrawn component, CCF ``1.0``).

        Parameters
        ----------
        drawn : float
            Drawn principal (>= 0).

        Returns
        -------
        EadCalculator
            Calculator whose ``ead`` equals ``drawn``.

        Raises
        ------
        ValueError
            If ``drawn`` is negative or non-finite.

        Examples
        --------
        >>> from finstack_quant.models.credit import lgd
        >>> lgd.EadCalculator.term_loan(100.0).ead
        100.0
        """
        ...

    @staticmethod
    def revolver(drawn: float, undrawn: float) -> EadCalculator:
        """
        Revolver with the Basel IRB CCF of ``0.75``.

        Parameters
        ----------
        drawn : float
            Currently drawn amount (>= 0).
        undrawn : float
            Undrawn commitment (>= 0).

        Returns
        -------
        EadCalculator
            Calculator with ``ead = drawn + 0.75 * undrawn``.

        Raises
        ------
        ValueError
            If an amount is negative or non-finite.

        Examples
        --------
        >>> from finstack_quant.models.credit import lgd
        >>> lgd.EadCalculator.revolver(60.0, 40.0).ead
        90.0
        """
        ...

    def leq_from_observed_ead(self, observed_ead: float) -> float | None:
        """
        Loan-equivalent exposure implied by an observed EAD.

        Parameters
        ----------
        observed_ead : float
            Realised exposure at default in the facility's currency.

        Returns
        -------
        float | None
            ``(observed_ead - drawn) / undrawn``, or ``None`` when there is
            no undrawn amount.

        Notes
        -----
        This method does not raise.
        """
        ...

    @property
    def ead(self) -> float:
        """
        ``drawn + undrawn * ccf``.

        Returns
        -------
        float
            ``drawn + undrawn * ccf``.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def utilization(self) -> float:
        """
        ``drawn / (drawn + undrawn)``, or ``0.0`` when there is no commitment.

        Returns
        -------
        float
            ``drawn / (drawn + undrawn)``, or ``0.0`` when there is no commitment.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def total_commitment(self) -> float:
        """
        ``drawn + undrawn``.

        Returns
        -------
        float
            ``drawn + undrawn``.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @staticmethod
    def from_json(json: str) -> EadCalculator:
        """
        Deserialize a ``EadCalculator`` from its canonical JSON form.

        Parameters
        ----------
        json : str
            JSON text produced by :meth:`to_json` (strict serde; unknown or
            invalid fields are rejected).

        Returns
        -------
        EadCalculator
            Reconstructed value.

        Raises
        ------
        ValueError
            If the payload is not valid ``EadCalculator`` JSON.

        Examples
        --------
        >>> from finstack_quant.models.credit import lgd
        >>> value = lgd.EadCalculator.revolver(60.0, 40.0)
        >>> lgd.EadCalculator.from_json(value.to_json()).to_json() == value.to_json()
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

def seniority_recovery_stats(
    seniority: str,
    rating_agency: str | None = None,
) -> BetaRecovery:
    """
    Historical Beta recovery distribution for a seniority class.

    Parameters
    ----------
    seniority : str
        One of ``1st_lien_secured``, ``2nd_lien_secured``,
        ``senior_secured``, ``senior_unsecured``, ``subordinated``,
        ``junior_subordinated``.
    rating_agency : str | None
        ``"moodys"`` (canonical) or ``"sp"``. ``None`` selects the
        registry default calibration (Moody's historical).

    Returns
    -------
    BetaRecovery
        Moment-matched Beta distribution for the class.

    Raises
    ------
    ValueError
        If ``seniority`` or ``rating_agency`` is unknown, or the selected
        calibration has no entry for the class.

    Examples
    --------
    >>> from finstack_quant.models.credit import lgd
    >>> lgd.seniority_recovery_stats("senior_secured").mean
    0.52

    """
    ...

def workout_lgd(
    ead: float,
    collateral: list[tuple[str, float, float]],
    direct_cost_pct: float,
    indirect_cost_pct: float,
    time_to_resolution_years: float,
    discount_rate: float,
) -> WorkoutLgdResult:
    """
    Workout net recovery, LGD, and recovery rate in one call.

    One-shot twin of ``WorkoutLgd.builder()...build().evaluate(ead)``.

    Parameters
    ----------
    ead : float
        Exposure at default (> 0).
    collateral : list[tuple[str, float, float]]
        ``(collateral_type, book_value, haircut)`` triples; the type is one
        of ``cash``, ``securities``, ``receivables``, ``inventory``, ``equipment``, ``real_estate``, ``intellectual_property``, ``other`` and ``haircut`` is a decimal in ``[0, 1]``.
    direct_cost_pct : float
        Direct resolution costs as a decimal fraction of EAD (>= 0).
    indirect_cost_pct : float
        Indirect resolution costs as a decimal fraction of EAD (>= 0).
    time_to_resolution_years : float
        Expected workout duration in years (>= 0).
    discount_rate : float
        Annual decimal discount rate for the workout period (>= 0).

    Returns
    -------
    WorkoutLgdResult
        ``net_recovery``, ``lgd`` and ``recovery_rate``.

    Raises
    ------
    ValueError
        For an unknown collateral type or any invalid input.

    Examples
    --------
    >>> from finstack_quant.models.credit import lgd
    >>> result = lgd.workout_lgd(100.0, [("cash", 50.0, 0.0)], 0.0, 0.0, 0.0, 0.0)
    >>> (result.net_recovery, result.lgd)
    (50.0, 0.5)

    """
    ...
