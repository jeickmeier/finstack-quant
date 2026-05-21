"""Type stubs for ``finstack.core.rating_scales``.

Bindings for the shared credit rating-scale registry (scorecard scales such as
S&P, Moody's, and Fitch) from the ``finstack-core`` Rust crate.
"""

from __future__ import annotations

from finstack.core.config import FinstackConfig

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
        """Parse a snake_case policy name (case-insensitive)."""

    @property
    def name(self) -> str:
        """Canonical snake_case policy name."""

    def to_json(self) -> str: ...
    @classmethod
    def from_json(cls, json: str) -> UnknownScalePolicy: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...
    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...

class RatingLevel:
    """A single rating threshold row on a scorecard scale."""

    def __init__(self, name: str, score: float, min_score: float) -> None: ...
    @property
    def name(self) -> str:
        """Rating name, for example ``"AAA"`` or ``"Aaa"``."""

    @property
    def score(self) -> float:
        """Numeric score on the 0-100 scorecard scale."""

    @property
    def min_score(self) -> float:
        """Minimum score threshold for this rating."""

    def to_json(self) -> str: ...
    @classmethod
    def from_json(cls, json: str) -> RatingLevel: ...
    def __repr__(self) -> str: ...

class ScorecardScale:
    """A named, ordered list of scorecard rating thresholds.

    Distinct from ``finstack.core.credit.migration.RatingScale`` (which models
    the ordered state set of a credit-migration / transition matrix).
    """

    def __init__(
        self,
        scale_name: str,
        ratings: list[RatingLevel],
        description: str | None = None,
    ) -> None: ...
    @property
    def scale_name(self) -> str:
        """Scale name, for example ``"S&P"`` or ``"Moody's"``."""

    @property
    def description(self) -> str | None:
        """Optional human-readable description."""

    def get_ratings(self) -> list[RatingLevel]:
        """Ordered rating levels from best to worst."""

    def to_json(self) -> str: ...
    @classmethod
    def from_json(cls, json: str) -> ScorecardScale: ...
    def __len__(self) -> int: ...
    def __repr__(self) -> str: ...

class RatingScaleRegistry:
    """Versioned registry of scorecard scales and policy."""

    def get_default_scorecard_score(self) -> float:
        """Configured default scorecard score for threshold gaps."""

    def get_default_scale_id(self) -> str:
        """Configured default rating-scale id."""

    def get_unknown_scale_policy(self) -> UnknownScalePolicy:
        """Configured policy for unknown scale names."""

    def is_known_rating_scale(self, name: str) -> bool:
        """Return ``True`` if ``name`` is a known scale id or alias."""

    def rating_scale(self, name: str) -> ScorecardScale:
        """Resolve a scale name or alias to a :class:`ScorecardScale`.

        Honours the configured unknown-scale policy: this may fall back to the
        default scale or raise ``ValueError``.
        """

    def to_json(self) -> str: ...
    @classmethod
    def from_json(cls, json: str) -> RatingScaleRegistry: ...
    def __repr__(self) -> str: ...

def embedded_registry() -> RatingScaleRegistry:
    """Return the embedded (built-in) rating-scale registry."""

def registry_from_config(config: FinstackConfig) -> RatingScaleRegistry:
    """Load a registry from a :class:`FinstackConfig` extension, or fall back
    to the embedded registry when the config does not override
    :data:`RATING_SCALES_EXTENSION_KEY`.
    """

def extension_key() -> str:
    """Return :data:`RATING_SCALES_EXTENSION_KEY`."""

RATING_SCALES_EXTENSION_KEY: str
"""Configuration-extension key used to override the embedded registry."""

__all__ = [
    "RATING_SCALES_EXTENSION_KEY",
    "RatingLevel",
    "RatingScaleRegistry",
    "ScorecardScale",
    "UnknownScalePolicy",
    "embedded_registry",
    "extension_key",
    "registry_from_config",
]
