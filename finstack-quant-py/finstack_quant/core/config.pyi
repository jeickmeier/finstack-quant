"""
Configuration types from ``finstack-quant-core``: rounding, tolerances, and global config.

Provides :class:`RoundingMode`, :class:`ToleranceConfig`, and
:class:`FinstackConfig` for controlling rounding behaviour and
numerical tolerance thresholds across the library.

Examples
--------
>>> import finstack_quant.core.config as config
>>> config.__name__
'finstack_quant.core.config'
"""

from __future__ import annotations

from typing import Any, Optional

__all__ = [
    "RoundingMode",
    "ToleranceConfig",
    "FinstackConfig",
]

class RoundingMode:
    """
    Rounding mode for monetary and rate calculations.

    Enum-style class with class-level constants for each supported mode.

    Example
    -------
    >>> from finstack_quant.core.config import RoundingMode
    >>> RoundingMode.BANKERS  # doctest: +ELLIPSIS
    <finstack_quant.core.config.RoundingMode ...>

    Examples
    --------
    >>> from finstack_quant.core.config import RoundingMode
    >>> RoundingMode.__name__
    'RoundingMode'
    """

    BANKERS: RoundingMode
    """Banker's rounding (ties to even)."""
    AWAY_FROM_ZERO: RoundingMode
    """Round halves away from zero."""
    TOWARD_ZERO: RoundingMode
    """Round toward zero (truncate)."""
    FLOOR: RoundingMode
    """Round toward negative infinity."""
    CEIL: RoundingMode
    """Round toward positive infinity."""

    @classmethod
    def from_name(cls, name: str) -> RoundingMode:
        """
        Parse a rounding mode from a human-readable label (case-insensitive).

        Parameters
        ----------
        name : str
            Label such as ``"bankers"``, ``"away_from_zero"``, ``"floor"``.

        Returns
        -------
        RoundingMode

            Result of from name for this `RoundingMode` in the annotated representation.
        Raises
        ------
        ValueError
            If *name* is not recognised.

        Examples
        --------
        >>> from finstack_quant.core.config import RoundingMode
        >>> RoundingMode.from_name("bankers")  # doctest: +ELLIPSIS
        <finstack_quant.core.config.RoundingMode ...>
        """
        ...

    def __repr__(self) -> str:
        """Return a debug representation of this rounding mode.

        Returns
        -------
        str
        """
        ...
    def __str__(self) -> str:
        """Return a human-readable name for this rounding mode.

        Returns
        -------
        str
        """
        ...
    def __hash__(self) -> int:
        """Return a hash for this rounding mode.

        Returns
        -------
        int
        """
        ...
    def __eq__(self, other: object) -> bool:
        """Return whether two rounding modes are equal.

        Returns
        -------
        bool
        """
        ...

