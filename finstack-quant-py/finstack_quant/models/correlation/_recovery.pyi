"""Stub declarations mirroring ``finstack-quant-py/src/bindings/models/correlation/recovery.rs``."""

from __future__ import annotations

from typing import Any, Sequence
import pandas as pd
import polars as pl
from finstack_quant.core.table import ArrowTable

class RecoverySpec:
    """
    Recovery model specification for configuration and deferred construction.

    Example
    -------
    >>> from finstack_quant.models.correlation import RecoverySpec
    >>> spec = RecoverySpec.constant(0.4)
    >>> model = spec.build()
    >>> model.expected_recovery
    0.4

    Examples
    --------
    >>> from finstack_quant.models.correlation import RecoverySpec
    >>> spec = RecoverySpec.constant(0.4)
    >>> (spec.build().expected_recovery, spec.build().lgd)
    (0.4, 0.6)

    """

    @classmethod
    def constant(cls, rate: float) -> RecoverySpec:
        """
        Constant recovery rate as a decimal of notional.

        Parameters
        ----------
        rate : float
            Fixed recovery rate in ``[0, 1]``.

        Returns
        -------
        RecoverySpec
            Constant recovery specification.

        Raises
        ------
        ValueError
            If ``rate`` is non-finite or outside ``[0, 1]``.

        Examples
        --------
        >>> from finstack_quant.models.correlation import RecoverySpec
        >>> spec = RecoverySpec.constant(0.4)
        >>> (spec.build().expected_recovery, spec.build().conditional_recovery(0.0))
        (0.4, 0.4)

        """
        ...

    @classmethod
    def market_correlated(cls, mean: float, vol: float, correlation: float) -> RecoverySpec:
        """
        Market-correlated (Andersen-Sidenius) stochastic recovery.

        Parameters
        ----------
        mean : float
            Finite zero-total-shock recovery location (not the unconditional mean) in ``[0, 1]``; endpoints are constant.
        vol : float
            Finite local shock scale (not realized recovery standard deviation) in ``[0, 0.5]``.
        correlation : float
            Finite Gaussian correlation with the market factor in ``[-1, 1]``.

        Returns
        -------
        RecoverySpec
            Stochastic recovery specification.

        Raises
        ------
        ValueError
            If ``mean`` is non-finite or outside ``[0, 1]``, or if ``vol`` or
            ``correlation`` is non-finite, ``vol`` lies outside ``[0, 0.5]``,
            or ``correlation`` lies outside ``[-1, 1]``.

        Examples
        --------
        >>> from finstack_quant.models.correlation import RecoverySpec
        >>> model = RecoverySpec.market_correlated(0.4, 0.2, 0.3).build()
        >>> (model.is_stochastic, model.recovery_volatility)
        (True, 0.2)

        """
        ...

    @classmethod
    def market_standard_stochastic(cls) -> RecoverySpec:
        """
        Market-standard stochastic recovery (40% mean, 25% vol, +40% corr).

        Recovery falls in stress under the canonical low-factor-stress
        convention.

        Returns
        -------
        RecoverySpec
            Standard stochastic recovery specification.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.models.correlation import RecoverySpec
        >>> model = RecoverySpec.market_standard_stochastic().build()
        >>> (model.model_name, round(model.expected_recovery, 3))
        ('Market-Correlated Stochastic Recovery', 0.418)
        """
        ...

    def build(self) -> RecoveryModel:
        """
        Build a concrete :class:`RecoveryModel` from this specification.

        Returns
        -------
        RecoveryModel
            Concrete recovery model.

        Raises
        ------
        ValueError
            If a recovery parameter is non-finite or outside its documented range.
        RuntimeError
            If numerical integration of expected recovery fails to converge.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical tagged JSON wire format (``{"type": ...}``).

        Returns
        -------
        str
            JSON document, e.g. ``{"type":"constant","rate":0.4}``.

        Raises
        ------
        ValueError
            If the specification cannot be serialized to JSON
            (raised as ``"RecoverySpec serialization failed"``).

        Examples
        --------
        >>> from finstack_quant.models.correlation import RecoverySpec
        >>> RecoverySpec.from_json(RecoverySpec.constant(0.4).to_json()) == RecoverySpec.constant(0.4)
        True

        """
        ...

    @staticmethod
    def from_json(json: str) -> RecoverySpec:
        """
        Deserialize a spec produced by :meth:`to_json`.

        Parameters
        ----------
        json : str
            Tagged JSON document produced by :meth:`to_json`.

        Returns
        -------
        RecoverySpec
            The reconstructed specification.

        Raises
        ------
        ValueError
            If the payload is malformed or the tag is unknown.

        Examples
        --------
        >>> from finstack_quant.models.correlation import RecoverySpec
        >>> RecoverySpec.from_json('{"type":"constant","rate":0.4}').build().expected_recovery
        0.4

        """
        ...

    def __eq__(self, other: object) -> bool: ...
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
    def __repr__(self) -> str:
        """Python-style repr, e.g. ``RecoverySpec(type='constant', rate=0.4)``."""
        ...

