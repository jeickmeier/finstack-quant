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
    >>> matrix = FxMatrix(None)
    >>> matrix.set_quotes([("EUR", "USD", 1.1)])
    >>> result = matrix.rate("EUR", "USD", datetime.date(2025, 1, 1), "cashflow_date")
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

class FxPairConvention:
    """
    Validated quote metadata for one ordered FX pair.

    The supplied order is kept: an outright rate is quote-currency units per
    one base unit. The pip size is explicit; nothing is inferred from the
    currencies, and the reversed pair needs its own convention.

    Examples
    --------
    >>> from finstack_quant.core.market_data import FxPairConvention
    >>> conv = FxPairConvention("USD", "JPY", 0.01)
    >>> (str(conv.base), str(conv.quote), conv.pip_size)
    ('USD', 'JPY', 0.01)
    """

    def __init__(self, base: Union[Currency, str], quote: Union[Currency, str], pip_size: float) -> None:
        """
        Quote metadata for the ordered pair ``base``/``quote``.

        Parameters
        ----------
        base : Currency or str
            Base currency: one unit of it is quoted.
        quote : Currency or str
            Quote currency; must differ from ``base``.
        pip_size : float
            Outright-rate increment per pip in this pair's quote units (for
            example ``0.0001`` for EUR/USD, ``0.01`` for USD/JPY). Not an ISO
            minor unit, display precision or venue tick; finite and positive.

        Raises
        ------
        TypeError
            If ``base`` or ``quote`` is not a :class:`Currency` or ISO code, or
            ``pip_size`` is not a number (or is ``bool``).
        ValueError
            If a code is unrecognized, the currencies are equal, or
            ``pip_size`` is not finite and positive.

        Examples
        --------
        >>> from finstack_quant.core.market_data import FxPairConvention
        >>> FxPairConvention("EUR", "USD", 0.0001).pip_size
        0.0001
        """
        ...

    @property
    def base(self) -> Currency:
        """
        Base currency of the ordered pair.

        Returns
        -------
        Currency
            The base currency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def quote(self) -> Currency:
        """
        Quote currency of the ordered pair.

        Returns
        -------
        Currency
            The quote currency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def pip_size(self) -> float:
        """
        Outright-rate increment per pip.

        Returns
        -------
        float
            The pip size in quote units per base unit.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def rate_points(self, pips: float) -> float:
        """
        Rate points for a signed number of forward pips: ``pips * pip_size``.

        Parameters
        ----------
        pips : float
            Signed forward points in pips; must be finite.

        Returns
        -------
        float
            The rate points to add to an outright rate.

        Raises
        ------
        TypeError
            If ``pips`` is not a number, or is ``bool``.
        ValueError
            If ``pips`` or the product is not finite.

        Examples
        --------
        >>> from finstack_quant.core.market_data import FxPairConvention
        >>> round(FxPairConvention("EUR", "USD", 0.0001).rate_points(50.0), 6)
        0.005
        """
        ...

    def __repr__(self) -> str: ...

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
    TypeError
        If *rate* is not a number, or is ``bool``.
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

    Date/policy-scoped fixings pinned with :meth:`set_quote_on` take priority
    for their date and policy, followed by explicit pair-global quotes, then
    the provider; within each source the supplied direction wins, otherwise the
    reciprocal of the opposite quote. Market-context JSON retains those
    distinct source roles. Only a pair the provider reports missing is routed
    ``from -> pivot -> to``, and only through the explicit ``pivot_currency``
    chosen at construction; there is no default pivot, and ``None`` uses
    direct quotes only. Matrices obtained from a
    :class:`MarketContext` share state with it.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.market_data import FxMatrix
    >>> matrix = FxMatrix.from_dict({"EURUSD": 1.1, "GBP/USD": 1.25}, "USD")
    >>> round(matrix.rate("EUR", "GBP", datetime.date(2025, 1, 1), "cashflow_date").rate, 4)
    0.88
    >>> list(matrix.quotes()["base"])
    ['EUR', 'GBP']

    """

    def __init__(self, pivot_currency: Union[Currency, str, None]) -> None:
        """
        Create an empty FX matrix backed by an in-memory quote provider.

        Parameters
        ----------
        pivot_currency : Currency | str | None
            Required routing choice: the single pivot for ``from -> pivot ->
            to`` crosses, or ``None`` for direct quotes only. There is no
            default.

        Raises
        ------
        ValueError
            If *pivot_currency* is an unknown currency code.
        TypeError
            If *pivot_currency* is not a ``Currency``, ``str`` or ``None``.

        Examples
        --------
        >>> from finstack_quant.core.market_data import FxMatrix
        >>> FxMatrix(None)
        FxMatrix(quotes=0, pinned_quotes=0)

        """
        ...

    @staticmethod
    def from_dict(quotes: dict[str, float], pivot_currency: Union[Currency, str, None]) -> FxMatrix:
        """
        Build a matrix from a ``{"EURUSD": 1.1, "GBP/USD": 1.25}`` mapping.

        Parameters
        ----------
        quotes : dict[str, float]
            Keys are six ASCII-letter ISO pairs (``"EURUSD"``) or
            slash-separated pairs (``"EUR/USD"``); values are
            ``1 base = rate quote``.
        pivot_currency : Currency | str | None
            Required routing choice: the single cross pivot, or ``None`` for
            direct quotes only.

        Returns
        -------
        FxMatrix
            The constructed FX matrix holding the supplied quotes.

        Raises
        ------
        TypeError
            If a key is not ``str``, a rate is not a number (or is ``bool``),
            or *pivot_currency* has an unsupported type.
        ValueError
            If a key is not a currency pair, a code is unknown, or a rate is
            non-finite or non-positive.

        Examples
        --------
        >>> import datetime
        >>> from finstack_quant.core.market_data import FxMatrix
        >>> FxMatrix.from_dict({"EURUSD": 1.1}, None).rate(
        ...     "EUR", "USD", datetime.date(2025, 1, 1), "cashflow_date"
        ... ).rate
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
        TypeError
            If a currency or rate has an unsupported type (including ``bool``).
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

        For its date and policy the fixing takes priority over a pair-global
        quote in either orientation and over the provider.

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
        TypeError
            If an argument has an unsupported type (including ``bool`` for ``rate``).
        ValueError
            If ``rate`` is invalid or ``policy`` is not a recognised label.
        """
        ...

    def rate(
        self,
        base: Union[Currency, str],
        quote: Union[Currency, str],
        date: DateLike,
        policy: Union[FxConversionPolicy, str],
    ) -> FxRateResult:
        """
        Look up an FX rate, triangulating through the pivot when needed.

        Source priority precedes orientation: pinned fixings for the date and
        policy, global quotes, then the provider, taking a reciprocal within
        each source. Only a pair the provider reports missing is routed through
        the pivot; any other provider error is raised unchanged.

        Parameters
        ----------
        base : Currency or str
            Base (from) currency.
        quote : Currency or str
            Quote (to) currency.
        date : datetime.date or str
            Applicable date for the rate.
        policy : FxConversionPolicy or str
            Required conversion policy (``"cashflow_date"`` or
            ``"period_end"``); there is no default.

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
