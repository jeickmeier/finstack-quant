"""Stub declarations mirroring ``finstack-quant-py/src/bindings/calibration/quotes/market.rs``."""

from __future__ import annotations

import datetime
from decimal import Decimal
from typing import Any
from finstack_quant.core.currency import Currency

class InflationQuote:
    """An inflation-swap market quote (zero-coupon or year-on-year) for inflation-curve steps.

    Examples:
    --------
    >>> from finstack_quant.calibration import InflationQuote
    >>> InflationQuote.inflation_swap("zc5y", "2031-05-08", 0.025, "USA-CPI-U", "USD").type
    'inflation_swap'

    """

    @staticmethod
    def inflation_swap(id: str, maturity: str, rate: float, index: str, convention: str) -> InflationQuote:
        """Build a zero-coupon inflation swap quote.

        Parameters
        ----------
        id : str
            Unique quote identifier.
        maturity : str | datetime.date
            Swap maturity date (ISO string or ``date``).
        rate : float
            Fixed zero-coupon swap rate as a decimal per annum (``0.025`` for 2.5%).
        index : str
            Inflation index identifier, for example ``"USA-CPI-U"``.
        convention : str
            Inflation-swap convention identifier, for example ``"USD"``.

        Returns
        -------
        InflationQuote
            The typed zero-coupon quote.

        Raises
        ------
        ValueError
            If ``maturity`` cannot be parsed or ``rate`` is not finite.

        Examples:
        --------
        >>> from finstack_quant.calibration import InflationQuote
        >>> InflationQuote.inflation_swap("zc5y", "2031-05-08", 0.025, "USA-CPI-U", "USD").rate
        0.025

        """

    @staticmethod
    def yoy_inflation_swap(
        id: str, maturity: str, rate: float, index: str, frequency: str, convention: str
    ) -> InflationQuote:
        """Build a year-on-year inflation swap quote.

        Parameters
        ----------
        id : str
            Unique quote identifier.
        maturity : str | datetime.date
            Swap maturity date (ISO string or ``date``).
        rate : float
            Fixed year-on-year swap rate as a decimal per annum.
        index : str
            Inflation index identifier, for example ``"USA-CPI-U"``.
        frequency : str
            Payment frequency tenor, for example ``"1Y"``.
        convention : str
            Inflation-swap convention identifier, for example ``"USD"``.

        Returns
        -------
        InflationQuote
            The typed year-on-year quote.

        Raises
        ------
        ValueError
            If ``maturity`` or ``frequency`` cannot be parsed or ``rate`` is
            not finite.

        Examples:
        --------
        >>> from finstack_quant.calibration import InflationQuote
        >>> InflationQuote.yoy_inflation_swap("yoy5y", "2031-05-08", 0.024, "USA-CPI-U", "1Y", "USD").frequency
        '1Y'

        """

    @property
    def id(self) -> str:
        """
        Unique quote identifier.

        This property does not raise.

        Returns
        -------
        str
            Unique quote identifier.
        """

    @property
    def type(self) -> str:
        """
        Quote variant: ``"inflation_swap"`` or ``"yoy_inflation_swap"``.

        This property does not raise.

        Returns
        -------
        str
            Quote variant: ``"inflation_swap"`` or ``"yoy_inflation_swap"``.
        """

    @property
    def maturity(self) -> datetime.date:
        """
        Swap maturity date.

        This property does not raise.

        Returns
        -------
        datetime.date
            Swap maturity date.
        """

    @property
    def rate(self) -> float:
        """
        Fixed swap rate as a decimal per annum.

        This property does not raise.

        Returns
        -------
        float
            Fixed swap rate as a decimal per annum.
        """

    @property
    def index(self) -> str:
        """
        Inflation index identifier.

        This property does not raise.

        Returns
        -------
        str
            Inflation index identifier.
        """

    @property
    def frequency(self) -> str | None:
        """
        Payment frequency tenor of a year-on-year quote; None for zero-coupon.

        This property does not raise.

        Returns
        -------
        str | None
            Payment frequency tenor of a year-on-year quote; None for zero-coupon.
        """

    @property
    def convention(self) -> str:
        """
        Inflation-swap convention identifier.

        This property does not raise.

        Returns
        -------
        str
            Inflation-swap convention identifier.
        """

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            Compact JSON encoding of this value.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """

    @staticmethod
    def from_json(json: str) -> InflationQuote:
        """
        Rebuild from JSON produced by :meth:`to_json`.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        InflationQuote
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed, carries unknown fields, or fails validation.

        Examples
        --------
        >>> from finstack_quant.calibration import InflationQuote
        >>> InflationQuote.from_json(
        ...     InflationQuote.inflation_swap("zc5y", "2031-05-08", 0.025, "USA-CPI-U", "USD").to_json()
        ... ).id
        'zc5y'
        """

    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...

class XccyQuote:
    """A cross-currency basis-swap market quote for ``xccy_basis`` steps.

    Examples:
    --------
    >>> from finstack_quant.calibration import XccyQuote
    >>> XccyQuote("x5y", "EUR-USD", "5Y", -12.5).basis_spread_bp
    -12.5

    """

    def __init__(
        self,
        id: str,
        convention: str,
        far_pillar: str,
        basis_spread_bp: float,
        spot_fx: float | None = None,
    ) -> None:
        """Build a cross-currency basis-swap quote.

        Parameters
        ----------
        id : str
            Unique quote identifier.
        convention : str
            Cross-currency swap convention identifier, for example ``"EUR-USD"``.
        far_pillar : str | datetime.date | dict
            Swap maturity pillar: tenor code (``"5Y"``), date, or pillar mapping.
        basis_spread_bp : float
            Basis spread in basis points.
        spot_fx : float | None, default None
            Spot FX override (quote currency per base currency, positive);
            the market FX is used when None.

        Raises
        ------
        ValueError
            If ``far_pillar`` cannot be parsed, ``basis_spread_bp`` is not
            finite, or ``spot_fx`` is not positive.

        """

    @property
    def id(self) -> str:
        """
        Unique quote identifier.

        This property does not raise.

        Returns
        -------
        str
            Unique quote identifier.
        """

    @property
    def convention(self) -> str:
        """
        Cross-currency swap convention identifier.

        This property does not raise.

        Returns
        -------
        str
            Cross-currency swap convention identifier.
        """

    @property
    def far_pillar(self) -> dict[str, Any]:
        """
        Maturity pillar as its wire mapping (``{"tenor": ...}`` or ``{"date": ...}``).

        This property does not raise.

        Returns
        -------
        dict[str, Any]
            Maturity pillar as its wire mapping (``{"tenor": ...}`` or ``{"date": ...}``).
        """

    @property
    def basis_spread_bp(self) -> float:
        """
        Basis spread in basis points.

        This property does not raise.

        Returns
        -------
        float
            Basis spread in basis points.
        """

    @property
    def spot_fx(self) -> float | None:
        """
        Spot FX override, or None when the market FX is used.

        This property does not raise.

        Returns
        -------
        float | None
            Spot FX override, or None when the market FX is used.
        """

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            Compact JSON encoding of this value.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """

    @staticmethod
    def from_json(json: str) -> XccyQuote:
        """
        Rebuild from JSON produced by :meth:`to_json`.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        XccyQuote
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed, carries unknown fields, or fails validation.

        Examples
        --------
        >>> from finstack_quant.calibration import XccyQuote
        >>> XccyQuote.from_json(XccyQuote("x5y", "EUR-USD", "5Y", -12.5).to_json()).id
        'x5y'
        """

    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...

