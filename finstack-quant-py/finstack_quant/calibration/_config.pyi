"""Stub declarations mirroring ``finstack-quant-py/src/bindings/calibration/config.rs``."""

from typing import Any

class SolverConfig:
    """Root-finder settings shared by every bootstrap step.

    Examples:
    --------
    >>> from finstack_quant.calibration import SolverConfig
    >>> SolverConfig(tolerance=1e-10).tolerance
    1e-10

    """

    def __init__(self, tolerance: float | None = None, max_iterations: int | None = None) -> None:
        """Build a solver configuration.

        Parameters
        ----------
        tolerance : float | None
            Finite, positive absolute residual tolerance in the target's own
            units (decimal rate, price, or volatility depending on the step).
            Rust default when None.
        max_iterations : int | None
            Maximum root-finder iterations per pillar. Rust default when None.

        Raises
        ------
        ValueError
            If ``tolerance`` is non-finite or not strictly positive, or
            ``max_iterations`` is zero.

        """

    @property
    def tolerance(self) -> float:
        """
        Absolute residual tolerance in the target's own units.

        This property does not raise.

        Returns
        -------
        float
            Absolute residual tolerance in the target's own units.
        """

    @property
    def max_iterations(self) -> int:
        """
        Maximum root-finder iterations per pillar.

        This property does not raise.

        Returns
        -------
        int
            Maximum root-finder iterations per pillar.
        """

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            Compact JSON encoding of this configuration.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """

    @staticmethod
    def from_json(json: str) -> SolverConfig:
        """
        Rebuild a configuration from JSON.

        Parameters
        ----------
        json : str
            Compact or pretty solver settings JSON with finite, positive
            tolerance and a positive iteration count. Omitted fields use
            Rust defaults.

        Returns
        -------
        SolverConfig
            The decoded configuration.

        Raises
        ------
        ValueError
            If ``json`` is malformed, carries unknown fields, or supplies a
            non-finite or non-positive tolerance, or a non-positive iteration count.

        Examples
        --------
        >>> from finstack_quant.calibration import SolverConfig
        >>> SolverConfig.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...

class RateBounds:
    """Admissible zero-rate bracket used to guard bootstrap solves.

    Examples:
    --------
    >>> from finstack_quant.calibration import RateBounds
    >>> RateBounds(-0.02, 0.25).max_rate
    0.25

    """

    def __init__(self, min_rate: float, max_rate: float) -> None:
        """Build explicit rate bounds.

        Parameters
        ----------
        min_rate : float
            Lower bracket for the solved zero rate, decimal per annum
            (for example ``-0.02`` for -200 bp).
        max_rate : float
            Upper bracket for the solved zero rate, decimal per annum.

        Raises
        ------
        ValueError
            If ``min_rate`` is not strictly below ``max_rate``.

        """

    @staticmethod
    def for_currency(currency: str) -> RateBounds:
        """Conventional bounds for a currency.

        Parameters
        ----------
        currency : str
            ISO 4217 alphabetic code, for example ``"USD"``.

        Returns
        -------
        RateBounds
            Bounds appropriate for that currency's rate regime.

        Raises
        ------
        ValueError
            If ``currency`` is not a valid ISO 4217 code.

        Examples:
        --------
        >>> from finstack_quant.calibration import RateBounds
        >>> RateBounds.for_currency("USD").min_rate <= 0.0
        True

        """

    @staticmethod
    def emerging_markets() -> RateBounds:
        """Wide bounds suited to high-inflation emerging-market curves.

        Returns
        -------
        RateBounds
            Bounds wide enough for triple-digit percentage rates.

        This preset is a compile-time constant and does not raise.

        Examples:
        --------
        >>> from finstack_quant.calibration import RateBounds
        >>> RateBounds.emerging_markets().max_rate > 1.0
        True

        """

    @property
    def min_rate(self) -> float:
        """
        Lower zero-rate bracket, decimal per annum.

        This property does not raise.

        Returns
        -------
        float
            Lower zero-rate bracket, decimal per annum.
        """

    @property
    def max_rate(self) -> float:
        """
        Upper zero-rate bracket, decimal per annum.

        This property does not raise.

        Returns
        -------
        float
            Upper zero-rate bracket, decimal per annum.
        """

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            Compact JSON encoding of these bounds.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """

    @staticmethod
    def from_json(json: str) -> RateBounds:
        """
        Rebuild bounds from JSON.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        RateBounds
            The decoded bounds.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.calibration import RateBounds
        >>> RateBounds.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...

