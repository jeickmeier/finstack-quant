"""Stub declarations mirroring ``finstack-quant-py/src/bindings/calibration/envelope.rs``."""

from typing import Any
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

    Each constructor corresponds to a Rust ``StepParams`` variant and takes the
    step ``id``, a ``params`` dict (or JSON text) holding that variant's wire
    fields, and an optional ``quote_set``. Quote payloads belong to
    ``CalibrationEnvelope.market_data`` and their IDs to ``plan.quote_sets``.

    Examples:
    --------
    >>> from finstack_quant.calibration import CalibrationStep
    >>> step = CalibrationStep.discount("usd_ois", {"currency": "USD", "base_date": "2024-06-28"})
    >>> step.id
    'usd_ois'

    """

    @staticmethod
    def discount(id: str, params: dict[str, Any] | str, quote_set: str | None = None) -> CalibrationStep:
        """Build a discount-curve bootstrap step.

        Parameters
        ----------
        id : str
            Unique step identifier; also the residual key in the result and the default ``quote_set`` and
            ``curve_id``.
        params : dict[str, Any] | str
            Wire fields of the ``discount`` step as a dict, or the same object as JSON text, keyed as in the
            Rust ``StepParams`` schema. Values are JSON values; ``Currency`` wrappers are accepted. Keys:

            - ``currency`` (str | Currency, required): Upper-case ISO 4217 code (or a ``Currency``) of the
              curve currency.
            - ``base_date`` (str, required): Curve base (spot) date as an ISO-8601 date string
              (``"YYYY-MM-DD"``).
            - ``curve_id`` (str, optional, default ``id``): Identifier of the produced curve.
            - ``method`` (str | dict, optional, default ``"bootstrap"``): Calibration method: ``"bootstrap"``
              or ``{"global_solve": {"use_analytical_jacobian": bool}}``.
            - ``interpolation`` (str, optional, default ``"log_linear"``): Discount-factor interpolation:
              ``"linear"``, ``"log_linear"``, ``"monotone_convex"``, ``"cubic_hermite"`` or
              ``"piecewise_quadratic_forward"``.
            - ``extrapolation`` (str, optional, default ``"flat_forward"``): Extrapolation policy:
              ``"flat_zero"``, ``"flat_forward"`` or ``"none"``.
            - ``pricing_discount_id`` (str | None, optional, default ``None``): Separate discount curve used
              to price the quotes; ``None`` uses ``curve_id``.
            - ``pricing_forward_id`` (str | None, optional, default ``None``): Forward curve used to price the
              quotes, if any.
            - ``conventions`` (dict, optional, default both ``None``): Step-level rates conventions
              ``{"curve_day_count": ..., "ois_compounding": ...}``; both entries default to ``None`` (the
              quote conventions apply).

        quote_set : str | None, default None
            Name of a quote set declared in ``CalibrationPlan.quote_sets``; ``None`` uses ``id``.

        Returns
        -------
        CalibrationStep
            The typed step.

        Raises
        ------
        ValueError
            If ``params`` is not an object, a required key is missing, a key is unknown to the Rust step
            schema, or a value has the wrong shape or an unknown label.
        TypeError
            If a ``params`` value is not JSON-serializable.

        Examples:
        --------
        >>> from finstack_quant.calibration import CalibrationStep
        >>> step = CalibrationStep.discount("usd_ois", {"currency": "USD", "base_date": "2024-06-28"})
        >>> step.kind
        'discount'

        """

    @staticmethod
    def forward(id: str, params: dict[str, Any] | str, quote_set: str | None = None) -> CalibrationStep:
        """Build a projection (forward) curve calibration step.

        Parameters
        ----------
        id : str
            Unique step identifier; also the residual key in the result and the default ``quote_set`` and
            ``curve_id``.
        params : dict[str, Any] | str
            Wire fields of the ``forward`` step as a dict, or the same object as JSON text, keyed as in the
            Rust ``StepParams`` schema. Values are JSON values; ``Currency`` wrappers are accepted. Keys:

            - ``currency`` (str | Currency, required): Upper-case ISO 4217 code (or a ``Currency``) of the
              curve currency.
            - ``base_date`` (str, required): Curve base date as an ISO-8601 date string (``"YYYY-MM-DD"``).
            - ``tenor_years`` (float, required): Index accrual tenor in years (``0.25`` for a 3M index).
            - ``discount_curve_id`` (str, required): Identifier of the discount curve solved in an earlier
              step.
            - ``curve_id`` (str, optional, default ``id``): Identifier of the produced curve.
            - ``method`` (dict, optional, default ``global_solve`` with a finite-difference Jacobian):
              Calibration method; forward curves require ``global_solve`` (``"bootstrap"`` is rejected).
            - ``interpolation`` (str, optional, default ``"cubic_hermite"``): Forward-rate interpolation:
              ``"linear"``, ``"log_linear"``, ``"monotone_convex"``, ``"cubic_hermite"`` or
              ``"piecewise_quadratic_forward"``; ``"monotone_convex"`` is rejected.
            - ``conventions`` (dict, optional, default both ``None``): Step-level rates conventions
              ``{"curve_day_count": ..., "ois_compounding": ...}``; both entries default to ``None`` (the
              quote conventions apply).

        quote_set : str | None, default None
            Name of a quote set declared in ``CalibrationPlan.quote_sets``; ``None`` uses ``id``.

        Returns
        -------
        CalibrationStep
            The typed step.

        Raises
        ------
        ValueError
            If ``params`` is not an object, a required key is missing, a key is unknown to the Rust step
            schema, or a value has the wrong shape or an unknown label.
        TypeError
            If a ``params`` value is not JSON-serializable.

        Examples:
        --------
        >>> from finstack_quant.calibration import CalibrationStep
        >>> params = {
        ...     "currency": "USD",
        ...     "base_date": "2024-06-28",
        ...     "tenor_years": 0.25,
        ...     "discount_curve_id": "usd_ois",
        ... }
        >>> step = CalibrationStep.forward("usd_3m", params)
        >>> step.kind
        'forward'

        """

    @staticmethod
    def hazard(id: str, params: dict[str, Any] | str, quote_set: str | None = None) -> CalibrationStep:
        """Build a credit hazard-curve bootstrap step.

        Parameters
        ----------
        id : str
            Unique step identifier; also the residual key in the result and the default ``quote_set`` and
            ``curve_id``.
        params : dict[str, Any] | str
            Wire fields of the ``hazard`` step as a dict, or the same object as JSON text, keyed as in the
            Rust ``StepParams`` schema. Values are JSON values; ``Currency`` wrappers are accepted. Keys:

            - ``entity`` (str, required): Reference entity identifier.
            - ``currency`` (str | Currency, required): Upper-case ISO 4217 code (or a ``Currency``) of the
              contract currency.
            - ``base_date`` (str, required): Curve base date as an ISO-8601 date string (``"YYYY-MM-DD"``).
            - ``discount_curve_id`` (str, required): Identifier of the discount curve solved in an earlier
              step.
            - ``recovery_rate`` (float, required): Assumed recovery as a decimal fraction of notional in ``[0,
              1)``.
            - ``seniority`` (str | None, optional, default ``"senior"``): Debt seniority recorded on the
              curve: ``"senior_secured"``, ``"senior"``, ``"subordinated"`` or ``"junior"``. Omitted or
              ``None`` uses the Rust authoring default ``CalibrationStep::DEFAULT_HAZARD_SENIORITY``.
            - ``curve_id`` (str, optional, default ``id``): Identifier of the produced curve.
            - ``notional`` (float, optional, default ``1.0``): Notional of the synthetic CDS repriced during
              calibration.
            - ``method`` (str | dict, optional, default ``"bootstrap"``): Calibration method: ``"bootstrap"``
              or ``{"global_solve": {"use_analytical_jacobian": bool}}``.
            - ``par_interp`` (str, optional, default ``"linear"``): Interpolation of reported par spreads:
              ``"linear"`` or ``"log_linear"``.
            - ``doc_clause`` (str | None, optional, default ``None``): CDS documentation-clause assertion;
              when set it must match the quote conventions.
            - ``cds_valuation_convention`` (str | None, optional, default ``None``): Synthetic-CDS valuation
              policy: ``"isda_dirty"``, ``"bloomberg_cdsw_clean"``, ``"bloomberg_cdsw_clean_full_premium"`` or
              ``"quant_lib_isda_parity"``.

        quote_set : str | None, default None
            Name of a quote set declared in ``CalibrationPlan.quote_sets``; ``None`` uses ``id``.

        Returns
        -------
        CalibrationStep
            The typed step.

        Raises
        ------
        ValueError
            If ``params`` is not an object, a required key is missing, a key is unknown to the Rust step
            schema, or a value has the wrong shape or an unknown label.
        TypeError
            If a ``params`` value is not JSON-serializable.

        Examples:
        --------
        >>> from finstack_quant.calibration import CalibrationStep
        >>> params = {
        ...     "entity": "ACME",
        ...     "currency": "USD",
        ...     "base_date": "2024-06-28",
        ...     "discount_curve_id": "usd_ois",
        ...     "recovery_rate": 0.4,
        ... }
        >>> step = CalibrationStep.hazard("acme_hz", params)
        >>> step.kind
        'hazard'

        """

    @staticmethod
    def inflation(id: str, params: dict[str, Any] | str, quote_set: str | None = None) -> CalibrationStep:
        """Build an inflation-curve bootstrap step.

        Parameters
        ----------
        id : str
            Unique step identifier; also the residual key in the result and the default ``quote_set`` and
            ``curve_id``.
        params : dict[str, Any] | str
            Wire fields of the ``inflation`` step as a dict, or the same object as JSON text, keyed as in the
            Rust ``StepParams`` schema. Values are JSON values; ``Currency`` wrappers are accepted. Keys:

            - ``currency`` (str | Currency, required): Upper-case ISO 4217 code (or a ``Currency``) of the
              curve currency.
            - ``base_date`` (str, required): Valuation and calibration-instrument start date as an ISO-8601
              date string (``"YYYY-MM-DD"``). The curve's reference CPI date is this date minus
              ``observation_lag``.
            - ``discount_curve_id`` (str, required): Identifier of the discount curve solved in an earlier
              step.
            - ``index`` (str, required): Inflation index identifier, for example ``"USA-CPI-U"``.
            - ``observation_lag`` (str, required): Whole-month observation lag, for example ``"3M"``, up to
              255 months. No lag accepts ``""``, ``"none"``, ``"0"``, ``"0M"`` or ``"0D"`` (case-insensitive);
              nonzero day lags are rejected.
            - ``base_cpi`` (float, required): Contractual reference CPI at the start date in index points,
              after observation lag and monthly interpolation; supplied index fixings must reproduce it,
              including seasonality.
            - ``curve_id`` (str, optional, default ``id``): Identifier of the produced curve.
            - ``notional`` (float, optional, default ``1.0``): Notional of the synthetic inflation swaps
              repriced during calibration.
            - ``method`` (str | dict, optional, default ``"bootstrap"``): Calibration method: ``"bootstrap"``
              or ``{"global_solve": {"use_analytical_jacobian": bool}}``.
            - ``interpolation`` (str, optional, default ``"linear"``): CPI interpolation: ``"linear"``,
              ``"log_linear"``, ``"monotone_convex"``, ``"cubic_hermite"`` or
              ``"piecewise_quadratic_forward"``.

        quote_set : str | None, default None
            Name of a quote set declared in ``CalibrationPlan.quote_sets``; ``None`` uses ``id``.

        Returns
        -------
        CalibrationStep
            The typed step.

        Raises
        ------
        ValueError
            If ``params`` is not an object, a required key is missing, a key is unknown to the Rust step
            schema, or a value has the wrong shape or an unknown label.
        TypeError
            If a ``params`` value is not JSON-serializable.

        Examples:
        --------
        >>> from finstack_quant.calibration import CalibrationStep
        >>> params = {
        ...     "currency": "USD",
        ...     "base_date": "2024-06-28",
        ...     "discount_curve_id": "usd_ois",
        ...     "index": "USA-CPI-U",
        ...     "observation_lag": "3M",
        ...     "base_cpi": 310.0,
        ... }
        >>> step = CalibrationStep.inflation("us_cpi", params)
        >>> step.kind
        'inflation'

        """

    @staticmethod
    def vol_surface(id: str, params: dict[str, Any] | str, quote_set: str | None = None) -> CalibrationStep:
        """Build an equity/FX SABR volatility-surface fit step from option vol quotes.

        Parameters
        ----------
        id : str
            Unique step identifier; also the residual key in the result and the default ``quote_set`` and
            ``vol_surface_id``.
        params : dict[str, Any] | str
            Wire fields of the ``vol_surface`` step as a dict, or the same object as JSON text, keyed as in
            the Rust ``StepParams`` schema. Values are JSON values; ``Currency`` wrappers are accepted. Keys:

            - ``base_date`` (str, required): Surface base date as an ISO-8601 date string (``"YYYY-MM-DD"``).
            - ``underlying_ticker`` (str, required): Underlying identifier the quotes reference.
            - ``vol_surface_id`` (str, optional, default ``id``): Identifier of the produced surface.
            - ``discount_curve_id`` (str | None, optional, default ``None``): Discount curve used for
              forwards.
            - ``beta`` (float, optional, default ``0.5``): SABR beta.
            - ``target_expiries`` (list[float], optional, default ``[]``): Expiry grid of the published
              surface in years.
            - ``target_strikes`` (list[float], optional, default ``[]``): Strike grid of the published surface
              in price units.
            - ``spot_override`` (float | None, optional, default ``None``): Spot price used instead of the
              market scalar.
            - ``dividend_yield_override`` (float | None, optional, default ``None``): Continuous dividend
              yield as a decimal.
            - ``expiry_extrapolation`` (str, optional, default ``"error"``): Total-variance fill outside the
              calibrated expiries: ``"error"`` or ``"clamp"``.

        quote_set : str | None, default None
            Name of a quote set declared in ``CalibrationPlan.quote_sets``; ``None`` uses ``id``.

        Returns
        -------
        CalibrationStep
            The typed step.

        Raises
        ------
        ValueError
            If ``params`` is not an object, a required key is missing, a key is unknown to the Rust step
            schema, or a value has the wrong shape or an unknown label.
        TypeError
            If a ``params`` value is not JSON-serializable.

        Examples:
        --------
        >>> from finstack_quant.calibration import CalibrationStep
        >>> params = {
        ...     "base_date": "2024-06-28",
        ...     "underlying_ticker": "SPX",
        ... }
        >>> step = CalibrationStep.vol_surface("spx_vol", params)
        >>> step.kind
        'vol_surface'

        """

    @staticmethod
    def swaption_vol(id: str, params: dict[str, Any] | str, quote_set: str | None = None) -> CalibrationStep:
        """Build a swaption volatility-cube (SABR) fit step.

        Parameters
        ----------
        id : str
            Unique step identifier; also the residual key in the result and the default ``quote_set`` and
            ``vol_surface_id``.
        params : dict[str, Any] | str
            Wire fields of the ``swaption_vol`` step as a dict, or the same object as JSON text, keyed as in
            the Rust ``StepParams`` schema. Values are JSON values; ``Currency`` wrappers are accepted. Keys:

            - ``base_date`` (str, required): Surface base date as an ISO-8601 date string (``"YYYY-MM-DD"``).
            - ``discount_curve_id`` (str, required): Identifier of the discount curve solved in an earlier
              step.
            - ``currency`` (str | Currency, required): Upper-case ISO 4217 code (or a ``Currency``) of the
              swaption currency.
            - ``vol_surface_id`` (str, optional, default ``id``): Identifier of the produced surface.
            - ``forward_id`` (str | None, optional, default ``None``): Forward curve, if different from the
              discount curve.
            - ``swap_index`` (str | None, optional, default ``None``): Floating index whose conventions define
              the underlying swaps; when omitted every quote must carry ``float_leg_conventions.index``.
            - ``vol_convention`` (str | dict, optional, default ``"lognormal"``): Quoting convention:
              ``"normal"``, ``"lognormal"`` or ``{"shifted_lognormal": ...}``.
            - ``sabr_beta`` (float, optional, default ``0.5``): SABR beta (``0.0`` normal-like, ``1.0``
              lognormal).
            - ``sabr_extrapolation`` (str, optional, default ``"error"``): SABR-parameter extrapolation across
              the expiry-tenor grid: ``"error"`` or ``"clamp"``.
            - ``allow_sabr_missing_bucket_fallback`` (bool, optional, default ``False``): Allow a nearby
              bucket to stand in for a missing SABR grid corner.
            - ``target_expiries`` (list[float], optional, default ``[]``): Option expiries in ACT/365F years
              from ``base_date``.
            - ``target_tenors`` (list[float], optional, default ``[]``): Underlying swap tenors in years.
            - ``fixed_day_count`` (str | None, optional, default ``None``): Fixed-leg coupon accrual day count
              only; option expiry and variance always use ACT/365F.
            - ``calendar_id`` (str | None, optional, default ``None``): Holiday calendar for date adjustments.
            - ``vol_tolerance`` (float | None, optional, default ``None`` (0.0015)): Maximum absolute error of
              any fitted volatility, in the quote's volatility units.

        quote_set : str | None, default None
            Name of a quote set declared in ``CalibrationPlan.quote_sets``; ``None`` uses ``id``.

        Returns
        -------
        CalibrationStep
            The typed step.

        Raises
        ------
        ValueError
            If ``params`` is not an object, a required key is missing, a key is unknown to the Rust step
            schema, or a value has the wrong shape or an unknown label.
        TypeError
            If a ``params`` value is not JSON-serializable.

        Examples:
        --------
        >>> from finstack_quant.calibration import CalibrationStep
        >>> params = {
        ...     "base_date": "2024-06-28",
        ...     "discount_curve_id": "usd_ois",
        ...     "currency": "USD",
        ... }
        >>> step = CalibrationStep.swaption_vol("usd_swpn", params)
        >>> step.kind
        'swaption_vol'

        """

    @staticmethod
    def base_correlation(id: str, params: dict[str, Any] | str, quote_set: str | None = None) -> CalibrationStep:
        """Build an index-tranche base-correlation bootstrap step.

        Parameters
        ----------
        id : str
            Unique step identifier; also the residual key in the result and the default ``quote_set``.
        params : dict[str, Any] | str
            Wire fields of the ``base_correlation`` step as a dict, or the same object as JSON text, keyed as
            in the Rust ``StepParams`` schema. Values are JSON values; ``Currency`` wrappers are accepted.
            Keys:

            - ``index_id`` (str, required): Credit index identifier, for example ``"CDX.NA.IG"``.
            - ``series`` (int, required): Index series number.
            - ``maturity_years`` (float, required): Finite positive CDS tenor in years, a whole number of
              months; quotes must resolve to the same convention maturity.
            - ``base_date`` (str, required): Calibration base date as an ISO-8601 date string
              (``"YYYY-MM-DD"``).
            - ``discount_curve_id`` (str, required): Identifier of the discount curve solved in an earlier
              step.
            - ``currency`` (str | Currency, required): Upper-case ISO 4217 code (or a ``Currency``) of the
              tranche currency.
            - ``detachment_points`` (list[float], optional, default ``[]``): Expected detachment points as
              decimals in ``(0, 1]``; when non-empty every quote must match one point and every point must be
              quoted.
            - ``notional`` (float, optional, default ``1.0``): Notional of the synthetic tranches.
            - ``frequency`` (dict | None, optional, default ``None``): Premium frequency as a tenor object
              (convention default).
            - ``day_count`` (str | None, optional, default ``None``): Premium accrual day count (convention
              default).
            - ``business_day_convention`` (str | None, optional, default ``None``): Schedule business-day
              convention.
            - ``calendar_id`` (str | None, optional, default ``None``): Holiday calendar for schedule
              generation.
            - ``roll_rule`` (str, optional, default ``"none"``): Coupon roll grid: ``"cds_imm"`` (20
              Mar/Jun/Sep/Dec) or ``"none"``; ``"imm"`` is rejected.

        quote_set : str | None, default None
            Name of a quote set declared in ``CalibrationPlan.quote_sets``; ``None`` uses ``id``.

        Returns
        -------
        CalibrationStep
            The typed step.

        Raises
        ------
        ValueError
            If ``params`` is not an object, a required key is missing, a key is unknown to the Rust step
            schema, or a value has the wrong shape or an unknown label.
        TypeError
            If a ``params`` value is not JSON-serializable.

        Examples:
        --------
        >>> from finstack_quant.calibration import CalibrationStep
        >>> params = {
        ...     "index_id": "CDX.NA.IG",
        ...     "series": 42,
        ...     "maturity_years": 5.0,
        ...     "base_date": "2024-06-28",
        ...     "discount_curve_id": "usd_ois",
        ...     "currency": "USD",
        ... }
        >>> step = CalibrationStep.base_correlation("cdx_bc", params)
        >>> step.kind
        'base_correlation'

        """

    @staticmethod
    def student_t(id: str, params: dict[str, Any] | str, quote_set: str | None = None) -> CalibrationStep:
        """Build a Student-t copula degrees-of-freedom fit step for one tranche.

        Parameters
        ----------
        id : str
            Unique step identifier; also the residual key in the result and the default ``quote_set``.
        params : dict[str, Any] | str
            Wire fields of the ``student_t`` step as a dict, or the same object as JSON text, keyed as in the
            Rust ``StepParams`` schema. Values are JSON values; ``Currency`` wrappers are accepted. Keys:

            - ``tranche_instrument_id`` (str, required): Tranche instrument repriced during the fit; the
              result is written to the scalar ``"{tranche_instrument_id}_STUDENT_T_DF"``.
            - ``base_correlation_curve_id`` (str, required): Identifier of the base-correlation curve solved
              in an earlier step.
            - ``discount_curve_id`` (str | None, optional, default ``None``): Discount curve pricing the
              tranche; ``None`` uses the only discount curve in the market.
            - ``initial_df`` (float, optional, default ``5.0``): Starting degrees of freedom.
            - ``df_bounds`` (list[float], optional, default ``[2.1, 50.0]``): Feasible ``[lower, upper]``
              degrees of freedom (> 2).
            - ``correlation`` (float, optional, default ``0.3``): Flat correlation of the tranche.

        quote_set : str | None, default None
            Name of a quote set declared in ``CalibrationPlan.quote_sets``; ``None`` uses ``id``.

        Returns
        -------
        CalibrationStep
            The typed step.

        Raises
        ------
        ValueError
            If ``params`` is not an object, a required key is missing, a key is unknown to the Rust step
            schema, or a value has the wrong shape or an unknown label.
        TypeError
            If a ``params`` value is not JSON-serializable.

        Examples:
        --------
        >>> from finstack_quant.calibration import CalibrationStep
        >>> params = {
        ...     "tranche_instrument_id": "tr_0_3",
        ...     "base_correlation_curve_id": "cdx_bc",
        ... }
        >>> step = CalibrationStep.student_t("t", params)
        >>> step.kind
        'student_t'

        """

    @staticmethod
    def hull_white(id: str, params: dict[str, Any] | str, quote_set: str | None = None) -> CalibrationStep:
        """Build a Hull-White one-factor swaption calibration step.

        Parameters
        ----------
        id : str
            Unique step identifier; also the residual key in the result and the default ``quote_set``.
        params : dict[str, Any] | str
            Wire fields of the ``hull_white`` step as a dict, or the same object as JSON text, keyed as in the
            Rust ``StepParams`` schema. Values are JSON values; ``Currency`` wrappers are accepted. Keys:

            - ``curve_id`` (str, required): Discount curve the model is fitted against (must already exist).
            - ``currency`` (str | Currency, required): Upper-case ISO 4217 code (or a ``Currency``) of the
              model currency.
            - ``base_date`` (str, required): Model base date as an ISO-8601 date string (``"YYYY-MM-DD"``).
            - ``fit_tolerance`` (float, required): Finite positive maximum implied-quote error in the supplied
              volatility units (decimal rate volatility for normal quotes, relative volatility for Black
              quotes), separate from the solver tolerance.
            - ``initial_kappa`` (float | None, optional, default ``None``): Initial mean-reversion guess.
            - ``initial_sigma`` (float | None, optional, default ``None``): Initial short-rate volatility
              guess.

        quote_set : str | None, default None
            Name of a quote set declared in ``CalibrationPlan.quote_sets``; ``None`` uses ``id``.

        Returns
        -------
        CalibrationStep
            The typed step.

        Raises
        ------
        ValueError
            If ``params`` is not an object, a required key is missing, a key is unknown to the Rust step
            schema, or a value has the wrong shape or an unknown label.
        TypeError
            If a ``params`` value is not JSON-serializable.

        Examples:
        --------
        >>> from finstack_quant.calibration import CalibrationStep
        >>> params = {
        ...     "curve_id": "usd_ois",
        ...     "currency": "USD",
        ...     "base_date": "2024-06-28",
        ...     "fit_tolerance": 0.0001,
        ... }
        >>> step = CalibrationStep.hull_white("hw", params)
        >>> step.kind
        'hull_white'

        """

    @staticmethod
    def cap_floor_hull_white(id: str, params: dict[str, Any] | str, quote_set: str | None = None) -> CalibrationStep:
        """Build a Hull-White one-factor cap/floor calibration step.

        Parameters
        ----------
        id : str
            Unique step identifier; also the residual key in the result and the default ``quote_set``.
        params : dict[str, Any] | str
            Wire fields of the ``cap_floor_hull_white`` step as a dict, or the same object as JSON text, keyed
            as in the Rust ``StepParams`` schema. Values are JSON values; ``Currency`` wrappers are accepted.
            Keys:

            - ``discount_curve_id`` (str, required): Discounting curve and output-key prefix. Scalar fits
              write ``"{discount_curve_id}_CAPFLOOR_HW1F_KAPPA"`` and
              ``"{discount_curve_id}_CAPFLOOR_HW1F_SIGMA"``; piecewise fits write the same kappa scalar and
              the series ``"{discount_curve_id}_CAPFLOOR_HW1F_SIGMA_SCHEDULE"``.
            - ``forward_curve_id`` (str, required): Curve projecting the caplet forwards; equal to
              ``discount_curve_id`` for single-curve.
            - ``index_id`` (str, required): Term-rate index, for example ``"EUR-EURIBOR-3M"``, whose
              conventions set the caplet dates and accruals; overnight indices are rejected.
            - ``currency`` (str | Currency, required): Upper-case ISO 4217 code (or a ``Currency``) of the
              model currency; must match the index.
            - ``base_date`` (str, required): Model base date as an ISO-8601 date string (``"YYYY-MM-DD"``).
            - ``fit_tolerance`` (float, required): Finite positive maximum implied-quote error in the supplied
              volatility units, separate from the solver tolerance.
            - ``volatility_mode`` (str, optional, default ``"scalar"``): ``"scalar"`` or ``"piecewise"``
              sigma.
            - ``fixed_kappa`` (float | None, optional, default ``None``): Fixed mean reversion; required for a
              one-quote fit.
            - ``initial_kappa`` (float | None, optional, default ``None``): Initial mean-reversion guess.
            - ``initial_sigma`` (float | None, optional, default ``None``): Initial short-rate volatility
              guess.

        quote_set : str | None, default None
            Name of a quote set declared in ``CalibrationPlan.quote_sets``; ``None`` uses ``id``.

        Returns
        -------
        CalibrationStep
            The typed step.

        Raises
        ------
        ValueError
            If ``params`` is not an object, a required key is missing, a key is unknown to the Rust step
            schema, or a value has the wrong shape or an unknown label.
        TypeError
            If a ``params`` value is not JSON-serializable.

        Examples:
        --------
        >>> from finstack_quant.calibration import CalibrationStep
        >>> params = {
        ...     "discount_curve_id": "eur_ois",
        ...     "forward_curve_id": "eur_3m",
        ...     "index_id": "EUR-EURIBOR-3M",
        ...     "currency": "EUR",
        ...     "base_date": "2024-06-28",
        ...     "fit_tolerance": 0.0001,
        ... }
        >>> step = CalibrationStep.cap_floor_hull_white("hw_cf", params)
        >>> step.kind
        'cap_floor_hull_white'

        """

    @staticmethod
    def svi_surface(id: str, params: dict[str, Any] | str, quote_set: str | None = None) -> CalibrationStep:
        """Build an SVI volatility-surface fit step from option vol quotes.

        Parameters
        ----------
        id : str
            Unique step identifier; also the residual key in the result and the default ``quote_set`` and
            ``vol_surface_id``.
        params : dict[str, Any] | str
            Wire fields of the ``svi_surface`` step as a dict, or the same object as JSON text, keyed as in
            the Rust ``StepParams`` schema. Values are JSON values; ``Currency`` wrappers are accepted. Keys:

            - ``base_date`` (str, required): Surface base date as an ISO-8601 date string (``"YYYY-MM-DD"``).
            - ``underlying_ticker`` (str, required): Underlying identifier the quotes reference.
            - ``vol_surface_id`` (str, optional, default ``id``): Identifier of the produced surface.
            - ``discount_curve_id`` (str | None, optional, default ``None``): Discount curve used for
              forwards.
            - ``target_expiries`` (list[float], optional, default ``[]``): Expiry grid of the published
              surface in years.
            - ``target_strikes`` (list[float], optional, default ``[]``): Strike grid of the published surface
              in price units.
            - ``spot_override`` (float | None, optional, default ``None``): Spot price used instead of the
              market scalar.
            - ``dividend_yield_override`` (float | None, optional, default ``None``): Continuous dividend
              yield as a decimal; ``None`` reads the scalar ``"{underlying_ticker}-DIVYIELD"`` (or a
              cash-dividend schedule when a discount curve is set).

        quote_set : str | None, default None
            Name of a quote set declared in ``CalibrationPlan.quote_sets``; ``None`` uses ``id``.

        Returns
        -------
        CalibrationStep
            The typed step.

        Raises
        ------
        ValueError
            If ``params`` is not an object, a required key is missing, a key is unknown to the Rust step
            schema, or a value has the wrong shape or an unknown label.
        TypeError
            If a ``params`` value is not JSON-serializable.

        Examples:
        --------
        >>> from finstack_quant.calibration import CalibrationStep
        >>> params = {
        ...     "base_date": "2024-06-28",
        ...     "underlying_ticker": "SPX",
        ... }
        >>> step = CalibrationStep.svi_surface("spx_svi", params)
        >>> step.kind
        'svi_surface'

        """

    @staticmethod
    def xccy_basis(id: str, params: dict[str, Any] | str, quote_set: str | None = None) -> CalibrationStep:
        """Build a cross-currency basis (foreign discount curve) bootstrap step.

        Parameters
        ----------
        id : str
            Unique step identifier; also the residual key in the result and the default ``quote_set`` and
            ``curve_id``.
        params : dict[str, Any] | str
            Wire fields of the ``xccy_basis`` step as a dict, or the same object as JSON text, keyed as in the
            Rust ``StepParams`` schema. Values are JSON values; ``Currency`` wrappers are accepted. Keys:

            - ``currency`` (str | Currency, required): Upper-case ISO 4217 code (or a ``Currency``) of the
              foreign (solved) currency.
            - ``base_date`` (str, required): Curve base date as an ISO-8601 date string (``"YYYY-MM-DD"``).
            - ``fx_spot`` (float, required): T+0 cash FX rate (domestic per foreign) used when a quote omits
              ``spot_fx``.
            - ``domestic_discount_id`` (str, required): Domestic discount curve solved in an earlier step.
            - ``curve_id`` (str, optional, default ``id``): Identifier of the produced foreign curve.
            - ``basis_spread_curve_id`` (str | None, optional, default ``None``): Identifier of the byproduct
              basis-spread curve.
            - ``method`` (str, optional, default ``"bootstrap"``): Sequential ``"bootstrap"``;
              ``global_solve`` is rejected.
            - ``interpolation`` (str, optional, default ``"log_linear"``): Discount-factor interpolation:
              ``"linear"``, ``"log_linear"``, ``"monotone_convex"``, ``"cubic_hermite"`` or
              ``"piecewise_quadratic_forward"``.
            - ``extrapolation`` (str, optional, default ``"flat_forward"``): Extrapolation policy:
              ``"flat_zero"``, ``"flat_forward"`` or ``"none"``.
            - ``conventions`` (dict, optional, default both ``None``): Step-level rates conventions
              ``{"curve_day_count": ..., "ois_compounding": ...}``; both entries default to ``None`` (the
              quote conventions apply).

        quote_set : str | None, default None
            Name of a quote set declared in ``CalibrationPlan.quote_sets``; ``None`` uses ``id``.

        Returns
        -------
        CalibrationStep
            The typed step.

        Raises
        ------
        ValueError
            If ``params`` is not an object, a required key is missing, a key is unknown to the Rust step
            schema, or a value has the wrong shape or an unknown label.
        TypeError
            If a ``params`` value is not JSON-serializable.

        Examples:
        --------
        >>> from finstack_quant.calibration import CalibrationStep
        >>> params = {
        ...     "currency": "EUR",
        ...     "base_date": "2024-06-28",
        ...     "fx_spot": 1.08,
        ...     "domestic_discount_id": "usd_ois",
        ... }
        >>> step = CalibrationStep.xccy_basis("eur_xccy", params)
        >>> step.kind
        'xccy_basis'

        """

    @staticmethod
    def parametric(id: str, params: dict[str, Any] | str, quote_set: str | None = None) -> CalibrationStep:
        """Build a parametric (Nelson-Siegel / Svensson) single-curve fit step.

        Parameters
        ----------
        id : str
            Unique step identifier; also the residual key in the result and the default ``quote_set`` and
            ``curve_id``.
        params : dict[str, Any] | str
            Wire fields of the ``parametric`` step as a dict, or the same object as JSON text, keyed as in the
            Rust ``StepParams`` schema. Values are JSON values; ``Currency`` wrappers are accepted. Keys:

            - ``base_date`` (str, required): Curve base date as an ISO-8601 date string (``"YYYY-MM-DD"``).
            - ``curve_id`` (str, optional, default ``id``): Identifier of the single discount curve fitted;
              every instrument discounts and projects on it.
            - ``model`` (str | None, optional, default ``"ns"``): ``"ns"`` (Nelson-Siegel) or ``"nss"``
              (Svensson). Omitted or ``None`` uses the Rust authoring default
              ``CalibrationStep::DEFAULT_PARAMETRIC_MODEL``.
            - ``initial_params`` (dict | None, optional, default ``None``): Initial guesses as a
              ``NelsonSiegelModel`` object (``beta0``, ``beta1``, ``beta2``, ``tau`` for NS; plus ``beta3``,
              ``tau1``, ``tau2`` for NSS). Fit acceptance uses the discount-curve tolerance in per-notional PV
              units without a floor; ``discount_curve_id`` is not accepted.

        quote_set : str | None, default None
            Name of a quote set declared in ``CalibrationPlan.quote_sets``; ``None`` uses ``id``.

        Returns
        -------
        CalibrationStep
            The typed step.

        Raises
        ------
        ValueError
            If ``params`` is not an object, a required key is missing, a key is unknown to the Rust step
            schema, or a value has the wrong shape or an unknown label.
        TypeError
            If a ``params`` value is not JSON-serializable.

        Examples:
        --------
        >>> from finstack_quant.calibration import CalibrationStep
        >>> step = CalibrationStep.parametric("ns", {"base_date": "2024-06-28", "model": "ns"})
        >>> step.kind
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
        >>> step = CalibrationStep.discount("USD-OIS", {"currency": "USD", "base_date": "2026-05-08"})
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
        ...     [CalibrationStep.discount("USD-OIS", {"currency": "USD", "base_date": "2026-05-08"})],
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