class CdsTrancheQuote:
    """A CDS index tranche market quote for ``base_correlation`` steps.

    Examples:
    --------
    >>> from finstack_quant.calibration import CdsTrancheQuote
    >>> CdsTrancheQuote({
    ...     "id": "ig-3-7",
    ...     "index": "CDX.NA.IG",
    ...     "series": 42,
    ...     "attachment": 0.03,
    ...     "detachment": 0.07,
    ...     "maturity": "2031-06-20",
    ...     "upfront_pct": 0.01,
    ...     "coupon_bp": 100.0,
    ...     "convention": {"currency": "USD", "doc_clause": "isda_na"},
    ... }).detachment
    0.07

    """

    def __init__(self, params: dict[str, Any]) -> None:
        """Build a CDS index tranche quote.

        Parameters
        ----------
        params : dict[str, Any]
            Wire fields of the Rust ``CdsTrancheQuote`` as a dict. Every key is required:

            - ``id`` (str): Unique quote identifier.
            - ``index`` (str): Credit index name, for example ``"CDX.NA.IG"``.
            - ``series`` (int): Index series number.
            - ``attachment`` (float): Attachment point as a decimal loss fraction in [0, 1].
            - ``detachment`` (float): Detachment point as a decimal loss fraction in [0, 1],
              above ``attachment``.
            - ``maturity`` (str): Tranche maturity as an ISO-8601 date string.
            - ``upfront_pct`` (float): Upfront payment as a decimal fraction of tranche
              notional, ``|upfront_pct| <= 1``.
            - ``coupon_bp`` (float): Running coupon in basis points per annum (positive).
            - ``convention`` (dict): CDS convention key ``{"currency": ..., "doc_clause": ...}``;
              ``currency`` is an upper-case ISO 4217 code (or a ``Currency``) and ``doc_clause``
              one of ``"cr14"``, ``"mr14"``, ``"mm14"``, ``"xr14"``, ``"isda_na"``, ``"isda_eu"``, ``"isda_as"``, ``"isda_au"``, ``"isda_nz"`` or ``"custom"``.

        Raises
        ------
        ValueError
            If a key is missing or unknown, a date, convention or numeric value is invalid, or
            ``attachment >= detachment``.
        TypeError
            If a ``params`` value is not JSON-serializable (pass ``maturity`` as an ISO string,
            not ``datetime.date``).

        """

    @property
    def id(self) -> str:
        """
        Unique quote identifier.

        This property does not raise.

        Returns
        -------
        str
            Unique quote identifier.
        """

    @property
    def index(self) -> str:
        """
        Credit index name.

        This property does not raise.

        Returns
        -------
        str
            Credit index name.
        """

    @property
    def series(self) -> int:
        """
        Index series number.

        This property does not raise.

        Returns
        -------
        int
            Index series number.
        """

    @property
    def attachment(self) -> float:
        """
        Attachment point as a decimal loss fraction.

        This property does not raise.

        Returns
        -------
        float
            Attachment point as a decimal loss fraction.
        """

    @property
    def detachment(self) -> float:
        """
        Detachment point as a decimal loss fraction.

        This property does not raise.

        Returns
        -------
        float
            Detachment point as a decimal loss fraction.
        """

    @property
    def maturity(self) -> datetime.date:
        """
        Tranche maturity date.

        This property does not raise.

        Returns
        -------
        datetime.date
            Tranche maturity date.
        """

    @property
    def upfront_pct(self) -> float:
        """
        Upfront payment as a decimal fraction of notional.

        This property does not raise.

        Returns
        -------
        float
            Upfront payment as a decimal fraction of notional.
        """

    @property
    def coupon_bp(self) -> float:
        """
        Running coupon in basis points per annum.

        This property does not raise.

        Returns
        -------
        float
            Running coupon in basis points per annum.
        """

    @property
    def convention(self) -> dict[str, Any]:
        """
        CDS convention as ``{"currency": ..., "doc_clause": ...}``.

        This property does not raise.

        Returns
        -------
        dict[str, Any]
            CDS convention as ``{"currency": ..., "doc_clause": ...}``.
        """

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            Compact JSON encoding of this value.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """

    @staticmethod
    def from_json(json: str) -> CdsTrancheQuote:
        """
        Rebuild from JSON produced by :meth:`to_json`.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        CdsTrancheQuote
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed, carries unknown fields, or fails validation.

        Examples
        --------
        >>> from finstack_quant.calibration import CdsTrancheQuote
        >>> CdsTrancheQuote.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...

