"""Stub declarations mirroring ``finstack-quant-py/src/bindings/calibration/quotes.rs``."""

import datetime
from decimal import Decimal
from typing import Any
from finstack_quant.core.currency import Currency

class RateQuote:
    """A single interest-rate market quote feeding a curve bootstrap.

    Examples:
    --------
    >>> from finstack_quant.calibration import RateQuote
    >>> RateQuote.deposit("d3m", "USD-SOFR", "3M", 0.0525).value
    0.0525

    """

    @staticmethod
    def deposit(id: str, index: str, pillar: str, rate: float) -> RateQuote:
        """Build a cash deposit quote.

        Parameters
        ----------
        id : str
            Unique quote identifier used as the residual label in reports.
        index : str
            Rate index identifier, for example ``"USD-SOFR"``.
        pillar : str
            Tenor code of the deposit, for example ``"3M"``.
        rate : float
            Deposit rate as a decimal per annum (``0.0525`` for 5.25%).

        Returns
        -------
        RateQuote
            The typed deposit quote.

        Raises
        ------
        ValueError
            If a field is empty or the quote fails Rust-side validation.

        Examples:
        --------
        >>> from finstack_quant.calibration import RateQuote
        >>> RateQuote.deposit("d1m", "USD-SOFR", "1M", 0.053).type
        'deposit'

        """

    @staticmethod
    def fra(id: str, index: str, start: str, end: str, rate: float) -> RateQuote:
        """Build a forward-rate-agreement quote.

        Parameters
        ----------
        id : str
            Unique quote identifier.
        index : str
            Rate index identifier, for example ``"USD-SOFR"``.
        start : str
            Accrual start tenor code, for example ``"3M"``.
        end : str
            Accrual end tenor code, for example ``"6M"``.
        rate : float
            FRA rate as a decimal per annum.

        Returns
        -------
        RateQuote
            The typed FRA quote.

        Raises
        ------
        ValueError
            If a field is empty or the quote fails Rust-side validation.

        Examples:
        --------
        >>> from finstack_quant.calibration import RateQuote
        >>> RateQuote.fra("f36", "USD-SOFR", "3M", "6M", 0.051).type
        'fra'

        """

    @staticmethod
    def futures(
        id: str,
        contract: str,
        expiry: str,
        price: float,
        convexity_adjustment: float | None = None,
    ) -> RateQuote:
        """Build a short-rate futures quote.

        Parameters
        ----------
        id : str
            Unique quote identifier.
        contract : str
            Exchange contract code, for example ``"SR3"``.
        expiry : str
            Contract expiry, as an ISO date or tenor code.
        price : float
            Quoted futures price (``94.75`` implies a 5.25% rate).
        convexity_adjustment : float | None, default None
            Additive convexity adjustment applied to the implied rate, decimal
            per annum. ``None`` uses the Rust authoring default
            ``RateQuote::DEFAULT_CONVEXITY_ADJUSTMENT`` (``0.0``).

        Returns
        -------
        RateQuote
            The typed futures quote.

        Raises
        ------
        ValueError
            If a field is empty or the quote fails Rust-side validation.

        Examples:
        --------
        >>> from finstack_quant.calibration import RateQuote
        >>> RateQuote.futures("sr3z4", "SR3", "2024-12-18", 94.75).type
        'futures'

        """

    @staticmethod
    def swap(
        id: str,
        index: str,
        pillar: str,
        rate: float,
        spread_decimal: float | None = None,
    ) -> RateQuote:
        """Build a par swap quote.

        Parameters
        ----------
        id : str
            Unique quote identifier.
        index : str
            Floating-leg index identifier, for example ``"USD-SOFR"``.
        pillar : str
            Swap tenor code, for example ``"10Y"``.
        rate : float
            Par fixed rate as a decimal per annum.
        spread_decimal : float | None, default None
            Finite additive basis spread on the floating leg, decimal per annum
            (``0.0005`` for 5 bp). Omitted when None; NaN and infinities are rejected.

        Returns
        -------
        RateQuote
            The typed swap quote.

        Raises
        ------
        ValueError
            If the rate or supplied spread is non-finite, or the quote fails
            Rust-side validation.

        Examples:
        --------
        >>> from finstack_quant.calibration import RateQuote
        >>> RateQuote.swap("s10y", "USD-SOFR", "10Y", 0.041).type
        'swap'

        """

    @property
    def id(self) -> str:
        """
        Unique quote identifier used as the residual label.

        This property does not raise.

        Returns
        -------
        str
            Unique quote identifier used as the residual label.
        """

    @property
    def type(self) -> str:
        """
        Quote variant: ``"deposit"``, ``"fra"``, ``"futures"`` or ``"swap"``.

        This property does not raise.

        Returns
        -------
        str
            Quote variant: ``"deposit"``, ``"fra"``, ``"futures"`` or ``"swap"``.
        """

    @property
    def value(self) -> float:
        """
        Quoted value in its native units (rate decimal, or futures price).

        This property does not raise.

        Returns
        -------
        float
            Quoted value in its native units (rate decimal, or futures price).
        """

    @property
    def implied_rate(self) -> float:
        """
        Rate implied by the quote, decimal per annum (futures use 1 - price/100).

        This property does not raise.

        Returns
        -------
        float
            Rate implied by the quote, decimal per annum (futures use 1 - price/100).
        """

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            Compact JSON encoding of this quote.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """

    @staticmethod
    def from_json(json: str) -> RateQuote:
        """
        Rebuild a quote from JSON.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        RateQuote
            The decoded quote.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.calibration import RateQuote
        >>> RateQuote.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...

