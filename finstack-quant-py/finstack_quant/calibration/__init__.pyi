"""Quote ingestion, market construction, and explicit model calibration.

Rust owns the calibration plan schema, its validation, and every solver. Python
exposes typed authoring classes over the same serde types, so an envelope can be
built in code (``RateQuote`` / ``CdsQuote`` / ``VolQuote`` -> ``CalibrationStep``
-> ``CalibrationPlan`` -> ``CalibrationEnvelope``) or handed to the entry points
as a ``dict`` or JSON string.

Examples:
--------
>>> from finstack_quant.calibration import CalibrationPlan, calibrate
>>> result = calibrate(CalibrationPlan([], id="smoke"))
>>> result.success
True
"""

from typing import Any

import pandas as pd

from finstack_quant.calibration import hull_white as hull_white
from finstack_quant.calibration import schema as schema
from finstack_quant.core.market_data import MarketContext

__all__ = [
    "CalibrationConfig",
    "CalibrationDiagnostics",
    "CalibrationEnvelope",
    "CalibrationEnvelopeError",
    "CalibrationPlan",
    "CalibrationReport",
    "CalibrationResult",
    "CalibrationStep",
    "CalibrationValidationReport",
    "CdsQuote",
    "QuoteQuality",
    "RateBounds",
    "RateQuote",
    "SolverConfig",
    "ValidationConfig",
    "VolQuote",
    "calibrate",
    "calibrate_bermudan_lmm_base_vol",
    "dry_run",
    "dry_run_json",
    "hull_white",
    "schema",
    "validate_calibration",
    "validate_calibration_json",
]

class SolverConfig:
    """Root-finder settings shared by every bootstrap step.

    Examples:
    --------
    >>> from finstack_quant.calibration import SolverConfig
    >>> SolverConfig(tolerance=1e-10).tolerance
    1e-10

    """

    def __init__(self, tolerance: float | None = None, max_iterations: int | None = None) -> None:
        """Build a solver configuration.

        Parameters
        ----------
        tolerance : float | None
            Absolute residual tolerance in the target's own units (decimal rate,
            price, or volatility depending on the step). Rust default when None.
        max_iterations : int | None
            Maximum root-finder iterations per pillar. Rust default when None.

        Raises
        ------
        ValueError
            If ``tolerance`` is not strictly positive or ``max_iterations`` is zero.

        """

    @property
    def tolerance(self) -> float:
        """
        Absolute residual tolerance in the target's own units.

        This property does not raise.

        Returns
        -------
        float
            Absolute residual tolerance in the target's own units.
        """

    @property
    def max_iterations(self) -> int:
        """
        Maximum root-finder iterations per pillar.

        This property does not raise.

        Returns
        -------
        int
            Maximum root-finder iterations per pillar.
        """

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            Compact JSON encoding of this configuration.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """

    @staticmethod
    def from_json(json: str) -> SolverConfig:
        """
        Rebuild a configuration from JSON.

        Parameters
        ----------
        json : str
            Compact or pretty JSON produced by :meth:`to_json`.

        Returns
        -------
        SolverConfig
            The decoded configuration.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.calibration import SolverConfig
        >>> SolverConfig.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...

class RateBounds:
    """Admissible zero-rate bracket used to guard bootstrap solves.

    Examples:
    --------
    >>> from finstack_quant.calibration import RateBounds
    >>> RateBounds(-0.02, 0.25).max_rate
    0.25

    """

    def __init__(self, min_rate: float, max_rate: float) -> None:
        """Build explicit rate bounds.

        Parameters
        ----------
        min_rate : float
            Lower bracket for the solved zero rate, decimal per annum
            (for example ``-0.02`` for -200 bp).
        max_rate : float
            Upper bracket for the solved zero rate, decimal per annum.

        Raises
        ------
        ValueError
            If ``min_rate`` is not strictly below ``max_rate``.

        """

    @staticmethod
    def for_currency(currency: str) -> RateBounds:
        """Conventional bounds for a currency.

        Parameters
        ----------
        currency : str
            ISO 4217 alphabetic code, for example ``"USD"``.

        Returns
        -------
        RateBounds
            Bounds appropriate for that currency's rate regime.

        Raises
        ------
        ValueError
            If ``currency`` is not a valid ISO 4217 code.

        Examples:
        --------
        >>> from finstack_quant.calibration import RateBounds
        >>> RateBounds.for_currency("USD").min_rate <= 0.0
        True

        """

    @staticmethod
    def emerging_markets() -> RateBounds:
        """Wide bounds suited to high-inflation emerging-market curves.

        Returns
        -------
        RateBounds
            Bounds wide enough for triple-digit percentage rates.

        This preset is a compile-time constant and does not raise.

        Examples:
        --------
        >>> from finstack_quant.calibration import RateBounds
        >>> RateBounds.emerging_markets().max_rate > 1.0
        True

        """

    @property
    def min_rate(self) -> float:
        """
        Lower zero-rate bracket, decimal per annum.

        This property does not raise.

        Returns
        -------
        float
            Lower zero-rate bracket, decimal per annum.
        """

    @property
    def max_rate(self) -> float:
        """
        Upper zero-rate bracket, decimal per annum.

        This property does not raise.

        Returns
        -------
        float
            Upper zero-rate bracket, decimal per annum.
        """

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            Compact JSON encoding of these bounds.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """

    @staticmethod
    def from_json(json: str) -> RateBounds:
        """
        Rebuild bounds from JSON.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        RateBounds
            The decoded bounds.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.calibration import RateBounds
        >>> RateBounds.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...

class ValidationConfig:
    """Post-solve acceptance thresholds applied to each calibrated step.

    Examples:
    --------
    >>> from finstack_quant.calibration import ValidationConfig
    >>> isinstance(ValidationConfig().to_dict(), dict)
    True

    """

    def __init__(self, **overrides: Any) -> None:
        """Build validation thresholds from the Rust defaults plus overrides.

        Parameters
        ----------
        overrides : Any
            Field overrides applied on top of the Rust defaults, named exactly as
            the serde field names reported by :meth:`to_dict` (for example
            ``max_df_deviation``). Values use the units of the field they set.

        Raises
        ------
        ValueError
            If an override names an unknown field or has the wrong type.

        """

    def to_dict(self) -> dict[str, Any]:
        """Return the thresholds as a plain dictionary.

        Returns
        -------
        dict[str, Any]
            Serde field names mapped to their current values.

        Raises
        ------
        ValueError
            If the configuration cannot be serialized to its wire form.

        """

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            Compact JSON encoding of these thresholds.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """

    @staticmethod
    def from_json(json: str) -> ValidationConfig:
        """
        Rebuild thresholds from JSON.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        ValidationConfig
            The decoded thresholds.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.calibration import ValidationConfig
        >>> ValidationConfig.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...

