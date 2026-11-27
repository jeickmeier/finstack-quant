"""Stub declarations mirroring ``finstack-quant-py/src/bindings/calibration/quotes/rates_vol.rs``."""

from __future__ import annotations

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
    def upfront(params: dict[str, Any]) -> CdsQuote:
        """Build a standard-coupon CDS quote with an upfront payment.

        Parameters
        ----------
        params : dict[str, Any]
            Wire fields of the Rust ``CdsQuote::CdsUpfront`` variant as a dict
            (the ``"type"`` tag is set to ``"cds_upfront"``).
            Every key is required:

            - ``id`` (str): Unique quote identifier, the residual label.
            - ``entity`` (str): Reference entity identifier.
            - ``convention`` (dict): CDS convention key ``{"currency": ..., "doc_clause": ...}``;
              ``currency`` is an upper-case ISO 4217 code (or a ``Currency``) and ``doc_clause``
              one of ``"cr14"``, ``"mr14"``, ``"mm14"``, ``"xr14"``, ``"isda_na"``, ``"isda_eu"``, ``"isda_as"``, ``"isda_au"``, ``"isda_nz"`` or ``"custom"``.
            - ``pillar`` (dict): Maturity pillar in its serde form, a tenor
              ``{"tenor": {"count": 5, "unit": "years"}}`` (units ``"days"``, ``"weeks"``,
              ``"months"``, ``"years"``) or a date ``{"date": "2029-06-20"}``.
            - ``coupon_bp`` (float): Fixed running coupon in basis points per annum (``100.0``
              or ``500.0`` for the standard North American coupons).
            - ``upfront_basis``: Required ``"clean"`` or ``"actual_cash"`` settlement quote basis.
            - ``upfront_pct`` (float): Upfront payment as a decimal fraction of notional,
              positive when the protection buyer pays (``0.023`` for 2.3 points).
            - ``recovery_rate`` (float): Assumed recovery as a decimal fraction of notional in
              ``[0, 1)``.

        Returns
        -------
        CdsQuote
            The typed upfront quote.

        Raises
        ------
        ValueError
            If a key is missing or unknown, a value has the wrong shape (an unknown currency or
            doc clause, a malformed pillar), or the quote fails validation (for example
            ``recovery_rate`` outside [0, 1)).
        TypeError
            If a ``params`` value is not JSON-serializable.

        Examples:
        --------
        >>> from finstack_quant.calibration import CdsQuote
        >>> CdsQuote.upfront({
        ...     "id": "u5y",
        ...     "entity": "ACME",
        ...     "convention": {"currency": "USD", "doc_clause": "xr14"},
        ...     "pillar": {"tenor": {"count": 5, "unit": "years"}},
        ...     "coupon_bp": 100.0,
        ...     "upfront_pct": 0.023,
        ...     "upfront_basis": "clean",
        ...     "recovery_rate": 0.4,
        ... }).type
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
