"""
Type stubs for ``finstack_quant.core.rating_scales``.

Bindings for the caller-supplied credit rating-scale registry (scorecard
scales) from the ``finstack-quant-core`` Rust crate. No scale is embedded and
nothing falls back to a default: every registry is built from entries, loaded
with ``RatingScaleRegistry.from_json`` or read from the
``core.rating_scales.v1`` configuration extension. A scale's numbers are the
scorecard author's utility policy, not an agency calibration.

Distinct from ``finstack_quant.models.credit.migration.RatingScale``, which models
the ordered state set of a credit-migration transition matrix.

Examples
--------
>>> from finstack_quant.core.rating_scales import RatingLevel, ScorecardScale
>>> scale = ScorecardScale("Internal", [RatingLevel("A", 90.0, 80.0), RatingLevel("B", 70.0, 0.0)])
>>> scale.classify(79.5).name
'B'

"""

from __future__ import annotations

from collections.abc import Iterator

import pandas as pd

from finstack_quant.core.config import FinstackConfig

class RatingLevel:
    """
    One band of a scorecard scale: a label, its representative score and the
    band's inclusive lower bound.

    Examples
    --------
    >>> from finstack_quant.core.rating_scales import RatingLevel
    >>> level = RatingLevel(" BBB ", 70.0, 65.0)
    >>> (level.name, level.score, level.min_score)
    ('BBB', 70.0, 65.0)

    """

    def __init__(self, name: str, score: float, min_score: float) -> None:
        """
        Construct and validate one band.

        Parameters
        ----------
        name : str
            Band label (e.g. ``"BBB+"``, ``"Baa1"``); outer whitespace is
            trimmed and the label must be nonblank and not an unrated status
            (``NR``, ``NOTRATED``, ``NOT RATED``, ``UNRATED``).
        score : float
            Representative score of the band, finite in ``[0, 100]`` and not
            below *min_score*.
        min_score : float
            Inclusive lower bound of the band, finite in ``[0, 100]``.

        Raises
        ------
        ValueError
            If *name* is blank or reserved, a score is non-finite or outside
            ``[0, 100]``, or *score* is below *min_score*.
        """
        ...

    def __eq__(self, other: object) -> bool:
        """Return whether two bands have the same name, score and min_score.

        Returns
        -------
        bool
        """
        ...
    @property
    def name(self) -> str:
        """
        Band label as supplied, outer whitespace trimmed.

        Returns
        -------
        str
            The band's label with outer whitespace removed, as matched by
            threshold and minimum-rating lookups.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    @property
    def score(self) -> float:
        """
        Representative score of the band on the 0-100 scale.

        Returns
        -------
        float
            Representative score.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    @property
    def min_score(self) -> float:
        """
        Inclusive lower bound of the band on the 0-100 scale.

        Returns
        -------
        float
            Lower bound.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    def to_json(self) -> str:
        """
        Serialize this band to a JSON string.

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
        Deserialize and validate a band from JSON.

        Parameters
        ----------
        json : str
            JSON document with ``name``, ``score`` and ``min_score``.

        Returns
        -------
        RatingLevel
            Validated `RatingLevel` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If *json* is malformed, has unknown or missing fields, or fails the
            constructor's validation.

        Examples
        --------
        >>> from finstack_quant.core.rating_scales import RatingLevel
        >>> level = RatingLevel("BBB", 70.0, 65.0)
        >>> RatingLevel.from_json(level.to_json()).name
        'BBB'

        """
        ...
    def __repr__(self) -> str:
        """Return a debug representation of this band.

        Returns
        -------
        str
        """
        ...

class ScorecardScale:
    """
    A named best-to-worst list of scorecard bands covering ``[0, 100]``.

    For minima ``m0 > m1 > ... > m_last = 0`` the best band is ``[m0, 100]``
    and each later band is ``[m_i, m_(i-1))``; every representative score
    classifies to its own band. Distinct from
    ``finstack_quant.models.credit.migration.RatingScale`` (which models the
    ordered state set of a credit-migration / transition matrix).

    Examples
    --------
    >>> from finstack_quant.core.rating_scales import RatingLevel, ScorecardScale
    >>> scale = ScorecardScale("custom", [RatingLevel("A", 90.0, 80.0), RatingLevel("B", 70.0, 0.0)])
    >>> ([level.name for level in scale], scale.classify(80.0).name, scale.level_position(" b "))
    (['A', 'B'], 'A', 1)

    """

    def __init__(
        self,
        scale_name: str,
        ratings: list[RatingLevel],
        description: str | None = None,
    ) -> None:
        """
        Construct and validate a scorecard scale.

        Parameters
        ----------
        scale_name : str
            Scale name; outer whitespace is trimmed and it must be nonblank.
        ratings : list[RatingLevel]
            Bands ordered best-to-worst: ``score`` and ``min_score`` strictly
            descend, labels are unique after trim and ASCII case folding, the
            worst band's ``min_score`` is ``0``, and each representative score
            is below the preceding band's minimum.
        description : str | None, default None
            Optional human-readable description.

        Raises
        ------
        ValueError
            If the name is blank, *ratings* is empty, labels collide, the order
            is not strictly best-to-worst, the worst minimum is not ``0``, or a
            representative score falls outside its band.
        """
        ...
    def __getitem__(self, index: int) -> RatingLevel:
        """
        Return the band at *index* (negative indices supported).

        Parameters
        ----------
        index : int
            Zero-based position, best band first.

        Returns
        -------
        RatingLevel
            The band at that position.

        Raises
        ------
        IndexError
            If *index* is out of range.
        """
        ...
    def __iter__(self) -> Iterator[RatingLevel]:
        """Iterate over bands best-to-worst.

        Returns
        -------
        Iterator[RatingLevel]
        """
        ...
    def __eq__(self, other: object) -> bool:
        """Return whether two scales have the same name, description and bands.

        Returns
        -------
        bool
        """
        ...
    def to_dataframe(self) -> pd.DataFrame:
        """
        Bands as a pandas ``DataFrame``.

        A presentation view with columns ``name``, ``score`` and ``min_score``
        (one row per band, best first), not a complete policy snapshot.

        Returns
        -------
        pandas.DataFrame
            One row per band.

        Raises
        ------
        ImportError
            If pandas is not installed.
        """
        ...
    def classify(self, score: float) -> RatingLevel:
        """
        Classify a score into its band.

        Returns the first band whose minimum the score meets; each boundary
        belongs to the better band. There is no epsilon, clipping or fallback.

        Parameters
        ----------
        score : float
            Score on the inclusive 0-100 scale.

        Returns
        -------
        RatingLevel
            The band the score falls in.

        Raises
        ------
        ValueError
            If *score* is non-finite or outside ``[0, 100]``.
        """
        ...
    def level_position(self, label: str) -> int:
        """
        Best-to-worst position of a band label (``0`` is best).

        Parameters
        ----------
        label : str
            Band label, matched after outer trim and ASCII case folding.

        Returns
        -------
        int
            Zero-based position of the band.

        Raises
        ------
        ValueError
            If the scale has no band with that label.
        """
        ...
    @property
    def scale_name(self) -> str:
        """
        Scale name, outer whitespace trimmed.

        Returns
        -------
        str
            The scale's display name with outer whitespace removed.

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
        str | None
            Description, or ``None`` when absent.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    @property
    def ratings(self) -> list[RatingLevel]:
        """
        Bands ordered best to worst.

        Returns
        -------
        list[RatingLevel]
            Copies of the scale's bands.

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
        Deserialize and validate a scale from JSON.

        Parameters
        ----------
        json : str
            JSON document with ``scale_name``, optional ``description`` and
            ``ratings``.

        Returns
        -------
        ScorecardScale
            Validated `ScorecardScale` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If *json* is malformed, has unknown or missing fields, or fails the
            constructor's validation.

        Examples
        --------
        >>> from finstack_quant.core.rating_scales import RatingLevel, ScorecardScale
        >>> scale = ScorecardScale("custom", [RatingLevel("A", 90.0, 80.0), RatingLevel("B", 70.0, 0.0)])
        >>> ScorecardScale.from_json(scale.to_json()) == scale
        True

        """
        ...
    def __len__(self) -> int:
        """Return the number of bands.

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

class RatingScaleEntry:
    """
    One registered scale with its normalized id and provenance.

    Examples
    --------
    >>> from finstack_quant.core.rating_scales import RatingLevel, RatingScaleEntry, ScorecardScale
    >>> scale = ScorecardScale("Internal", [RatingLevel("A", 90.0, 80.0), RatingLevel("B", 70.0, 0.0)])
    >>> entry = RatingScaleEntry(" Internal ", "credit policy 2026", scale)
    >>> (entry.id, entry.source, entry.scale.scale_name)
    ('internal', 'credit policy 2026', 'Internal')

    """

    def __init__(self, id: str, source: str, scale: ScorecardScale) -> None:
        """
        Construct and validate an entry.

        Parameters
        ----------
        id : str
            Scale id; normalized by outer trim and ASCII lowercasing and
            nonblank after normalization.
        source : str
            Provenance of the scale's numbers (policy, owner, vintage); outer
            whitespace trimmed and nonblank. A brand name alone is not a
            quantitative calibration.
        scale : ScorecardScale
            The scale definition.

        Raises
        ------
        ValueError
            If the normalized id or the source is blank.
        """
        ...
    @property
    def id(self) -> str:
        """
        Normalized scale id.

        Returns
        -------
        str
            Trimmed, ASCII-lowercase id.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    @property
    def source(self) -> str:
        """
        Provenance of the scale's numbers.

        Returns
        -------
        str
            Trimmed source text.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    @property
    def scale(self) -> ScorecardScale:
        """
        The scale definition.

        Returns
        -------
        ScorecardScale
            A copy of the entry's scale.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    def __eq__(self, other: object) -> bool:
        """Return whether two entries have the same id, source and scale.

        Returns
        -------
        bool
        """
        ...
    def __repr__(self) -> str:
        """Return a debug representation of this entry.

        Returns
        -------
        str
        """
        ...

