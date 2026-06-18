"""Type stubs for ``finstack_quant.core.rating_scales``.

Bindings for the shared credit rating-scale registry (scorecard scales such as
S&P, Moody's, and Fitch) from the ``finstack-quant-core`` Rust crate.

Distinct from ``finstack_quant.core.credit.migration.RatingScale``, which models
the ordered state set of a credit-migration transition matrix.
"""

from __future__ import annotations

from finstack_quant.core.config import FinstackConfig

class UnknownScalePolicy:
    """Policy for unknown scorecard rating-scale names."""

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
        """

    @property
    def name(self) -> str:
        """Canonical snake_case policy name.

        Returns
        -------
        str
            Policy identifier string.
        """

    def to_json(self) -> str: ...
    @classmethod
    def from_json(cls, json: str) -> UnknownScalePolicy: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...
    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...

class RatingLevel:
    """A single rating threshold row on a scorecard scale."""

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
        """
        ...
    @property
    def name(self) -> str:
        """Rating name, for example ``"AAA"`` or ``"Aaa"``.

        Returns
        -------
        str
            Rating label.
        """

    @property
    def score(self) -> float:
        """Numeric score on the 0-100 scorecard scale.

        Returns
        -------
        float
            Representative score for the rating.
        """

    @property
    def min_score(self) -> float:
        """Minimum score threshold for this rating.

        Returns
        -------
        float
            Lower bound on the scorecard scale.
        """

    def to_json(self) -> str: ...
    @classmethod
    def from_json(cls, json: str) -> RatingLevel: ...
    def __repr__(self) -> str: ...

class ScorecardScale:
    """A named, ordered list of scorecard rating thresholds.

    Distinct from ``finstack_quant.core.credit.migration.RatingScale`` (which models
    the ordered state set of a credit-migration / transition matrix).
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
        """
        ...
    @property
    def scale_name(self) -> str:
        """Scale name, for example ``"S&P"`` or ``"Moody's"``.

        Returns
        -------
        str
            Scale identifier.
        """

    @property
    def description(self) -> str | None:
        """Optional human-readable description.

        Returns
        -------
        str or None
            Description text when set.
        """

    @property
    def ratings(self) -> list[RatingLevel]:
        """Ordered rating levels from best to worst.

        Returns
        -------
        list[RatingLevel]
            Rating threshold rows.
        """

    def to_json(self) -> str: ...
    @classmethod
    def from_json(cls, json: str) -> ScorecardScale: ...
    def __len__(self) -> int: ...
    def __repr__(self) -> str: ...

class RatingScaleRegistry:
    """Versioned registry of scorecard scales and policy."""

    def default_scorecard_score(self) -> float:
        """Return the configured default scorecard score for threshold gaps.

        Returns
        -------
        float
            Default score on the 0–100 scale used when interpolating between
            published rating thresholds.
        """

    def default_scale_id(self) -> str:
        """Return the configured default rating-scale id.

        Returns
        -------
        str
            Default scale identifier (e.g. ``"sp"``).
        """

    def unknown_scale_policy(self) -> UnknownScalePolicy:
        """Return the configured policy for unknown scale names.

        Returns
        -------
        UnknownScalePolicy
            Error, fallback, or warn-and-fallback policy.
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
        """

    def to_json(self) -> str: ...
    @classmethod
    def from_json(cls, json: str) -> RatingScaleRegistry: ...
    def __repr__(self) -> str: ...

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
