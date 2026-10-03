"""
Type stubs for ``finstack_quant.core.rating_scales``.

Bindings for the shared credit rating-scale registry (scorecard scales such as
S&P, Moody's, and Fitch) from the ``finstack-quant-core`` Rust crate.

Distinct from ``finstack_quant.models.credit.migration.RatingScale``, which models
the ordered state set of a credit-migration transition matrix.

Examples
--------
>>> from finstack_quant.core.rating_scales import RatingLevel
>>> RatingLevel("BBB", 70.0, 65.0).name
'BBB'

"""

from __future__ import annotations

from collections.abc import Iterator

import pandas as pd

from finstack_quant.core.config import FinstackConfig

class UnknownScalePolicy:
    """
    Policy for unknown scorecard rating-scale names.

    Examples
    --------
    >>> from finstack_quant.core.rating_scales import UnknownScalePolicy
    >>> UnknownScalePolicy.from_name("error").name
    'error'

    """

    ERROR: UnknownScalePolicy
    """Reject unknown scale names (raises ``ValueError``)."""

    FALLBACK_TO_DEFAULT: UnknownScalePolicy
    """Use the configured default scale for unknown scale names."""

    WARN_AND_FALLBACK: UnknownScalePolicy
    """Use the default scale and let callers emit a warning."""

    @classmethod
    def from_name(cls, name: str) -> UnknownScalePolicy:
        """
        Parse a policy from its exact lowercase snake_case name (case-sensitive).

        Parameters
        ----------
        name : str
            Exactly one of ``"error"``, ``"fallback_to_default"``, or
            ``"warn_and_fallback"``; ``"ERROR"`` is rejected.

        Returns
        -------
        UnknownScalePolicy
            Matching policy constant.

        Raises
        ------
        ValueError
            If ``name`` is not a recognized policy.

        Examples
        --------
        >>> from finstack_quant.core.rating_scales import UnknownScalePolicy
        >>> UnknownScalePolicy.from_name("error").name
        'error'

        """

    @property
    def name(self) -> str:
        """
        Canonical snake_case policy name.

        Returns
        -------
        str
            Policy identifier string.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    def to_json(self) -> str:
        """
        Serialize this policy to a JSON string.

        Returns
        -------
        str
            Canonical JSON representation of this `UnknownScalePolicy`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    @classmethod
    def from_json(cls, json: str) -> UnknownScalePolicy:
        """
        Deserialize a policy from JSON.

        Parameters
        ----------
        json : str
            JSON document matching the policy schema.

        Returns
        -------
        UnknownScalePolicy

        Raises
        ------
        ValueError
            If *json* is not one of the exact JSON strings ``"error"``,
            ``"fallback_to_default"``, or ``"warn_and_fallback"``.

        Examples
        --------
        >>> from finstack_quant.core.rating_scales import UnknownScalePolicy
        >>> UnknownScalePolicy.from_json(UnknownScalePolicy.ERROR.to_json()).name
        'error'

        """
        ...
    def __repr__(self) -> str:
        """Return a debug representation of this policy.

        Returns
        -------
        str
        """
        ...
    def __str__(self) -> str:
        """Return the policy name.

        Returns
        -------
        str
        """
        ...
    def __eq__(self, other: object) -> bool:
        """Return whether two policies are equal.

        Returns
        -------
        bool
        """
        ...
    def __hash__(self) -> int:
        """Return a hash for this policy.

        Returns
        -------
        int
        """
        ...

