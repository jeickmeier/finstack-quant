"""Stub declarations mirroring ``finstack-quant-py/src/bindings/calibration/envelope.rs``."""

from typing import Any
from finstack_quant.core.currency import Currency
from finstack_quant.calibration._config import CalibrationConfig
from finstack_quant.calibration._quotes import (
    CdsQuote,
    CdsTrancheQuote,
    FxSpotDatum,
    InflationQuote,
    PriceDatum,
    RateQuote,
    VolQuote,
    XccyQuote,
)

class CalibrationStep:
    """One calibration step: a target kind, parameters, and a named quote set.

    Each constructor corresponds to a Rust ``StepParams`` variant. Quote payloads
    belong to ``CalibrationEnvelope.market_data`` and their IDs to ``plan.quote_sets``.

    Examples:
    --------
    >>> from finstack_quant.calibration import CalibrationStep, RateQuote
    >>> step = CalibrationStep.discount(
    ...     "usd_ois",
    ...     "USD",
    ...     "2024-06-28",
    ... )
    >>> step.id
    'usd_ois'

    """

    @staticmethod
    def discount(
        id: str,
        currency: str | Currency,
        base_date: str,
        quote_set: str | None = None,
        curve_id: str | None = None,
        **params: Any,
    ) -> CalibrationStep:
        """Build a discount-curve bootstrap step.

        Parameters
        ----------
        id : str
            Unique step identifier; also the residual key in the result.
        currency : str | Currency
            ISO 4217 code of the curve currency.
        base_date : str
            Curve base (spot) date as an ISO date string.
        quote_set : str | None, default None
            Name of a quote set declared on the plan; defaults to ``id``.
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
            If a parameter is unknown to the Rust step schema or has the wrong shape.
        TypeError
            If ``currency`` is neither a currency-code string nor a ``Currency`` instance.

        Examples:
        --------
        >>> from finstack_quant.calibration import CalibrationStep, RateQuote
        >>> CalibrationStep.discount(
        ...     "usd_ois",
        ...     "USD",
        ...     "2024-06-28",
        ... ).kind
        'discount'

        """

    @staticmethod
    def forward(
        id: str,
        currency: str | Currency,
        base_date: str,
        tenor_years: float,
        discount_curve_id: str,
        quote_set: str | None = None,
        curve_id: str | None = None,
        **params: Any,
    ) -> CalibrationStep:
        """Build a projection (forward) curve bootstrap step.

        Parameters
        ----------
        id : str
            Unique step identifier.
        currency : str | Currency
            ISO 4217 code of the curve currency.
        base_date : str
            Curve base date as an ISO date string.
        tenor_years : float
            Index tenor in years (``0.25`` for a 3M index).
        discount_curve_id : str
            Identifier of the discount curve solved in an earlier step.
        quote_set : str | None, default None
            Name of a shared quote set declared on the plan.
        curve_id : str | None, default None
            Identifier of the produced curve; defaults to ``id``.
        params : Any
            Extra ``StepParams`` fields following the Rust step schema.
            ``target_expiries`` are ACT/365F years from ``base_date``;
            ``fixed_day_count`` controls coupon accrual fractions only.

        Returns
        -------
        CalibrationStep
            The typed step.

        Raises
        ------
        ValueError
            If quote wiring is ambiguous or a parameter is unknown.
        TypeError
            If ``currency`` is neither a currency-code string nor a ``Currency`` instance.

        Examples:
        --------
        >>> from finstack_quant.calibration import CalibrationStep, RateQuote
        >>> CalibrationStep.forward(
        ...     "usd_3m",
        ...     "USD",
        ...     "2024-06-28",
        ...     0.25,
        ...     "usd_ois",
        ... ).kind
        'forward'

        """

    @staticmethod
    def hazard(
        id: str,
        entity: str,
        currency: str | Currency,
        base_date: str,
        discount_curve_id: str,
        recovery_rate: float,
        seniority: str | None = None,
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
        currency : str | Currency
            ISO 4217 code of the contract currency.
        base_date : str
            Curve base date as an ISO date string.
        discount_curve_id : str
            Identifier of the discount curve solved in an earlier step.
        recovery_rate : float
            Assumed recovery as a decimal fraction of notional.
        seniority : str | None, default None
            Debt seniority tier recorded on the curve. ``None`` uses the Rust
            authoring default ``CalibrationStep::DEFAULT_HAZARD_SENIORITY``
            (``"senior"``).
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
        TypeError
            If ``currency`` is neither a currency-code string nor a ``Currency`` instance.

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
        ... ).kind
        'hazard'

        """

    @staticmethod
    def inflation(
        id: str,
        currency: str | Currency,
        base_date: str,
        discount_curve_id: str,
        index: str,
        observation_lag: str,
        base_cpi: float,
        quote_set: str | None = None,
        curve_id: str | None = None,
        **params: Any,
    ) -> CalibrationStep:
        """Build an inflation-curve bootstrap step.

        Parameters
        ----------
        id : str
            Unique step identifier.
        currency : str | Currency
            ISO 4217 code of the curve currency.
        base_date : str
            Valuation and calibration-instrument start date as an ISO date string.
            The curve's reference CPI date is this date minus ``observation_lag``.
        discount_curve_id : str
            Identifier of the discount curve solved in an earlier step.
        index : str
            Inflation index identifier, for example ``"US-CPI-U"``.
        observation_lag : str
            Whole-month observation lag, for example ``"3M"``, up to 255 months.
            No lag accepts ``""``, ``"none"``, ``"0"``, ``"0M"``, or ``"0D"``;
            parsing is case-insensitive. Calibration rejects nonzero day lags.
        base_cpi : float
            Contractual reference CPI at the start date, in index points, after
            observation lag and monthly interpolation. Supplied index fixings must
            reproduce this level, including their seasonality adjustment.
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
        TypeError
            If ``currency`` is neither a currency-code string nor a ``Currency`` instance.

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
        ... ).kind
        'inflation'

        """

    @staticmethod
    def vol_surface(
        id: str,
        base_date: str,
        underlying_ticker: str,
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
        ... ).kind
        'vol_surface'

        """

    @staticmethod
    def swaption_vol(
        id: str,
        base_date: str,
        discount_curve_id: str,
        currency: str | Currency,
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
        currency : str | Currency
            ISO 4217 code of the swaption currency.
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
        TypeError
            If ``currency`` is neither a currency-code string nor a ``Currency`` instance.

        Examples:
        --------
        >>> from finstack_quant.calibration import CalibrationStep, VolQuote
        >>> CalibrationStep.swaption_vol(
        ...     "usd_swpn",
        ...     "2024-06-28",
        ...     "usd_ois",
        ...     "USD",
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
        currency: str | Currency,
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
        currency : str | Currency
            ISO 4217 code of the tranche currency.
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
        TypeError
            If ``currency`` is neither a currency-code string nor a ``Currency`` instance.

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
        currency: str | Currency,
        base_date: str,
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
        currency : str | Currency
            ISO 4217 code of the model currency.
        base_date : str
            Model base date as an ISO date string.
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
        TypeError
            If ``currency`` is neither a currency-code string nor a ``Currency`` instance.

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
        index_id: str,
        currency: str | Currency,
        base_date: str,
        quote_set: str | None = None,
        **params: Any,
    ) -> CalibrationStep:
        """Build a Hull-White cap/floor calibration step.

        Parameters
        ----------
        id : str
            Unique step identifier.
        discount_curve_id : str
            Identifier of the discounting curve and output-key prefix. Scalar fits
            write ``"{discount_curve_id}_CAPFLOOR_HW1F_KAPPA"`` and
            ``"{discount_curve_id}_CAPFLOOR_HW1F_SIGMA"``. Piecewise fits write the
            same kappa scalar and the sigma series
            ``"{discount_curve_id}_CAPFLOOR_HW1F_SIGMA_SCHEDULE"``.
        forward_curve_id : str
            Identifier of the curve projecting the caplet forwards.
        index_id : str
            Term-rate index identifier, for example ``"EUR-EURIBOR-3M"``. Its
            conventions determine settlement, fixing and payment dates, calendars,
            and accrual fractions. Calibration rejects overnight indices.
        currency : str | Currency
            ISO 4217 code of the model currency; must match the index conventions.
        base_date : str
            Model base date as an ISO date string.
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
        TypeError
            If ``currency`` is neither a currency-code string nor a ``Currency`` instance.

        Examples:
        --------
        >>> from finstack_quant.calibration import CalibrationStep
        >>> CalibrationStep.cap_floor_hull_white(
        ...     "hw_cf",
        ...     "eur_ois",
        ...     "eur_3m",
        ...     "EUR-EURIBOR-3M",
        ...     "EUR",
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
        ... ).kind
        'svi_surface'

        """

    @staticmethod
    def xccy_basis(
        id: str,
        currency: str | Currency,
        base_date: str,
        fx_spot: float,
        domestic_discount_id: str,
        quote_set: str | None = None,
        curve_id: str | None = None,
        **params: Any,
    ) -> CalibrationStep:
        """Build a cross-currency basis discount-curve bootstrap step.

        Parameters
        ----------
        id : str
            Unique step identifier.
        currency : str | Currency
            ISO 4217 code of the foreign (solved) currency.
        base_date : str
            Curve base date as an ISO date string.
        fx_spot : float
            Spot FX rate quoted as foreign units per one domestic unit.
        domestic_discount_id : str
            Identifier of the domestic discount curve solved in an earlier step.
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
        TypeError
            If ``currency`` is neither a currency-code string nor a ``Currency`` instance.

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
        model: str | None = None,
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
        model : str | None, default None
            Parametric family key: ``"ns"`` for Nelson-Siegel or ``"nss"``
            for Nelson-Siegel-Svensson. ``None`` uses the Rust authoring
            default ``CalibrationStep::DEFAULT_PARAMETRIC_MODEL`` (``"ns"``).
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

    def to_json(self) -> str:
        """
        Serialize the step to compact JSON.

        Returns
        -------
        str
            Compact JSON encoding of the step: its ``id``, ``quote_set``,
            ``kind`` and parameters.

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
            The decoded plain step with its quote-set reference.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.calibration import CalibrationStep, RateQuote
        >>> quotes = [RateQuote.deposit("D3M", "USD-SOFR-OIS", "3M", 0.052)]
        >>> step = CalibrationStep.discount("USD-OIS", "USD", "2026-05-08")
        >>> CalibrationStep.from_json(step.to_json()).quote_set
        'USD-OIS'
        >>> CalibrationStep.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
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
            Quote IDs grouped by set name. IDs resolve in the envelope market data.

        Raises
        ------
        ValueError
            If ``settings`` is invalid.

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
        The plain plan's steps in execution order.

        This property does not raise.

        Returns
        -------
        list[CalibrationStep]
            Plain steps referencing the plan's named quote sets.
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

    def to_json(self) -> str:
        """
        Serialize the plain plan to compact JSON.

        Returns
        -------
        str
            Compact JSON containing the plan ID, steps, quote sets, and settings.

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
            The decoded plain plan. Quote resolution is checked on its envelope.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries
            unknown fields.

        Examples
        --------
        >>> from finstack_quant.calibration import CalibrationEnvelope, CalibrationPlan, CalibrationStep, RateQuote
        >>> quotes = [RateQuote.deposit("D3M", "USD-SOFR-OIS", "3M", 0.052)]
        >>> plan = CalibrationPlan(
        ...     [CalibrationStep.discount("USD-OIS", "USD", "2026-05-08")],
        ...     quote_sets={"USD-OIS": [q.id for q in quotes]},
        ... )
        >>> restored = CalibrationPlan.from_json(plan.to_json())
        >>> restored.to_json() == plan.to_json(), restored.steps[0].quote_set
        (True, 'USD-OIS')
        >>> CalibrationPlan.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
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
        plan: CalibrationPlan,
        market_data: list[
            RateQuote
            | CdsQuote
            | VolQuote
            | InflationQuote
            | XccyQuote
            | CdsTrancheQuote
            | FxSpotDatum
            | PriceDatum
            | dict[str, Any]
        ]
        | None = None,
        prior_market: list[dict[str, Any]] | None = None,
    ) -> None:
        """Build a calibration envelope.

        Parameters
        ----------
        plan : CalibrationPlan
            Plain plan whose quote IDs resolve in ``market_data``.
        market_data : list[RateQuote | CdsQuote | VolQuote | InflationQuote | XccyQuote | CdsTrancheQuote | FxSpotDatum | PriceDatum | dict[str, Any]] | None, default None
            Flat market data: typed quotes and datums or ``{"kind": ...}`` dicts.
            Dividend schedules use ``{"kind": "dividend_schedule", "id": ...,
            "underlying": ..., "events": [...], "currency": ...}`` directly,
            with ISO dates and cash amounts in the stated currency. Optional
            underlying/currency metadata may be omitted. The former nested
            ``schedule`` field is rejected. Duplicate kind-and-ID inputs are
            rejected during validation.
        prior_market : list[dict] | None, default None
            Pre-built calibrated objects as ``{"kind": ..., ...}`` dicts.

        Raises
        ------
        ValueError
            If a market-data or prior-market entry has an invalid shape.

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
        The plain plan carried by this envelope.

        This property does not raise.

        Returns
        -------
        CalibrationPlan
            Plain steps, quote-set IDs, and settings. Market data stays on the envelope.
        """

    @property
    def market_data(self) -> list[dict[str, Any]]:
        """
        Flat market data containing quotes and other observed inputs.

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
