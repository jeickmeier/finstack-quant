"""Stub declarations mirroring ``finstack-quant-py/src/bindings/core/market_data/fx.rs``."""

from __future__ import annotations

from typing import Any, Optional, Sequence, Union
import pandas as pd
from finstack_quant.core.currency import Currency
from finstack_quant.core.market_data import DateLike

# FX

class FxConversionPolicy:
    """
    FX conversion policy controlling when rates are sampled.

    Immutable enum-style type with class-level constants.

    Examples
    --------
    >>> from finstack_quant.core.market_data import FxConversionPolicy
    >>> str(FxConversionPolicy.from_name("cashflow_date"))
    'cashflow_date'

    """

    CASHFLOW_DATE: FxConversionPolicy
    """Use spot/forward on the cashflow date."""
    PERIOD_END: FxConversionPolicy
    """Use period end date."""
    PERIOD_AVERAGE: FxConversionPolicy
    """Use an average over the period."""

    @classmethod
    def from_name(cls, name: str) -> FxConversionPolicy:
        """
        Parse from a string label.

        Parameters
        ----------
        name : str
            Policy label (e.g. ``"cashflow_date"``, ``"period_end"``).

        Returns
        -------
        FxConversionPolicy

        Raises
        ------
        ValueError
            If *name* is not recognised.

        Examples
        --------
        >>> from finstack_quant.core.market_data import FxConversionPolicy
        >>> str(FxConversionPolicy.from_name("cashflow_date"))
        'cashflow_date'

        """
        ...

    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class FxRateResult:
    """
    Result of an FX rate query.

    Immutable value type returned by :meth:`FxMatrix.rate`; ``float(result)``
    yields the rate.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.market_data import FxMatrix
    >>> matrix = FxMatrix()
    >>> matrix.set_quotes([("EUR", "USD", 1.1)])
    >>> result = matrix.rate("EUR", "USD", datetime.date(2025, 1, 1))
    >>> (result.rate, result.triangulated, float(result))
    (1.1, False, 1.1)

    """

    @staticmethod
    def from_json(json: str) -> FxRateResult:
        """
        Deserialize an FX lookup result from canonical JSON.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        FxRateResult
            The reconstructed FX conversion result, including its triangulation flag.

        Raises
        ------
        ValueError
            If the JSON is malformed or has unknown fields.

        Examples
        --------
        >>> from finstack_quant.core.market_data import FxRateResult
        >>> FxRateResult.from_json('{"rate": 1.1, "triangulated": false}').rate
        1.1

        """
        ...

    def to_json(self) -> str:
        """
        Serialize this result to compact canonical JSON.

        Returns
        -------
        str
            ``{"rate": ..., "triangulated": ...}``.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    @property
    def rate(self) -> float:
        """
        Rate actually used to convert between the two currencies.

        Returns
        -------
        float
            Units of quote currency per one unit of base currency (``1.10`` for EURUSD means 1 EUR buys 1.10 USD).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def triangulated(self) -> bool:
        """
        Provenance flag for the returned rate.

        Returns
        -------
        bool
            ``True`` when the rate was derived by chaining two quotes through the pivot currency, ``False`` when a direct quote was found.

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
            Columns ``rate`` and ``triangulated``.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def __float__(self) -> float: ...
    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...
    def _repr_html_(self) -> Optional[str]: ...

class FxQuoteConvention:
    """
    USD quotation style for a market FX pair.

    **Direct** means USD is the quote currency (EURUSD, GBPUSD). **Indirect**
    means USD is the base (USDJPY, USDCAD). Non-USD crosses inherit the USD
    quotation of market CCY1 versus USD.

    Examples
    --------
    >>> from finstack_quant.core.market_data import FxQuoteConvention
    >>> str(FxQuoteConvention.from_name("direct"))
    'direct'

    """

    DIRECT: FxQuoteConvention
    """USD is the quote currency (units of USD per one unit of CCY1)."""
    INDIRECT: FxQuoteConvention
    """USD is the base currency (units of CCY2 per one USD)."""

    @classmethod
    def from_name(cls, name: str) -> FxQuoteConvention:
        """
        Parse from a string label.

        Parameters
        ----------
        name : str
            Convention label (``"direct"`` or ``"indirect"``).

        Returns
        -------
        FxQuoteConvention
            Parsed USD quotation style.

        Raises
        ------
        ValueError
            If *name* is not ``"direct"`` or ``"indirect"``.

        Examples
        --------
        >>> from finstack_quant.core.market_data import FxQuoteConvention
        >>> str(FxQuoteConvention.from_name("indirect"))
        'indirect'

        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class FxPairConvention:
    """
    Market convention for one FX pair after Bloomberg/Reuters CCY1 ordering.

    Instances come from :func:`fx_pair_convention`. ``base`` / ``quote`` are
    always market CCY1/CCY2, even when the lookup arguments were inverted.

    Examples
    --------
    >>> from finstack_quant.core.market_data import fx_pair_convention
    >>> conv = fx_pair_convention("USD", "EUR")
    >>> (str(conv.base), str(conv.quote), str(conv.usd_quotation), conv.pip_size, conv.settlement_days)
    ('EUR', 'USD', 'direct', 0.0001, 2)

    """

    @property
    def base(self) -> Currency:
        """
        Base currency of the market-convention pair.

        Returns
        -------
        Currency
            ISO-4217 code of CCY1: the currency whose single unit the quoted rate prices.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def quote(self) -> Currency:
        """
        Quote currency of the market-convention pair.

        Returns
        -------
        Currency
            ISO-4217 code of CCY2: the currency the rate is expressed in, per one unit of CCY1.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def usd_quotation(self) -> FxQuoteConvention:
        """
        How USD sits in the market convention for this pair.

        Returns
        -------
        FxQuoteConvention
            Quotation label: direct when USD is the quote currency (as in EURUSD), indirect when USD is the base (as in USDJPY).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def pip_size(self) -> float:
        """
        Smallest conventional price increment for the pair.

        Returns
        -------
        float
            Pip size in outright-rate units: ``0.01`` for JPY, KRW and HUF pairs and ``0.0001`` for the rest.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def settlement_days(self) -> int:
        """
        Conventional settlement lag from trade date to spot.

        Returns
        -------
        int
            Business days to spot: ``1`` for USDCAD and similar pairs, ``2`` for the market standard.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __repr__(self) -> str: ...

def fx_pair_convention(
    base: Union[Currency, str],
    quote: Union[Currency, str],
) -> FxPairConvention:
    """
    Market convention for an unordered currency pair.

    Returned ``base`` / ``quote`` are always the market CCY1/CCY2, even when
    the arguments are inverted.

    Parameters
    ----------
    base : Currency or str
        One currency of the pair. Orientation is ignored.
    quote : Currency or str
        The other currency of the pair. Orientation is ignored.

    Returns
    -------
    FxPairConvention
        Market CCY1/CCY2, USD quotation, pip size, and standard spot lag.

    Raises
    ------
    TypeError
        If *base* or *quote* is not a :class:`Currency` or ISO code string.
    ValueError
        If either value is an unrecognized currency code.

    Examples
    --------
    >>> from finstack_quant.core.market_data import fx_pair_convention
    >>> conv = fx_pair_convention("USD", "JPY")
    >>> (str(conv.base), str(conv.quote), str(conv.usd_quotation), conv.pip_size, conv.settlement_days)
    ('USD', 'JPY', 'indirect', 0.01, 2)

    """
    ...

def invert_fx_rate(rate: float) -> float:
    """
    Reciprocal of a strictly positive finite FX rate.

    Parameters
    ----------
    rate : float
        Outright FX rate to invert, in quote-per-base units. Must be finite
        and strictly positive; the reciprocal must also be a valid FX rate.

    Returns
    -------
    float
        ``1 / rate`` when that reciprocal is a valid FX rate.

    Raises
    ------
    ValueError
        If *rate* is non-finite, zero or negative, or its reciprocal
        overflows.

    Examples
    --------
    >>> from finstack_quant.core.market_data import invert_fx_rate
    >>> round(invert_fx_rate(1.10), 5)
    0.90909

    """
    ...

class FxMatrix:
    """
    Foreign-exchange rate matrix for currency conversion.

    Explicit pair-global quotes take priority in either orientation, followed
    by date/policy-scoped fixings pinned with :meth:`set_quote_on`, then provider
    observations. Market-context JSON retains those distinct source roles.
    Missing pairs are triangulated through the pivot
    currency (USD by default). Matrices obtained from a
    :class:`MarketContext` share state with it.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.market_data import FxMatrix
    >>> matrix = FxMatrix.from_dict({"EURUSD": 1.1, "GBP/USD": 1.25})
    >>> round(matrix.rate("EUR", "GBP", datetime.date(2025, 1, 1)).rate, 4)
    0.88
    >>> list(matrix.quotes()["base"])
    ['GBP', 'EUR']

    """

    def __init__(self) -> None:
        """
        Create an empty FX matrix backed by an in-memory quote provider.

        Notes
        -----
        This constructor does not raise; add quotes with the mutating
        methods after construction.

        Examples
        --------
        >>> from finstack_quant.core.market_data import FxMatrix
        >>> FxMatrix()
        FxMatrix(quotes=0, pinned_quotes=0)

        """
        ...

    @staticmethod
    def from_dict(quotes: dict[str, float]) -> FxMatrix:
        """
        Build a matrix from a ``{"EURUSD": 1.1, "GBP/USD": 1.25}`` mapping.

        Parameters
        ----------
        quotes : dict[str, float]
            Keys are six ASCII-letter ISO pairs (``"EURUSD"``) or
            slash-separated pairs (``"EUR/USD"``); values are
            ``1 base = rate quote``.

        Returns
        -------
        FxMatrix
            The constructed FX matrix holding the supplied quotes.

        Raises
        ------
        ValueError
            If a key is not a currency pair, a code is unknown, or a rate is
            non-finite or non-positive.

        Examples
        --------
        >>> import datetime
        >>> from finstack_quant.core.market_data import FxMatrix
        >>> FxMatrix.from_dict({"EURUSD": 1.1}).rate("EUR", "USD", datetime.date(2025, 1, 1)).rate
        1.1

        """
        ...

    def set_quotes(
        self,
        quotes: Sequence[tuple[Union[Currency, str], Union[Currency, str], float]],
    ) -> None:
        """
        Set several explicit FX quotes atomically.

        Parameters
        ----------
        quotes : Sequence[tuple[Currency or str, Currency or str, float]]
            ``(base, quote, rate)`` triples with ``1 base = rate quote``.

        Raises
        ------
        ValueError
            If any rate is non-finite or non-positive; no quote is applied
            in that case.
        """
        ...

    def set_quote_on(
        self,
        base: Union[Currency, str],
        quote: Union[Currency, str],
        date: DateLike,
        policy: Union[FxConversionPolicy, str],
        rate: float,
    ) -> None:
        """
        Set an authoritative FX quote scoped to one date and policy.

        A pair-global quote in either orientation takes priority over this
        fixing. The fixing takes priority over provider observations.

        Parameters
        ----------
        base : Currency or str
            Base (from) currency.
        quote : Currency or str
            Quote (to) currency.
        date : datetime.date or str
            Date the pinned quote applies to.
        policy : FxConversionPolicy or str
            Conversion policy the quote is pinned under.
        rate : float
            Conversion rate ``1 base = rate quote``; finite and positive.

        Raises
        ------
        ValueError
            If ``rate`` is invalid or ``policy`` is not a recognised label.
        """
        ...

    def rate(
        self,
        base: Union[Currency, str],
        quote: Union[Currency, str],
        date: DateLike,
        policy: Optional[Union[FxConversionPolicy, str]] = None,
    ) -> FxRateResult:
        """
        Look up an FX rate, triangulating through the pivot when needed.

        Source priority precedes orientation: global quotes, pinned fixings,
        then provider observations, taking a reciprocal within each source.

        Parameters
        ----------
        base : Currency or str
            Base (from) currency.
        quote : Currency or str
            Quote (to) currency.
        date : datetime.date or str
            Applicable date for the rate.
        policy : FxConversionPolicy or str, optional
            Conversion policy; default ``"cashflow_date"``.

        Returns
        -------
        FxRateResult
            Rate and whether it was triangulated.

        Raises
        ------
        KeyError
            If no direct, inverse or triangulated quote is available.
        ValueError
            If ``policy`` is not a recognised label or repeated concurrent quote
            updates prevent a coherent rate from being resolved.
        """
        ...

    def quotes(self) -> pd.DataFrame:
        """
        Explicit pair-global quotes as a pandas ``DataFrame``.

        Date-scoped quotes set with :meth:`set_quote_on` are not included.

        Returns
        -------
        pd.DataFrame
            Columns ``base``, ``quote`` (ISO codes) and ``rate``, sorted by pair.

        Raises
        ------
        ValueError
            If the frame cannot be built, concurrent quote updates prevent a
            coherent snapshot, or the provider cannot version its snapshot quotes.
        """
        ...

    def __repr__(self) -> str: ...
