"""Stub declarations mirroring ``finstack-quant-py/src/bindings/core/market_data/curves/cube_expiry_slice.rs``."""

from __future__ import annotations

from typing import Any, Optional
import pandas as pd

class VolCubeExpirySlice:
    """Volatility grid at one fixed option expiry, indexed by tenor and strike.

    Black and shifted-Black values are annual decimal volatilities; normal
    values are absolute volatilities in forward-rate units. The fixed expiry
    is distinct from every underlying tenor.

    Examples
    --------
    >>> from finstack_quant.core.market_data import VolCubeExpirySlice
    >>> grid = VolCubeExpirySlice("SLICE", 1.0, [5.0], [0.02, 0.03], [0.2, 0.21])
    >>> (grid.expiry, grid.tenors, grid.grid_shape)
    (1.0, [5.0], (1, 2))
    """

    def __init__(
        self,
        id: str,
        expiry: float,
        tenors: list[float],
        strikes: list[float],
        vols: list[float],
        *,
        quote_type: str = "black_lognormal",
        displacements: list[float] | None = None,
    ) -> None:
        """Construct a validated tenor-major grid at one fixed expiry.

        Parameters
        ----------
        id : str
            Identifier of the materialized grid.
        expiry : float
            Fixed option expiry in years, finite and positive.
        tenors : list[float]
            Increasing positive underlying tenors in years.
        strikes : list[float]
            Increasing finite strike coordinates in forward-rate units.
        vols : list[float]
            Flat tenor-major grid with ``len(tenors) * len(strikes)`` entries,
            expressed in the declared quote convention.
        quote_type : str, default "black_lognormal"
            ``black_lognormal``, ``shifted_black_lognormal``, or ``normal``.
        displacements : list[float] | None, default None
            One rate-unit shift per tenor for shifted Black; absent otherwise.

        Raises
        ------
        ValueError
            If axes, dimensions, volatility values, convention, or shifts fail
            the canonical Rust validation.
        """
        ...

    @property
    def id(self) -> str:
        """Identifier of the materialized grid.

        Returns
        -------
        str
            Stable grid identifier.

        Notes
        -----
        This accessor does not raise; it returns the validated stored value.
        """
        ...

    @property
    def expiry(self) -> float:
        """Fixed option expiry in years.

        Returns
        -------
        float
            Option expiry, separate from the tenor axis.

        Notes
        -----
        This accessor does not raise; it returns the validated stored value.
        """
        ...

    @property
    def tenors(self) -> list[float]:
        """Underlying-tenor axis in years.

        Returns
        -------
        list[float]
            Copy of the increasing tenor coordinates.

        Notes
        -----
        This accessor does not raise; it returns the validated stored value.
        """
        ...

    @property
    def strikes(self) -> list[float]:
        """Strike axis in forward-rate units.

        Returns
        -------
        list[float]
            Copy of the increasing strike coordinates.

        Notes
        -----
        This accessor does not raise; it returns the validated stored value.
        """
        ...

    @property
    def vols(self) -> list[float]:
        """Flat tenor-major volatility grid.

        Returns
        -------
        list[float]
            Volatilities in the declared quote convention, with strike varying fastest.

        Notes
        -----
        This accessor does not raise; it returns the validated stored value.
        """
        ...

    @property
    def quote_type(self) -> str:
        """Convention used by the stored volatility values.

        Returns
        -------
        str
            One of ``black_lognormal``, ``shifted_black_lognormal``, or ``normal``.

        Notes
        -----
        This accessor does not raise; it returns the validated stored value.
        """
        ...

    @property
    def displacements(self) -> list[float] | None:
        """Shifted-Black displacements aligned to the tenor axis.

        Returns
        -------
        list[float] | None
            One shift in rate units per tenor, or ``None`` for unshifted quotes.

        Notes
        -----
        This accessor does not raise; it returns the validated stored value.
        """
        ...

    @property
    def grid_shape(self) -> tuple[int, int]:
        """Dimensions of the tenor-by-strike grid.

        Returns
        -------
        tuple[int, int]
            Pair ``(number of tenors, number of strikes)``.

        Notes
        -----
        This accessor does not raise; it returns the validated stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """Export the grid in long form, retaining the fixed option expiry.

        Returns
        -------
        pandas.DataFrame
            Columns ``expiry``, ``tenor``, ``strike``, and ``vol`` in tenor-major
            order. Time coordinates are years and quotes use ``quote_type``.

        Raises
        ------
        ImportError
            If pandas is unavailable.
        """
        ...

    def to_json(self) -> str:
        """Serialize the validated slice and all convention metadata.

        Returns
        -------
        str
            Canonical JSON accepted by Rust and ``from_json``.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    @staticmethod
    def from_json(json: str) -> VolCubeExpirySlice:
        """Load a fixed-expiry slice through canonical Rust validation.

        Parameters
        ----------
        json : str
            Canonical JSON produced by ``to_json`` or the Rust serializer.

        Returns
        -------
        VolCubeExpirySlice
            Immutable slice retaining its fixed expiry, tenor axis, and shifts.

        Raises
        ------
        ValueError
            If JSON is malformed, contains unknown fields, or fails validation.

        Examples
        --------
        >>> from finstack_quant.core.market_data import VolCubeExpirySlice
        >>> grid = VolCubeExpirySlice("S", 1.0, [5.0], [0.03], [0.2])
        >>> VolCubeExpirySlice.from_json(grid.to_json()).expiry
        1.0
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...
    def _repr_html_(self) -> Optional[str]: ...