class RatingLevel:
    """
    A single rating threshold row on a scorecard scale.

    Examples
    --------
    >>> from finstack_quant.core.rating_scales import RatingLevel
    >>> level = RatingLevel("BBB", 70.0, 65.0)
    >>> (level.name, level.score, level.min_score)
    ('BBB', 70.0, 65.0)

    """

    def __init__(self, name: str, score: float, min_score: float) -> None:
        """
        Construct one rating threshold row.

        Parameters
        ----------
        name : str
            Rating label (e.g. ``"BBB+"``, ``"Baa1"``).
        score : float
            Representative score on the 0–100 scorecard scale for this rating.
        min_score : float
            Minimum score threshold required to qualify for this rating.

        Raises
        ------
        ValueError
            If *name* is blank or either score is non-finite or outside the
            inclusive 0-100 range.
        """
        ...

    def __eq__(self, other: object) -> bool:
        """Return whether two levels have the same name, score and min_score.

        Returns
        -------
        bool
        """
        ...
    @property
    def name(self) -> str:
        """
        Rating name, for example ``"AAA"`` or ``"Aaa"``.

        Returns
        -------
        str
            Rating label.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    @property
    def score(self) -> float:
        """
        Numeric score on the 0-100 scorecard scale.

        Returns
        -------
        float
            Representative score for the rating.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    @property
    def min_score(self) -> float:
        """
        Minimum score threshold for this rating.

        Returns
        -------
        float
            Lower bound on the scorecard scale.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    def to_json(self) -> str:
        """
        Serialize this rating level to a JSON string.

        Returns
        -------
        str
            Canonical JSON representation of this `RatingLevel`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    @classmethod
    def from_json(cls, json: str) -> RatingLevel:
        """
        Deserialize a rating level from JSON.

        Parameters
        ----------
        json : str
            JSON document matching the rating-level schema.

        Returns
        -------
        RatingLevel
            Validated `RatingLevel` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If *json* is malformed, has unknown or missing fields, names a
            blank level, or gives either score as non-finite or outside
            ``[0, 100]``.

        Examples
        --------
        >>> from finstack_quant.core.rating_scales import RatingLevel
        >>> level = RatingLevel("BBB", 70.0, 65.0)
        >>> RatingLevel.from_json(level.to_json()).name
        'BBB'

        """
        ...
    def __repr__(self) -> str:
        """Return a debug representation of this rating level.

        Returns
        -------
        str
        """
        ...

class ScorecardScale:
    """
    A named, ordered list of scorecard rating thresholds.

    Distinct from ``finstack_quant.models.credit.migration.RatingScale`` (which models
    the ordered state set of a credit-migration / transition matrix).

    Examples
    --------
    >>> from finstack_quant.core.rating_scales import RatingLevel, ScorecardScale
    >>> scale = ScorecardScale("custom", [RatingLevel("BBB", 70.0, 65.0)], description="Example")
    >>> (scale.scale_name, scale.description, len(scale.ratings))
    ('custom', 'Example', 1)

    """

    def __init__(
        self,
        scale_name: str,
        ratings: list[RatingLevel],
        description: str | None = None,
    ) -> None:
        """
        Construct a scorecard scale from ordered rating levels.

        Parameters
        ----------
        scale_name : str
            Scale identifier (e.g. ``"S&P"``, ``"Moody's"``).
        ratings : list[RatingLevel]
            Ordered levels from best to worst.
        description : str, optional
            Human-readable description of the scale.

        Raises
        ------
        ValueError
            If *ratings* is empty, contains duplicate names, or is not
            strictly ordered best-to-worst (both ``score`` and ``min_score``
            must strictly descend).
        """
        ...

    def __getitem__(self, index: int) -> RatingLevel:
        """Return the ``index``-th rating level (negative indices supported).

        Raises
        ------
        IndexError
            If *index* is out of range.

        Returns
        -------
        RatingLevel
        """
        ...

    def __iter__(self) -> Iterator[RatingLevel]:
        """Iterate over rating levels best-to-worst.

        Returns
        -------
        Iterator[RatingLevel]
        """
        ...

    def __eq__(self, other: object) -> bool:
        """Return whether two scales are structurally equal.

        Returns
        -------
        bool
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Rating levels as a pandas ``DataFrame``.

        Returns
        -------
        pandas.DataFrame
            One row per level, best first, with columns ``name`` (str),
            ``score`` (float64) and ``min_score`` (float64).

        Raises
        ------
        ImportError
            If pandas is not installed.

        Examples
        --------
        >>> from finstack_quant.core.rating_scales import RatingLevel, ScorecardScale
        >>> scale = ScorecardScale("custom", [RatingLevel("A", 90.0, 85.0), RatingLevel("B", 70.0, 65.0)])
        >>> list(scale.to_dataframe()["name"])
        ['A', 'B']
        """
        ...
    @property
    def scale_name(self) -> str:
        """
        Scale name, for example ``"S&P"`` or ``"Moody's"``.

        Returns
        -------
        str
            Scale identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    @property
    def description(self) -> str | None:
        """
        Optional human-readable description.

        Returns
        -------
        str or None
            Description text when set.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    @property
    def ratings(self) -> list[RatingLevel]:
        """
        Ordered rating levels from best to worst.

        Returns
        -------
        list[RatingLevel]
            Rating threshold rows.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    def to_json(self) -> str:
        """
        Serialize this scale to a JSON string.

        Returns
        -------
        str
            Canonical JSON representation of this `ScorecardScale`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    @classmethod
    def from_json(cls, json: str) -> ScorecardScale:
        """
        Deserialize a scorecard scale from JSON.

        Parameters
        ----------
        json : str
            JSON document matching the scorecard-scale schema.

        Returns
        -------
        ScorecardScale
            Validated `ScorecardScale` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If *json* is malformed, has unknown or missing fields, or its
            ratings are empty, duplicated, invalid, or not strictly ordered
            best-to-worst by both score fields.

        Examples
        --------
        >>> from finstack_quant.core.rating_scales import RatingLevel, ScorecardScale
        >>> scale = ScorecardScale("custom", [RatingLevel("BBB", 70.0, 65.0)])
        >>> ScorecardScale.from_json(scale.to_json()).ratings[0].name
        'BBB'

        """
        ...
    def __len__(self) -> int:
        """Return the number of rating levels on this scale.

        Returns
        -------
        int
        """
        ...
    def __repr__(self) -> str:
        """Return a debug representation of this scale.

        Returns
        -------
        str
        """
        ...

