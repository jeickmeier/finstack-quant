"""Stub declarations mirroring ``finstack-quant-py/src/bindings/core/market_data/curves/surfaces.rs``."""

from __future__ import annotations

from typing import Any, Optional, Sequence, Union
import pandas as pd

class VolSurface:
    """
    Two-dimensional implied volatility surface on an expiry x strike grid.

    Volatilities are decimal annualised standard deviations (``0.20`` is 20%)
    stored row-major by expiry. The secondary axis is a strike, tenor or
    moneyness coordinate depending on ``secondary_axis``.

    Examples
    --------
    >>> from finstack_quant.core.market_data import VolSurface
    >>> surface = VolSurface("EQ-VOL", [1.0, 2.0], [90.0, 100.0, 110.0], [[0.22, 0.20, 0.21], [0.23, 0.21, 0.22]])
    >>> surface.grid_shape
    (2, 3)
    >>> from finstack_quant.models.volatility import get_surface_vol
    >>> round(get_surface_vol(surface, 1.5, 100.0), 4)
    0.205
    >>> surface.vols[0]
    [0.22, 0.2, 0.21]

    """

    def __init__(
        self,
        id: str,
        expiries: Sequence[float],
        strikes: Sequence[float],
        vols: Any,
        *,
        secondary_axis: str = "strike",
        interpolation_mode: str = "vol",
        quote_type: str = "black_lognormal",
        displacements: list[float] | None = None,
    ) -> None:
        """
        Construct a vol surface from an expiry x strike grid.

        Parameters
        ----------
        id : str
            Unique surface identifier.
        expiries : Sequence[float]
            Strictly increasing expiry times in years.
        strikes : Sequence[float]
            Strictly increasing secondary-axis coordinates.
        vols : Sequence[float], Sequence[Sequence[float]] or numpy.ndarray
            Volatilities as decimals, either flat row-major
            (``len(expiries) * len(strikes)``) or as ``len(expiries)`` rows of
            ``len(strikes)`` values (a 2-D array is accepted).
        secondary_axis : str, optional
            ``"strike"`` (default) or ``"tenor"``.
        interpolation_mode : str, optional
            ``"vol"`` (default, bilinear in vol) or ``"total_variance"``.
        quote_type : str, optional
            ``"black_lognormal"`` (default), ``"shifted_black_lognormal"``, or ``"normal"``.
        displacements : list[float] | None, optional
            One finite rate-unit displacement per expiry for shifted Black;
            absent for other quote conventions.

        Raises
        ------
        ValueError
            If the grid size does not match the axes, an axis is not strictly
            increasing, a vol is non-finite or negative, or a label is unknown.
        TypeError
            If ``vols`` is not a list, nested list or array-like.

        Examples
        --------
        >>> from finstack_quant.core.market_data import VolSurface
        >>> VolSurface("EQ-VOL", [1.0], [90.0, 100.0], [0.22, 0.20]).strikes
        [90.0, 100.0]

        """
        ...

    @staticmethod
    def from_json(json: str) -> VolSurface:
        """
        Deserialize a surface from its canonical JSON wire form.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        VolSurface
            The constructed volatility surface, ready for interpolated vol queries.

        Raises
        ------
        ValueError
            If the JSON is malformed, has unknown fields, or fails validation.

        Examples
        --------
        >>> from finstack_quant.core.market_data import VolSurface
        >>> surface = VolSurface("EQ-VOL", [1.0], [90.0, 100.0], [0.22, 0.20])
        >>> VolSurface.from_json(surface.to_json()) == surface
        True

        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON wire form.

        Returns
        -------
        str
            Compact JSON accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If the surface cannot be serialized.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the grid in long form.

        Returns
        -------
        pd.DataFrame
            Columns ``expiry``, ``strike`` and ``vol``; one row per grid node,
            expiries outer and strikes inner.

        Raises
        ------
        ValueError
            If the frame cannot be built.
        """
        ...

    @property
    def id(self) -> str:
        """
        Identifier this surface is registered and looked up under.

        Returns
        -------
        str
            The surface id supplied at construction; market-data containers key on it exactly.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def expiries(self) -> list[float]:
        """
        Option expiries the surface is quoted on.

        Returns
        -------
        list[float]
            Ascending expiry year fractions; they index the rows of the stored volatility grid.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def strikes(self) -> list[float]:
        """
        Second grid axis of the surface.

        Returns
        -------
        list[float]
            Ascending axis values whose meaning is given by :attr:`secondary_axis`: absolute strikes, swap tenors in years, or moneyness.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def vols(self) -> list[list[float]]:
        """
        Quoted volatilities on the expiry/strike grid.

        Returns
        -------
        list[list[float]]
            Row-major grid of decimal volatilities (``0.20`` is 20%) in the convention named by :attr:`quote_type`; one row per expiry.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def secondary_axis(self) -> str:
        """
        What the second axis of the grid measures.

        Returns
        -------
        str
            Either ``"strike"`` (absolute strike levels) or ``"tenor"`` (underlying swap tenor in years).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def quote_type(self) -> str:
        """
        Volatility convention the grid values are quoted in.

        Returns
        -------
        str
            ``"black_lognormal"`` or ``"shifted_black_lognormal"`` for decimal Black
            vol, or ``"normal"`` for absolute Bachelier vol in rate units.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def displacements(self) -> list[float] | None:
        """Return the shifted-Black displacement attached to each expiry.

        Returns
        -------
        list[float] | None
            Rate-unit displacements aligned to ``expiries``, or ``None`` for
            unshifted quotes.

        Notes
        -----
        This accessor does not raise; it returns the stored metadata.
        """
        ...

    def with_displacements(self, displacements: list[float]) -> VolSurface:
        """Return a surface with validated shifted-Black quote metadata.

        Parameters
        ----------
        displacements : list[float]
            Finite rate-unit shifts, one per expiry, compatible with the
            stored strike coordinates.

        Returns
        -------
        VolSurface
            New immutable surface with shifted-Black quote convention.

        Raises
        ------
        ValueError
            If the displacement count or shifted strike domains are invalid.
        """
        ...

    @property
    def interpolation_mode(self) -> str:
        """
        Quantity interpolated between grid nodes.

        Returns
        -------
        str
            Either ``"vol"`` (interpolate volatilities directly) or ``"total_variance"`` (interpolate ``vol**2 * T``, which is arbitrage-safer in expiry).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def grid_shape(self) -> tuple[int, int]:
        """
        Dimensions of the stored volatility grid.

        Returns
        -------
        tuple[int, int]
            ``(n_expiries, n_strikes)``: the row and column counts of the grid, useful for reshaping the flat value list.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...
    def _repr_html_(self) -> Optional[str]: ...

