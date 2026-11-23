"""Stub declarations mirroring ``finstack-quant-py/src/bindings/models/credit/rating.rs``."""

from __future__ import annotations

from finstack_quant.core.types import CreditRating
import datetime
import numpy as np
from numpy.typing import NDArray
import pandas
import polars as pl
from finstack_quant.core.dates import DayCount
from finstack_quant.core.market_data import HazardCurve
from finstack_quant.core.table import ArrowTable, LabeledSeries
from typing import Any

class RatingFactorTable:
    """
    Rating-factor table (Moody's WARF methodology) from the embedded credit registry.

    Examples
    --------
    >>> from finstack_quant.models.credit import RatingFactorTable
    >>> table = RatingFactorTable.default_calibration()
    >>> (table.get_factor("B"), table.agency)
    (2720.0, "Moody's")

    """

    @staticmethod
    def default_calibration() -> RatingFactorTable:
        """
        Load the calibration selected by the embedded registry's default ID.

        Returns
        -------
        RatingFactorTable
            Validated calibration selected by the current registry data.

        Raises
        ------
        ValueError
            If the embedded registry or its configured default is malformed.

        Examples
        --------
        >>> from finstack_quant.models.credit import RatingFactorTable
        >>> isinstance(RatingFactorTable.default_calibration(), RatingFactorTable)
        True
        """
        ...

    @staticmethod
    def from_registry_id(id: str) -> RatingFactorTable:
        """
        Load a named rating-factor table from the embedded registry.

        Parameters
        ----------
        id : str
            Exact registry identifier of the methodology; an unknown id never
            falls back to the default table.

        Returns
        -------
        RatingFactorTable
            The named table.

        Raises
        ------
        KeyError
            If ``id`` is unknown.
        ValueError
            If the embedded registry is invalid.

        Examples
        --------
        >>> from finstack_quant.models.credit import RatingFactorTable
        >>> RatingFactorTable.from_registry_id("moodys_standard").agency
        "Moody's"
        """
        ...

    def get_factor(self, rating: str | CreditRating) -> float:
        """
        Rating factor for an exact rating notch.

        Parameters
        ----------
        rating : str | CreditRating
            Rating notch as a ``core.types.CreditRating`` or a rating string
            (``"BBB-"``, ``"Baa3"``).

        Returns
        -------
        float
            Rating factor.

        Raises
        ------
        ValueError
            If the notch has no factor in this table or the string is not a
            recognised rating.
        TypeError
            If ``rating`` is neither a string nor a ``CreditRating``.
        """
        ...

    @property
    def agency(self) -> str:
        """
        Agency the table is sourced from (e.g. ``"moodys"``).

        Returns
        -------
        str
            Agency label.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def methodology(self) -> str:
        """
        Methodology label recorded in the registry for this table.

        Returns
        -------
        str
            Methodology label.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def default_factor(self) -> float:
        """
        Factor assigned to unrated or defaulted names.

        Returns
        -------
        float
            Default rating factor.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @staticmethod
    def from_json(json: str) -> RatingFactorTable:
        """
        Deserialize a ``RatingFactorTable`` from its canonical JSON form.

        Parameters
        ----------
        json : str
            JSON text produced by :meth:`to_json` (strict serde; unknown or
            invalid fields are rejected).

        Returns
        -------
        RatingFactorTable
            Reconstructed value.

        Raises
        ------
        ValueError
            If the payload is not valid ``RatingFactorTable`` JSON.

        Examples
        --------
        >>> from finstack_quant.models.credit import RatingFactorTable
        >>> value = RatingFactorTable.default_calibration()
        >>> RatingFactorTable.from_json(value.to_json()).to_json() == value.to_json()
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

def moodys_warf_factor(rating: str | CreditRating) -> float:
    """
    Return the Moody's WARF factor for an exact canonical credit-rating notch.

    Parameters
    ----------
    rating : str | CreditRating
        Canonical rating from :mod:`finstack_quant.core.types`, or a rating
        string in S&P/Fitch (``"BBB-"``) or Moody's (``"Baa3"``) notation.

    Returns
    -------
    float
        Moody's ordinal weighted-average rating factor.

    Raises
    ------
    ValueError
        If the string is not a recognised rating, the embedded
        credit-assumptions registry is invalid, or the rating has no factor in
        the configured Moody's table.
    TypeError
        If ``rating`` is neither a string nor a ``CreditRating``.

    Examples
    --------
    >>> from finstack_quant.core.types import CreditRating
    >>> from finstack_quant.models.credit import moodys_warf_factor
    >>> moodys_warf_factor(CreditRating.B)
    2720.0
    >>> moodys_warf_factor("B")
    2720.0
    """
    ...