class RatingScaleRegistry:
    """
    Versioned registry of scorecard scales and policy.

    Examples
    --------
    >>> from finstack_quant.core.config import FinstackConfig
    >>> from finstack_quant.core.rating_scales import registry_from_config
    >>> registry = registry_from_config(FinstackConfig())
    >>> (registry.default_scale_id(), registry.default_scorecard_score(), registry.unknown_scale_policy().name)
    ('sp', 50.0, 'fallback_to_default')
    >>> (registry.is_known_rating_scale("S&P"), registry.rating_scale("S&P").scale_name)
    (True, 'S&P')

    """

    def default_scorecard_score(self) -> float:
        """
        Return the configured default scorecard score for threshold gaps.

        Returns
        -------
        float
            Default score on the 0–100 scale used when interpolating between
            published rating thresholds.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """

    def default_scale_id(self) -> str:
        """
        Return the configured default rating-scale id.

        Returns
        -------
        str
            Default scale identifier (e.g. ``"sp"``).

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """

    def scale_ids(self) -> list[str]:
        """
        Primary id of every registered scale, in registry order.

        Aliases (e.g. ``"Fitch"``) are not included; resolve them through
        :meth:`rating_scale` or :meth:`is_known_rating_scale`.

        Returns
        -------
        list[str]
            Primary scale ids such as ``["sp", "moodys"]``.

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.core.rating_scales import embedded_registry
        >>> "sp" in embedded_registry().scale_ids()
        True
        """
        ...

    def unknown_scale_policy(self) -> UnknownScalePolicy:
        """
        Return the configured policy for unknown scale names.

        Returns
        -------
        UnknownScalePolicy
            Error, fallback, or warn-and-fallback policy.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """

    def is_known_rating_scale(self, name: str) -> bool:
        """
        Return whether ``name`` is a known scale id or alias.

        Parameters
        ----------
        name : str
            Scale id or alias to test.

        Returns
        -------
        bool
            ``True`` when the name resolves without applying the unknown-scale
            policy.

        Notes
        -----
        This method does not raise; it returns ``True`` or ``False``.
        """

    def rating_scale(self, name: str) -> ScorecardScale:
        """
        Resolve a scale name or alias to a :class:`ScorecardScale`.

        Honours the configured unknown-scale policy: this may fall back to the
        default scale or raise ``ValueError``.

        Parameters
        ----------
        name : str
            Scale id or alias (e.g. ``"sp"``, ``"moodys"``).

        Returns
        -------
        ScorecardScale
            Resolved scale with ordered rating thresholds.

        Raises
        ------
        ValueError
            When policy is ``ERROR`` and ``name`` is unknown.

        """

    def to_json(self) -> str:
        """
        Serialize this registry to a JSON string.

        Returns
        -------
        str
            Canonical JSON representation of this `RatingScaleRegistry`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    @classmethod
    def from_json(cls, json: str) -> RatingScaleRegistry:
        """
        Deserialize a registry from JSON.

        Parameters
        ----------
        json : str
            JSON document matching the registry schema.

        Returns
        -------
        RatingScaleRegistry

        Raises
        ------
        ValueError
            If *json* is malformed or violates the supported schema version,
            identifier, alias, default-scale, score-range, or rating-order
            invariants.

        Examples
        --------
        >>> from finstack_quant.core.config import FinstackConfig
        >>> from finstack_quant.core.rating_scales import RatingScaleRegistry, registry_from_config
        >>> registry = registry_from_config(FinstackConfig())
        >>> RatingScaleRegistry.from_json(registry.to_json()).default_scale_id()
        'sp'

        """
        ...
    def __repr__(self) -> str:
        """Return a debug representation of this registry.

        Returns
        -------
        str
        """
        ...
    def __eq__(self, other: object) -> bool:
        """Return whether two registries are structurally equal under canonical Rust equality; signed zeros compare equal.

        Returns
        -------
        bool
        """
        ...

def embedded_registry() -> RatingScaleRegistry:
    """
    Return the embedded (built-in) rating-scale registry.

    Returns
    -------
    RatingScaleRegistry
        Registry shipped with the library containing standard agency scales.

    Raises
    ------
    ValueError
        If the embedded rating-scale registry cannot be constructed.

    Examples
    --------
    >>> from finstack_quant.core.rating_scales import embedded_registry
    >>> reg = embedded_registry()
    >>> reg.is_known_rating_scale("sp")
    True
    """

def registry_from_config(config: FinstackConfig) -> RatingScaleRegistry:
    """
    Load a registry from a :class:`FinstackConfig` extension.

    Falls back to :func:`embedded_registry` when the config does not override
    :data:`RATING_SCALES_EXTENSION_KEY`.

    Parameters
    ----------
    config : FinstackConfig
        Application configuration possibly carrying a custom scales extension.

    Returns
    -------
    RatingScaleRegistry
        Embedded or config-overridden registry.

    Raises
    ------
    ValueError
        If the configured extension is malformed or violates rating-scale
        registry invariants.

    Examples
    --------
    >>> from finstack_quant.core.config import FinstackConfig
    >>> from finstack_quant.core.rating_scales import registry_from_config
    >>> registry = registry_from_config(FinstackConfig())
    >>> (registry.default_scale_id(), registry.default_scorecard_score(), registry.unknown_scale_policy().name)
    ('sp', 50.0, 'fallback_to_default')
    >>> (registry.is_known_rating_scale("S&P"), registry.rating_scale("S&P").scale_name)
    (True, 'S&P')

    """

RATING_SCALES_EXTENSION_KEY: str
"""Configuration-extension key used to override the embedded registry."""

__all__ = [
    "RATING_SCALES_EXTENSION_KEY",
    "RatingLevel",
    "RatingScaleRegistry",
    "ScorecardScale",
    "UnknownScalePolicy",
    "embedded_registry",
    "registry_from_config",
]