class RecoveryModel:
    """
    Concrete recovery model for credit portfolio pricing.

    Obtain an instance via :meth:`RecoverySpec.build`. The stochastic law is
    ``sigmoid(logit(m) + sigma/(m*(1-m)) * (rho*z + sqrt(1-rho**2)*e))``
    for independent standard normals ``z`` and ``e``. The location ``m`` is
    recovery at zero total shock, not the unconditional mean; ``sigma`` is a
    local scale, not realized standard deviation. Conditional recovery averages
    over ``e``. Endpoint locations and zero scale give exact constants.

    Examples
    --------
    >>> from finstack_quant.models.correlation import RecoverySpec
    >>> model = RecoverySpec.constant(0.4).build()
    >>> (model.expected_recovery, model.lgd, model.is_stochastic)
    (0.4, 0.6, False)

    """

    @property
    def expected_recovery(self) -> float:
        """
        Expected (unconditional) recovery rate.

        Returns
        -------
        float
            Expected (unconditional) recovery rate.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def conditional_recovery(self, market_factor: float) -> float:
        """
        Expected recovery conditional on the systematic factor, averaging over the independent obligor shock.

        Parameters
        ----------
        market_factor : float
            Finite systematic standard-normal realization; low values represent stress.

        Returns
        -------
        float
            Conditional recovery rate.

        Raises
        ------
        ValueError
            If the systematic factor is non-finite.
        RuntimeError
            If numerical integration of conditional recovery fails.
        """
        ...

    @property
    def lgd(self) -> float:
        """
        Loss given default (1 − recovery).

        Returns
        -------
        float
            Loss given default (1 − recovery).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def conditional_lgd(self, market_factor: float) -> float:
        """
        Conditional LGD given market factor.

        Parameters
        ----------
        market_factor : float
            Finite systematic standard-normal realization; low values represent stress.

        Returns
        -------
        float
            Conditional LGD.

        Raises
        ------
        ValueError
            If the systematic factor is non-finite.
        RuntimeError
            If numerical integration of conditional recovery fails.
        """
        ...

    def sample_recovery(self, market_factor: float, idiosyncratic_normal: float) -> float:
        """Sample bounded recovery from the canonical logistic-normal law.

        Parameters
        ----------
        market_factor : float
            Finite systematic standard-normal shock; low values represent stress.
        idiosyncratic_normal : float
            Finite independent obligor standard-normal shock; ignored for constant
            distributions and perfect systematic correlation.

        Returns
        -------
        float
            Sampled recovery in ``[0, 1]`` as a decimal fraction of notional.

        Raises
        ------
        ValueError
            If either shock is non-finite.
        """
        ...

    @property
    def has_idiosyncratic_dispersion(self) -> bool:
        """Whether sampling needs an independent obligor recovery draw.

        Returns
        -------
        bool
            False for constant distributions and perfect systematic correlation.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def recovery_volatility(self) -> float:
        """
        Local recovery shock scale in decimals, not realized recovery standard deviation.

        Returns
        -------
        float
            Local recovery shock scale in decimals, not realized recovery standard deviation.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def is_stochastic(self) -> bool:
        """
        Whether the recovery distribution is nonconstant, including obligor-only variation.

        Returns
        -------
        bool
            Whether stochastic holds for this `RecoveryModel`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def model_name(self) -> str:
        """
        Model name for diagnostics.

        Returns
        -------
        str
            Model name for diagnostics.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
