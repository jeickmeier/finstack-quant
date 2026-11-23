"""Stub declarations mirroring ``finstack-quant-py/src/bindings/core/market_data/curves/vol_cube.rs``."""

from __future__ import annotations

from typing import Any, Optional, Sequence, Union
import pandas as pd
import polars as pl
from finstack_quant.core.currency import Currency
from finstack_quant.core.dates import VolatilityClock
from finstack_quant.core.table import ArrowTable

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

    def to_arrow(self) -> ArrowTable:
        """
        Export nodes in long form as an :class:`~finstack_quant.core.table.ArrowTable`.

        Returns
        -------
        ArrowTable
            Columns ``expiry``, ``tenor``, ``alpha``, ``beta``, ``rho``, ``nu``,
            ``shift`` (``NaN`` when absent) and ``forward``.

        Raises
        ------
        ValueError
            If the underlying Arrow record batch cannot be built.
        """
        ...

    def to_pandas(self) -> pd.DataFrame:
        """
        Export nodes in long form.

        Returns
        -------
        pd.DataFrame
            Columns ``expiry``, ``tenor``, ``alpha``, ``beta``, ``rho``, ``nu``,
            ``shift`` (``NaN`` when absent) and ``forward``.

        Raises
        ------
        ImportError
            If ``pandas`` is not installed.
        """
        ...

    def to_polars(self) -> pl.DataFrame:
        """
        Export nodes in long form as a ``polars.DataFrame``.

        Returns
        -------
        pl.DataFrame
            Polars DataFrame constructed via the Arrow PyCapsule interface.

        Raises
        ------
        ImportError
            If ``polars`` is not installed.
        """
        ...

    def to_pyarrow(self) -> Any:
        """
        Export nodes in long form as a ``pyarrow.Table``.

        Returns
        -------
        pyarrow.Table
            Zero-copy table built through the Arrow PyCapsule interface.

        Raises
        ------
        ImportError
            If ``pyarrow`` is not installed.
        """
        ...

    def __arrow_c_stream__(self, requested_schema: Any | None = None) -> Any: ...
    def __arrow_c_schema__(self) -> Any: ...
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