class ValidationConfig:
    """Post-solve acceptance thresholds applied to each calibrated step.

    Examples:
    --------
    >>> from finstack_quant.calibration import ValidationConfig
    >>> isinstance(ValidationConfig().to_dict(), dict)
    True

    """

    def __init__(self, **overrides: Any) -> None:
        """Build validation thresholds from the Rust defaults plus overrides.

        Parameters
        ----------
        overrides : Any
            Field overrides applied on top of the Rust defaults, named exactly as
            the serde field names reported by :meth:`to_dict` (for example
            ``max_df_deviation``). Values use the units of the field they set.

        Raises
        ------
        ValueError
            If an override names an unknown field or has the wrong type.

        """

    def to_dict(self) -> dict[str, Any]:
        """Return the thresholds as a plain dictionary.

        Returns
        -------
        dict[str, Any]
            Serde field names mapped to their current values.

        Raises
        ------
        ValueError
            If the configuration cannot be serialized to its wire form.

        """

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            Compact JSON encoding of these thresholds.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """

    @staticmethod
    def from_json(json: str) -> ValidationConfig:
        """
        Rebuild thresholds from JSON.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        ValidationConfig
            The decoded thresholds.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.calibration import ValidationConfig
        >>> ValidationConfig.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...

class CalibrationConfig:
    """Plan-level solver, parallelism and acceptance settings owned by Rust.

    Keyword names and nested objects follow the serialized ``CalibrationConfig``
    schema. Omitted fields keep Rust defaults. Solver stopping settings belong
    under ``solver``; they are not top-level fields or properties.

    Examples
    --------
    >>> from finstack_quant.calibration import CalibrationConfig, SolverConfig
    >>> config = CalibrationConfig(solver={"max_iterations": 64})
    >>> config.solver.max_iterations
    64
    >>> config = CalibrationConfig(solver=SolverConfig(tolerance=1e-10))
    >>> config.solver.tolerance
    1e-10
    """

    def __init__(self, **overrides: Any) -> None:
        """Merge wire-shaped keyword fields into the Rust defaults and validate.

        Parameters
        ----------
        overrides : Any
            Serialized configuration fields. Nested dictionaries merge recursively,
            retaining omitted defaults; typed ``SolverConfig``, ``RateBounds`` and
            ``ValidationConfig`` objects supply their complete serialized fields.
            Use ``solver={"tolerance": 1e-10, "max_iterations": 200}`` for positive,
            finite numerical stopping tolerance and a positive iteration budget.
            ``use_parallel`` controls independent step execution;
            ``fail_on_bad_fit`` controls rejection of unsuccessful fits;
            ``compute_diagnostics`` enables per-quote diagnostics;
            ``validation_mode`` accepts ``"warn"`` or ``"error"``.
            ``validation`` contains acceptance thresholds. Explicit decimal-rate
            ``rate_bounds`` require ``rate_bounds_policy="explicit"`` to select
            them. Other fields use their canonical schema names and conventions.
            Unknown fields, including top-level ``tolerance`` and
            ``max_iterations``, are rejected. Pass no keyword to retain a default;
            ``None`` is not a general default sentinel.

        Raises
        ------
        ValueError
            If fields cannot be serialized, are unknown, have invalid types or
            violate Rust configuration validation.
        """

    @property
    def solver(self) -> SolverConfig:
        """
        The solver block.

        This property does not raise.

        Returns
        -------
        SolverConfig
            The solver block.
        """

    @property
    def use_parallel(self) -> bool:
        """
        Whether independent steps run in parallel.

        This property does not raise.

        Returns
        -------
        bool
            Whether independent steps run in parallel.
        """

    @property
    def fail_on_bad_fit(self) -> bool:
        """
        Whether a missed step tolerance raises instead of returning.

        This property does not raise.

        Returns
        -------
        bool
            Whether a missed step tolerance raises instead of returning.
        """

    @property
    def compute_diagnostics(self) -> bool:
        """
        Whether per-quote diagnostics are computed.

        This property does not raise.

        Returns
        -------
        bool
            Whether per-quote diagnostics are computed.
        """

    @property
    def validation_mode(self) -> str:
        """
        Post-solve validation severity, ``"warn"`` or ``"error"``.

        This property does not raise.

        Returns
        -------
        str
            Post-solve validation severity, ``"warn"`` or ``"error"``.
        """

    @property
    def rate_bounds(self) -> RateBounds:
        """
        Admissible zero-rate bracket.

        This property does not raise.

        Returns
        -------
        RateBounds
            Admissible zero-rate bracket.
        """

    @property
    def validation(self) -> ValidationConfig:
        """
        Post-solve acceptance thresholds.

        This property does not raise.

        Returns
        -------
        ValidationConfig
            Post-solve acceptance thresholds.
        """

    def to_dict(self) -> dict[str, Any]:
        """Return the settings as a plain dictionary.

        Returns
        -------
        dict[str, Any]
            Serde field names mapped to their current values.

        Raises
        ------
        ValueError
            If the configuration cannot be serialized to its wire form.

        """

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            Compact JSON encoding of these settings.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """

    @staticmethod
    def from_json(json: str) -> CalibrationConfig:
        """
        Rebuild settings from JSON.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        CalibrationConfig
            The decoded settings.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.calibration import CalibrationConfig
        >>> CalibrationConfig.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            Reconstruction callable and canonical JSON arguments.

        Raises
        ------
        ValueError
            If serialization of the stored value fails.
        """
        ...
    def __repr__(self) -> str: ...
