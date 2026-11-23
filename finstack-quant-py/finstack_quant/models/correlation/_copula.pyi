"""Stub declarations mirroring ``finstack-quant-py/src/bindings/models/correlation/copula.rs``."""

from __future__ import annotations

from typing import Any, Sequence
import pandas as pd
import polars as pl
from finstack_quant.core.table import ArrowTable

class CopulaSpec:
    """
    Copula model specification for configuration and deferred construction.

    Use class methods to create a spec, then call :meth:`build` to obtain
    a concrete :class:`Copula` instance.

    Example
    -------
    >>> from finstack_quant.models.correlation import CopulaSpec
    >>> spec = CopulaSpec.gaussian()
    >>> copula = spec.build()
    >>> copula.model_name
    'One-Factor Gaussian Copula'

    Examples
    --------
    >>> from finstack_quant.models.correlation import CopulaSpec
    >>> spec = CopulaSpec.gaussian()
    >>> (spec.is_gaussian, spec.build().model_name)
    (True, 'One-Factor Gaussian Copula')

    """

    @classmethod
    def gaussian(cls) -> CopulaSpec:
        """
        One-factor Gaussian copula (market standard).

        Returns
        -------
        CopulaSpec
            Gaussian copula specification.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.models.correlation import CopulaSpec
        >>> (CopulaSpec.gaussian().is_gaussian, CopulaSpec.gaussian().build().num_factors)
        (True, 1)
        """
        ...

    @classmethod
    def student_t(cls, degrees_of_freedom: float) -> CopulaSpec:
        """
        Student-t copula with specified degrees of freedom.

        Parameters
        ----------
        degrees_of_freedom : float
            Degrees of freedom; must be finite and ``> 2`` (finite variance).
            Typical calibration range for CDX tranches is 4-10. Matches the
            Rust ``CopulaSpec::StudentT { degrees_of_freedom }`` field.

        Returns
        -------
        CopulaSpec
            Student-t copula specification.

        Raises
        ------
        ValueError
            If ``degrees_of_freedom`` is not finite or is ``<= 2``.

        Examples
        --------
        >>> from finstack_quant.models.correlation import CopulaSpec
        >>> copula = CopulaSpec.student_t(degrees_of_freedom=5.0).build()
        >>> (copula.model_name, round(copula.tail_dependence(0.3), 6))
        ('Student-t Copula', 0.122387)

        """
        ...

    @classmethod
    def random_factor_loading(cls, loading_vol: float) -> CopulaSpec:
        """
        Random Factor Loading copula with stochastic correlation.

        Parameters
        ----------
        loading_vol : float
            Volatility of the factor loading, clamped to ``[0, 0.5]``.

        Returns
        -------
        CopulaSpec
            RFL copula specification.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.models.correlation import CopulaSpec
        >>> spec = CopulaSpec.random_factor_loading(0.2)
        >>> (spec.is_rfl, spec.build().model_name)
        (True, 'Random Factor Loading Copula')
        """
        ...

    @classmethod
    def multi_factor(cls) -> CopulaSpec:
        """
        Two-factor Gaussian copula with global and shared-sector factors.

        Returns
        -------
        CopulaSpec
            Two-factor copula specification.
        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.models.correlation import CopulaSpec
        >>> spec = CopulaSpec.multi_factor()
        >>> (spec.is_multi_factor, spec.build().num_factors)
        (True, 2)
        """
        ...

    def build(self) -> Copula:
        """
        Build a concrete :class:`Copula` from this specification.

        Returns
        -------
        Copula
            Concrete copula model.

        Raises
        ------
        ValueError
            If a deserialized Student-t spec has invalid degrees of freedom,
            or random-loading volatility is non-finite or outside ``[0, 0.5]``.
        """
        ...

    @property
    def is_gaussian(self) -> bool:
        """
        ``True`` if this is a Gaussian spec.

        Returns
        -------
        bool
            Whether gaussian holds for this `CopulaSpec`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def is_student_t(self) -> bool:
        """
        ``True`` if this is a Student-t spec.

        Returns
        -------
        bool
            Whether student t holds for this `CopulaSpec`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def is_rfl(self) -> bool:
        """
        ``True`` if this is a Random Factor Loading spec.

        Returns
        -------
        bool
            Whether rfl holds for this `CopulaSpec`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def is_multi_factor(self) -> bool:
        """
        ``True`` if this is a Multi-factor spec.

        Returns
        -------
        bool
            Whether multi factor holds for this `CopulaSpec`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical tagged JSON wire format (``{"type": ...}``).

        Returns
        -------
        str
            JSON document, e.g. ``{"type":"student_t","degrees_of_freedom":5.0}``.

        Raises
        ------
        ValueError
            If the specification cannot be serialized to JSON
            (raised as ``"CopulaSpec serialization failed"``).

        Examples
        --------
        >>> from finstack_quant.models.correlation import CopulaSpec
        >>> CopulaSpec.from_json(CopulaSpec.student_t(5.0).to_json()) == CopulaSpec.student_t(5.0)
        True

        """
        ...

    @staticmethod
    def from_json(json: str) -> CopulaSpec:
        """
        Deserialize a spec produced by :meth:`to_json`.

        Parameters
        ----------
        json : str
            Tagged JSON document produced by :meth:`to_json`.

        Returns
        -------
        CopulaSpec
            The reconstructed specification.

        Raises
        ------
        ValueError
            If the payload is malformed or the tag is unknown.

        Examples
        --------
        >>> from finstack_quant.models.correlation import CopulaSpec
        >>> CopulaSpec.from_json('{"type":"gaussian"}').is_gaussian
        True

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
        """Python-style repr, e.g. ``CopulaSpec(type='student_t', degrees_of_freedom=5.0)``."""
        ...