class FxDeltaVolSurface:
    """
    Delta-quoted FX volatility surface (ATM, 25-delta RR/BF, optional 10-delta wings).

    Uses forward delta (premium-unadjusted). Quotes are decimal vols
    (``0.08`` is 8%); risk reversals are call vol minus put vol and
    butterflies are average wing vol minus ATM.

    This type stores quotes only. Recover pillar vols with
    :func:`finstack_quant.models.volatility.get_fx_delta_pillar_vols`,
    evaluate a strike with
    :func:`finstack_quant.models.volatility.get_fx_delta_vol`, or
    materialize a strike-axis :class:`VolSurface` with
    :func:`finstack_quant.models.volatility.materialize_fx_delta_surface`.
    There is no Python quote-builder class; those helpers are the conversion
    surface.

    Examples
    --------
    >>> from finstack_quant.core.market_data import FxDeltaVolSurface
    >>> surface = FxDeltaVolSurface("EURUSD", [0.25, 1.0], [0.08, 0.09], [0.01, 0.015], [0.005, 0.007])
    >>> (len(surface.expiries), surface.rr_10d)
    (2, None)

    """

    def __init__(
        self,
        id: str,
        expiries: Sequence[float],
        atm_vols: Sequence[float],
        rr_25d: Sequence[float],
        bf_25d: Sequence[float],
        rr_10d: Optional[Sequence[float]] = None,
        bf_10d: Optional[Sequence[float]] = None,
    ) -> None:
        """
        Construct an FX delta-quoted vol surface with 25-delta wings.

        Parameters
        ----------
        id : str
            Unique surface identifier.
        expiries : Sequence[float]
            Strictly increasing positive expiry times in years.
        atm_vols : Sequence[float]
            ATM delta-neutral straddle vols per expiry (decimal, positive).
        rr_25d : Sequence[float]
            25-delta risk reversal per expiry (decimal, call vol - put vol).
        bf_25d : Sequence[float]
            25-delta butterfly per expiry (decimal, wing average - ATM).
        rr_10d : Sequence[float], optional
            10-delta risk reversal per expiry; requires ``bf_10d``.
        bf_10d : Sequence[float], optional
            10-delta butterfly per expiry; requires ``rr_10d``.

        Raises
        ------
        ValueError
            If only one of ``rr_10d`` / ``bf_10d`` is given, any vector is
            empty or mismatched in length, expiries are not strictly
            increasing and positive, or a quote is non-finite.

        Examples
        --------
        >>> from finstack_quant.core.market_data import FxDeltaVolSurface
        >>> FxDeltaVolSurface("EURUSD", [1.0], [0.09], [0.015], [0.007]).atm_vols
        [0.09]

        """
        ...

    @staticmethod
    def from_json(json: str) -> FxDeltaVolSurface:
        """
        Deserialize a surface from its canonical JSON wire form.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        FxDeltaVolSurface

        Raises
        ------
        ValueError
            If the JSON is malformed, has unknown fields, or fails validation.

        Examples
        --------
        >>> from finstack_quant.core.market_data import FxDeltaVolSurface
        >>> surface = FxDeltaVolSurface("EURUSD", [1.0], [0.09], [0.015], [0.007])
        >>> FxDeltaVolSurface.from_json(surface.to_json()) == surface
        True

        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON wire form.

        Returns
        -------
        str
            Compact JSON accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If the surface cannot be serialized.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export pillars as a pandas ``DataFrame``.

        Returns
        -------
        pd.DataFrame
            Columns ``expiry``, ``atm_vol``, ``rr_25d``, ``bf_25d`` and, when
            10-delta wings are stored, ``rr_10d`` and ``bf_10d``.

        Raises
        ------
        ValueError
            If the frame cannot be built.
        """
        ...

    @property
    def id(self) -> str:
        """
        Identifier this surface is registered and looked up under.

        Returns
        -------
        str
            The surface id supplied at construction; market-data containers key on it exactly.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def expiries(self) -> list[float]:
        """
        Option expiries the surface is quoted on.

        Returns
        -------
        list[float]
            Ascending expiry year fractions; they index the rows of the stored volatility grid.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def atm_vols(self) -> list[float]:
        """
        At-the-money volatility term structure.

        Returns
        -------
        list[float]
            One decimal delta-neutral-straddle volatility per expiry (``0.12`` is 12%), aligned with :attr:`expiries`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def rr_25d(self) -> list[float]:
        """
        25-delta skew quotes by expiry.

        Returns
        -------
        list[float]
            Call-minus-put 25-delta volatility differences as decimals, one per expiry; positive values mean calls trade above puts.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def bf_25d(self) -> list[float]:
        """
        25-delta smile curvature quotes by expiry.

        Returns
        -------
        list[float]
            Market-strangle butterfly quotes as decimals, one per expiry; they lift the 25-delta wings above the ATM level.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def rr_10d(self) -> Optional[list[float]]:
        """
        10-delta skew quotes by expiry, when supplied.

        Returns
        -------
        Optional[list[float]]
            Call-minus-put 10-delta volatility differences as decimals, one per expiry, or ``None`` when the wings were not quoted.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def bf_10d(self) -> Optional[list[float]]:
        """
        10-delta smile curvature quotes by expiry, when supplied.

        Returns
        -------
        Optional[list[float]]
            10-delta butterfly quotes as decimals, one per expiry, or ``None`` when the wings were not quoted.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...
    def _repr_html_(self) -> Optional[str]: ...

class SabrParameterData:
    """
    Calibrated SABR parameters for one vol-cube node.

    Examples
    --------
    >>> from finstack_quant.core.market_data import SabrParameterData
    >>> p = SabrParameterData(0.02, 0.5, 0.3, -0.2)
    >>> (p.alpha, p.beta, p.rho, p.nu, p.shift)
    (0.02, 0.5, -0.2, 0.3, None)

    """

    def __init__(self, alpha: float, beta: float, nu: float, rho: float, shift: Optional[float] = None) -> None:
        """
        Construct a validated SABR parameter node.

        Parameters
        ----------
        alpha : float
            Initial volatility level; strictly positive.
        beta : float
            CEV exponent in ``[0, 1]``.
        nu : float
            Volatility of volatility; nonnegative, with zero giving deterministic volatility.
        rho : float
            Forward/volatility correlation in ``(-1, 1)``.
        shift : float, optional
            Displacement added to forward and strike (decimal rate units).

        Raises
        ------
        ValueError
            If any value is non-finite or outside its range.

        Examples
        --------
        >>> from finstack_quant.core.market_data import SabrParameterData
        >>> SabrParameterData(0.02, 0.5, 0.3, -0.2, shift=0.03).shift
        0.03

        """
        ...

    @staticmethod
    def from_json(json: str) -> SabrParameterData:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        SabrParameterData

        Raises
        ------
        ValueError
            If the JSON is malformed or the parameters fail validation.

        Examples
        --------
        >>> from finstack_quant.core.market_data import SabrParameterData
        >>> p = SabrParameterData(0.02, 0.5, 0.3, -0.2)
        >>> SabrParameterData.from_json(p.to_json()) == p
        True

        """
        ...

    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            ``{"alpha": ..., "beta": ..., "rho": ..., "nu": ...}`` plus ``shift`` when set.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    @property
    def alpha(self) -> float:
        """
        SABR ``alpha``: the level of instantaneous volatility.

        Returns
        -------
        float
            Strictly positive volatility scale at the forward; its units follow the ``beta`` convention (decimal lognormal at ``beta = 1``).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def beta(self) -> float:
        """
        SABR ``beta``: the backbone exponent.

        Returns
        -------
        float
            Dimensionless exponent in ``[0, 1]``; ``1.0`` gives a lognormal backbone, ``0.0`` a normal one, ``0.5`` the CIR-style middle case.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def rho(self) -> float:
        """
        SABR ``rho``: correlation between forward and volatility.

        Returns
        -------
        float
            Dimensionless correlation in ``(-1, 1)``; negative values produce a downward-sloping skew.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def nu(self) -> float:
        """
        SABR ``nu``: the volatility of the volatility process.

        Returns
        -------
        float
            Strictly positive vol-of-vol per square root of a year; it controls smile curvature.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def shift(self) -> Optional[float]:
        """
        Shift applied before the SABR expansion.

        Returns
        -------
        Optional[float]
            Displacement added to both forward and strike in the forward's rate units, allowing negative rates, or ``None`` for the unshifted model.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...

class VolCube:
    """
    SABR volatility cube on an expiry x tenor grid.

    Each node stores calibrated SABR parameters and the forward swap rate
    (decimal) for that expiry/tenor pair, row-major by expiry.

    Examples
    --------
    >>> from finstack_quant.core.market_data import SabrParameterData, VolCube
    >>> p = SabrParameterData(0.02, 0.5, 0.3, -0.2)
    >>> cube = VolCube("USD-SWPT", [1.0, 2.0], [5.0], [p, p], [0.03, 0.032])
    >>> (cube.grid_shape, cube.forward_at(1, 0), cube.params_at(0, 0).alpha)
    ((2, 1), 0.032, 0.02)

    """

    def __init__(
        self,
        id: str,
        expiries: Sequence[float],
        tenors: Sequence[float],
        params_row_major: Sequence[Union[SabrParameterData, dict[str, float]]],
        forwards_row_major: Sequence[float],
        interpolation_mode: Optional[str] = None,
    ) -> None:
        """
        Construct a vol cube from row-major grid data.

        Parameters
        ----------
        id : str
            Unique cube identifier.
        expiries : Sequence[float]
            Option expiry axis in years, strictly increasing.
        tenors : Sequence[float]
            Underlying swap tenor axis in years, strictly increasing.
        params_row_major : Sequence[SabrParameterData or dict]
            ``len(expiries) * len(tenors)`` SABR nodes, row-major by expiry.
            Dicts use keys ``"alpha"``, ``"beta"``, ``"rho"``, ``"nu"`` and
            optionally ``"shift"`` (the Rust ``SabrParameterData`` wire
            shape); missing or unknown keys are rejected.
        forwards_row_major : Sequence[float]
            Forward swap rates (decimal) in the same row-major order.
        interpolation_mode : str, optional
            ``"vol"`` or ``"total_variance"``; ``None`` keeps the Rust
            ``VolCube::from_grid`` default (``"vol"``).

        Raises
        ------
        ValueError
            If grid sizes do not match the axes, a node dict has a missing or
            unknown key or fails SABR validation, or a label is unknown.
        TypeError
            If a node is neither ``SabrParameterData`` nor a dict.

        Examples
        --------
        >>> from finstack_quant.core.market_data import VolCube
        >>> node = {"alpha": 0.02, "beta": 0.5, "rho": -0.2, "nu": 0.3}
        >>> VolCube("USD-SWPT", [1.0], [5.0, 10.0], [node, node], [0.03, 0.035]).grid_shape
        (1, 2)

        """
        ...

    @staticmethod
    def from_json(json: str) -> VolCube:
        """
        Deserialize a cube from its canonical JSON wire form.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        VolCube
            The constructed volatility cube, ready for node and SABR queries.

        Raises
        ------
        ValueError
            If the JSON is malformed, has unknown fields, or fails validation.

        Examples
        --------
        >>> from finstack_quant.core.market_data import VolCube
        >>> node = {"alpha": 0.02, "beta": 0.5, "rho": -0.2, "nu": 0.3}
        >>> cube = VolCube("USD-SWPT", [1.0], [5.0], [node], [0.03])
        >>> VolCube.from_json(cube.to_json()) == cube
        True

        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON wire form.

        Returns
        -------
        str
            Compact JSON accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If the cube cannot be serialized.
        """
        ...

    def params_at(self, exp_idx: int, tenor_idx: int) -> SabrParameterData:
        """
        SABR parameters at grid indices.

        Parameters
        ----------
        exp_idx : int
            Zero-based expiry index.
        tenor_idx : int
            Zero-based tenor index.

        Returns
        -------
        SabrParameterData
            Node parameters.

        Raises
        ------
        ValueError
            If an index is outside the grid (checked by Rust ``VolCube``).
        """
        ...

    def forward_at(self, exp_idx: int, tenor_idx: int) -> float:
        """
        Forward swap rate (decimal) at grid indices.

        Parameters
        ----------
        exp_idx : int
            Zero-based expiry index.
        tenor_idx : int
            Zero-based tenor index.

        Returns
        -------
        float
            Forward rate as a decimal.

        Raises
        ------
        ValueError
            If an index is outside the grid (checked by Rust ``VolCube``).
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export nodes in long form.

        Returns
        -------
        pd.DataFrame
            Columns ``expiry``, ``tenor``, ``alpha``, ``beta``, ``rho``, ``nu``,
            ``shift`` (``NaN`` when absent) and ``forward``.

        Raises
        ------
        ValueError
            If the frame cannot be built.
        """
        ...

    @property
    def id(self) -> str:
        """
        Identifier this cube is registered and looked up under.

        Returns
        -------
        str
            The cube id supplied at construction; market-data containers key on it exactly.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def expiries(self) -> list[float]:
        """
        Swaption expiries the cube is quoted on.

        Returns
        -------
        list[float]
            Ascending expiry year fractions; they form the outer axis of the row-major node grid.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def tenors(self) -> list[float]:
        """
        Underlying swap tenors the cube is quoted on.

        Returns
        -------
        list[float]
            Ascending swap tenors in years; they form the inner axis of the row-major node grid.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def params(self) -> list[SabrParameterData]:
        """
        Calibrated SABR parameters at every cube node.

        Returns
        -------
        list[SabrParameterData]
            One :class:`SabrParameterData` per (expiry, tenor) node, row-major with expiry as the outer axis.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def forwards(self) -> list[float]:
        """
        Forward swap rate anchoring each cube node.

        Returns
        -------
        list[float]
            One decimal forward swap rate per node (``0.035`` is 3.5%), row-major in the same order as :attr:`params`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def grid_shape(self) -> tuple[int, int]:
        """
        Dimensions of the cube's node grid.

        Returns
        -------
        tuple[int, int]
            ``(n_expiries, n_tenors)``: the outer and inner axis lengths of the row-major node lists.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def interpolation_mode(self) -> str:
        """
        Quantity interpolated between grid nodes.

        Returns
        -------
        str
            Either ``"vol"`` (interpolate volatilities directly) or ``"total_variance"`` (interpolate ``vol**2 * T``, which is arbitrage-safer in expiry).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...
    def _repr_html_(self) -> Optional[str]: ...
