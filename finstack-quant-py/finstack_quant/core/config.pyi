"""
Configuration types from ``finstack-quant-core``: rounding, tolerances, and global config.

Provides :class:`RoundingMode`, :class:`ToleranceConfig`, and
:class:`FinstackConfig` for controlling rounding behaviour and
numerical tolerance thresholds across the library.

Examples
--------
>>> from finstack_quant.core.config import RoundingMode
>>> RoundingMode.from_name("bankers") == RoundingMode.BANKERS
True

"""

from __future__ import annotations

from typing import Any, Optional, Union

from finstack_quant.core.currency import Currency

__all__ = [
    "RoundingMode",
    "ToleranceConfig",
    "FinstackConfig",
]

class RoundingMode:
    """
    Rounding mode for monetary and rate calculations.

    Enum-style class with class-level constants for each supported mode.

    Examples
    --------
    >>> from finstack_quant.core.config import RoundingMode
    >>> RoundingMode.from_name("bankers") == RoundingMode.BANKERS
    True

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
        Parse a rounding mode from its exact lowercase label (case-sensitive).

        Parameters
        ----------
        name : str
            One of ``"bankers"``, ``"away_from_zero"``, ``"toward_zero"``,
            ``"floor"``, ``"ceil"``. ``"BANKERS"`` is rejected.

        Returns
        -------
        RoundingMode
            Rounding policy matching the exact canonical lowercase name,
            including its documented tie-breaking convention.

        Raises
        ------
        ValueError
            If *name* is not one of the exact lowercase labels.

        Examples
        --------
        >>> from finstack_quant.core.config import RoundingMode
        >>> str(RoundingMode.from_name("bankers"))
        'bankers'
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> RoundingMode:
        """
        Deserialize from JSON (a quoted lowercase name such as ``"floor"``).

        Parameters
        ----------
        json : str
            JSON string literal.

        Returns
        -------
        RoundingMode
            Parsed mode.

        Raises
        ------
        ValueError
            If *json* is not a recognised mode name.

        Examples
        --------
        >>> from finstack_quant.core.config import RoundingMode
        >>> RoundingMode.from_json('"floor"') == RoundingMode.FLOOR
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to JSON (the quoted lowercase name).

        Returns
        -------
        str
            JSON string literal such as ``'"bankers"'``.

        Raises
        ------
        ValueError
            If serialization fails (cannot happen for a valid mode).
        """
        ...

    @property
    def name(self) -> str:
        """
        Canonical lowercase name, e.g. ``"bankers"``.

        Returns
        -------
        str
            The serde label.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    def __reduce__(self) -> tuple[object, tuple[str]]: ...
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

    Examples
    --------
    >>> from finstack_quant.core.config import ToleranceConfig
    >>> tolerances = ToleranceConfig(rate_epsilon=1e-9, generic_epsilon=1e-12)
    >>> (tolerances.rate_epsilon, tolerances.generic_epsilon)
    (1e-09, 1e-12)

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

        Raises
        ------
        ValueError
            If either supplied epsilon is non-finite or not strictly positive.

        """
        ...

    @property
    def rate_epsilon(self) -> float:
        """
        Epsilon used for rate-style comparisons.

        Returns
        -------
        float

            Epsilon used for rate-style comparisons.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def generic_epsilon(self) -> float:
        """
        Epsilon used for generic floating-point comparisons.

        Returns
        -------
        float

            Epsilon used for generic floating-point comparisons.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __repr__(self) -> str:
        """Return a debug representation of this tolerance config.

        Returns
        -------
        str
        """
        ...
    def __eq__(self, other: object) -> bool:
        """Return whether both epsilons are equal.

        Returns
        -------
        bool
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to JSON ``{"rate_epsilon": ..., "generic_epsilon": ...}``.

        Returns
        -------
        str
            JSON object text.

        Raises
        ------
        ValueError
            If serialization fails (cannot happen for a valid config).
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> ToleranceConfig:
        """
        Deserialize from JSON; the epsilons are re-validated.

        Parameters
        ----------
        json : str
            JSON object text.

        Returns
        -------
        ToleranceConfig
            Parsed tolerances.

        Raises
        ------
        ValueError
            If *json* is malformed or an epsilon is non-finite or not
            strictly positive.

        Examples
        --------
        >>> from finstack_quant.core.config import ToleranceConfig
        >>> t = ToleranceConfig(rate_epsilon=1e-9)
        >>> ToleranceConfig.from_json(t.to_json()) == t
        True
        """
        ...

    def __reduce__(self) -> tuple[object, tuple[str]]: ...

