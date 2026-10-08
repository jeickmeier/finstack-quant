"""
Probability of default: PiT/TtC conversion, central-tendency calibration,
the Basel IRB floor, and rating master scales.

This submodule shadows the ``import pandas as pd`` alias; import it as
``from finstack_quant.models.credit import pd as pdm``.

Examples
--------
>>> from finstack_quant.models.credit import pd as pdm
>>> pdm.central_tendency([0.01, 0.02, 0.03])
0.02

"""

from __future__ import annotations

from typing import Any

import pandas

from finstack_quant.models.credit import scoring

__all__ = [
    "BASEL_IRB_PD_FLOOR",
    "MasterScale",
    "MasterScaleGrade",
    "MasterScaleResult",
    "apply_basel_irb_pd_floor",
    "central_tendency",
    "pit_to_ttc",
    "ttc_to_pit",
]

BASEL_IRB_PD_FLOOR: float
"""Basel IRB corporate PD floor, ``0.0003`` (3 bp) as a decimal."""

class MasterScaleGrade:
    """
    One PD band in a rating master scale.

    Examples
    --------
    >>> from finstack_quant.models.credit import pd as pdm
    >>> pdm.MasterScaleGrade("BBB", 0.005, 0.002).label
    'BBB'

    """

    def __init__(self, label: str, upper_pd: float, central_pd: float) -> None:
        """
        Construct one probability-of-default band on a master scale.

        Parameters
        ----------
        label : str
            Grade label (e.g. ``"BBB"``).
        upper_pd : float
            Inclusive upper PD bound of the band, a decimal in ``(0, 1]``.
        central_pd : float
            Representative PD assigned to the band, a decimal in ``(0, 1)``.

        Notes
        -----
        Construction does not raise; validation happens when the grade is
        placed in a ``MasterScale``.
        """
        ...

    @property
    def label(self) -> str:
        """
        Grade label (e.g. ``"BBB"``).

        Returns
        -------
        str
            Grade label (e.g. ``"BBB"``).

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def upper_pd(self) -> float:
        """
        Inclusive upper PD bound of the band (decimal).

        Returns
        -------
        float
            Inclusive upper PD bound of the band (decimal).

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def central_pd(self) -> float:
        """
        Representative PD assigned to anything falling in the band (decimal).

        Returns
        -------
        float
            Representative PD assigned to anything falling in the band (decimal).

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @staticmethod
    def from_json(json: str) -> MasterScaleGrade:
        """
        Deserialize a ``MasterScaleGrade`` from its canonical JSON form.

        Parameters
        ----------
        json : str
            JSON text produced by :meth:`to_json` (strict serde; unknown or
            invalid fields are rejected).

        Returns
        -------
        MasterScaleGrade
            Reconstructed value.

        Raises
        ------
        ValueError
            If the payload is not valid ``MasterScaleGrade`` JSON.

        Examples
        --------
        >>> from finstack_quant.models.credit import pd
        >>> value = pd.MasterScaleGrade("BBB", 0.005, 0.002)
        >>> pd.MasterScaleGrade.from_json(value.to_json()) == value
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to compact canonical JSON.

        Returns
        -------
        str
            JSON text accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the canonical JSON representation.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(from_json, (json,))`` so unpickling rebuilds the value.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style representation rendered from the canonical fields.

        Returns
        -------
        str
            ``Name(field=value, ...)`` with Python literals.

        Notes
        -----
        This method does not raise.
        """
        ...

class MasterScaleResult:
    """
    Result of mapping a PD onto a master scale.

    Examples
    --------
    >>> from finstack_quant.models.credit import pd as pdm
    >>> result = pdm.MasterScale.sp_assumptions().map_pd(0.003)
    >>> (result.grade, result.grade_index, result.central_pd)
    ('BBB', 3, 0.002)

    """

    @property
    def grade(self) -> str:
        """
        Label of the grade the PD mapped into.

        Returns
        -------
        str
            Label of the grade the PD mapped into.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def central_pd(self) -> float:
        """
        Central PD of the assigned grade (the notched value).

        Returns
        -------
        float
            Central PD of the assigned grade (the notched value).

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def input_pd(self) -> float:
        """
        The PD that was mapped, before notching.

        Returns
        -------
        float
            The PD that was mapped, before notching.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def grade_index(self) -> int:
        """
        Zero-based index of the assigned grade in the scale.

        Returns
        -------
        int
            Zero-based index of the assigned grade in the scale.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @staticmethod
    def from_json(json: str) -> MasterScaleResult:
        """
        Deserialize a ``MasterScaleResult`` from its canonical JSON form.

        Parameters
        ----------
        json : str
            JSON text produced by :meth:`to_json` (strict serde; unknown or
            invalid fields are rejected).

        Returns
        -------
        MasterScaleResult
            Reconstructed value.

        Raises
        ------
        ValueError
            If the payload is not valid ``MasterScaleResult`` JSON.

        Examples
        --------
        >>> from finstack_quant.models.credit import pd
        >>> value = pd.MasterScale.sp_assumptions().map_pd(0.003)
        >>> pd.MasterScaleResult.from_json(value.to_json()) == value
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to compact canonical JSON.

        Returns
        -------
        str
            JSON text accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the canonical JSON representation.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(from_json, (json,))`` so unpickling rebuilds the value.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...
    def to_dataframe(self) -> pandas.DataFrame:
        """
        Single-row frame with ``grade``, ``grade_index``, ``input_pd``, ``central_pd``.

        Returns
        -------
        pandas.DataFrame
            Single-row frame with ``grade``, ``grade_index``, ``input_pd``, ``central_pd``.

        Raises
        ------
        ValueError
            If the value cannot be serialized into a pandas object.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style representation rendered from the canonical fields.

        Returns
        -------
        str
            ``Name(field=value, ...)`` with Python literals.

        Notes
        -----
        This method does not raise.
        """
        ...

