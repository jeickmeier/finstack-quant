"""Stub declarations mirroring ``finstack-quant-py/src/bindings/core/market_data/context.rs``."""

from __future__ import annotations

from decimal import Decimal
from typing import Any, Optional, Union
import pandas as pd
from finstack_quant.core.currency import Currency
from finstack_quant.core.money import Money
from finstack_quant.core.market_data import DateLike
from finstack_quant.core.market_data._curves.credit import BaseCorrelationCurve, CreditIndexData
from finstack_quant.core.market_data._curves.discount import DiscountCurve
from finstack_quant.core.market_data._curves.forward import ForwardCurve
from finstack_quant.core.market_data._curves.hazard import HazardCurve
from finstack_quant.core.market_data._curves.inflation import InflationCurve
from finstack_quant.core.market_data._curves.price import PriceCurve
from finstack_quant.core.market_data._curves.surfaces import FxDeltaVolSurface, VolCube, VolSurface
from finstack_quant.core.market_data._fx import FxMatrix, FxRateResult
from finstack_quant.core.market_data._scalars import InflationIndex, ScalarTimeSeries

class ConvertedMoney:
    """
    Result of :meth:`MarketContext.convert_money`.

    Immutable value type holding the converted amount and the FX rate lookup
    it used. ``rate`` is ``None`` when the input was already in the target
    currency; otherwise its ``triangulated`` flag records whether the rate was
    derived through a pivot currency, so the applied FX policy can be stamped
    on downstream results.

    Examples
    --------
    >>> from finstack_quant.core.market_data import FxMatrix, MarketContext
    >>> from finstack_quant.core.money import Money
    >>> fx = FxMatrix()
    >>> fx.set_quotes([("EUR", "USD", 1.1)])
    >>> converted = MarketContext().insert_fx(fx).convert_money(Money(10.0, "EUR"), "USD", "2025-01-02")
    >>> (str(converted.amount.currency), converted.rate.rate)
    ('USD', 1.1)

    """

    @staticmethod
    def from_json(json: str) -> ConvertedMoney:
        """
        Deserialize a conversion result from canonical JSON.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json` (strict field names ``amount`` and
            ``rate``).

        Returns
        -------
        ConvertedMoney
            The reconstructed amount and rate lookup.

        Raises
        ------
        ValueError
            If the JSON is malformed or has unknown or missing fields.

        Examples
        --------
        >>> from finstack_quant.core.market_data import ConvertedMoney
        >>> payload = '{"amount": {"amount": "5", "currency": "USD"}, "rate": null}'
        >>> ConvertedMoney.from_json(payload).rate is None
        True

        """
        ...

    def to_json(self) -> str:
        """
        Serialize this result to compact canonical JSON.

        Returns
        -------
        str
            ``{"amount": {...}, "rate": {...} | null}``.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    @property
    def amount(self) -> Money:
        """
        Amount in the target currency.

        Returns
        -------
        Money
            The converted amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def rate(self) -> Optional[FxRateResult]:
        """
        FX rate lookup behind the conversion.

        Returns
        -------
        Optional[FxRateResult]
            The applied rate and its ``triangulated`` flag, or ``None`` for a
            same-currency input.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export as a single-row pandas DataFrame.

        Returns
        -------
        pd.DataFrame
            Columns ``amount`` (float), ``currency`` (ISO code), ``rate`` and
            ``triangulated`` (``None`` for a same-currency input).

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...
    def _repr_html_(self) -> Optional[str]: ...

# Context

class MarketContext:
    """
    Unified market data container for curves, surfaces, scalars and FX.

    Curves are stored behind shared handles, so getters are cheap. Every
    ``insert_*`` method returns ``self`` for fluent chaining; ``id in ctx``
    and ``len(ctx)`` are supported; ``pickle`` and ``to_json`` round-trip the
    full state.

    Examples
    --------
    >>> from finstack_quant.core.market_data import DiscountCurve, HazardCurve, MarketContext
    >>> ctx = (
    ...     MarketContext()
    ...     .insert(DiscountCurve.flat("USD-OIS", "2025-01-01", 0.05))
    ...     .insert(HazardCurve.flat("ACME", "2025-01-01", 0.02, 0.4))
    ... )
    >>> ("USD-OIS" in ctx, len(ctx), ctx.curve_ids())
    (True, 2, ['ACME', 'USD-OIS'])
    >>> ctx.stats()["curve_counts"]
    {'Discount': 1, 'Hazard': 1}

    """

    def __init__(self) -> None:
        """
        Create an empty market context.

        Notes
        -----
        This constructor does not raise; populate the context with the fluent
        ``insert_*`` methods after construction.

        Examples
        --------
        >>> from finstack_quant.core.market_data import MarketContext
        >>> MarketContext().is_empty()
        True

        """
        ...

    @staticmethod
    def from_json(json: str) -> MarketContext:
        """
        Strictly load a persisted market context from its canonical state JSON.

        Uses the Rust ``MarketContext::from_state_slice`` contract loader with
        the default load limits (64 MiB of input, 96 nested JSON containers),
        exactly as WASM ``core.MarketContext.fromJson`` does.

        Parameters
        ----------
        json : str
            Canonical JSON produced by :meth:`to_json` or by the calibration
            pipeline. It must carry ``schema_version: 1``; unknown fields are
            rejected.

        Returns
        -------
        MarketContext
            The reconstructed context, holding every curve, surface, scalar and
            mapping present in the payload.

        Raises
        ------
        finstack_quant.portfolio.ContractValidationError
            If the JSON is malformed, ``schema_version`` is missing or
            unsupported, or the state has unknown fields, invalid market
            objects, duplicate ids or unresolved curve references; its
            ``report`` attribute lists the diagnostics. A subclass of
            ``ValueError``.
        finstack_quant.portfolio.ContractLimitExceededError
            If the JSON exceeds the input-size or nesting-depth limit.

        Examples
        --------
        >>> from finstack_quant.core.market_data import DiscountCurve, MarketContext
        >>> ctx = MarketContext().insert(DiscountCurve.flat("USD-OIS", "2025-01-01", 0.05))
        >>> MarketContext.from_json(ctx.to_json()).get_discount("USD-OIS").id
        'USD-OIS'

        """
        ...

    def to_json(self) -> str:
        """
        Serialize this market context to compact JSON (round-trips with pricers).

        Returns
        -------
        str
            Canonical ``MarketContextState`` JSON.

        Raises
        ------
        ValueError
            If the FX provider cannot supply a coherent snapshot or serialization fails.
        """
        ...

    def insert(
        self,
        curve: Union[
            DiscountCurve,
            ForwardCurve,
            HazardCurve,
            InflationCurve,
            PriceCurve,
            BaseCorrelationCurve,
            VolSurface,
            FxDeltaVolSurface,
            VolCube,
        ],
    ) -> MarketContext:
        """
        Insert a curve or surface under its own id (fluent).

        Parameters
        ----------
        curve : DiscountCurve, ForwardCurve, HazardCurve, InflationCurve, PriceCurve, BaseCorrelationCurve, VolSurface, FxDeltaVolSurface or VolCube
            Object to store. A ``PriceCurve`` with ``kind="vol_index"`` is
            stored as a vol-index curve.

        Returns
        -------
        MarketContext
            ``self``.

        Raises
        ------
        TypeError
            If ``curve`` is not one of the supported types.
        """
        ...

    def insert_fx(self, fx: FxMatrix) -> MarketContext:
        """
        Attach an FX matrix (fluent).

        Parameters
        ----------
        fx : FxMatrix
            Matrix shared by reference; later quote updates are visible here.

        Returns
        -------
        MarketContext
            ``self``, so inserts can be chained fluently.

        Notes
        -----
        This method does not raise; any previously attached matrix is replaced.
        """
        ...

    def insert_price(
        self,
        id: str,
        value: Union[float, int, Decimal],
        currency: Optional[Union[Currency, str]] = None,
    ) -> MarketContext:
        """
        Insert a scalar market price (fluent).

        Parameters
        ----------
        id : str
            Identifier for the scalar.
        value : float, int or Decimal
            Price or unitless value. Monetary ``Decimal`` values keep full
            precision; a unitless ``Decimal`` converts to ``float``.
        currency : Currency or str, optional
            When given, the scalar is a monetary price in this currency;
            otherwise it is unitless.

        Returns
        -------
        MarketContext
            ``self``.

        Raises
        ------
        ValueError
            If ``value`` is non-finite.
        TypeError
            If ``value`` is not numeric.
        """
        ...

    def insert_credit_index(self, id: str, data: CreditIndexData) -> MarketContext:
        """
        Insert credit index data under ``id`` (fluent).

        Parameters
        ----------
        id : str
            Identifier for the bundle (e.g. ``"CDX-IG"``); the bundle carries
            no id of its own.
        data : CreditIndexData
            Bundle whose hazard, base-correlation, and issuer curve IDs must
            already resolve in this context. Canonical context curves replace
            the bundle's embedded curve references; insert replacement curves
            first when changing their values.

        Returns
        -------
        MarketContext
            ``self``, so inserts can be chained fluently.

        Raises
        ------
        KeyError
            If a referenced curve is absent. The context remains unchanged.
        ValueError
            If constituent count, recovery, or issuer data is invalid, or a
            referenced ID resolves to the wrong curve type.
        """
        ...

    def insert_series(self, series: ScalarTimeSeries) -> MarketContext:
        """
        Insert a scalar time series under its own id (fluent).

        Parameters
        ----------
        series : ScalarTimeSeries
            Series to store.

        Returns
        -------
        MarketContext
            ``self``, so inserts can be chained fluently.

        Notes
        -----
        This method does not raise; an existing series with the same id is
        replaced.
        """
        ...

    def insert_inflation_index(self, index: InflationIndex) -> MarketContext:
        """
        Insert an inflation index under its own id (fluent).

        Parameters
        ----------
        index : InflationIndex
            Index to store.

        Returns
        -------
        MarketContext
            ``self``, so inserts can be chained fluently.

        Notes
        -----
        This method does not raise; an existing index with the same id is
        replaced.
        """
        ...

    def get_discount(self, id: str) -> DiscountCurve:
        """
        Retrieve a discount curve by identifier.

        Parameters
        ----------
        id : str
            Curve identifier.

        Returns
        -------
        DiscountCurve
            The constructed discount curve, ready for ``df`` and zero-rate queries.

        Raises
        ------
        KeyError
            If no curve is stored under ``id``.
        ValueError
            If the stored curve is not a discount curve.
        """
        ...

    def get_forward(self, id: str) -> ForwardCurve:
        """
        Retrieve a forward curve by identifier.

        Parameters
        ----------
        id : str
            Curve identifier.

        Returns
        -------
        ForwardCurve
            The constructed forward curve, ready for index projection.

        Raises
        ------
        KeyError
            If no curve is stored under ``id``.
        ValueError
            If the stored curve is not a forward curve.
        """
        ...

    def get_hazard(self, id: str) -> HazardCurve:
        """
        Retrieve a hazard curve by identifier.

        Parameters
        ----------
        id : str
            Curve identifier.

        Returns
        -------
        HazardCurve
            The constructed hazard curve, ready for survival-probability queries.

        Raises
        ------
        KeyError
            If no curve is stored under ``id``.
        ValueError
            If the stored curve is not a hazard curve.
        """
        ...

    def get_base_correlation(self, id: str) -> BaseCorrelationCurve:
        """
        Retrieve a base-correlation curve by identifier.

        Parameters
        ----------
        id : str
            Curve identifier.

        Returns
        -------
        BaseCorrelationCurve

        Raises
        ------
        KeyError
            If no curve is stored under ``id``.
        ValueError
            If the stored curve is not a base-correlation curve.
        """
        ...

    def get_inflation_curve(self, id: str) -> InflationCurve:
        """
        Retrieve an inflation curve by identifier.

        Parameters
        ----------
        id : str
            Curve identifier.

        Returns
        -------
        InflationCurve
            The constructed inflation curve, ready for CPI and uplift queries.

        Raises
        ------
        KeyError
            If no curve is stored under ``id``.
        ValueError
            If the stored curve is not an inflation curve.
        """
        ...

    def get_price_curve(self, id: str) -> PriceCurve:
        """
        Retrieve a price curve (``kind="price"``) by identifier.

        Parameters
        ----------
        id : str
            Curve identifier.

        Returns
        -------
        PriceCurve
            The constructed price curve, ready for forward-price queries.

        Raises
        ------
        KeyError
            If no curve is stored under ``id``.
        ValueError
            If the stored curve is not a price curve.
        """
        ...

    def get_vol_index_curve(self, id: str) -> PriceCurve:
        """
        Retrieve a volatility-index curve (``PriceCurve`` with ``kind="vol_index"``).

        Parameters
        ----------
        id : str
            Curve identifier.

        Returns
        -------
        PriceCurve
            Curve whose ``kind`` is ``"vol_index"``.

        Raises
        ------
        KeyError
            If no curve is stored under ``id``.
        ValueError
            If the stored curve is not a vol-index curve.
        """
        ...

    def get_price(self, id: str) -> tuple[Union[float, Decimal], Optional[str]]:
        """
        Retrieve a scalar market price as ``(value, currency)``.

        Parameters
        ----------
        id : str
            Scalar identifier.

        Returns
        -------
        tuple[float or Decimal, str or None]
            Currency-tagged values return a lossless ``Decimal`` and ISO code;
            unitless values return a ``float`` and ``None``.

        Raises
        ------
        KeyError
            If no scalar is stored under ``id``.
        """
        ...

    def get_series(self, id: str) -> ScalarTimeSeries:
        """
        Retrieve a scalar time series by identifier.

        Parameters
        ----------
        id : str
            Series identifier.

        Returns
        -------
        ScalarTimeSeries

        Raises
        ------
        KeyError
            If no series is stored under ``id``.
        """
        ...

    def get_inflation_index(self, id: str) -> InflationIndex:
        """
        Retrieve an inflation index by identifier.

        Parameters
        ----------
        id : str
            Index identifier.

        Returns
        -------
        InflationIndex
            The stored inflation index registered under that identifier.

        Raises
        ------
        KeyError
            If no index is stored under ``id``.
        """
        ...

    def get_surface(self, id: str) -> VolSurface:
        """
        Retrieve a vol surface by identifier.

        Parameters
        ----------
        id : str
            Surface identifier.

        Returns
        -------
        VolSurface
            The constructed volatility surface, ready for interpolated vol queries.

        Raises
        ------
        KeyError
            If no surface is stored under ``id``.
        """
        ...

    def get_fx_delta_vol_surface(self, id: str) -> FxDeltaVolSurface:
        """
        Retrieve a delta-quoted FX vol surface by identifier.

        Parameters
        ----------
        id : str
            Surface identifier.

        Returns
        -------
        FxDeltaVolSurface

        Raises
        ------
        KeyError
            If no surface is stored under ``id``.
        """
        ...

    def get_vol_cube(self, id: str) -> VolCube:
        """
        Retrieve a vol cube by identifier.

        Parameters
        ----------
        id : str
            Cube identifier.

        Returns
        -------
        VolCube
            The constructed volatility cube, ready for node and SABR queries.

        Raises
        ------
        KeyError
            If no cube is stored under ``id``.
        """
        ...

    def get_credit_index(self, id: str) -> CreditIndexData:
        """
        Retrieve credit-index data by identifier.

        Parameters
        ----------
        id : str
            Bundle identifier.

        Returns
        -------
        CreditIndexData
            The stored credit-index bundle for that identifier.

        Raises
        ------
        KeyError
            If no bundle is stored under ``id``.
        """
        ...

    @property
    def fx(self) -> Optional[FxMatrix]:
        """
        FX quotes available for currency conversion.

        Returns
        -------
        Optional[FxMatrix]
            The attached :class:`FxMatrix`, or ``None`` when the container holds no FX data and cross-currency conversion is unavailable.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def fx_required(self) -> FxMatrix:
        """
        Attached FX matrix, raising if none is present.

        Returns
        -------
        FxMatrix
            The constructed FX matrix holding the supplied quotes.

        Raises
        ------
        KeyError
            If no FX matrix has been inserted.
        """
        ...

    def convert_money(
        self,
        amount: Money,
        target_currency: Union[Currency, str],
        as_of: DateLike,
    ) -> ConvertedMoney:
        """
        Convert a monetary amount into another currency with the attached FX matrix.

        Same-currency amounts are returned unchanged without consulting FX.

        Parameters
        ----------
        amount : Money
            Amount to convert.
        target_currency : Currency or str
            Destination currency.
        as_of : datetime.date or str
            Date used for the FX rate lookup.

        Returns
        -------
        ConvertedMoney
            The converted ``amount`` in ``target_currency`` and the applied FX
            ``rate`` lookup (``None`` for a same-currency input), so the caller
            can record whether the rate was triangulated.

        Raises
        ------
        KeyError
            If no FX matrix is attached or the pair cannot be resolved.

        Examples
        --------
        >>> from finstack_quant.core.market_data import FxMatrix, MarketContext
        >>> from finstack_quant.core.money import Money
        >>> fx = FxMatrix()
        >>> fx.set_quotes([("EUR", "USD", 1.1)])
        >>> ctx = MarketContext().insert_fx(fx)
        >>> converted = ctx.convert_money(Money(100.0, "EUR"), "USD", "2025-01-02")
        >>> (round(converted.amount.amount, 6), converted.rate.triangulated)
        (110.0, False)
        >>> ctx.convert_money(Money(5.0, "USD"), "USD", "2025-01-02").rate is None
        True
        """
        ...

    def contains(self, id: str) -> bool:
        """
        Whether any object is stored under ``id``.

        Parameters
        ----------
        id : str
            Identifier to look up across curves, surfaces, scalars, series,
            indices and credit indices.

        Returns
        -------
        bool
            ``True`` when some object is stored under that identifier in any
            of those categories, ``False`` otherwise.

        Notes
        -----
        This method does not raise; an unknown identifier returns ``False``.
        """
        ...

    def curve_ids(self) -> list[str]:
        """
        Identifiers of all stored term-structure curves.

        Returns
        -------
        list[str]
            Sorted identifiers (surfaces, scalars and series are excluded).

        Notes
        -----
        This method does not raise; an empty context returns an empty list.
        """
        ...

    def is_empty(self) -> bool:
        """
        Whether nothing has been inserted.

        Returns
        -------
        bool
            ``True`` when the context holds no curves, surfaces, scalars,
            series, indices, dividend schedules, FX matrix (``len(ctx) == 0``).

        Notes
        -----
        This method does not raise.
        """
        ...

    def stats(self) -> dict[str, Any]:
        """
        Counts of stored objects by category.

        Returns
        -------
        dict[str, Any]
            Keys ``curve_counts`` (dict of curve type to count),
            ``total_curves``, ``has_fx``, ``surface_count``,
            ``vol_cube_count``, ``price_count``, ``series_count``,
            ``inflation_index_count``, ``credit_index_count``,
            ``dividend_schedule_count`` and ``fx_delta_vol_surface_count``.

        Raises
        ------
        RuntimeError
            If the statistics dictionary cannot be constructed.
        """
        ...

    def roll_forward(self, days: int) -> MarketContext:
        """
        Roll every dated term structure forward by ``days`` calendar days.

        Parameters
        ----------
        days : int
            Signed 64-bit calendar days to roll (may be negative).

        Returns
        -------
        MarketContext
            New context; ``self`` is unchanged.

        Raises
        ------
        ValueError
            If a rolled base date exceeds the supported date range or a curve cannot be rebuilt after rolling.
        OverflowError
            If ``days`` is outside the signed 64-bit integer range.
        """
        ...

    def __contains__(self, id: str) -> bool: ...
    def __len__(self) -> int:
        """
        Number of stored market-data objects (Rust ``MarketContext::len``).

        Counts curves, surfaces, cubes, scalars, series, inflation and credit
        indices, dividend schedules, FX delta-vol surfaces, plus one for an attached FX matrix.

        Returns
        -------
        int
            Stored object count; ``0`` exactly when ``is_empty()`` is true.
        """
        ...

    def __bool__(self) -> bool:
        """
        ``True`` when anything has been inserted (``not is_empty()``).

        An FX-only context is truthy.

        Returns
        -------
        bool
            ``not self.is_empty()``.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...