class RatingScaleRegistry:
    """
    Caller-supplied registry of scorecard rating scales.

    Build one from entries, with ``RatingScaleRegistry.from_json(json)`` or
    with ``registry_from_config(config)``; resolve scales strictly by id.
    There is no embedded or default registry.

    Examples
    --------
    >>> from finstack_quant.core.rating_scales import (
    ...     RatingLevel,
    ...     RatingScaleEntry,
    ...     RatingScaleRegistry,
    ...     ScorecardScale,
    ... )
    >>> scale = ScorecardScale("Internal", [RatingLevel("A", 90.0, 80.0), RatingLevel("B", 70.0, 0.0)])
    >>> registry = RatingScaleRegistry([RatingScaleEntry("internal", "credit policy 2026", scale)])
    >>> (registry.scale_ids(), registry.rating_scale(" INTERNAL ").scale_name)
    (['internal'], 'Internal')

    """

    def __init__(self, rating_scales: list[RatingScaleEntry]) -> None:
        """
        Construct and validate a registry.

        Parameters
        ----------
        rating_scales : list[RatingScaleEntry]
            Entries in registry order; at least one, with unique ids.

        Raises
        ------
        ValueError
            If *rating_scales* is empty or ids repeat.
        """
        ...
    @property
    def rating_scales(self) -> list[RatingScaleEntry]:
        """
        Registered entries in registry order.

        Returns
        -------
        list[RatingScaleEntry]
            Copies of the entries.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """

    def scale_ids(self) -> list[str]:
        """
        Normalized id of every registered scale, in registry order.

        Returns
        -------
        list[str]
            Trimmed, ASCII-lowercase ids.

        Notes
        -----
        This method does not raise.
        """
        ...
    def is_known_rating_scale(self, id: str) -> bool:
        """
        Return whether *id* is registered after trim and ASCII lowercasing.

        Parameters
        ----------
        id : str
            Scale id to look up.

        Returns
        -------
        bool
            ``True`` when the registry holds the id.

        Notes
        -----
        This method does not raise.
        """
        ...
    def entry(self, id: str) -> RatingScaleEntry:
        """
        Resolve a registered entry by id.

        Parameters
        ----------
        id : str
            Scale id; outer whitespace and ASCII case are ignored. No alias or
            fallback is applied.

        Returns
        -------
        RatingScaleEntry
            The entry with that id.

        Raises
        ------
        ValueError
            If the registry has no scale with that id.
        """
        ...
    def rating_scale(self, id: str) -> ScorecardScale:
        """
        Resolve a registered scale by id.

        Parameters
        ----------
        id : str
            Scale id; outer whitespace and ASCII case are ignored. No alias or
            fallback is applied.

        Returns
        -------
        ScorecardScale
            The scale with that id.

        Raises
        ------
        ValueError
            If the registry has no scale with that id.
        """
        ...
    def to_json(self) -> str:
        """
        Serialize the registry to a JSON string.

        Returns
        -------
        str
            ``finstack_quant.rating_scales/2`` document, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    @classmethod
    def from_json(cls, json: str) -> RatingScaleRegistry:
        """
        Deserialize and validate a registry from JSON.

        Parameters
        ----------
        json : str
            ``finstack_quant.rating_scales/2`` document with a nonempty
            ``rating_scales`` list of ``{id, source, scale}`` entries.

        Returns
        -------
        RatingScaleRegistry
            Validated registry.

        Raises
        ------
        ValueError
            If *json* is malformed or the registry, an entry or a scale fails
            validation.

        Examples
        --------
        >>> from finstack_quant.core.rating_scales import (
        ...     RatingLevel,
        ...     RatingScaleEntry,
        ...     RatingScaleRegistry,
        ...     ScorecardScale,
        ... )
        >>> scale = ScorecardScale("Internal", [RatingLevel("A", 90.0, 80.0), RatingLevel("B", 70.0, 0.0)])
        >>> registry = RatingScaleRegistry([RatingScaleEntry("internal", "credit policy 2026", scale)])
        >>> RatingScaleRegistry.from_json(registry.to_json()) == registry
        True

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
        """Return whether two registries hold the same entries.

        Returns
        -------
        bool
        """
        ...

def registry_from_config(config: FinstackConfig) -> RatingScaleRegistry:
    """
    Load the rating-scale registry held in a configuration.

    Parameters
    ----------
    config : FinstackConfig
        Configuration whose :data:`RATING_SCALES_EXTENSION_KEY` extension holds
        the registry document.

    Returns
    -------
    RatingScaleRegistry
        The validated configured registry.

    Raises
    ------
    ValueError
        If the extension is absent, malformed or violates registry invariants;
        there is no embedded fallback.

    Examples
    --------
    >>> from finstack_quant.core.config import FinstackConfig
    >>> from finstack_quant.core.rating_scales import registry_from_config
    >>> try:
    ...     registry_from_config(FinstackConfig())
    ... except ValueError:
    ...     print("no registry configured")
    no registry configured

    """

RATING_SCALES_EXTENSION_KEY: str
"""Configuration-extension key holding a caller-supplied registry."""

__all__ = [
    "RATING_SCALES_EXTENSION_KEY",
    "RatingLevel",
    "RatingScaleEntry",
    "RatingScaleRegistry",
    "ScorecardScale",
    "registry_from_config",
]