class MasterScale:
    """
    Ordered PD bands mapping a continuous PD onto discrete rating grades.

    Bands must be strictly increasing in ``upper_pd`` and each grade's
    ``central_pd`` must fall inside its own band; PDs are decimals in
    ``[0, 1]``.

    Examples
    --------
    >>> from finstack_quant.models.credit import pd as pdm
    >>> scale = pdm.MasterScale.sp_assumptions()
    >>> (scale.n_grades, scale.map_pd(0.003).grade)
    (8, 'BBB')

    """

    def __init__(self, grades: list[MasterScaleGrade]) -> None:
        """
        Build a master scale from ordered grades.

        Parameters
        ----------
        grades : list[MasterScaleGrade]
            Bands in ascending ``upper_pd`` order, strongest grade first.

        Raises
        ------
        ValueError
            If the list is empty, a PD lies outside its valid range, or
            the bands are not strictly ascending.
        """
        ...

    @staticmethod
    def sp_assumptions() -> MasterScale:
        """
        Library PD-band assumptions using S&P-style labels.

        The labels resemble S&P notation as a reporting convention only;
        the boundaries and central PDs are library assumptions, not
        agency-published statistics.

        Returns
        -------
        MasterScale
            Eight-grade scale from ``AAA`` to ``CC/C``.

        Raises
        ------
        ValueError
            If the embedded credit registry is invalid.

        Examples
        --------
        >>> from finstack_quant.models.credit import pd as pdm
        >>> pdm.MasterScale.sp_assumptions().n_grades
        8
        """
        ...

    @staticmethod
    def moodys_assumptions() -> MasterScale:
        """
        Library PD-band assumptions using Moody's-style labels.

        As with :meth:`sp_assumptions`, the labels are a reporting
        convention rather than an agency calibration.

        Returns
        -------
        MasterScale
            Moody's-labelled library scale.

        Raises
        ------
        ValueError
            If the embedded credit registry is invalid.

        Examples
        --------
        >>> from finstack_quant.models.credit import pd as pdm
        >>> pdm.MasterScale.moodys_assumptions().n_grades > 0
        True
        """
        ...

    @staticmethod
    def from_registry_id(scale_id: str) -> MasterScale:
        """
        Load a master scale by id from the embedded credit registry.

        Parameters
        ----------
        scale_id : str
            Registry identifier of the scale.

        Returns
        -------
        MasterScale
            The registry scale.

        Raises
        ------
        KeyError
            If ``scale_id`` is unknown.
        ValueError
            If the registry is invalid.

        Examples
        --------
        >>> from finstack_quant.models.credit import pd as pdm
        >>> isinstance(pdm.MasterScale.from_registry_id("sp_assumptions"), pdm.MasterScale)
        True
        """
        ...

    def map_pd(self, pd: float) -> MasterScaleResult:
        """
        Map a PD onto its rating grade.

        The first band whose inclusive ``upper_pd`` covers ``pd`` wins.

        Parameters
        ----------
        pd : float
            Probability of default as a decimal in ``[0, 1]``.

        Returns
        -------
        MasterScaleResult
            Assigned grade, its index, the input and central PD.

        Raises
        ------
        ValueError
            If ``pd`` is non-finite or outside ``[0, 1]`` (a percent /
            decimal mix-up such as ``5.0`` is rejected, not clamped).
        """
        ...

    def map_pds(self, pds: list[float]) -> pandas.DataFrame:
        """
        Map several PDs and return one grading table.

        Parameters
        ----------
        pds : list[float]
            Probabilities of default as decimals in ``[0, 1]``.

        Returns
        -------
        pandas.DataFrame
            Columns ``grade``, ``grade_index``, ``input_pd``,
            ``central_pd``; one row per input in input order.

        Raises
        ------
        ValueError
            If any PD is non-finite or outside ``[0, 1]``.
        """
        ...

    def map_score(self, result: scoring.ScoringResult) -> MasterScaleResult:
        """
        Map a scoring result's implied PD onto its rating grade.

        Parameters
        ----------
        result : ScoringResult
            Output of a ``scoring`` model carrying an ``implied_pd``
            (Ohlson, Zmijewski).

        Returns
        -------
        MasterScaleResult
            Grade assigned to ``result.implied_pd``.

        Raises
        ------
        ValueError
            If the result has no implied PD (Altman family) or the PD is
            non-finite.
        """
        ...

    @property
    def n_grades(self) -> int:
        """
        Number of grades in the scale.

        Returns
        -------
        int
            Number of grades in the scale.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def grades(self) -> list[MasterScaleGrade]:
        """
        The scale's grades, in ascending PD order.

        Returns
        -------
        list[pd.MasterScaleGrade]
            The scale's grades, in ascending PD order.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    def to_dataframe(self) -> pandas.DataFrame:
        """
        Frame with ``label``, ``upper_pd``, ``central_pd``; one row per grade in ascending PD order.

        Returns
        -------
        pandas.DataFrame
            Frame with ``label``, ``upper_pd``, ``central_pd``; one row per grade in ascending PD order.

        Raises
        ------
        ValueError
            If the value cannot be serialized into a pandas object.
        """
        ...
    @staticmethod
    def from_json(json: str) -> MasterScale:
        """
        Deserialize a ``MasterScale`` from its canonical JSON form.

        Parameters
        ----------
        json : str
            JSON text produced by :meth:`to_json` (strict serde; unknown or
            invalid fields are rejected).

        Returns
        -------
        MasterScale
            Reconstructed value.

        Raises
        ------
        ValueError
            If the payload is not valid ``MasterScale`` JSON.

        Examples
        --------
        >>> from finstack_quant.models.credit import pd
        >>> value = pd.MasterScale.sp_assumptions()
        >>> pd.MasterScale.from_json(value.to_json()).to_json() == value.to_json()
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to compact canonical JSON.

        Returns
        -------
        str
            JSON text accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the canonical JSON representation.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(from_json, (json,))`` so unpickling rebuilds the value.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...
    def __len__(self) -> int:
        """
        Number of grades in the scale.

        Returns
        -------
        int
            Same as :attr:`n_grades`.

        Notes
        -----
        This method does not raise.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style representation rendered from the canonical fields.

        Returns
        -------
        str
            ``Name(field=value, ...)`` with Python literals.

        Notes
        -----
        This method does not raise.
        """
        ...

def pit_to_ttc(pd_pit: float, asset_correlation: float, cycle_index: float) -> float:
    """
    Convert a Point-in-Time PD to a Through-the-Cycle PD.

    Merton-Vasicek single-factor model (Basel II IRB):
    ``PD_TtC = Phi(Phi^-1(PD_PiT) * sqrt(1 - rho) + sqrt(rho) * z)``.

    Parameters
    ----------
    pd_pit : float
        Point-in-Time PD as a decimal in ``(0, 1)``.
    asset_correlation : float
        Asset correlation ``rho`` in ``(0, 1)``; Basel uses 0.12-0.24 for
        corporates.
    cycle_index : float
        Systematic factor ``z``: ``0`` average, ``< 0`` downturn, ``> 0``
        benign.

    Returns
    -------
    float
        Through-the-Cycle PD as a decimal.

    Raises
    ------
    ValueError
        If ``pd_pit`` or ``asset_correlation`` is outside ``(0, 1)`` or any
        input is non-finite.

    Examples
    --------
    >>> from finstack_quant.models.credit import pd as pdm
    >>> round(pdm.pit_to_ttc(0.03, 0.12, -1.0), 4)
    0.0174
    """
    ...

def ttc_to_pit(pd_ttc: float, asset_correlation: float, cycle_index: float) -> float:
    """
    Convert a Through-the-Cycle PD to a Point-in-Time PD.

    Merton-Vasicek single-factor model (Basel II IRB):
    ``PD_PiT = Phi((Phi^-1(PD_TtC) - sqrt(rho) * z) / sqrt(1 - rho))``.

    Parameters
    ----------
    pd_ttc : float
        Through-the-Cycle PD as a decimal in ``(0, 1)``.
    asset_correlation : float
        Asset correlation ``rho`` in ``(0, 1)``.
    cycle_index : float
        Systematic factor ``z``: ``0`` average, ``< 0`` downturn, ``> 0``
        benign.

    Returns
    -------
    float
        Point-in-Time PD as a decimal.

    Raises
    ------
    ValueError
        If ``pd_ttc`` or ``asset_correlation`` is outside ``(0, 1)`` or any
        input is non-finite.

    Examples
    --------
    >>> from finstack_quant.models.credit import pd as pdm
    >>> pdm.ttc_to_pit(0.02, 0.12, -1.0) > 0.02
    True
    """
    ...

def central_tendency(annual_default_rates: list[float]) -> float:
    """
    Long-run average PD from annual default rates (arithmetic mean).

    This is the standard regulatory TtC approach (Basel IRB, EBA
    GL/2017/16); zero-default years are valid observations.

    Parameters
    ----------
    annual_default_rates : list[float]
        Observed annual default rates as decimals in ``[0, 1]``; at least
        one.

    Returns
    -------
    float
        Arithmetic mean in ``[0, 1]``.

    Raises
    ------
    ValueError
        If the list is empty or any rate is non-finite or outside
        ``[0, 1]``.

    Examples
    --------
    >>> from finstack_quant.models.credit import pd as pdm
    >>> pdm.central_tendency([0.01, 0.02, 0.03])
    0.02
    """
    ...

def apply_basel_irb_pd_floor(pd: float) -> float:
    """
    Apply the Basel IRB corporate PD floor: ``max(pd, BASEL_IRB_PD_FLOOR)``.

    Parameters
    ----------
    pd : float
        Probability of default as a decimal.

    Returns
    -------
    float
        The floored PD (``0.0003`` when ``pd`` is below 3 bp).

    Notes
    -----
    This function does not raise.

    Examples
    --------
    >>> from finstack_quant.models.credit import pd as pdm
    >>> pdm.apply_basel_irb_pd_floor(0.0001)
    0.0003
    """
    ...
