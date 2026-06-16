"""Configuration types from ``finstack-quant-core``: rounding, tolerances, and global config.

Provides :class:`RoundingMode`, :class:`ToleranceConfig`, and
:class:`FinstackConfig` for controlling rounding behaviour and
numerical tolerance thresholds across the library.
"""

from __future__ import annotations

from typing import Any, Optional

__all__ = [
    "RoundingMode",
    "ToleranceConfig",
    "FinstackConfig",
]

class RoundingMode:
    """Rounding mode for monetary and rate calculations.

    Enum-style class with class-level constants for each supported mode.
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
        """Parse a rounding mode from a human-readable label (case-insensitive).

        Parameters
        ----------
        name : str
            Label such as ``"bankers"``, ``"away_from_zero"``, ``"floor"``.

        Returns
        -------
        RoundingMode

        Raises
        ------
        ValueError
            If *name* is not recognised.
        """
        ...

    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...
    def __hash__(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...

class ToleranceConfig:
    """Numerical tolerance settings for rate and generic comparisons.

    Parameters
    ----------
    rate_epsilon : float | None
        Epsilon for rate-style comparisons. If ``None``, the library
        default is used.
    generic_epsilon : float | None
        Epsilon for generic floating-point comparisons. If ``None``,
        the library default is used.
    """

    def __init__(
        self,
        rate_epsilon: Optional[float] = None,
        generic_epsilon: Optional[float] = None,
    ) -> None:
        """Create tolerance settings, optionally overriding default epsilons.

        Parameters
        ----------
        rate_epsilon : float | None
            Epsilon for rate-style comparisons.
        generic_epsilon : float | None
            Epsilon for generic floating-point comparisons.
        """
        ...

    @property
    def rate_epsilon(self) -> float:
        """Epsilon used for rate-style comparisons."""
        ...

    @property
    def generic_epsilon(self) -> float:
        """Epsilon used for generic floating-point comparisons."""
        ...

    def __repr__(self) -> str: ...

class FinstackConfig:
    """Top-level library configuration combining rounding and tolerances.

    Parameters
    ----------
    rounding_mode : RoundingMode | None
        Rounding mode override. If ``None``, the library default is used.
    tolerances : ToleranceConfig | None
        Tolerance configuration override. If ``None``, the library default
        is used.
    """

    def __init__(
        self,
        rounding_mode: Optional[RoundingMode] = None,
        tolerances: Optional[ToleranceConfig] = None,
    ) -> None:
        """Create a configuration, optionally overriding rounding mode and tolerances.

        Parameters
        ----------
        rounding_mode : RoundingMode | None
            Rounding mode.
        tolerances : ToleranceConfig | None
            Tolerance configuration.
        """
        ...

    def output_scale(self, currency: str) -> int:
        """Effective output decimal scale for a currency.

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
        """
        ...

    def ingest_scale(self, currency: str) -> int:
        """Effective ingest decimal scale for a currency.

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
        """
        ...

    def set_extension(self, key: str, value: Any) -> None:
        """Set a versioned registry/config extension from Python data or a JSON string."""
        ...

    def remove_extension(self, key: str) -> bool:
        """Remove a versioned registry/config extension.

        Returns ``True`` when an extension was present.
        """
        ...

    def extension_keys(self) -> list[str]:
        """Return configured extension keys."""
        ...

    def get_extension_json(self, key: str) -> Optional[str]:
        """Return one extension as a JSON string, or ``None`` if absent."""
        ...

    def get_extension(self, key: str) -> Optional[Any]:
        """Return one extension as native Python data, or ``None`` if absent."""
        ...

    def to_json(self) -> str:
        """Serialize this config, including extensions, to JSON."""
        ...

    @classmethod
    def from_json(cls, json: str) -> FinstackConfig:
        """Deserialize a config from JSON."""
        ...

    def __repr__(self) -> str: ...
