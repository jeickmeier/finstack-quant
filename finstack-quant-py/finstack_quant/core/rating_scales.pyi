"""Type stubs for ``finstack_quant.core.rating_scales``.

Bindings for the shared credit rating-scale registry (scorecard scales such as
S&P, Moody's, and Fitch) from the ``finstack-quant-core`` Rust crate.

Distinct from ``finstack_quant.core.credit.migration.RatingScale``, which models
the ordered state set of a credit-migration transition matrix.
"""

from __future__ import annotations

from finstack_quant.core.config import FinstackConfig

class UnknownScalePolicy:
    """Policy for unknown scorecard rating-scale names.

    Examples
    --------
    >>> from finstack_quant.core.rating_scales import UnknownScalePolicy
    >>> UnknownScalePolicy.ERROR.name  # doctest: +SKIP
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
        """Parse a snake_case policy name (case-insensitive).

        Parameters
        ----------
        name : str
            One of ``"error"``, ``"fallback_to_default"``, or
            ``"warn_and_fallback"``.

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
        >>> UnknownScalePolicy.from_name("error").name  # doctest: +SKIP
        'error'
        """

    @property
    def name(self) -> str:
        """Canonical snake_case policy name.

        Returns
        -------
        str
            Policy identifier string.

        Examples
        --------
        >>> UnknownScalePolicy.ERROR.name  # doctest: +SKIP
        'error'
        """

    def to_json(self) -> str:
        """Serialize this policy to a JSON string.

        Returns
        -------
        str
        """
        ...
    @classmethod
    def from_json(cls, json: str) -> UnknownScalePolicy:
        """Deserialize a policy from JSON.

        Parameters
        ----------
        json : str
            JSON document matching the policy schema.

        Returns
        -------
        UnknownScalePolicy
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
    """A single rating threshold row on a scorecard scale.

    Examples
    --------
    >>> from finstack_quant.core.rating_scales import RatingLevel
    >>> lvl = RatingLevel("BBB", 70.0, 65.0)  # doctest: +SKIP
    >>> lvl.name  # doctest: +SKIP
    'BBB'
    """

    def __init__(self, name: str, score: float, min_score: float) -> None:
        """Construct one rating threshold row.

        Parameters
        ----------
        name : str
            Rating label (e.g. ``"BBB+"``, ``"Baa1"``).
        score : float
            Representative score on the 0–100 scorecard scale for this rating.
        min_score : float
            Minimum score threshold required to qualify for this rating.

        Examples
        --------
        >>> from finstack_quant.core.rating_scales import RatingLevel
        >>> lvl = RatingLevel("BBB", 70.0, 65.0)  # doctest: +SKIP
        """
        ...
    @property
    def name(self) -> str:
        """Rating name, for example ``"AAA"`` or ``"Aaa"``.

        Returns
        -------
        str
            Rating label.

        Examples
        --------
        >>> lvl = RatingLevel("BBB", 70.0, 65.0)  # doctest: +SKIP
        >>> lvl.name  # doctest: +SKIP
        'BBB'
        """

    @property
    def score(self) -> float:
        """Numeric score on the 0-100 scorecard scale.

        Returns
        -------
        float
            Representative score for the rating.

        Examples
        --------
        >>> lvl = RatingLevel("BBB", 70.0, 65.0)  # doctest: +SKIP
        >>> lvl.score  # doctest: +SKIP
        70.0
        """

    @property
    def min_score(self) -> float:
        """Minimum score threshold for this rating.

        Returns
        -------
        float
            Lower bound on the scorecard scale.

        Examples
        --------
        >>> lvl = RatingLevel("BBB", 70.0, 65.0)  # doctest: +SKIP
        >>> lvl.min_score  # doctest: +SKIP
        65.0
        """

    def to_json(self) -> str:
        """Serialize this rating level to a JSON string.

        Returns
        -------
        str
        """
        ...
    @classmethod
    def from_json(cls, json: str) -> RatingLevel:
        """Deserialize a rating level from JSON.

        Parameters
        ----------
        json : str
            JSON document matching the rating-level schema.

        Returns
        -------
        RatingLevel
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
    """A named, ordered list of scorecard rating thresholds.

    Distinct from ``finstack_quant.core.credit.migration.RatingScale`` (which models
    the ordered state set of a credit-migration / transition matrix).

    Examples
    --------
    >>> from finstack_quant.core.rating_scales import ScorecardScale, RatingLevel
    >>> scale = ScorecardScale("custom", [RatingLevel("A", 90.0, 85.0)])  # doctest: +SKIP
    >>> scale.scale_name  # doctest: +SKIP
    'custom'
    """

    def __init__(
        self,
        scale_name: str,
        ratings: list[RatingLevel],
        description: str | None = None,
    ) -> None:
        """Construct a scorecard scale from ordered rating levels.

        Parameters
        ----------
        scale_name : str
            Scale identifier (e.g. ``"S&P"``, ``"Moody's"``).
        ratings : list[RatingLevel]
            Ordered levels from best to worst.
        description : str, optional
            Human-readable description of the scale.

        Examples
        --------
        >>> from finstack_quant.core.rating_scales import ScorecardScale, RatingLevel
        >>> scale = ScorecardScale("custom", [RatingLevel("A", 90.0, 85.0)])  # doctest: +SKIP
        """
        ...
    @property
    def scale_name(self) -> str:
        """Scale name, for example ``"S&P"`` or ``"Moody's"``.

        Returns
        -------
        str
            Scale identifier.

        Examples
        --------
        >>> scale = ScorecardScale("custom", [])  # doctest: +SKIP
        >>> scale.scale_name  # doctest: +SKIP
        'custom'
        """

    @property
    def description(self) -> str | None:
        """Optional human-readable description.

        Returns
        -------
        str or None
            Description text when set.

        Examples
        --------
        >>> scale = ScorecardScale("custom", [], description="My scale")  # doctest: +SKIP
        >>> scale.description  # doctest: +SKIP
        'My scale'
        """

    @property
    def ratings(self) -> list[RatingLevel]:
        """Ordered rating levels from best to worst.

        Returns
        -------
        list[RatingLevel]
            Rating threshold rows.

        Examples
        --------
        >>> scale = ScorecardScale("custom", [RatingLevel("A", 90.0, 85.0)])  # doctest: +SKIP
        >>> len(scale.ratings)  # doctest: +SKIP
        1
        """

    def to_json(self) -> str:
        """Serialize this scale to a JSON string.

        Returns
        -------
        str
        """
        ...
    @classmethod
    def from_json(cls, json: str) -> ScorecardScale:
        """Deserialize a scorecard scale from JSON.

        Parameters
        ----------
        json : str
            JSON document matching the scorecard-scale schema.

        Returns
        -------
        ScorecardScale
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
    """Versioned registry of scorecard scales and policy.

    Examples
    --------
    >>> from finstack_quant.core.rating_scales import embedded_registry
    >>> reg = embedded_registry()  # doctest: +SKIP
    >>> reg.is_known_rating_scale("sp")  # doctest: +SKIP
    True
    """

    def default_scorecard_score(self) -> float:
        """Return the configured default scorecard score for threshold gaps.

        Returns
        -------
        float
            Default score on the 0–100 scale used when interpolating between
            published rating thresholds.

        Examples
        --------
        >>> reg = embedded_registry()  # doctest: +SKIP
        >>> reg.default_scorecard_score()  # doctest: +SKIP
        50.0
        """

    def default_scale_id(self) -> str:
        """Return the configured default rating-scale id.

        Returns
        -------
        str
            Default scale identifier (e.g. ``"sp"``).

        Examples
        --------
        >>> reg = embedded_registry()  # doctest: +SKIP
        >>> reg.default_scale_id()  # doctest: +SKIP
        'sp'
        """

    def unknown_scale_policy(self) -> UnknownScalePolicy:
        """Return the configured policy for unknown scale names.

        Returns
        -------
        UnknownScalePolicy
            Error, fallback, or warn-and-fallback policy.

        Examples
        --------
        >>> reg = embedded_registry()  # doctest: +SKIP
        >>> reg.unknown_scale_policy().name  # doctest: +SKIP
        'error'
        """

    def is_known_rating_scale(self, name: str) -> bool:
        """Return whether ``name`` is a known scale id or alias.

        Parameters
        ----------
        name : str
            Scale id or alias to test.

        Returns
        -------
        bool
            ``True`` when the name resolves without applying the unknown-scale
            policy.

        Examples
        --------
        >>> reg = embedded_registry()  # doctest: +SKIP
        >>> reg.is_known_rating_scale("sp")  # doctest: +SKIP
        True
        """

    def rating_scale(self, name: str) -> ScorecardScale:
        """Resolve a scale name or alias to a :class:`ScorecardScale`.

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

        Examples
        --------
        >>> reg = embedded_registry()  # doctest: +SKIP
        >>> scale = reg.rating_scale("sp")  # doctest: +SKIP
        >>> scale.scale_name  # doctest: +SKIP
        'S&P'
        """

    def to_json(self) -> str:
        """Serialize this registry to a JSON string.

        Returns
        -------
        str
        """
        ...
    @classmethod
    def from_json(cls, json: str) -> RatingScaleRegistry:
        """Deserialize a registry from JSON.

        Parameters
        ----------
        json : str
            JSON document matching the registry schema.

        Returns
        -------
        RatingScaleRegistry
        """
        ...
    def __repr__(self) -> str:
        """Return a debug representation of this registry.

        Returns
        -------
        str
        """
        ...

def embedded_registry() -> RatingScaleRegistry:
    """Return the embedded (built-in) rating-scale registry.

    Returns
    -------
    RatingScaleRegistry
        Registry shipped with the library containing standard agency scales.

    Examples
    --------
    >>> from finstack_quant.core.rating_scales import embedded_registry
    >>> reg = embedded_registry()
    >>> reg.is_known_rating_scale("sp")
    True
    """

def registry_from_config(config: FinstackConfig) -> RatingScaleRegistry:
    """Load a registry from a :class:`FinstackConfig` extension.

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

    Examples
    --------
    >>> from finstack_quant.core.config import FinstackConfig
    >>> from finstack_quant.core.rating_scales import registry_from_config
    >>> reg = registry_from_config(FinstackConfig.default())  # doctest: +SKIP
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