class ToleranceConfig:
    """
    Numerical tolerance settings for rate and generic comparisons.

    Parameters
    ----------
    rate_epsilon : float | None
        Epsilon for rate-style comparisons. If ``None``, the library
        default is used.
    generic_epsilon : float | None
        Epsilon for generic floating-point comparisons. If ``None``,
        the library default is used.

    Example
    -------
    >>> from finstack_quant.core.config import ToleranceConfig
    >>> tol = ToleranceConfig(rate_epsilon=1e-9)  # doctest: +SKIP

    Examples
    --------
    >>> from finstack_quant.core.config import ToleranceConfig
    >>> ToleranceConfig.__name__
    'ToleranceConfig'
    """

    def __init__(
        self,
        rate_epsilon: Optional[float] = None,
        generic_epsilon: Optional[float] = None,
    ) -> None:
        """
        Create tolerance settings, optionally overriding default epsilons.

        Parameters
        ----------
        rate_epsilon : float | None
            Epsilon for rate-style comparisons.
        generic_epsilon : float | None
            Epsilon for generic floating-point comparisons.

        Examples
        --------
        >>> from finstack_quant.core.config import ToleranceConfig
        >>> tol = ToleranceConfig(rate_epsilon=1e-9, generic_epsilon=1e-12)  # doctest: +SKIP

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    @property
    def rate_epsilon(self) -> float:
        """
        Epsilon used for rate-style comparisons.

        Returns
        -------
        float

            The rate epsilon exposed by this `ToleranceConfig`.
        Examples
        --------
        >>> tol = ToleranceConfig(rate_epsilon=1e-9)  # doctest: +SKIP
        >>> tol.rate_epsilon  # doctest: +SKIP
        1e-09
        """
        ...

    @property
    def generic_epsilon(self) -> float:
        """
        Epsilon used for generic floating-point comparisons.

        Returns
        -------
        float

            The generic epsilon exposed by this `ToleranceConfig`.
        Examples
        --------
        >>> tol = ToleranceConfig(generic_epsilon=1e-12)  # doctest: +SKIP
        >>> tol.generic_epsilon  # doctest: +SKIP
        1e-12
        """
        ...

    def __repr__(self) -> str:
        """Return a debug representation of this tolerance config.

        Returns
        -------
        str
        """
        ...

class FinstackConfig:
    """
    Top-level library configuration combining rounding and tolerances.

    Parameters
    ----------
    rounding_mode : RoundingMode | None
        Rounding mode override. If ``None``, the library default is used.
    tolerances : ToleranceConfig | None
        Tolerance configuration override. If ``None``, the library default
        is used.

    Example
    -------
    >>> from finstack_quant.core.config import FinstackConfig
    >>> cfg = FinstackConfig()  # doctest: +SKIP

    Examples
    --------
    >>> from finstack_quant.core.config import FinstackConfig
    >>> FinstackConfig.__name__
    'FinstackConfig'
    """

    def __init__(
        self,
        rounding_mode: Optional[RoundingMode] = None,
        tolerances: Optional[ToleranceConfig] = None,
    ) -> None:
        """
        Create a configuration, optionally overriding rounding mode and tolerances.

        Parameters
        ----------
        rounding_mode : RoundingMode | None
            Rounding mode.
        tolerances : ToleranceConfig | None
            Tolerance configuration.

        Examples
        --------
        >>> from finstack_quant.core.config import FinstackConfig, RoundingMode
        >>> cfg = FinstackConfig(rounding_mode=RoundingMode.BANKERS)  # doctest: +SKIP

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def output_scale(self, currency: str) -> int:
        """
        Effective output decimal scale for a currency.

        Parameters
        ----------
        currency : str
            ISO-4217 alphabetic currency code.

        Returns
        -------
        int
            Number of decimal places for output formatting.

        Raises
        ------
        ValueError
            If *currency* is not recognised.

        Examples
        --------
        >>> cfg = FinstackConfig()  # doctest: +SKIP
        >>> cfg.output_scale("USD")  # doctest: +SKIP
        2
        """
        ...

    def ingest_scale(self, currency: str) -> int:
        """
        Effective ingest decimal scale for a currency.

        Parameters
        ----------
        currency : str
            ISO-4217 alphabetic currency code.

        Returns
        -------
        int
            Number of decimal places for input parsing.

        Raises
        ------
        ValueError
            If *currency* is not recognised.

        Examples
        --------
        >>> cfg = FinstackConfig()  # doctest: +SKIP
        >>> cfg.ingest_scale("USD")  # doctest: +SKIP
        2
        """
        ...

    def set_extension(self, key: str, value: Any) -> None:
        """
        Set a versioned registry/config extension from Python data or a JSON string.

        Parameters
        ----------
        key:
            Namespaced extension key used to locate the versioned configuration
            payload in this process-wide registry.
        value:
            Python data or a JSON string.

        Examples
        --------
        >>> cfg = FinstackConfig()  # doctest: +SKIP
        >>> cfg.set_extension("custom_key", '{"v":1}')  # doctest: +SKIP

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def remove_extension(self, key: str) -> bool:
        """
        Remove a versioned registry/config extension.

        Parameters
        ----------
        key:
            Extension key to remove.

        Returns
        -------
        bool
            ``True`` when an extension was present.

        Examples
        --------
        >>> cfg = FinstackConfig()  # doctest: +SKIP
        >>> cfg.remove_extension("custom_key")  # doctest: +SKIP
        False

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def extension_keys(self) -> list[str]:
        """
        Return configured extension keys.

        Returns
        -------
        list[str]
            Extension key list.

        Examples
        --------
        >>> cfg = FinstackConfig()  # doctest: +SKIP
        >>> cfg.extension_keys()  # doctest: +SKIP
        []
        """
        ...

    def get_extension_json(self, key: str) -> Optional[str]:
        """
        Return one extension as a JSON string, or ``None`` if absent.

        Parameters
        ----------
        key:
            Namespaced extension key whose serialized payload is requested.

        Returns
        -------
        str or None
            JSON string, or ``None``.

        Examples
        --------
        >>> cfg = FinstackConfig()  # doctest: +SKIP
        >>> cfg.get_extension_json("custom_key")  # doctest: +SKIP
        None

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def get_extension(self, key: str) -> Optional[Any]:
        """
        Return one extension as native Python data, or ``None`` if absent.

        Parameters
        ----------
        key:
            Namespaced extension key whose JSON payload is decoded to Python.

        Returns
        -------
        Any or None
            Python data, or ``None``.

        Examples
        --------
        >>> cfg = FinstackConfig()  # doctest: +SKIP
        >>> cfg.get_extension("custom_key")  # doctest: +SKIP
        None

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this config, including extensions, to JSON.

        Returns
        -------
        str
            JSON text.

            Canonical JSON representation of this `FinstackConfig`, suitable for a matching `from_json` call.
        Examples
        --------
        >>> cfg = FinstackConfig()  # doctest: +SKIP
        >>> '"rounding_mode"' in cfg.to_json()  # doctest: +SKIP
        True
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> FinstackConfig:
        """
        Deserialize a config from JSON.

        Parameters
        ----------
        json:
            JSON document matching the config schema.

        Returns
        -------
        FinstackConfig
            Parsed configuration.

        Raises
        ------
        ValueError
            If JSON parsing or schema validation fails.

        Examples
        --------
        >>> cfg = FinstackConfig.from_json('{"rounding_mode":"bankers"}')  # doctest: +SKIP
        """
        ...

    def __repr__(self) -> str:
        """Return a debug representation of this config.

        Returns
        -------
        str
        """
        ...