class CalibrationConfig:
    """Plan-level solver, parallelism and acceptance settings.

    Examples:
    --------
    >>> from finstack_quant.calibration import CalibrationConfig
    >>> CalibrationConfig(max_iterations=64).max_iterations
    64

    """

    def __init__(
        self,
        *,
        tolerance: float | None = None,
        max_iterations: int | None = None,
        solver: SolverConfig | None = None,
        use_parallel: bool | None = None,
        fail_on_bad_fit: bool | None = None,
        compute_diagnostics: bool | None = None,
        validation_mode: str | None = None,
        rate_bounds: RateBounds | None = None,
        validation: ValidationConfig | None = None,
    ) -> None:
        """Build a plan-level calibration configuration.

        Parameters
        ----------
        tolerance : float | None
            Absolute residual tolerance in each target's own units; Rust default
            when None.
        max_iterations : int | None
            Maximum root-finder iterations per pillar; Rust default when None.
        solver : SolverConfig | None
            Complete solver block, overriding ``tolerance``/``max_iterations``.
        use_parallel : bool | None
            Run independent steps in parallel. Decimal-mode results are identical
            either way.
        fail_on_bad_fit : bool | None
            Raise instead of returning a result when a step misses its tolerance.
        compute_diagnostics : bool | None
            Populate per-quote diagnostics (condition number, singular values).
        validation_mode : str | None
            Post-solve validation severity, ``"warn"`` or ``"error"``.
        rate_bounds : RateBounds | None
            Admissible zero-rate bracket for bootstrap solves.
        validation : ValidationConfig | None
            Post-solve acceptance thresholds.

        Raises
        ------
        ValueError
            If a value is out of range or ``validation_mode`` is not recognized.

        """

    @property
    def tolerance(self) -> float:
        """
        Absolute residual tolerance in each target's own units.

        This property does not raise.

        Returns
        -------
        float
            Absolute residual tolerance in each target's own units.
        """

    @property
    def max_iterations(self) -> int:
        """
        Maximum root-finder iterations per pillar.

        This property does not raise.

        Returns
        -------
        int
            Maximum root-finder iterations per pillar.
        """

    @property
    def solver(self) -> SolverConfig:
        """
        The solver block.

        This property does not raise.

        Returns
        -------
        SolverConfig
            The solver block.
        """

    @property
    def use_parallel(self) -> bool:
        """
        Whether independent steps run in parallel.

        This property does not raise.

        Returns
        -------
        bool
            Whether independent steps run in parallel.
        """

    @property
    def fail_on_bad_fit(self) -> bool:
        """
        Whether a missed step tolerance raises instead of returning.

        This property does not raise.

        Returns
        -------
        bool
            Whether a missed step tolerance raises instead of returning.
        """

    @property
    def compute_diagnostics(self) -> bool:
        """
        Whether per-quote diagnostics are computed.

        This property does not raise.

        Returns
        -------
        bool
            Whether per-quote diagnostics are computed.
        """

    @property
    def validation_mode(self) -> str:
        """
        Post-solve validation severity, ``"warn"`` or ``"error"``.

        This property does not raise.

        Returns
        -------
        str
            Post-solve validation severity, ``"warn"`` or ``"error"``.
        """

    @property
    def rate_bounds(self) -> RateBounds:
        """
        Admissible zero-rate bracket.

        This property does not raise.

        Returns
        -------
        RateBounds
            Admissible zero-rate bracket.
        """

    @property
    def validation(self) -> ValidationConfig:
        """
        Post-solve acceptance thresholds.

        This property does not raise.

        Returns
        -------
        ValidationConfig
            Post-solve acceptance thresholds.
        """

    def to_dict(self) -> dict[str, Any]:
        """Return the settings as a plain dictionary.

        Returns
        -------
        dict[str, Any]
            Serde field names mapped to their current values.

        Raises
        ------
        ValueError
            If the configuration cannot be serialized to its wire form.

        """

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            Compact JSON encoding of these settings.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """

    @staticmethod
    def from_json(json: str) -> CalibrationConfig:
        """
        Rebuild settings from JSON.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        CalibrationConfig
            The decoded settings.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.calibration import CalibrationConfig
        >>> CalibrationConfig.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...

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
        convexity_adjustment: float = 0.0,
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
        convexity_adjustment : float, default 0.0
            Additive convexity adjustment applied to the implied rate, decimal
            per annum.

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
            Additive basis spread on the floating leg, decimal per annum
            (``0.0005`` for 5 bp). Omitted when None.

        Returns
        -------
        RateQuote
            The typed swap quote.

        Raises
        ------
        ValueError
            If a field is empty or the quote fails Rust-side validation.

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
        currency: str,
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
        currency : str
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
        currency: str,
        doc_clause: str,
        pillar: str,
        running_spread_bp: float,
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
        currency : str
            ISO 4217 code of the contract currency.
        doc_clause : str
            ISDA documentation clause, for example ``"xr14"``.
        pillar : str
            Contract tenor code, for example ``"5Y"``.
        running_spread_bp : float
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
    def running_spread_bp(self) -> float:
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
        option_type: str = "call",
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
        option_type : str, default "call"
            ``"call"`` or ``"put"``.

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
        quote_type: str = "normal",
        convention: str = "USD",
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
        quote_type : str, default "normal"
            ``"normal"`` (Bachelier) or ``"lognormal"`` (Black).
        convention : str, default "USD"
            Swaption convention key resolved against the convention registry.

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
        >>> VolQuote.swaption_vol("s1", "2026-01-15", "2036-01-15", 0.04, 0.0085).type
        'swaption_vol'

        """

    @staticmethod
    def cap_floor_vol(
        id: str,
        expiry: str,
        strike: float,
        vol: float,
        quote_type: str = "normal",
        is_cap: bool = True,
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
        quote_type : str, default "normal"
            ``"normal"`` (Bachelier) or ``"lognormal"`` (Black).
        is_cap : bool, default True
            True for a cap, False for a floor.

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
        >>> VolQuote.cap_floor_vol("c1", "2031-01-15", 0.04, 0.0090).type
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

type Quote = RateQuote | CdsQuote | VolQuote

class CalibrationStep:
    """One calibration step: a target kind, its parameters, and its quotes.

    Each constructor corresponds to one Rust ``StepParams`` variant. Quotes may
    be attached inline via ``quotes`` (they become the step's own quote set) or
    referenced by name via ``quote_set``.

    Examples:
    --------
    >>> from finstack_quant.calibration import CalibrationStep, RateQuote
    >>> step = CalibrationStep.discount(
    ...     "usd_ois",
    ...     "USD",
    ...     "2024-06-28",
    ...     quotes=[RateQuote.deposit("d3m", "USD-SOFR", "3M", 0.0525)],
    ... )
    >>> step.id
    'usd_ois'

    """

    @staticmethod
    def discount(
        id: str,
        currency: str,
        base_date: str,
        quotes: list[Quote] | None = None,
        quote_set: str | None = None,
        curve_id: str | None = None,
        **params: Any,
    ) -> CalibrationStep:
        """Build a discount-curve bootstrap step.

        Parameters
        ----------
        id : str
            Unique step identifier; also the residual key in the result.
        currency : str
            ISO 4217 code of the curve currency.
        base_date : str
            Curve base (spot) date as an ISO date string.
        quotes : list[RateQuote | CdsQuote | VolQuote] | None, default None
            Quotes attached inline as this step's own quote set.
        quote_set : str | None, default None
            Name of a shared quote set declared on the plan, used instead of
            ``quotes``.
        curve_id : str | None, default None
            Identifier of the produced curve; defaults to ``id``.
        params : Any
            Extra ``StepParams`` fields such as ``interpolation``; names and
            units follow the Rust step schema.

        Returns
        -------
        CalibrationStep
            The typed step.

        Raises
        ------
        ValueError
            If neither or both of ``quotes`` and ``quote_set`` are given, or a
            parameter is unknown to the Rust step schema.

        Examples:
        --------
        >>> from finstack_quant.calibration import CalibrationStep, RateQuote
        >>> CalibrationStep.discount(
        ...     "usd_ois",
        ...     "USD",
        ...     "2024-06-28",
        ...     quotes=[RateQuote.swap("s2y", "USD-SOFR", "2Y", 0.043)],
        ... ).kind
        'discount'

        """

    @staticmethod
    def forward(
        id: str,
        currency: str,
        base_date: str,
        tenor_years: float,
        discount_curve_id: str,
        quotes: list[Quote] | None = None,
        quote_set: str | None = None,
        curve_id: str | None = None,
        **params: Any,
    ) -> CalibrationStep:
        """Build a projection (forward) curve bootstrap step.

        Parameters
        ----------
        id : str
            Unique step identifier.
        currency : str
            ISO 4217 code of the curve currency.
        base_date : str
            Curve base date as an ISO date string.
        tenor_years : float
            Index tenor in years (``0.25`` for a 3M index).
        discount_curve_id : str
            Identifier of the discount curve solved in an earlier step.
        quotes : list[RateQuote | CdsQuote | VolQuote] | None, default None
            Quotes attached inline as this step's own quote set.
        quote_set : str | None, default None
            Name of a shared quote set declared on the plan.
        curve_id : str | None, default None
            Identifier of the produced curve; defaults to ``id``.
        params : Any
            Extra ``StepParams`` fields following the Rust step schema.

        Returns
        -------
        CalibrationStep
            The typed step.

        Raises
        ------
        ValueError
            If quote wiring is ambiguous or a parameter is unknown.

        Examples:
        --------
        >>> from finstack_quant.calibration import CalibrationStep, RateQuote
        >>> CalibrationStep.forward(
        ...     "usd_3m",
        ...     "USD",
        ...     "2024-06-28",
        ...     0.25,
        ...     "usd_ois",
        ...     quotes=[RateQuote.swap("s2y", "USD-LIBOR-3M", "2Y", 0.044)],
        ... ).kind
        'forward'

        """

    @staticmethod
    def hazard(
        id: str,
        entity: str,
        currency: str,
        base_date: str,
        discount_curve_id: str,
        recovery_rate: float,
        seniority: str = "senior",
        quotes: list[Quote] | None = None,
        quote_set: str | None = None,
        curve_id: str | None = None,
        **params: Any,
    ) -> CalibrationStep:
        """Build a credit hazard-curve bootstrap step.

        Parameters
        ----------
        id : str
            Unique step identifier.
        entity : str
            Reference entity identifier.
        currency : str
            ISO 4217 code of the contract currency.
        base_date : str
            Curve base date as an ISO date string.
        discount_curve_id : str
            Identifier of the discount curve solved in an earlier step.
        recovery_rate : float
            Assumed recovery as a decimal fraction of notional.
        seniority : str, default "senior"
            Debt seniority tier recorded on the curve.
        quotes : list[RateQuote | CdsQuote | VolQuote] | None, default None
            CDS quotes attached inline.
        quote_set : str | None, default None
            Name of a shared quote set declared on the plan.
        curve_id : str | None, default None
            Identifier of the produced curve; defaults to ``id``.
        params : Any
            Extra ``StepParams`` fields following the Rust step schema.

        Returns
        -------
        CalibrationStep
            The typed step.

        Raises
        ------
        ValueError
            If quote wiring is ambiguous, ``recovery_rate`` is outside [0, 1),
            or a parameter is unknown.

        Examples:
        --------
        >>> from finstack_quant.calibration import CalibrationStep, CdsQuote
        >>> CalibrationStep.hazard(
        ...     "acme_hz",
        ...     "ACME",
        ...     "USD",
        ...     "2024-06-28",
        ...     "usd_ois",
        ...     0.4,
        ...     quotes=[CdsQuote.par_spread("c5y", "ACME", "USD", "xr14", "5Y", 125.0, 0.4)],
        ... ).kind
        'hazard'

        """

    @staticmethod
    def inflation(
        id: str,
        currency: str,
        base_date: str,
        discount_curve_id: str,
        index: str,
        observation_lag: str,
        base_cpi: float,
        quotes: list[Quote] | None = None,
        quote_set: str | None = None,
        curve_id: str | None = None,
        **params: Any,
    ) -> CalibrationStep:
        """Build an inflation-curve bootstrap step.

        Parameters
        ----------
        id : str
            Unique step identifier.
        currency : str
            ISO 4217 code of the curve currency.
        base_date : str
            Curve base date as an ISO date string.
        discount_curve_id : str
            Identifier of the discount curve solved in an earlier step.
        index : str
            Inflation index identifier, for example ``"US-CPI-U"``.
        observation_lag : str
            Index observation lag as a tenor code, for example ``"3M"``.
        base_cpi : float
            Index level at ``base_date``, in index points.
        quotes : list[RateQuote | CdsQuote | VolQuote] | None, default None
            Zero-coupon inflation swap quotes attached inline.
        quote_set : str | None, default None
            Name of a shared quote set declared on the plan.
        curve_id : str | None, default None
            Identifier of the produced curve; defaults to ``id``.
        params : Any
            Extra ``StepParams`` fields following the Rust step schema.

        Returns
        -------
        CalibrationStep
            The typed step.

        Raises
        ------
        ValueError
            If quote wiring is ambiguous or a parameter is unknown.

        Examples:
        --------
        >>> from finstack_quant.calibration import CalibrationStep, RateQuote
        >>> CalibrationStep.inflation(
        ...     "us_cpi",
        ...     "USD",
        ...     "2024-06-28",
        ...     "usd_ois",
        ...     "US-CPI-U",
        ...     "3M",
        ...     310.0,
        ...     quotes=[RateQuote.swap("z5y", "US-CPI-U", "5Y", 0.024)],
        ... ).kind
        'inflation'

        """

    @staticmethod
    def vol_surface(
        id: str,
        base_date: str,
        underlying_ticker: str,
        model: str = "sabr",
        quotes: list[Quote] | None = None,
        quote_set: str | None = None,
        vol_surface_id: str | None = None,
        **params: Any,
    ) -> CalibrationStep:
        """Build an option volatility-surface fit step.

        Parameters
        ----------
        id : str
            Unique step identifier.
        base_date : str
            Surface base date as an ISO date string.
        underlying_ticker : str
            Underlying ticker the surface belongs to.
        model : str, default "sabr"
            Surface model key, for example ``"sabr"``.
        quotes : list[RateQuote | CdsQuote | VolQuote] | None, default None
            Volatility quotes attached inline.
        quote_set : str | None, default None
            Name of a shared quote set declared on the plan.
        vol_surface_id : str | None, default None
            Identifier of the produced surface; defaults to ``id``.
        params : Any
            Extra ``StepParams`` fields following the Rust step schema.

        Returns
        -------
        CalibrationStep
            The typed step.

        Raises
        ------
        ValueError
            If quote wiring is ambiguous or a parameter is unknown.

        Examples:
        --------
        >>> from finstack_quant.calibration import CalibrationStep, VolQuote
        >>> CalibrationStep.vol_surface(
        ...     "spx_vol",
        ...     "2024-06-28",
        ...     "SPX",
        ...     quotes=[VolQuote.option_vol("v1", "SPX", "2026-01-15", 4500.0, 0.18)],
        ... ).kind
        'vol_surface'

        """

    @staticmethod
    def swaption_vol(
        id: str,
        base_date: str,
        discount_curve_id: str,
        currency: str,
        quotes: list[Quote] | None = None,
        quote_set: str | None = None,
        vol_surface_id: str | None = None,
        **params: Any,
    ) -> CalibrationStep:
        """Build a swaption volatility-cube fit step.

        Parameters
        ----------
        id : str
            Unique step identifier.
        base_date : str
            Surface base date as an ISO date string.
        discount_curve_id : str
            Identifier of the discount curve solved in an earlier step.
        currency : str
            ISO 4217 code of the swaption currency.
        quotes : list[RateQuote | CdsQuote | VolQuote] | None, default None
            Swaption volatility quotes attached inline.
        quote_set : str | None, default None
            Name of a shared quote set declared on the plan.
        vol_surface_id : str | None, default None
            Identifier of the produced surface; defaults to ``id``.
        params : Any
            Extra ``StepParams`` fields following the Rust step schema.

        Returns
        -------
        CalibrationStep
            The typed step.

        Raises
        ------
        ValueError
            If quote wiring is ambiguous or a parameter is unknown.

        Examples:
        --------
        >>> from finstack_quant.calibration import CalibrationStep, VolQuote
        >>> CalibrationStep.swaption_vol(
        ...     "usd_swpn",
        ...     "2024-06-28",
        ...     "usd_ois",
        ...     "USD",
        ...     quotes=[VolQuote.swaption_vol("s1", "2026-01-15", "2036-01-15", 0.04, 0.0085)],
        ... ).kind
        'swaption_vol'

        """

    @staticmethod
    def base_correlation(
        id: str,
        index_id: str,
        series: int,
        maturity_years: float,
        base_date: str,
        discount_curve_id: str,
        currency: str,
        quotes: list[Quote] | None = None,
        quote_set: str | None = None,
        **params: Any,
    ) -> CalibrationStep:
        """Build an index-tranche base-correlation bootstrap step.

        Parameters
        ----------
        id : str
            Unique step identifier.
        index_id : str
            Credit index identifier, for example ``"CDX.NA.IG"``.
        series : int
            Index series number.
        maturity_years : float
            Tranche maturity in years.
        base_date : str
            Curve base date as an ISO date string.
        discount_curve_id : str
            Identifier of the discount curve solved in an earlier step.
        currency : str
            ISO 4217 code of the tranche currency.
        quotes : list[RateQuote | CdsQuote | VolQuote] | None, default None
            Tranche quotes attached inline.
        quote_set : str | None, default None
            Name of a shared quote set declared on the plan.
        params : Any
            Extra ``StepParams`` fields following the Rust step schema.

        Returns
        -------
        CalibrationStep
            The typed step.

        Raises
        ------
        ValueError
            If quote wiring is ambiguous or a parameter is unknown.

        Examples:
        --------
        >>> from finstack_quant.calibration import CalibrationStep
        >>> CalibrationStep.base_correlation(
        ...     "cdx_bc",
        ...     "CDX.NA.IG",
        ...     42,
        ...     5.0,
        ...     "2024-06-28",
        ...     "usd_ois",
        ...     "USD",
        ...     quote_set="tranches",
        ... ).kind
        'base_correlation'

        """

    @staticmethod
    def student_t(
        id: str,
        tranche_instrument_id: str,
        base_correlation_curve_id: str,
        quotes: list[Quote] | None = None,
        quote_set: str | None = None,
        **params: Any,
    ) -> CalibrationStep:
        """Build a Student-t copula degrees-of-freedom fit step.

        Parameters
        ----------
        id : str
            Unique step identifier.
        tranche_instrument_id : str
            Identifier of the tranche instrument repriced during the fit.
        base_correlation_curve_id : str
            Identifier of the base-correlation curve solved in an earlier step.
        quotes : list[RateQuote | CdsQuote | VolQuote] | None, default None
            Tranche quotes attached inline.
        quote_set : str | None, default None
            Name of a shared quote set declared on the plan.
        params : Any
            Extra ``StepParams`` fields following the Rust step schema.

        Returns
        -------
        CalibrationStep
            The typed step.

        Raises
        ------
        ValueError
            If quote wiring is ambiguous or a parameter is unknown.

        Examples:
        --------
        >>> from finstack_quant.calibration import CalibrationStep
        >>> CalibrationStep.student_t("t", "tr_0_3", "cdx_bc", quote_set="tranches").kind
        'student_t'

        """

    @staticmethod
    def hull_white(
        id: str,
        curve_id: str,
        currency: str,
        base_date: str,
        quotes: list[Quote] | None = None,
        quote_set: str | None = None,
        **params: Any,
    ) -> CalibrationStep:
        """Build a Hull-White swaption calibration step.

        Parameters
        ----------
        id : str
            Unique step identifier.
        curve_id : str
            Identifier of the discount curve the model is fitted against.
        currency : str
            ISO 4217 code of the model currency.
        base_date : str
            Model base date as an ISO date string.
        quotes : list[RateQuote | CdsQuote | VolQuote] | None, default None
            ATM swaption volatility quotes attached inline. Calibration rejects
            strikes differing from the contractual forward by more than 1e-8
            in decimal rate units (0.0001 bp).
        quote_set : str | None, default None
            Name of a shared quote set declared on the plan.
        params : Any
            Extra ``StepParams`` fields following the Rust step schema. Required
            ``fit_tolerance`` is a finite positive per-target tolerance in the
            supplied volatility quote units, separate from solver tolerance.

        Returns
        -------
        CalibrationStep
            The typed step.

        Raises
        ------
        ValueError
            If quote wiring is ambiguous, a parameter is unknown, or the required
            ``fit_tolerance`` is missing, non-finite or non-positive.

        Examples:
        --------
        >>> from finstack_quant.calibration import CalibrationStep
        >>> CalibrationStep.hull_white(
        ...     "hw", "usd_ois", "USD", "2024-06-28", quote_set="swaptions", fit_tolerance=1e-4
        ... ).kind
        'hull_white'

        """

    @staticmethod
    def cap_floor_hull_white(
        id: str,
        discount_curve_id: str,
        forward_curve_id: str,
        currency: str,
        base_date: str,
        quotes: list[Quote] | None = None,
        quote_set: str | None = None,
        **params: Any,
    ) -> CalibrationStep:
        """Build a Hull-White cap/floor calibration step.

        Parameters
        ----------
        id : str
            Unique step identifier.
        discount_curve_id : str
            Identifier of the discounting curve.
        forward_curve_id : str
            Identifier of the curve projecting the caplet forwards.
        currency : str
            ISO 4217 code of the model currency.
        base_date : str
            Model base date as an ISO date string.
        quotes : list[RateQuote | CdsQuote | VolQuote] | None, default None
            Cap/floor volatility quotes attached inline.
        quote_set : str | None, default None
            Name of a shared quote set declared on the plan.
        params : Any
            Extra ``StepParams`` fields following the Rust step schema. Required
            ``fit_tolerance`` is a finite positive per-target tolerance in the
            supplied volatility quote units, separate from solver tolerance.

        Returns
        -------
        CalibrationStep
            The typed step.

        Raises
        ------
        ValueError
            If quote wiring is ambiguous, a parameter is unknown, or the required
            ``fit_tolerance`` is missing, non-finite or non-positive.

        Examples:
        --------
        >>> from finstack_quant.calibration import CalibrationStep
        >>> CalibrationStep.cap_floor_hull_white(
        ...     "hw_cf",
        ...     "usd_ois",
        ...     "usd_3m",
        ...     "USD",
        ...     "2024-06-28",
        ...     quote_set="caps",
        ...     fit_tolerance=1e-4,
        ... ).kind
        'cap_floor_hull_white'

        """

    @staticmethod
    def svi_surface(
        id: str,
        base_date: str,
        underlying_ticker: str,
        quotes: list[Quote] | None = None,
        quote_set: str | None = None,
        vol_surface_id: str | None = None,
        **params: Any,
    ) -> CalibrationStep:
        """Build an SVI volatility-surface fit step.

        Parameters
        ----------
        id : str
            Unique step identifier.
        base_date : str
            Surface base date as an ISO date string.
        underlying_ticker : str
            Underlying ticker the surface belongs to.
        quotes : list[RateQuote | CdsQuote | VolQuote] | None, default None
            Volatility quotes attached inline.
        quote_set : str | None, default None
            Name of a shared quote set declared on the plan.
        vol_surface_id : str | None, default None
            Identifier of the produced surface; defaults to ``id``.
        params : Any
            Extra ``StepParams`` fields following the Rust step schema.

        Returns
        -------
        CalibrationStep
            The typed step.

        Raises
        ------
        ValueError
            If quote wiring is ambiguous or a parameter is unknown.

        Examples:
        --------
        >>> from finstack_quant.calibration import CalibrationStep, VolQuote
        >>> CalibrationStep.svi_surface(
        ...     "spx_svi",
        ...     "2024-06-28",
        ...     "SPX",
        ...     quotes=[VolQuote.option_vol("v1", "SPX", "2026-01-15", 4500.0, 0.18)],
        ... ).kind
        'svi_surface'

        """

    @staticmethod
    def xccy_basis(
        id: str,
        currency: str,
        base_date: str,
        fx_spot: float,
        domestic_discount_id: str,
        quotes: list[Quote] | None = None,
        quote_set: str | None = None,
        curve_id: str | None = None,
        **params: Any,
    ) -> CalibrationStep:
        """Build a cross-currency basis discount-curve bootstrap step.

        Parameters
        ----------
        id : str
            Unique step identifier.
        currency : str
            ISO 4217 code of the foreign (solved) currency.
        base_date : str
            Curve base date as an ISO date string.
        fx_spot : float
            Spot FX rate quoted as foreign units per one domestic unit.
        domestic_discount_id : str
            Identifier of the domestic discount curve solved in an earlier step.
        quotes : list[RateQuote | CdsQuote | VolQuote] | None, default None
            Cross-currency basis swap quotes attached inline.
        quote_set : str | None, default None
            Name of a shared quote set declared on the plan.
        curve_id : str | None, default None
            Identifier of the produced curve; defaults to ``id``.
        params : Any
            Extra ``StepParams`` fields following the Rust step schema.

        Returns
        -------
        CalibrationStep
            The typed step.

        Raises
        ------
        ValueError
            If quote wiring is ambiguous or a parameter is unknown.

        Examples:
        --------
        >>> from finstack_quant.calibration import CalibrationStep
        >>> CalibrationStep.xccy_basis(
        ...     "eur_xccy",
        ...     "EUR",
        ...     "2024-06-28",
        ...     0.92,
        ...     "usd_ois",
        ...     quote_set="basis",
        ... ).kind
        'xccy_basis'

        """

    @staticmethod
    def parametric(
        id: str,
        base_date: str,
        model: str = "ns",
        quotes: list[Quote] | None = None,
        quote_set: str | None = None,
        curve_id: str | None = None,
        **params: Any,
    ) -> CalibrationStep:
        """Build a parametric-curve fit step.

        Parameters
        ----------
        id : str
            Unique step identifier.
        base_date : str
            Curve base date as an ISO date string.
        model : str, default "ns"
            Parametric family key: ``"ns"`` for Nelson-Siegel or ``"nss"``
            for Nelson-Siegel-Svensson.
        quotes : list[RateQuote | CdsQuote | VolQuote] | None, default None
            Rate quotes attached inline.
        quote_set : str | None, default None
            Name of a shared quote set declared on the plan.
        curve_id : str | None, default None
            Identifier of the produced curve; defaults to ``id``.
        params : Any
            Optional ``initial_params`` following the Rust step schema. Only
            single-curve discount fitting is supported; ``discount_curve_id``
            is not an accepted parameter. Fit acceptance honors the discount-curve
            tolerance in per-notional PV units without an implicit floor.

        Returns
        -------
        CalibrationStep
            The typed step.

        Raises
        ------
        ValueError
            If quote wiring is ambiguous or a parameter is unknown.

        Examples:
        --------
        >>> from finstack_quant.calibration import CalibrationStep, RateQuote
        >>> CalibrationStep.parametric(
        ...     "ns",
        ...     "2024-06-28",
        ...     "ns",
        ...     quotes=[RateQuote.swap("s5y", "USD-SOFR", "5Y", 0.042)],
        ... ).kind
        'parametric'

        """

    @property
    def id(self) -> str:
        """
        Unique step identifier and residual key.

        This property does not raise.

        Returns
        -------
        str
            Unique step identifier and residual key.
        """

    @property
    def quote_set(self) -> str:
        """
        Name of the quote set this step consumes.

        This property does not raise.

        Returns
        -------
        str
            Name of the quote set this step consumes.
        """

    @property
    def kind(self) -> str:
        """
        Step variant tag, for example ``"discount"`` or ``"hazard"``.

        This property does not raise.

        Returns
        -------
        str
            Step variant tag, for example ``"discount"`` or ``"hazard"``.
        """

    @property
    def params(self) -> dict[str, Any]:
        """
        Step parameters as a plain dictionary, using the Rust field names.

        This property does not raise.

        Returns
        -------
        dict[str, Any]
            Step parameters as a plain dictionary, using the Rust field names.
        """

    @property
    def quote_ids(self) -> list[str]:
        """
        Identifiers of the quotes attached inline to this step.

        This property does not raise.

        Returns
        -------
        list[str]
            Identifiers of the quotes attached inline to this step.
        """

    @property
    def quotes(self) -> list[Any]:
        """
        Quotes attached inline to this step, as plain dictionaries.

        This property does not raise.

        Returns
        -------
        list[Any]
            Quotes attached inline to this step, as plain dictionaries.
        """

    def to_json(self) -> str:
        """
        Serialize the step to compact JSON.

        Returns
        -------
        str
            Compact JSON encoding of the step definition (without its quotes).

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """

    @staticmethod
    def from_json(json: str) -> CalibrationStep:
        """
        Rebuild a step from JSON.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        CalibrationStep
            The decoded step, with no inline quotes.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.calibration import CalibrationStep
        >>> CalibrationStep.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

    def __reduce__(self) -> tuple[Any, tuple[str, str]]: ...
    def __repr__(self) -> str: ...

class CalibrationPlan:
    """An ordered set of calibration steps plus their shared settings.

    Examples:
    --------
    >>> from finstack_quant.calibration import CalibrationPlan
    >>> CalibrationPlan([], id="empty").step_ids
    []

    """

    def __init__(
        self,
        steps: list[CalibrationStep],
        id: str = "plan",
        description: str | None = None,
        settings: CalibrationConfig | dict[str, Any] | None = None,
        quote_sets: dict[str, list[str]] | None = None,
    ) -> None:
        """Build a calibration plan.

        Parameters
        ----------
        steps : list[CalibrationStep]
            Steps in dependency order; each step's ``id`` must be unique.
        id : str, default "plan"
            Plan identifier recorded in the result metadata.
        description : str | None, default None
            Free-text description recorded in the result metadata.
        settings : CalibrationConfig | dict | None, default None
            Plan-level solver and acceptance settings; Rust defaults when None.
        quote_sets : dict[str, list[str]] | None, default None
            Quote IDs grouped by set name. Attached quote payloads are retained
            even when their set is supplied explicitly. Identical payloads
            sharing an ID are collected once.

        Raises
        ------
        ValueError
            If attached sets disagree on quote IDs, an ID has conflicting
            payloads, or ``settings`` is invalid. Missing referenced quote IDs
            and duplicate step IDs are rejected when the envelope is validated.

        """

    @property
    def id(self) -> str:
        """
        Plan identifier.

        This property does not raise.

        Returns
        -------
        str
            Plan identifier.
        """

    @property
    def description(self) -> str | None:
        """
        Free-text plan description, or None.

        This property does not raise.

        Returns
        -------
        str | None
            Free-text plan description, or None.
        """

    @property
    def step_ids(self) -> list[str]:
        """
        Step identifiers in plan order.

        This property does not raise.

        Returns
        -------
        list[str]
            Step identifiers in plan order.
        """

    @property
    def steps(self) -> list[CalibrationStep]:
        """
        The plan's steps in order.

        This property does not raise.

        Returns
        -------
        list[CalibrationStep]
            The plan's steps in order.
        """

    @property
    def quote_sets(self) -> dict[str, Any]:
        """
        Named quote sets keyed by set name, as plain dictionaries.

        This property does not raise.

        Returns
        -------
        dict[str, Any]
            Named quote sets keyed by set name, as plain dictionaries.
        """

    @property
    def settings(self) -> CalibrationConfig:
        """
        Plan-level solver and acceptance settings.

        This property does not raise.

        Returns
        -------
        CalibrationConfig
            Plan-level solver and acceptance settings.
        """

    @property
    def market_data(self) -> dict[str, Any]:
        """
        Market-data payload assembled from the plan's inline quotes.

        This property does not raise.

        Returns
        -------
        dict[str, Any]
            Market-data payload assembled from the plan's inline quotes.
        """

    def to_json(self) -> str:
        """
        Serialize the plan to compact JSON.

        Returns
        -------
        str
            Compact JSON encoding of the plan.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """

    @staticmethod
    def from_json(json: str) -> CalibrationPlan:
        """
        Rebuild a plan from JSON.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        CalibrationPlan
            The decoded plan.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.calibration import CalibrationPlan
        >>> CalibrationPlan.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

    def __reduce__(self) -> tuple[Any, tuple[str, str]]: ...
    def __repr__(self) -> str: ...

class CalibrationEnvelope:
    """A calibration plan together with its market data and optional prior market.

    This is the versioned wire object Rust consumes; the schema marker is
    ``finstack_quant.calibration/1``.

    Examples:
    --------
    >>> from finstack_quant.calibration import CalibrationEnvelope, CalibrationPlan
    >>> CalibrationEnvelope(CalibrationPlan([], id="smoke")).schema
    'finstack_quant.calibration/1'

    """

    def __init__(
        self,
        plan: CalibrationPlan | dict[str, Any],
        market_data: dict[str, Any] | None = None,
        prior_market: dict[str, Any] | str | None = None,
    ) -> None:
        """Build a calibration envelope.

        Parameters
        ----------
        plan : CalibrationPlan | dict
            The plan to execute.
        market_data : dict | None, default None
            Quote payload keyed by quote-set name; the plan's inline quotes are
            used when None.
        prior_market : dict | str | None, default None
            Existing ``MarketContext`` payload whose curves and surfaces are
            available to the plan's steps.

        Raises
        ------
        ValueError
            If the plan or market payload cannot be read, or a referenced quote
            set is missing.

        """

    @property
    def schema(self) -> str:
        """
        Schema marker, ``"finstack_quant.calibration/1"``.

        This property does not raise.

        Returns
        -------
        str
            Schema marker, ``"finstack_quant.calibration/1"``.
        """

    @property
    def plan(self) -> CalibrationPlan:
        """
        The plan carried by this envelope.

        This property does not raise.

        Returns
        -------
        CalibrationPlan
            The plan carried by this envelope.
        """

    @property
    def market_data(self) -> dict[str, Any]:
        """
        Quote payload keyed by quote-set name.

        This property does not raise.

        Returns
        -------
        dict[str, Any]
            Quote payload keyed by quote-set name.
        """

    @property
    def prior_market(self) -> dict[str, Any] | None:
        """
        Prior market payload, or None.

        This property does not raise.

        Returns
        -------
        dict[str, Any] | None
            Prior market payload, or None.
        """

    def content_hash(self) -> str:
        """Stable content hash of the canonical envelope JSON.

        Returns
        -------
        str
            Hex digest identifying this envelope's exact content; equal
            envelopes hash equal regardless of key ordering.

        Raises
        ------
        RuntimeError
            If the envelope cannot be canonicalized.

        """

    def dry_run(self) -> CalibrationValidationReport:
        """Validate this envelope statically without solving.

        Returns
        -------
        CalibrationValidationReport
            Every static error found in a single pass, plus the step dependency
            graph. Findings are reported, so this method never raises.

        """

    def to_json(self) -> str:
        """
        Serialize the envelope to compact canonical JSON.

        Returns
        -------
        str
            Compact JSON with the ``finstack_quant.calibration/1`` marker.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """

    @staticmethod
    def from_json(json: str) -> CalibrationEnvelope:
        """
        Strictly load an envelope from JSON.

        Parameters
        ----------
        json : str
            Envelope JSON carrying the ``finstack_quant.calibration/1`` marker.

        Returns
        -------
        CalibrationEnvelope
            The loaded envelope.

        Raises
        ------
        CalibrationEnvelopeError
            If the JSON is malformed, the schema marker is wrong, unknown fields
            are present, or resource limits are exceeded. The exception's
            ``diagnostics`` list carries a JSON pointer, message and expected
            value for each failure.

        Examples
        --------
        >>> from finstack_quant.calibration import CalibrationEnvelope
        >>> CalibrationEnvelope.from_json("{")
        Traceback (most recent call last):
        finstack_quant.calibration.CalibrationEnvelopeError: ...
        """

    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...

class QuoteQuality:
    """Fit quality for one calibration quote.

    Examples:
    --------
    >>> from finstack_quant.calibration import QuoteQuality
    >>> q = QuoteQuality.from_json(
    ...     '{"quote_label":"s5y","target_value":0.042,"fitted_value":0.042,"residual":0.0,"sensitivity":1.0}'
    ... )
    >>> q.quote_label
    's5y'

    """

    @property
    def quote_label(self) -> str:
        """
        Identifier of the quote this row describes.

        This property does not raise.

        Returns
        -------
        str
            Identifier of the quote this row describes.
        """

    @property
    def target_value(self) -> float:
        """
        Quoted market value in the target's own units.

        This property does not raise.

        Returns
        -------
        float
            Quoted market value in the target's own units.
        """

    @property
    def fitted_value(self) -> float:
        """
        Value reproduced by the calibrated object, same units as the target.

        This property does not raise.

        Returns
        -------
        float
            Value reproduced by the calibrated object, same units as the target.
        """

    @property
    def residual(self) -> float:
        """
        ``fitted_value - target_value``, in the target's own units.

        This property does not raise.

        Returns
        -------
        float
            ``fitted_value - target_value``, in the target's own units.
        """

    @property
    def sensitivity(self) -> float:
        """
        Derivative of the fitted value with respect to the solved parameter.

        This property does not raise.

        Returns
        -------
        float
            Derivative of the fitted value with respect to the solved parameter.
        """

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            Compact JSON encoding of this row.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """

    @staticmethod
    def from_json(json: str) -> QuoteQuality:
        """
        Rebuild a row from JSON.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        QuoteQuality
            The decoded row.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.calibration import QuoteQuality
        >>> QuoteQuality.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...

class CalibrationDiagnostics:
    """Per-quote fit quality and conditioning for one calibration step.

    Examples:
    --------
    >>> from finstack_quant.calibration import CalibrationDiagnostics
    >>> d = CalibrationDiagnostics.from_json(
    ...     '{"per_quote":[],"condition_number":null,"singular_values":null,'
    ...     '"max_residual":0.0,"rms_residual":0.0,"r_squared":null}'
    ... )
    >>> d.max_residual
    0.0

    """

    @property
    def per_quote(self) -> list[QuoteQuality]:
        """
        Fit quality rows, one per calibration quote.

        This property does not raise.

        Returns
        -------
        list[QuoteQuality]
            Fit quality rows, one per calibration quote.
        """

    @property
    def condition_number(self) -> float | None:
        """
        Condition number of the Jacobian, or None when not computed.

        This property does not raise.

        Returns
        -------
        float | None
            Condition number of the Jacobian, or None when not computed.
        """

    @property
    def singular_values(self) -> list[float] | None:
        """
        Singular values of the Jacobian, or None when not computed.

        This property does not raise.

        Returns
        -------
        list[float] | None
            Singular values of the Jacobian, or None when not computed.
        """

    @property
    def max_residual(self) -> float:
        """
        Largest absolute residual across quotes, in the target's own units.

        This property does not raise.

        Returns
        -------
        float
            Largest absolute residual across quotes, in the target's own units.
        """

    @property
    def rms_residual(self) -> float:
        """
        Root-mean-square residual across quotes, in the target's own units.

        This property does not raise.

        Returns
        -------
        float
            Root-mean-square residual across quotes, in the target's own units.
        """

    @property
    def r_squared(self) -> float | None:
        """
        Coefficient of determination of the fit, or None when not computed.

        This property does not raise.

        Returns
        -------
        float | None
            Coefficient of determination of the fit, or None when not computed.
        """

    def to_dataframe(self) -> pd.DataFrame:
        """Return the per-quote rows as a DataFrame.

        Returns
        -------
        pandas.DataFrame
            Columns ``quote_label``, ``target``, ``fitted``, ``residual`` and
            ``sensitivity``, one row per quote.

        Raises
        ------
        RuntimeError
            If pandas is not installed.

        """

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            Compact JSON encoding of these diagnostics.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """

    @staticmethod
    def from_json(json: str) -> CalibrationDiagnostics:
        """
        Rebuild diagnostics from JSON.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        CalibrationDiagnostics
            The decoded diagnostics.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.calibration import CalibrationDiagnostics
        >>> CalibrationDiagnostics.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...

class CalibrationReport:
    """Convergence, residuals and diagnostics for one calibration step or plan.

    Raw residual statistics (``max_residual``, ``rmse``) are in the target's own
    units. The tolerance-scaled aggregates (``max_residual_ratio``,
    ``rmse_ratio``) divide each residual by its step tolerance, so they are
    comparable across steps and are below 1.0 for an accepted fit.

    Examples:
    --------
    >>> from finstack_quant.calibration import CalibrationPlan, calibrate
    >>> calibrate(CalibrationPlan([], id="smoke")).report.success
    True

    """

    @property
    def success(self) -> bool:
        """
        Whether every quote repriced inside its tolerance.

        This property does not raise.

        Returns
        -------
        bool
            Whether every quote repriced inside its tolerance.
        """

    @property
    def residuals(self) -> dict[str, float]:
        """
        Raw residual per quote id, in the target's own units.

        This property does not raise.

        Returns
        -------
        dict[str, float]
            Raw residual per quote id, in the target's own units.
        """

    @property
    def iterations(self) -> int:
        """
        Number of solver iterations consumed.

        This property does not raise.

        Returns
        -------
        int
            Number of solver iterations consumed.
        """

    @property
    def objective_value(self) -> float:
        """
        Final objective value reached by the solver.

        This property does not raise.

        Returns
        -------
        float
            Final objective value reached by the solver.
        """

    @property
    def max_residual(self) -> float:
        """
        Largest absolute raw residual, in the target's own units.

        This property does not raise.

        Returns
        -------
        float
            Largest absolute raw residual, in the target's own units.
        """

    @property
    def rmse(self) -> float:
        """
        Root-mean-square raw residual, in the target's own units.

        This property does not raise.

        Returns
        -------
        float
            Root-mean-square raw residual, in the target's own units.
        """

    @property
    def max_residual_ratio(self) -> float | None:
        """
        Largest ``|residual| / tolerance``, or None when no tolerance applies.

        This property does not raise.

        Returns
        -------
        float | None
            Largest ``|residual| / tolerance``, or None when no tolerance applies.
        """

    @property
    def rmse_ratio(self) -> float | None:
        """
        Root-mean-square ``|residual| / tolerance``, or None when unavailable.

        This property does not raise.

        Returns
        -------
        float | None
            Root-mean-square ``|residual| / tolerance``, or None when unavailable.
        """

    @property
    def validation_passed(self) -> bool:
        """
        Whether post-solve validation accepted the calibrated object.

        This property does not raise.

        Returns
        -------
        bool
            Whether post-solve validation accepted the calibrated object.
        """

    @property
    def validation_error(self) -> str | None:
        """
        Post-solve validation message, or None when validation passed.

        This property does not raise.

        Returns
        -------
        str | None
            Post-solve validation message, or None when validation passed.
        """

    @property
    def convergence_reason(self) -> str:
        """
        Why the solver stopped, for example ``"tolerance_reached"``.

        This property does not raise.

        Returns
        -------
        str
            Why the solver stopped, for example ``"tolerance_reached"``.
        """

    @property
    def metadata(self) -> dict[str, Any]:
        """
        Free-form solver metadata recorded by the step.

        This property does not raise.

        Returns
        -------
        dict[str, Any]
            Free-form solver metadata recorded by the step.
        """

    @property
    def solver_tolerance(self) -> float:
        """
        Tolerance applied to this step, in the target's own units.

        This property does not raise.

        Returns
        -------
        float
            Tolerance applied to this step, in the target's own units.
        """

    @property
    def solver_max_iterations(self) -> int:
        """
        Iteration budget applied to this step.

        This property does not raise.

        Returns
        -------
        int
            Iteration budget applied to this step.
        """

    @property
    def model_version(self) -> str | None:
        """
        Version tag of the calibrated model, or None.

        This property does not raise.

        Returns
        -------
        str | None
            Version tag of the calibrated model, or None.
        """

    @property
    def worst_quote_id(self) -> str | None:
        """
        Identifier of the worst-fitting quote, or None when there are no quotes.

        This property does not raise.

        Returns
        -------
        str | None
            Identifier of the worst-fitting quote, or None when there are no quotes.
        """

    @property
    def worst_quote_residual(self) -> float | None:
        """
        Raw residual of the worst-fitting quote, or None.

        This property does not raise.

        Returns
        -------
        float | None
            Raw residual of the worst-fitting quote, or None.
        """

    @property
    def success_tolerance(self) -> float | None:
        """
        Tolerance used for the success decision, or None.

        This property does not raise.

        Returns
        -------
        float | None
            Tolerance used for the success decision, or None.
        """

    @property
    def diagnostics(self) -> CalibrationDiagnostics | None:
        """
        Per-quote diagnostics, or None when diagnostics were not computed.

        This property does not raise.

        Returns
        -------
        CalibrationDiagnostics | None
            Per-quote diagnostics, or None when diagnostics were not computed.
        """

    def to_dataframe(self) -> pd.DataFrame:
        """Return the per-quote residuals as a DataFrame.

        Returns
        -------
        pandas.DataFrame
            Columns ``quote_id``, ``target``, ``fitted``, ``residual`` and
            ``sensitivity``. Target, fitted and sensitivity are NaN when
            diagnostics were not computed.

        Raises
        ------
        RuntimeError
            If pandas is not installed.

        """

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            Compact JSON encoding of this report.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """

    @staticmethod
    def from_json(json: str) -> CalibrationReport:
        """
        Rebuild a report from JSON.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        CalibrationReport
            The decoded report.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.calibration import CalibrationReport
        >>> CalibrationReport.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...

class CalibrationValidationReport:
    """Result of static, pre-solve validation of a calibration envelope.

    Examples:
    --------
    >>> from finstack_quant.calibration import CalibrationPlan, dry_run
    >>> dry_run(CalibrationPlan([], id="smoke")).is_valid
    True

    """

    @property
    def is_valid(self) -> bool:
        """
        Whether the envelope passed every static check.

        This property does not raise.

        Returns
        -------
        bool
            Whether the envelope passed every static check.
        """

    @property
    def errors(self) -> list[dict[str, Any]]:
        """
        Static findings, each with its code, message and offending identifier.

        This property does not raise.

        Returns
        -------
        list[dict[str, Any]]
            Static findings, each with its code, message and offending identifier.
        """

    @property
    def dependency_graph(self) -> dict[str, list[str]]:
        """
        Step identifiers mapped to the step ids they depend on.

        This property does not raise.

        Returns
        -------
        dict[str, list[str]]
            Step identifiers mapped to the step ids they depend on.
        """

    def to_dataframe(self) -> pd.DataFrame:
        """Return the static findings as a DataFrame.

        Returns
        -------
        pandas.DataFrame
            One row per finding, with the finding's fields as columns. Empty
            when the envelope is valid.

        Raises
        ------
        RuntimeError
            If pandas is not installed.

        """

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            Compact JSON encoding of this report.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """

    @staticmethod
    def from_json(json: str) -> CalibrationValidationReport:
        """
        Rebuild a report from JSON.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        CalibrationValidationReport
            The decoded report.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.calibration import CalibrationValidationReport
        >>> CalibrationValidationReport.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...

class CalibrationResult:
    """Calibrated market plus the plan-level and per-step calibration reports.

    Examples:
    --------
    >>> from finstack_quant.calibration import CalibrationPlan, calibrate
    >>> calibrate(CalibrationPlan([], id="smoke")).step_ids
    []

    """

    @property
    def success(self) -> bool:
        """
        Whether every step repriced its quotes inside tolerance.

        This property does not raise.

        Returns
        -------
        bool
            Whether every step repriced its quotes inside tolerance.
        """

    @property
    def market(self) -> MarketContext:
        """
        The calibrated market context.

        Returns
        -------
        MarketContext
            The calibrated market context.

        Raises
        ------
        ValueError
            If the calibrated market payload cannot be rebuilt.
        """

    @property
    def market_json(self) -> str:
        """
        The calibrated market as canonical ``MarketContext`` JSON.

        Returns
        -------
        str
            The calibrated market as canonical ``MarketContext`` JSON.

        Raises
        ------
        ValueError
            If the underlying value cannot be serialized to JSON.
        """

    @property
    def report(self) -> CalibrationReport:
        """
        Plan-level calibration report aggregated across steps.

        This property does not raise.

        Returns
        -------
        CalibrationReport
            Plan-level calibration report aggregated across steps.
        """

    @property
    def report_json(self) -> str:
        """
        JSON twin of :attr:`report`.

        Returns
        -------
        str
            JSON twin of :attr:`report`.

        Raises
        ------
        ValueError
            If the underlying value cannot be serialized to JSON.
        """

    @property
    def step_ids(self) -> list[str]:
        """
        Identifiers of the steps executed by the plan.

        This property does not raise.

        Returns
        -------
        list[str]
            Identifiers of the steps executed by the plan.
        """

    @property
    def iterations(self) -> int:
        """
        Total solver iterations across every step.

        This property does not raise.

        Returns
        -------
        int
            Total solver iterations across every step.
        """

    @property
    def max_residual_ratio(self) -> float:
        """
                Largest ``|residual| / step_tolerance`` across every quote.

        Below 1.0 when every quote repriced inside its step's tolerance. Raw
        per-step residual statistics live on ``step_report(step_id).max_residual``.
        NaN when no step reported a tolerance-scaled aggregate.

                This property does not raise.

                Returns
                -------
                float
                    Largest ``|residual| / step_tolerance`` across every quote. Below 1.0 when every quote repriced inside its step's tolerance. Raw per-step residual statistics live on ``step_report(step_id).max_residual``. NaN when no step reported a tolerance-scaled aggregate.
        """

    @property
    def rmse_ratio(self) -> float:
        """
        Root-mean-square ``|residual| / step_tolerance`` across every quote.

        This property does not raise.

        Returns
        -------
        float
            Root-mean-square ``|residual| / step_tolerance`` across every quote.
        """

    def step_report(self, step_id: str) -> CalibrationReport:
        """Return the calibration report for one step.

        Parameters
        ----------
        step_id : str
            Identifier of the calibration step, as listed in :attr:`step_ids`.

        Returns
        -------
        CalibrationReport
            Typed step report with raw residuals keyed by quote id.

        Raises
        ------
        KeyError
            If no step with that identifier exists.

        """

    def step_report_json(self, step_id: str) -> str:
        """JSON twin of :meth:`step_report`.

        Parameters
        ----------
        step_id : str
            Identifier of the calibration step.

        Returns
        -------
        str
            Compact JSON encoding of the step report.

        Raises
        ------
        KeyError
            If no step with that identifier exists.

        """

    def residuals(self, step_id: str) -> pd.DataFrame:
        """Return one step's per-quote residuals as a DataFrame.

        Parameters
        ----------
        step_id : str
            Identifier of the calibration step.

        Returns
        -------
        pandas.DataFrame
            Columns ``quote_id``, ``target``, ``fitted``, ``residual`` and
            ``sensitivity``, in the target's own units.

        Raises
        ------
        KeyError
            If no step with that identifier exists.
        RuntimeError
            If pandas is not installed.

        """

    def to_dataframe(self) -> pd.DataFrame:
        """Return one row per step summarizing its fit.

        Returns
        -------
        pandas.DataFrame
            Columns ``step_id``, ``success``, ``iterations``, ``max_residual``,
            ``rmse`` and ``convergence_reason``. Plan-level aggregates
            (``max_residual_ratio``, ``rmse_ratio``) are getters on the result.

        Raises
        ------
        RuntimeError
            If pandas is not installed.

        """

    def content_hash(self) -> str:
        """Stable content hash of the canonical result JSON.

        Returns
        -------
        str
            Hex digest identifying this result's exact content.

        Raises
        ------
        RuntimeError
            If the result cannot be canonicalized.

        """

    def to_json(self) -> str:
        """
        Serialize the result to compact canonical JSON.

        Returns
        -------
        str
            Compact JSON encoding of the calibrated market and reports.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """

    @staticmethod
    def from_json(json: str) -> CalibrationResult:
        """
        Strictly load a result from JSON.

        Parameters
        ----------
        json : str
            Result JSON produced by :meth:`to_json`.

        Returns
        -------
        CalibrationResult
            The loaded result.

        Raises
        ------
        CalibrationEnvelopeError
            If the JSON is malformed, the schema marker is wrong, or unknown
            fields are present. The exception's ``diagnostics`` list carries a
            JSON pointer, message and expected value for each failure.

        Examples
        --------
        >>> from finstack_quant.calibration import CalibrationResult
        >>> CalibrationResult.from_json("{")
        Traceback (most recent call last):
        finstack_quant.portfolio.ContractValidationError: ...
        """

    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...

class CalibrationEnvelopeError(RuntimeError):
    """Raised when calibration ingestion, validation, or solving fails.

    Carries ``kind``, ``stage``, ``step_id``, ``solver_diagnostics``, ``details``
    (JSON string) and ``diagnostics`` (list of dicts with ``pointer``,
    ``message``, ``code`` and ``expected_version`` for strict-load failures).

    Examples:
    --------
    >>> from finstack_quant.calibration import CalibrationEnvelopeError, dry_run
    >>> try:
    ...     dry_run("{}")
    ... except CalibrationEnvelopeError as exc:
    ...     isinstance(exc.diagnostics, list)
    True

    """

    kind: str
    stage: str
    step_id: str | None
    solver_diagnostics: str | None
    details: str
    diagnostics: list[dict[str, Any]]

def validate_calibration(
    envelope: CalibrationEnvelope | CalibrationPlan | dict[str, Any] | str,
) -> CalibrationEnvelope:
    """Validate a calibration envelope and return it in canonical typed form.

    Parameters
    ----------
    envelope : CalibrationEnvelope | CalibrationPlan | dict | str
        Typed envelope, plan, dict, or JSON string using the schema marker
        ``finstack_quant.calibration/1``.

    Returns
    -------
    CalibrationEnvelope
        The validated envelope.

    Raises
    ------
    CalibrationEnvelopeError
        If strict loading or static validation rejects the envelope. Static
        validation is fail-fast (first error only); use :func:`dry_run` to list
        every static error. Strict-load failures carry ``diagnostics``.

    Examples:
    --------
    >>> from finstack_quant.calibration import CalibrationPlan, validate_calibration
    >>> validate_calibration(CalibrationPlan([], id="smoke")).plan.id
    'smoke'

    """

def validate_calibration_json(
    envelope: CalibrationEnvelope | CalibrationPlan | dict[str, Any] | str,
) -> str:
    """JSON twin of :func:`validate_calibration`.

    Parameters
    ----------
    envelope : CalibrationEnvelope | CalibrationPlan | dict | str
        Typed envelope, plan, dict, or JSON string.

    Returns
    -------
    str
        Canonical pretty-printed envelope JSON.

    Raises
    ------
    CalibrationEnvelopeError
        If strict loading or static validation rejects the envelope.

    Examples:
    --------
    >>> from finstack_quant.calibration import CalibrationPlan, validate_calibration_json
    >>> "finstack_quant.calibration/1" in validate_calibration_json(CalibrationPlan([], id="smoke"))
    True

    """

def dry_run(
    envelope: CalibrationEnvelope | CalibrationPlan | dict[str, Any] | str,
) -> CalibrationValidationReport:
    """Validate an envelope statically without invoking the solver.

    Unlike :func:`validate_calibration`, semantic findings are returned in the
    report rather than raised, and every static error is collected in one pass.

    Parameters
    ----------
    envelope : CalibrationEnvelope | CalibrationPlan | dict | str
        Typed envelope, plan, dict, or JSON string.

    Returns
    -------
    CalibrationValidationReport
        Every static error plus the step dependency graph.

    Raises
    ------
    CalibrationEnvelopeError
        Only when the input cannot be strictly loaded as an envelope (malformed
        JSON, wrong schema marker, unknown fields, resource limits). Those
        failures carry ``diagnostics`` with JSON pointers.

    Examples:
    --------
    >>> from finstack_quant.calibration import CalibrationPlan, dry_run
    >>> dry_run(CalibrationPlan([], id="smoke")).errors
    []

    """

def dry_run_json(
    envelope: CalibrationEnvelope | CalibrationPlan | dict[str, Any] | str,
) -> str:
    """JSON twin of :func:`dry_run`.

    Parameters
    ----------
    envelope : CalibrationEnvelope | CalibrationPlan | dict | str
        Typed envelope, plan, dict, or JSON string.

    Returns
    -------
    str
        Pretty-printed ``CalibrationValidationReport`` JSON.

    Raises
    ------
    CalibrationEnvelopeError
        If the input cannot be strictly loaded as an envelope.

    Examples:
    --------
    >>> from finstack_quant.calibration import CalibrationPlan, dry_run_json
    >>> "errors" in dry_run_json(CalibrationPlan([], id="smoke"))
    True

    """

def calibrate(
    envelope: CalibrationEnvelope | CalibrationPlan | dict[str, Any] | str,
) -> CalibrationResult:
    """Execute a calibration plan and return the calibrated market and reports.

    Parameters
    ----------
    envelope : CalibrationEnvelope | CalibrationPlan | dict | str
        Typed envelope, plan (its inline quotes become the market data), dict,
        or JSON string using the schema marker ``finstack_quant.calibration/1``.

    Returns
    -------
    CalibrationResult
        Calibrated market, plan-level report, per-step reports and residuals.

    Raises
    ------
    CalibrationEnvelopeError
        If ingestion, validation, context construction, target construction,
        solving, or final fit acceptance fails. Static validation is fail-fast;
        use :func:`dry_run` to list every static error.

    Examples:
    --------
    >>> from finstack_quant.calibration import CalibrationPlan, calibrate
    >>> calibrate(CalibrationPlan([], id="smoke")).success
    True

    """

def calibrate_bermudan_lmm_base_vol(
    instrument: Any,
    market: MarketContext | str,
    as_of: Any,
) -> float:
    """Calibrate the explicit Bermudan LMM loading scale from the market surface.

    Parameters
    ----------
    instrument : Swaption | str
        Typed Bermudan swaption instrument or its canonical instrument JSON
        envelope.
    market : MarketContext | str
        Market carrying the required discount and swaption-volatility inputs.
    as_of : datetime.date | datetime.datetime | pandas.Timestamp | str
        Valuation date used for tenor and expiry construction.

    Returns
    -------
    float
        Positive finite LMM base volatility, annualized decimal, to place in
        ``model_config.lmm_base_vol``.

    Raises
    ------
    ValueError
        If the instrument is not a Bermudan swaption or inputs are invalid.
    KeyError
        If a referenced curve or surface is missing from ``market``.
    RuntimeError
        If the Rebonato calibration fails.

    Examples:
    --------
    >>> from finstack_quant.calibration import calibrate_bermudan_lmm_base_vol
    >>> calibrate_bermudan_lmm_base_vol("{}", "{}", "2025-01-01")
    Traceback (most recent call last):
    ValueError: ...

    """