class FxSpotDatum:
    """An FX spot market datum (``kind == "fx_spot"``).

    ``from_`` carries the Rust ``from`` field, because ``from`` is a Python
    keyword.

    Examples:
    --------
    >>> from finstack_quant.calibration import FxSpotDatum
    >>> FxSpotDatum("EURUSD", "EUR", "USD", 1.1).rate
    1.1

    """

    def __init__(self, id: str, from_: str, to: str, rate: float) -> None:
        """Build an FX spot datum.

        Parameters
        ----------
        id : str
            Stable datum identifier, for example ``"EURUSD"``.
        from_ : str | Currency
            Base currency (``EUR`` in ``EUR/USD``); the Rust ``from`` field.
        to : str | Currency
            Quote currency (``USD`` in ``EUR/USD``).
        rate : float
            Units of ``to`` per one unit of ``from_``.

        Raises
        ------
        ValueError
            If a currency code is unknown.

        """

    @property
    def id(self) -> str:
        """
        Stable datum identifier.

        This property does not raise.

        Returns
        -------
        str
            Stable datum identifier.
        """

    @property
    def from_(self) -> str:
        """
        Base currency code (the Rust ``from`` field).

        This property does not raise.

        Returns
        -------
        str
            Base currency code (the Rust ``from`` field).
        """

    @property
    def to(self) -> str:
        """
        Quote currency code.

        This property does not raise.

        Returns
        -------
        str
            Quote currency code.
        """

    @property
    def rate(self) -> float:
        """
        Units of ``to`` per one unit of ``from_``.

        This property does not raise.

        Returns
        -------
        float
            Units of ``to`` per one unit of ``from_``.
        """

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            Compact JSON encoding of this value.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """

    @staticmethod
    def from_json(json: str) -> FxSpotDatum:
        """
        Rebuild from JSON produced by :meth:`to_json`.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        FxSpotDatum
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.calibration import FxSpotDatum
        >>> FxSpotDatum.from_json(FxSpotDatum("EURUSD", "EUR", "USD", 1.1).to_json()).to
        'USD'
        """

    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...

class PriceDatum:
    """A single-asset spot price market datum (``kind == "price"``).

    Examples:
    --------
    >>> from finstack_quant.calibration import PriceDatum
    >>> PriceDatum("AAPL", 187.5).value
    187.5

    """

    def __init__(self, id: str, value: float | int | Decimal, currency: str | None = None) -> None:
        """Build a spot price datum.

        Parameters
        ----------
        id : str
            Stable datum identifier, for example the asset ticker.
        value : float | int | decimal.Decimal
            Spot value: a unitless scalar without ``currency``, a monetary
            price with one.
        currency : str | Currency | None, default None
            Currency of a monetary price; None stores a unitless scalar.

        Raises
        ------
        ValueError
            If ``value`` is non-finite or the currency is unknown. A unitless
            ``Decimal`` converts to ``float``.

        """

    @property
    def id(self) -> str:
        """
        Stable datum identifier.

        This property does not raise.

        Returns
        -------
        str
            Stable datum identifier.
        """

    @property
    def value(self) -> float | Decimal:
        """
        Spot value: float when unitless, lossless Decimal for a monetary price.

        This property does not raise.

        Returns
        -------
        float | Decimal
            Spot value: float when unitless, lossless Decimal for a monetary price.
        """

    @property
    def currency(self) -> str | None:
        """
        Currency code of a monetary price; None when unitless.

        This property does not raise.

        Returns
        -------
        str | None
            Currency code of a monetary price; None when unitless.
        """

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            Compact JSON encoding of this value.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """

    @staticmethod
    def from_json(json: str) -> PriceDatum:
        """
        Rebuild from JSON produced by :meth:`to_json`.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        PriceDatum
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.calibration import PriceDatum
        >>> PriceDatum.from_json(PriceDatum("AAPL", 187.5).to_json()).id
        'AAPL'
        """

    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...