class Copula:
    """
    Concrete copula model for portfolio default correlation.

    Obtain an instance via :meth:`CopulaSpec.build`.

    Example
    -------
    >>> from finstack_quant.models.correlation import CopulaSpec
    >>> copula = CopulaSpec.gaussian().build()
    >>> # P(default | Z=0) = norm.cdf(-2.33 / sqrt(1 - 0.3)) ≈ 0.0027,
    >>> # below the unconditional PD of norm.cdf(-2.33) ≈ 0.0099.
    >>> copula.conditional_default_prob(-2.33, [0.0], 0.3)
    0.002...

    Examples
    --------
    >>> from finstack_quant.models.correlation import CopulaSpec
    >>> copula = CopulaSpec.gaussian().build()
    >>> (copula.model_name, round(copula.conditional_default_prob(-2.33, [0.0], 0.3), 6))
    ('One-Factor Gaussian Copula', 0.002677)

    """

    def conditional_default_prob(
        self,
        default_threshold: float,
        factor_realization: Sequence[float],
        correlation: float,
    ) -> float:
        """
        Conditional default probability given factor realization(s).

        P(default | Z) where the default threshold is typically Φ⁻¹(PD).

        Parameters
        ----------
        default_threshold : float
            Default barrier (e.g. ``norm.ppf(PD)``).
        factor_realization : list[float]
            Systematic factor values.
        correlation : float
            Asset correlation.

        Returns
        -------
        float
            Conditional default probability.

        Raises
        ------
        ValueError
            If the factor vector length differs from the copula factor count;
            if the threshold, correlation, or any factor is non-finite; if
            correlation is outside ``[0, 1]``; or if the computed probability
            is non-finite or outside ``[0, 1]``.
        """
        ...

    @property
    def num_factors(self) -> int:
        """
        Number of systematic factors in the model.

        Returns
        -------
        int
            Number of systematic factors in the model.

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

    def tail_dependence(self, correlation: float) -> float:
        """
        Strict lower-tail dependence coefficient ``λ_L`` at the given correlation.

        Random Factor Loading returns the probability mass at unit loading
        under the calibrated clipped-normal loading distribution. Gaussian
        models have zero tail dependence except at perfect correlation;
        Student-t returns its closed-form positive coefficient.

        Parameters
        ----------
        correlation : float
            Asset correlation.

        Returns
        -------
        float
            The strict lower-tail coefficient in ``[0, 1]`` for valid inputs.

        Notes
        -----
        This method does not raise; out-of-domain or non-finite inputs yield ``NaN`` or ``inf`` rather than an exception.
        """
        ...

    def stress_correlation_proxy(self, correlation: float) -> float:
        """
        Heuristic stress-correlation proxy for the Random Factor Loading copula.

        This is **not** the strict copula lower-tail-dependence coefficient
        ``λ_L`` returned by :meth:`tail_dependence`. It gauges the extra correlation mass in the
        high-loading tail and vanishes in the Gaussian (``loading_vol = 0``)
        limit.

        Parameters
        ----------
        correlation : float
            Asset correlation.

        Returns
        -------
        float
            Non-negative stress-correlation proxy.

        Raises
        ------
        ValueError
            If the copula is not a Random Factor Loading copula.
        """
        ...