class CdsQuote:
    """A single-name CDS market quote feeding a hazard-curve bootstrap.

    Examples:
    --------
    >>> from finstack_quant.calibration import CdsQuote
    >>> CdsQuote.par_spread("c5y", "ACME", "USD", "xr14", "5Y", 125.0, 0.4).type
    'cds_par_spread'

    """

    @staticmethod
    def par_spread(
        id: str,
        entity: str,
        currency: str | Currency,
        doc_clause: str,
        pillar: str,
        spread_bp: float,
        recovery_rate: float,
    ) -> CdsQuote:
        """Build a par-spread CDS quote.

        Parameters
        ----------
        id : str
            Unique quote identifier.
        entity : str
            Reference entity identifier.
        currency : str | Currency
            ISO 4217 code of the contract currency.
        doc_clause : str
            ISDA documentation clause, for example ``"xr14"``.
        pillar : str
            Contract tenor code, for example ``"5Y"``.
        spread_bp : float
            Par spread in basis points per annum (``125.0`` for 125 bp).
        recovery_rate : float
            Assumed recovery as a decimal fraction of notional (``0.4`` for 40%).

        Returns
        -------
        CdsQuote
            The typed par-spread quote.

        Raises
        ------
        ValueError
            If a field is empty, ``currency`` is invalid, or ``recovery_rate``
            lies outside [0, 1).
        TypeError
            If ``currency`` is neither a currency-code string nor a ``Currency`` instance.

        Examples:
        --------
        >>> from finstack_quant.calibration import CdsQuote
        >>> CdsQuote.par_spread("c5y", "ACME", "USD", "xr14", "5Y", 125.0, 0.4).id
        'c5y'

        """

    @staticmethod
    def upfront(
        id: str,
        entity: str,
        currency: str | Currency,
        doc_clause: str,
        pillar: str,
        coupon_bp: float,
        upfront_pct: float,
        recovery_rate: float,
    ) -> CdsQuote:
        """Build a standard-coupon CDS quote with an upfront payment.

        Parameters
        ----------
        id : str
            Unique quote identifier.
        entity : str
            Reference entity identifier.
        currency : str | Currency
            ISO 4217 code of the contract currency.
        doc_clause : str
            ISDA documentation clause, for example ``"xr14"``.
        pillar : str
            Contract tenor code, for example ``"5Y"``.
        coupon_bp : float
            Fixed running coupon in basis points per annum (``100.0`` or ``500.0``
            for the standard North American coupons).
        upfront_pct : float
            Upfront payment as a decimal fraction of notional, positive when the
            protection buyer pays (``0.023`` for 2.3 points).
        recovery_rate : float
            Assumed recovery as a decimal fraction of notional.

        Returns
        -------
        CdsQuote
            The typed upfront quote.

        Raises
        ------
        ValueError
            If a field is empty, ``currency`` is invalid, or ``recovery_rate``
            lies outside [0, 1).
        TypeError
            If ``currency`` is neither a currency-code string nor a ``Currency`` instance.

        Examples:
        --------
        >>> from finstack_quant.calibration import CdsQuote
        >>> CdsQuote.upfront("u5y", "ACME", "USD", "xr14", "5Y", 100.0, 0.023, 0.4).type
        'cds_upfront'

        """

    @property
    def id(self) -> str:
        """
        Unique quote identifier used as the residual label.

        This property does not raise.

        Returns
        -------
        str
            Unique quote identifier used as the residual label.
        """

    @property
    def type(self) -> str:
        """
        Quote variant: ``"par_spread"`` or ``"upfront"``.

        This property does not raise.

        Returns
        -------
        str
            Quote variant: ``"par_spread"`` or ``"upfront"``.
        """

    @property
    def entity(self) -> str:
        """
        Reference entity identifier.

        This property does not raise.

        Returns
        -------
        str
            Reference entity identifier.
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

    @property
    def pillar(self) -> dict[str, Any]:
        """
        Maturity pillar as its wire mapping (``{"tenor": ...}`` or ``{"date": ...}``).

        This property does not raise.

        Returns
        -------
        dict[str, Any]
            Maturity pillar as its wire mapping (``{"tenor": ...}`` or ``{"date": ...}``).
        """

    @property
    def recovery_rate(self) -> float:
        """
        Assumed recovery as a decimal fraction of notional.

        This property does not raise.

        Returns
        -------
        float
            Assumed recovery as a decimal fraction of notional.
        """

    @property
    def coupon_bp(self) -> float:
        """
        Running or par spread in basis points per annum.

        This property does not raise.

        Returns
        -------
        float
            Running or par spread in basis points per annum.
        """

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            Compact JSON encoding of this quote.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """

    @staticmethod
    def from_json(json: str) -> CdsQuote:
        """
        Rebuild a quote from JSON.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        CdsQuote
            The decoded quote.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.calibration import CdsQuote
        >>> CdsQuote.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...

class VolQuote:
    """A volatility quote feeding a surface, SABR or SVI fit.

    Examples:
    --------
    >>> from finstack_quant.calibration import VolQuote
    >>> VolQuote.option_vol("v1", "SPX", "2026-01-15", 100.0, 0.2).volatility
    0.2

    """

    @staticmethod
    def option_vol(
        id: str,
        underlying: str,
        expiry: str,
        strike: float,
        vol: float,
        option_type: str | None = None,
    ) -> VolQuote:
        """Build a listed-option implied-volatility quote.

        Parameters
        ----------
        id : str
            Unique quote identifier.
        underlying : str
            Underlying ticker or identifier.
        expiry : str
            Option expiry as an ISO date or tenor code.
        strike : float
            Absolute strike in the underlying's price units.
        vol : float
            Black implied volatility, annualized decimal (``0.2`` for 20%).
        option_type : str | None, default None
            ``"call"`` or ``"put"``. ``None`` uses the Rust authoring default
            ``VolQuote::DEFAULT_OPTION_TYPE`` (``"call"``).

        Returns
        -------
        VolQuote
            The typed option-volatility quote.

        Raises
        ------
        ValueError
            If a field is empty, ``vol`` is not positive, or ``option_type`` is
            not recognized.

        Examples:
        --------
        >>> from finstack_quant.calibration import VolQuote
        >>> VolQuote.option_vol("v1", "SPX", "2026-01-15", 4500.0, 0.18, "put").type
        'option_vol'

        """

    @staticmethod
    def swaption_vol(
        id: str,
        expiry: str,
        maturity: str,
        strike: float,
        vol: float,
        quote_type: str,
        convention: str | None = None,
    ) -> VolQuote:
        """Build a swaption volatility quote.

        Parameters
        ----------
        id : str
            Unique quote identifier.
        expiry : str
            Option expiry date (``datetime.date`` or ISO ``"YYYY-MM-DD"``).
        maturity : str
            Underlying swap maturity date (``datetime.date`` or ISO string).
        strike : float
            Absolute strike rate, decimal per annum.
        vol : float
            Quoted volatility: annualized decimal for lognormal quotes, decimal
            per annum for normal quotes (``0.0085`` for 85 bp).
        quote_type : str
            ``"normal"`` (Bachelier; ``vol`` is an absolute rate volatility) or
            ``"black_lognormal"`` (Black; ``vol`` is a relative volatility).
            Required: it decides the unit of ``vol``, so there is no default.
        convention : str, optional
            Swaption convention key resolved against the convention registry.
            ``None`` uses the Rust authoring default
            ``VolQuote::DEFAULT_SWAPTION_CONVENTION`` (``"USD"``).

        Returns
        -------
        VolQuote
            The typed swaption-volatility quote.

        Raises
        ------
        ValueError
            If a field is empty, ``vol`` is not positive, or ``quote_type`` is
            not recognized.

        Examples:
        --------
        >>> from finstack_quant.calibration import VolQuote
        >>> VolQuote.swaption_vol("s1", "2026-01-15", "2036-01-15", 0.04, 0.0085, "normal").type
        'swaption_vol'

        """

    @staticmethod
    def cap_floor_vol(
        id: str,
        expiry: str,
        strike: float,
        vol: float,
        quote_type: str,
        is_cap: bool | None = None,
    ) -> VolQuote:
        """Build a cap or floor volatility quote.

        Parameters
        ----------
        id : str
            Unique quote identifier.
        expiry : str
            Cap or floor maturity, ISO date or tenor code.
        strike : float
            Absolute strike rate, decimal per annum.
        vol : float
            Quoted flat volatility, in the units implied by ``quote_type``.
        quote_type : str
            ``"normal"`` (Bachelier; ``vol`` is an absolute rate volatility) or
            ``"black_lognormal"`` (Black; ``vol`` is a relative volatility).
            Required: it decides the unit of ``vol``, so there is no default.
        is_cap : bool, optional
            True for a cap, False for a floor. ``None`` uses the Rust authoring
            default ``VolQuote::DEFAULT_IS_CAP`` (a cap).

        Returns
        -------
        VolQuote
            The typed cap/floor volatility quote.

        Raises
        ------
        ValueError
            If a field is empty, ``vol`` is not positive, or ``quote_type`` is
            not recognized.

        Examples:
        --------
        >>> from finstack_quant.calibration import VolQuote
        >>> VolQuote.cap_floor_vol("c1", "2031-01-15", 0.04, 0.0090, "normal").type
        'cap_floor_vol'

        """

    @property
    def id(self) -> str:
        """
        Unique quote identifier used as the residual label.

        This property does not raise.

        Returns
        -------
        str
            Unique quote identifier used as the residual label.
        """

    @property
    def type(self) -> str:
        """
        Quote variant: ``"option_vol"``, ``"swaption_vol"`` or ``"cap_floor_vol"``.

        This property does not raise.

        Returns
        -------
        str
            Quote variant: ``"option_vol"``, ``"swaption_vol"`` or ``"cap_floor_vol"``.
        """

    @property
    def volatility(self) -> float:
        """
        Quoted volatility in its native units.

        This property does not raise.

        Returns
        -------
        float
            Quoted volatility in its native units.
        """

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            Compact JSON encoding of this quote.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """

    @staticmethod
    def from_json(json: str) -> VolQuote:
        """
        Rebuild a quote from JSON.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        VolQuote
            The decoded quote.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.calibration import VolQuote
        >>> VolQuote.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...

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
    >>> CdsTrancheQuote("ig-3-7", "CDX.NA.IG", 42, 0.03, 0.07, "2031-06-20", 0.01, 100.0, "USD", "isda_na").detachment
    0.07

    """

    def __init__(
        self,
        id: str,
        index: str,
        series: int,
        attachment: float,
        detachment: float,
        maturity: str,
        upfront_pct: float,
        coupon_bp: float,
        currency: str | Currency,
        doc_clause: str,
    ) -> None:
        """Build a CDS index tranche quote.

        Parameters
        ----------
        id : str
            Unique quote identifier.
        index : str
            Credit index name, for example ``"CDX.NA.IG"``.
        series : int
            Index series number.
        attachment : float
            Attachment point as a decimal loss fraction in [0, 1].
        detachment : float
            Detachment point as a decimal loss fraction in [0, 1], above
            ``attachment``.
        maturity : str | datetime.date
            Tranche maturity date.
        upfront_pct : float
            Upfront payment as a decimal fraction of notional, ``|upfront_pct| <= 1``.
        coupon_bp : float
            Running coupon in basis points per annum (positive).
        currency : str | Currency
            ISO 4217 code of the contract currency.
        doc_clause : str
            ISDA documentation clause, for example ``"isda_na"``.

        Raises
        ------
        ValueError
            If a date, convention or numeric input is invalid, or
            ``attachment >= detachment``.
        TypeError
            If ``currency`` is neither a currency-code string nor a ``Currency`` instance.

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