class FinstackConfig:
    """
    Top-level library configuration: rounding policy, per-currency scale
    overrides, comparison tolerances and versioned extensions.

    Parameters
    ----------
    rounding_mode : RoundingMode | str | None
        Rounding mode override (object or exact lowercase name). If ``None``,
        the library default (bankers) is used.
    tolerances : ToleranceConfig | None
        Tolerance configuration override. If ``None``, the library default
        is used.

    Raises
    ------
    ValueError
        If *rounding_mode* is a string that is not a recognised mode name.

    Examples
    --------
    >>> from finstack_quant.core.config import FinstackConfig
    >>> config = FinstackConfig()
    >>> config.set_extension("example.settings.v1", {"enabled": True})
    >>> (config.output_scale("USD"), config.ingest_scale("JPY"), config.extension_keys())
    (2, 6, ['example.settings.v1'])
    >>> (config.get_extension("example.settings.v1"), config.get_extension_json("example.settings.v1"))
    ({'enabled': True}, '{"enabled":true}')
    >>> (config.remove_extension("example.settings.v1"), config.get_extension("example.settings.v1"))
    (True, None)
    >>> FinstackConfig.from_json(config.to_json()).output_scale("USD")
    2
    >>> cfg = FinstackConfig(rounding_mode="floor")
    >>> cfg.set_output_scale("JPY", 2)
    >>> (cfg.rounding_mode.name, cfg.output_scale("JPY"), cfg.output_scale_overrides())
    ('floor', 2, {'JPY': 2})

    """

    def __init__(
        self,
        rounding_mode: Union[RoundingMode, str, None] = None,
        tolerances: Optional[ToleranceConfig] = None,
    ) -> None:
        """
        Create a configuration, optionally overriding rounding mode and tolerances.

        Parameters
        ----------
        rounding_mode : RoundingMode | str | None
            Rounding mode object or its exact lowercase name.
        tolerances : ToleranceConfig | None
            Tolerance configuration.

        Raises
        ------
        ValueError
            If *rounding_mode* is an unrecognised name.
        TypeError
            If *rounding_mode* is neither a ``RoundingMode`` nor a ``str``.
        """
        ...

    @property
    def rounding_mode(self) -> RoundingMode:
        """
        Active rounding mode.

        Returns
        -------
        RoundingMode
            The configured mode (bankers by default).

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def tolerances(self) -> ToleranceConfig:
        """
        Comparison tolerances.

        Returns
        -------
        ToleranceConfig
            The configured tolerances.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    def output_scale(self, currency: Union[Currency, str]) -> int:
        """
        Effective output decimal scale for a currency.

        Parameters
        ----------
        currency : Currency | str
            Currency object or ISO-4217 alphabetic code.

        Returns
        -------
        int
            Number of decimal places for output formatting; the currency's
            ISO minor units unless overridden.

        Raises
        ------
        ValueError
            If *currency* is not recognised.

        """
        ...

    def ingest_scale(self, currency: Union[Currency, str]) -> int:
        """
        Effective ingest decimal scale for a currency.

        Parameters
        ----------
        currency : Currency | str
            Currency object or ISO-4217 alphabetic code.

        Returns
        -------
        int
            Number of decimal places for input parsing; ``max(6, minor
            units)`` unless overridden.

        Raises
        ------
        ValueError
            If *currency* is not recognised.

        """
        ...

    def set_output_scale(self, currency: Union[Currency, str], scale: int) -> None:
        """
        Override the output decimal scale for a currency.

        Parameters
        ----------
        currency : Currency | str
            Currency object or ISO-4217 alphabetic code.
        scale : int
            Number of decimal places in the inclusive range ``0..28``.

        Raises
        ------
        ValueError
            If *currency* is not recognised or *scale* exceeds 28. The
            configuration is unchanged when validation fails.
        OverflowError
            If *scale* is negative or exceeds the unsigned 32-bit integer range.
        """
        ...

    def set_ingest_scale(self, currency: Union[Currency, str], scale: int) -> None:
        """
        Override the ingest decimal scale for a currency.

        Parameters
        ----------
        currency : Currency | str
            Currency object or ISO-4217 alphabetic code.
        scale : int
            Number of decimal places in the inclusive range ``0..28``.

        Raises
        ------
        ValueError
            If *currency* is not recognised or *scale* exceeds 28. The
            configuration is unchanged when validation fails.
        OverflowError
            If *scale* is negative or exceeds the unsigned 32-bit integer range.
        """
        ...

    def output_scale_overrides(self) -> dict[str, int]:
        """
        Explicit output-scale overrides.

        Returns
        -------
        dict[str, int]
            ``{iso_code: scale}`` for every overridden currency (sorted keys).

        Notes
        -----
        This method does not raise.
        """
        ...

    def ingest_scale_overrides(self) -> dict[str, int]:
        """
        Explicit ingest-scale overrides.

        Returns
        -------
        dict[str, int]
            ``{iso_code: scale}`` for every overridden currency (sorted keys).

        Notes
        -----
        This method does not raise.
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

        Raises
        ------
        ValueError
            If *key* is not a versioned namespaced key of the form
            ``namespace.domain.vN``, or if *value* cannot be represented as
            JSON data.

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

        Notes
        -----
        This method does not raise; it returns ``True`` or ``False``.
        """
        ...

    def extension_keys(self) -> list[str]:
        """
        Return configured extension keys.

        Returns
        -------
        list[str]
            Extension key list.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
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

        Raises
        ------
        ValueError
            If a stored extension exists but cannot be serialized to JSON.
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

        Raises
        ------
        ValueError
            If a stored extension exists but cannot be decoded as JSON.
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

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
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
        >>> from finstack_quant.core.config import FinstackConfig
        >>> FinstackConfig.from_json(FinstackConfig().to_json()).output_scale("USD")
        2

        """
        ...

    def __repr__(self) -> str:
        """Return a debug representation showing the rounding mode and override counts.

        Returns
        -------
        str
        """
        ...
    def __eq__(self, other: object) -> bool:
        """Return whether two configs are structurally equal (JSON wire form).

        Returns
        -------
        bool
        """
        ...
    def __reduce__(self) -> tuple[object, tuple[str]]: ...
