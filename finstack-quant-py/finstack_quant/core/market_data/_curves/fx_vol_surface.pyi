"""Stub declarations mirroring ``finstack-quant-py/src/bindings/core/market_data/curves/fx_vol_surface.rs``."""

from __future__ import annotations

from typing import Any, Optional, Sequence, Union
import pandas as pd
import polars as pl
from finstack_quant.core.currency import Currency
from finstack_quant.core.dates import VolatilityClock
from finstack_quant.core.table import ArrowTable
from finstack_quant.core.market_data._curves.vol_surface import VolSurface

class FxDeltaVolSurface:
    """
    Delta-quoted FX volatility surface (ATM, 25-delta RR/BF, optional 10-delta wings).

    Uses explicit currency pair ordering (``"BASE/QUOTE"``), explicit quoting
    conventions (``forward_unadjusted`` delta, ``delta_neutral_straddle`` ATM,
    arithmetic ``smile`` butterfly), and an optional :class:`VolatilityClock`.
    Quotes are decimal vols (``0.08`` is 8%); risk reversals are call vol minus
    put vol and butterflies are wing average ``0.5 * (call + put) - ATM``.

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
    >>> surface = FxDeltaVolSurface(
    ...     "EURUSD",
    ...     "EUR/USD",
    ...     [0.25, 1.0],
    ...     [0.08, 0.09],
    ...     [0.01, 0.015],
    ...     [0.005, 0.007],
    ...     delta_convention="forward_unadjusted",
    ...     atm_convention="delta_neutral_straddle",
    ...     butterfly_convention="smile",
    ... )
    >>> (surface.pair, len(surface.expiries), surface.rr_10d)
    ('EUR/USD', 2, None)

    """

    def __init__(
        self,
        id: str,
        pair: Union[str, tuple[Union[Currency, str], Union[Currency, str]]],
        expiries: Sequence[float],
        atm_vols: Sequence[float],
        rr_25d: Sequence[float],
        bf_25d: Sequence[float],
        *,
        delta_convention: str,
        atm_convention: str,
        butterfly_convention: str,
        rr_10d: Optional[Sequence[float]] = None,
        bf_10d: Optional[Sequence[float]] = None,
        expiry_clock: Optional[VolatilityClock] = None,
    ) -> None:
        """
        Construct an FX delta-quoted vol surface with 25-delta wings.

        Parameters
        ----------
        id : str
            Unique surface identifier.
        pair : str or tuple[Currency | str, Currency | str]
            Distinct currency pair (for example ``"EUR/USD"``, ``"EURUSD"``, or
            ``(Currency.EUR, Currency.USD)``) defining base/quote order.
        expiries : Sequence[float]
            Strictly increasing positive expiry times in years.
        atm_vols : Sequence[float]
            ATM delta-neutral straddle vols per expiry (decimal, positive).
        rr_25d : Sequence[float]
            25-delta risk reversal per expiry (decimal, call vol - put vol).
        bf_25d : Sequence[float]
            25-delta smile butterfly per expiry (decimal, wing average - ATM).
        rr_10d : Sequence[float], optional
            10-delta risk reversal per expiry; requires ``bf_10d``.
        bf_10d : Sequence[float], optional
            10-delta smile butterfly per expiry; requires ``rr_10d``.
        delta_convention : str
            Delta quoting convention (``"forward_unadjusted"``).
        atm_convention : str
            ATM quoting convention (``"delta_neutral_straddle"``).
        butterfly_convention : str
            Butterfly quoting convention (``"smile"``).
        expiry_clock : VolatilityClock, optional
            Calendar and day-count clock defining how option expiry dates map to
            year fractions when resolving market valuations.

        Raises
        ------
        ValueError
            If ``pair`` is invalid or has identical base and quote currencies,
            any convention string is unrecognized, only one of ``rr_10d`` /
            ``bf_10d`` is given, any vector is empty or mismatched in length,
            expiries are not strictly increasing and positive, or a quote is
            non-finite.
        TypeError
            If ``pair`` is not a string or 2-element currency pair, or an
            unexpected keyword argument is passed, or a required convention is missing.

        Examples
        --------
        >>> from finstack_quant.core.market_data import FxDeltaVolSurface
        >>> FxDeltaVolSurface(
        ...     "EURUSD",
        ...     "EUR/USD",
        ...     [1.0],
        ...     [0.09],
        ...     [0.015],
        ...     [0.007],
        ...     delta_convention="forward_unadjusted",
        ...     atm_convention="delta_neutral_straddle",
        ...     butterfly_convention="smile",
        ... ).atm_vols
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
        >>> surface = FxDeltaVolSurface(
        ...     "EURUSD",
        ...     "EUR/USD",
        ...     [1.0],
        ...     [0.09],
        ...     [0.015],
        ...     [0.007],
        ...     delta_convention="forward_unadjusted",
        ...     atm_convention="delta_neutral_straddle",
        ...     butterfly_convention="smile",
        ... )
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

    def to_arrow(self) -> ArrowTable:
        """
        Export pillars as an :class:`~finstack_quant.core.table.ArrowTable`.

        Returns
        -------
        ArrowTable
            Columns ``expiry``, ``atm_vol``, ``rr_25d``, ``bf_25d`` and, when
            10-delta wings are stored, ``rr_10d`` and ``bf_10d``.

        Raises
        ------
        ValueError
            If the underlying Arrow record batch cannot be built.
        """
        ...

    def to_pandas(self) -> pd.DataFrame:
        """
        Export pillars as a pandas ``DataFrame``.

        Returns
        -------
        pd.DataFrame
            Columns ``expiry``, ``atm_vol``, ``rr_25d``, ``bf_25d`` and, when
            10-delta wings are stored, ``rr_10d`` and ``bf_10d``.

        Raises
        ------
        ImportError
            If ``pandas`` is not installed.
        """
        ...

    def to_polars(self) -> pl.DataFrame:
        """
        Export pillars as a ``polars.DataFrame``.

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
        Export pillars as a ``pyarrow.Table``.

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
            Arithmetic smile butterfly quotes ``0.5 * (call + put) - ATM`` as
            decimals, one per expiry; positive values lift the 25-delta wings
            above the ATM level.

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

    @property
    def pair(self) -> str:
        """
        Canonical currency pair in ``"BASE/QUOTE"`` notation.

        Returns
        -------
        str
            Base and quote currencies formatted as ``"BASE/QUOTE"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def delta_convention(self) -> str:
        """
        FX option delta quoting convention.

        Returns
        -------
        str
            Delta convention label (``"forward_unadjusted"``).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def atm_convention(self) -> str:
        """
        FX option ATM quoting convention.

        Returns
        -------
        str
            ATM convention label (``"delta_neutral_straddle"``).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def butterfly_convention(self) -> str:
        """
        FX option butterfly quoting convention.

        Returns
        -------
        str
            Butterfly convention label (``"smile"``).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def quote_type(self) -> str:
        """
        Volatility quote representation stored on this surface.

        Returns
        -------
        str
            Quote convention label (``"black_lognormal"``).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def expiry_clock(self) -> Optional[VolatilityClock]:
        """
        Calendar/day-count volatility clock attached to this surface, if set.

        Returns
        -------
        Optional[VolatilityClock]
            Attached volatility clock, or ``None`` when none was supplied.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...
    def _repr_html_(self) -> Optional[str]: ...
