"""Typed construction of native calibration step parameters."""

from __future__ import annotations

from collections.abc import Sequence
from datetime import date
from typing import NotRequired, TypeAlias, TypedDict, Unpack

from finstack_quant.core.currency import Currency
from finstack_quant.core.dates import DayCount, Tenor

class _DiscountCurveParamsFields(TypedDict):
    """Named inputs validated by the canonical discount curve calibration schema."""

    curve_id: str
    currency: Currency | str
    base_date: date | str
    method: NotRequired[str | dict[str, object]]
    interpolation: NotRequired[str]
    extrapolation: NotRequired[str]
    pricing_discount_id: NotRequired[str | None]
    pricing_forward_id: NotRequired[str | None]
    conventions: NotRequired[dict[str, object]]

class DiscountCurveParams:
    """Immutable parameters for discount curve calibration.

    Examples
    --------
    >>> from finstack_quant.calibration import DiscountCurveParams
    >>> params = DiscountCurveParams(
    ...     curve_id="USD-OIS",
    ...     currency="USD",
    ...     base_date="2026-05-08",
    ... )
    >>> params.to_dict()["curve_id"]
    'USD-OIS'
    """

    def __init__(self, **fields: Unpack[_DiscountCurveParamsFields]) -> None:
        """Read typed fields and supply the native defaults for omitted inputs.

        Parameters
        ----------
        fields : _DiscountCurveParamsFields
            `curve_id` specifies identifier for the discount curve being built.
            `currency` specifies currency of the curve.
            `base_date` specifies base date for the curve.
            `method` specifies calibration method to use.
            `interpolation` specifies interpolation style for the constructed discount curve. Defaults to log-linear discount factors, preserving positive discount factors and piecewise-constant continuously compounded forwards. `Linear` remains available only when explicitly requested.
            `extrapolation` specifies extrapolation policy for the curve.
            `pricing_discount_id` specifies optional separate ID for pricing logic (defaults to curve_id).
            `pricing_forward_id` specifies optional forward curve ID for pricing (if needed).
            `conventions` specifies step-level conventions for pricing and curve time axis.

        Raises
        ------
        ValueError
            If a field is unknown, missing, or has an invalid type or wire value.
        """
        ...

    def to_dict(self) -> _DiscountCurveParamsFields:
        """Export an independent snapshot of the native parameters.

        Returns
        -------
        _DiscountCurveParamsFields
            Effective parameter values as a plain mapping, including defaults.

        Raises
        ------
        ValueError
            If a native parameter cannot be exported.
        """
        ...

class _ForwardCurveParamsFields(TypedDict):
    """Named inputs validated by the canonical forward curve calibration schema."""

    curve_id: str
    currency: Currency | str
    base_date: date | str
    index_tenor: NotRequired[Tenor | dict[str, object] | None]
    discount_curve_id: str
    method: NotRequired[str | dict[str, object]]
    interpolation: NotRequired[str]
    extrapolation: NotRequired[str]
    conventions: NotRequired[dict[str, object]]

class ForwardCurveParams:
    """Immutable parameters for forward curve calibration.

    Examples
    --------
    >>> from finstack_quant.calibration import ForwardCurveParams
    >>> params = ForwardCurveParams(
    ...     curve_id="USD-SOFR-3M",
    ...     base_date="2026-05-08",
    ...     currency="USD",
    ...     discount_curve_id="USD-OIS",
    ... )
    >>> params.to_dict()["curve_id"]
    'USD-SOFR-3M'
    """

    def __init__(self, **fields: Unpack[_ForwardCurveParamsFields]) -> None:
        """Read typed fields and supply the native defaults for omitted inputs.

        Parameters
        ----------
        fields : _ForwardCurveParamsFields
            `curve_id` specifies identifier for the forward curve being built.
            `currency` specifies currency of the curve.
            `base_date` specifies base date for the curve.
            `index_tenor` specifies contractual index tenor metadata attached to the calibrated forward curve.
            `discount_curve_id` specifies identifier for the discount curve to use.
            `method` specifies calibration method to use. Forward curves require [`CalibrationMethod::GlobalSolve`], which is the default (finite-difference Jacobian). Sequential bootstrap is rejected because contractual off-grid reset intervals couple adjacent rates through projection discount factors.
            `interpolation` specifies interpolation style for the constructed forward curve's projection factors. Defaults to log-linear factor interpolation (`log_linear`). All five [`InterpStyle`] variants (`linear`, `log_linear`, `cubic_hermite`, `piecewise_quadratic_forward`, `monotone_convex`) are supported on the underlying positive projection-factor evaluator.
            `extrapolation` specifies policy controlling queries beyond the final calibrated pillar time.
            `conventions` specifies step-level conventions for pricing and curve time axis.

        Raises
        ------
        ValueError
            If a field is unknown, missing, or has an invalid type or wire value.
        """
        ...

    def to_dict(self) -> _ForwardCurveParamsFields:
        """Export an independent snapshot of the native parameters.

        Returns
        -------
        _ForwardCurveParamsFields
            Effective parameter values as a plain mapping, including defaults.

        Raises
        ------
        ValueError
            If a native parameter cannot be exported.
        """
        ...

class _HazardCurveParamsFields(TypedDict):
    """Named inputs validated by the canonical hazard curve calibration schema."""

    curve_id: str
    entity: str
    seniority: str
    currency: Currency | str
    base_date: date | str
    discount_curve_id: str
    recovery_rate: float
    notional: NotRequired[float]
    method: NotRequired[str | dict[str, object]]
    doc_clause: NotRequired[str | None]
    cds_valuation_convention: NotRequired[dict[str, object] | None]

class HazardCurveParams:
    """Immutable parameters for hazard curve calibration.

    Examples
    --------
    >>> from finstack_quant.calibration import HazardCurveParams
    >>> params = HazardCurveParams(
    ...     curve_id="ACME",
    ...     base_date="2026-05-08",
    ...     currency="USD",
    ...     discount_curve_id="USD-OIS",
    ...     entity="ACME",
    ...     seniority="senior",
    ...     recovery_rate=0.4,
    ... )
    >>> params.to_dict()["curve_id"]
    'ACME'
    """

    def __init__(self, **fields: Unpack[_HazardCurveParamsFields]) -> None:
        """Read typed fields and supply the native defaults for omitted inputs.

        Parameters
        ----------
        fields : _HazardCurveParamsFields
            `curve_id` specifies identifier for the hazard curve being built.
            `entity` specifies entity name.
            `seniority` specifies seniority of the debt.
            `currency` specifies currency of the curve.
            `base_date` specifies base date for the curve.
            `discount_curve_id` specifies identifier for the discount curve to use.
            `recovery_rate` specifies required recovery-rate assumption as a decimal fraction in `[0, 1]`.
            `notional` specifies notional used to price synthetic CDS instruments during calibration. Calibration normalizes residuals by notional, so this is typically left as the unit-notional default unless you have a specific reason to change it.
            `method` specifies calibration method to use.
            `doc_clause` specifies optional CDS documentation-clause assertion. Hazard schedule conventions come from the quote `CdsConventionKey`. When this field is set, it must be consistent with those quote-derived conventions (same clause or the matching regional family).
            `cds_valuation_convention` specifies optional CDS valuation convention used by synthetic CDS instruments during hazard calibration and rebootstrap.

        Raises
        ------
        ValueError
            If a field is unknown, missing, or has an invalid type or wire value.
        """
        ...

    def to_dict(self) -> _HazardCurveParamsFields:
        """Export an independent snapshot of the native parameters.

        Returns
        -------
        _HazardCurveParamsFields
            Effective parameter values as a plain mapping, including defaults.

        Raises
        ------
        ValueError
            If a native parameter cannot be exported.
        """
        ...

class _InflationCurveParamsFields(TypedDict):
    """Named inputs validated by the canonical inflation curve calibration schema."""

    curve_id: str
    currency: Currency | str
    base_date: date | str
    discount_curve_id: str
    index: str
    observation_lag: str
    base_cpi: float
    source_base_date: date | str
    source_base_cpi: float
    day_count: DayCount | str
    calendar_id: str | None
    notional: NotRequired[float]
    method: NotRequired[str | dict[str, object]]
    interpolation: str
    extrapolation: str

class InflationCurveParams:
    """Immutable parameters for inflation curve calibration.

    Examples
    --------
    >>> from finstack_quant.calibration import InflationCurveParams
    >>> params = InflationCurveParams(
    ...     curve_id="USA-CPI",
    ...     base_date="2026-05-08",
    ...     currency="USD",
    ...     discount_curve_id="USD-OIS",
    ...     index="USA-CPI-U",
    ...     observation_lag="3M",
    ...     base_cpi=310.0,
    ...     source_base_date="2026-02-01",
    ...     source_base_cpi=310.0,
    ...     day_count="act_365f",
    ...     interpolation="linear",
    ...     extrapolation="extend",
    ... )
    >>> params.to_dict()["curve_id"]
    'USA-CPI'
    """

    def __init__(self, **fields: Unpack[_InflationCurveParamsFields]) -> None:
        """Read typed fields and supply the native defaults for omitted inputs.

        Parameters
        ----------
        fields : _InflationCurveParamsFields
            `curve_id` specifies identifier for the inflation curve being built.
            `currency` specifies currency of the curve.
            `base_date` specifies valuation and calibration-instrument start date, independent of the source origin.
            `discount_curve_id` specifies identifier for the discount curve to use.
            `index` specifies reference index (e.g. "USA-CPI-U").
            `observation_lag` specifies explicit contractual month observation lag (e.g. "3M"); "none" means zero months. Overrides the quote convention's lag without changing the projection source origin.
            `base_cpi` specifies explicit contractual reference CPI at the start date, after lag and interpolation. This denominator is independent of the projection source's origin CPI.
            `source_base_date` specifies explicit original date of the absolute CPI projection source.
            `source_base_cpi` specifies finite positive absolute CPI level at the projection source's origin.
            `day_count` specifies complete source-clock convention used for every projected anchor.
            `calendar_id` specifies source-clock holiday calendar; required for business-day clocks.
            `notional` specifies notional used to price synthetic inflation swaps during calibration. Calibration normalizes residuals by notional, so this is typically left as the unit-notional default unless you have a specific reason to change it.
            `method` specifies calibration method to use.
            `interpolation` specifies interpolation style for the curve.
            `extrapolation` specifies explicit scalar CPI tail policy, applied without lag or normalization.

        Raises
        ------
        ValueError
            If a field is unknown, missing, or has an invalid type or wire value.
        """
        ...

    def to_dict(self) -> _InflationCurveParamsFields:
        """Export an independent snapshot of the native parameters.

        Returns
        -------
        _InflationCurveParamsFields
            Effective parameter values as a plain mapping, including defaults.

        Raises
        ------
        ValueError
            If a native parameter cannot be exported.
        """
        ...

class _VolSurfaceParamsFields(TypedDict):
    """Named inputs validated by the canonical vol surface calibration schema."""

    vol_surface_id: str
    base_date: date | str
    underlying_ticker: str
    discount_curve_id: NotRequired[str | None]
    beta: NotRequired[float]
    target_expiries: NotRequired[Sequence[float]]
    target_strikes: NotRequired[Sequence[float]]
    spot_override: NotRequired[float | None]
    dividend_yield_override: NotRequired[float | None]
    expiry_extrapolation: NotRequired[str]

class VolSurfaceParams:
    """Immutable parameters for vol surface calibration.

    Examples
    --------
    >>> from finstack_quant.calibration import VolSurfaceParams
    >>> params = VolSurfaceParams(
    ...     vol_surface_id="AAPL-VOL",
    ...     base_date="2026-05-08",
    ...     underlying_ticker="AAPL",
    ... )
    >>> params.to_dict()["vol_surface_id"]
    'AAPL-VOL'
    """

    def __init__(self, **fields: Unpack[_VolSurfaceParamsFields]) -> None:
        """Read typed fields and supply the native defaults for omitted inputs.

        Parameters
        ----------
        fields : _VolSurfaceParamsFields
            `vol_surface_id` specifies identifier for the volatility surface being built.
            `base_date` specifies base date for the surface.
            `underlying_ticker` specifies identifier for the underlying instrument.
            `discount_curve_id` specifies discount curve ID.
            `beta` specifies sABR Beta parameter.
            `target_expiries` specifies target expiries for calibration.
            `target_strikes` specifies target strikes for calibration.
            `spot_override` specifies optional spot price override.
            `dividend_yield_override` specifies optional dividend yield override.
            `expiry_extrapolation` specifies extrapolation policy for total-variance fill across expiries. After each quoted expiry is calibrated, the published expiry×strike grid is filled by interpolating total variance `w = σ²T` in expiry (not by interpolating SABR α/ν/ρ and evaluating Hagan at the target `T`). This policy controls targets that fall outside the calibrated expiry range: `Error` rejects them; `Clamp` holds the nearest slice's total variance flat.

        Raises
        ------
        ValueError
            If a field is unknown, missing, or has an invalid type or wire value.
        """
        ...

    def to_dict(self) -> _VolSurfaceParamsFields:
        """Export an independent snapshot of the native parameters.

        Returns
        -------
        _VolSurfaceParamsFields
            Effective parameter values as a plain mapping, including defaults.

        Raises
        ------
        ValueError
            If a native parameter cannot be exported.
        """
        ...

class _SwaptionVolParamsFields(TypedDict):
    """Named inputs validated by the canonical swaption vol calibration schema."""

    vol_surface_id: str
    base_date: date | str
    discount_curve_id: str
    forward_id: NotRequired[str | None]
    currency: Currency | str
    vol_convention: NotRequired[str]
    sabr_beta: NotRequired[float]
    target_expiries: NotRequired[Sequence[float]]
    target_tenors: NotRequired[Sequence[float]]
    calendar_id: NotRequired[str | None]
    fixed_day_count: NotRequired[DayCount | str | None]
    swap_index: NotRequired[str | None]
    vol_tolerance: NotRequired[float | None]
    sabr_extrapolation: NotRequired[str]
    allow_sabr_missing_bucket_fallback: NotRequired[bool]

class SwaptionVolParams:
    """Immutable parameters for swaption vol calibration.

    Examples
    --------
    >>> from finstack_quant.calibration import SwaptionVolParams
    >>> params = SwaptionVolParams(
    ...     vol_surface_id="SWPT",
    ...     base_date="2026-05-08",
    ...     currency="USD",
    ...     discount_curve_id="USD-OIS",
    ... )
    >>> params.to_dict()["vol_surface_id"]
    'SWPT'
    """

    def __init__(self, **fields: Unpack[_SwaptionVolParamsFields]) -> None:
        """Read typed fields and supply the native defaults for omitted inputs.

        Parameters
        ----------
        fields : _SwaptionVolParamsFields
            `vol_surface_id` specifies identifier for the volatility surface.
            `base_date` specifies base date for the calibration.
            `discount_curve_id` specifies discount curve identifier for pricing.
            `forward_id` specifies optional forward curve identifier (if different from discount curve).
            `currency` specifies currency for the swaption surface.
            `vol_convention` specifies volatility quoting convention (normal or lognormal).
            `sabr_beta` specifies sABR beta parameter (typically 0.0 for normal, 1.0 for lognormal).
            `target_expiries` specifies target option expiry times in ACT/365F years from `base_date`, independent of the fixed-leg coupon day-count convention.
            `target_tenors` specifies target tenor times (in years) for the surface grid.
            `calendar_id` specifies optional calendar identifier for date adjustments.
            `fixed_day_count` specifies optional day count convention for fixed-leg coupon accrual only. Option expiry and variance always use ACT/365F.
            `swap_index` specifies optional floating index identifier used to resolve market swap conventions. Swaption forward/par rate calculations require swap schedule conventions (fixed frequency, day count, calendar, BDC) that are now indexed off a rate index conventions registry. If omitted, individual swaption quotes must provide `float_leg_conventions.index`.
            `vol_tolerance` specifies maximum absolute error of any fitted volatility quote; defaults to 0.0015. This is distinct from `plan.settings.tolerance` (solver tolerance). For swaption-vol calibration, success should reflect whether the fitted smile residuals are within a market-appropriate tolerance (e.g., 10–20 normal vol bp), not machine epsilon. Normal quotes use decimal rate per square-root year; Black quotes use dimensionless annual volatility. Errors are never scaled by vega.
            `sabr_extrapolation` specifies extrapolation policy used when interpolating SABR parameters across the expiry–tenor grid for target points that do not have a directly calibrated bucket.
            `allow_sabr_missing_bucket_fallback` specifies allow deterministic fallbacks when SABR grid corners are missing during interpolation. If `false` (default), missing corner buckets are treated as an error instead of silently substituting a nearby bucket.

        Raises
        ------
        ValueError
            If a field is unknown, missing, or has an invalid type or wire value.
        """
        ...

    def to_dict(self) -> _SwaptionVolParamsFields:
        """Export an independent snapshot of the native parameters.

        Returns
        -------
        _SwaptionVolParamsFields
            Effective parameter values as a plain mapping, including defaults.

        Raises
        ------
        ValueError
            If a native parameter cannot be exported.
        """
        ...

class _BaseCorrelationParamsFields(TypedDict):
    """Named inputs validated by the canonical base correlation calibration schema."""

    index_id: str
    series: int
    maturity_years: float
    base_date: date | str
    discount_curve_id: str
    currency: Currency | str
    notional: NotRequired[float]
    frequency: NotRequired[Tenor | dict[str, object] | None]
    day_count: NotRequired[DayCount | str | None]
    business_day_convention: NotRequired[str | None]
    calendar_id: NotRequired[str | None]
    detachment_points: NotRequired[Sequence[float]]
    date_regime: NotRequired[str]

class BaseCorrelationParams:
    """Immutable parameters for base correlation calibration.

    Examples
    --------
    >>> from finstack_quant.calibration import BaseCorrelationParams
    >>> params = BaseCorrelationParams(
    ...     index_id="CDX.NA.IG",
    ...     series=42,
    ...     maturity_years=5.0,
    ...     base_date="2026-05-08",
    ...     currency="USD",
    ...     discount_curve_id="USD-OIS",
    ...     date_regime="bespoke",
    ... )
    >>> params.to_dict()["index_id"]
    'CDX.NA.IG'
    """

    def __init__(self, **fields: Unpack[_BaseCorrelationParamsFields]) -> None:
        """Read typed fields and supply the native defaults for omitted inputs.

        Parameters
        ----------
        fields : _BaseCorrelationParamsFields
            `index_id` specifies credit index identifier (e.g., CDX, iTraxx).
            `series` specifies series number of the credit index.
            `maturity_years` specifies finite positive CDS tenor in years, representable as a whole number of months. Quotes must resolve to the same CDS convention maturity as this tenor.
            `base_date` specifies base date for the calibration.
            `discount_curve_id` specifies discount curve identifier for pricing.
            `currency` specifies currency used for synthetic tranche pricing.
            `notional` specifies notional used to price synthetic tranches during calibration. Calibration can be expressed in upfront % terms, so this is typically left as unit-notional unless you have a specific reason to change it.
            `frequency` specifies payment frequency for synthetic tranches (e.g., quarterly).
            `day_count` specifies day count convention for synthetic tranche premium accrual.
            `business_day_convention` specifies business day convention for synthetic tranche schedule adjustments.
            `calendar_id` specifies optional calendar identifier for schedule generation and date adjustments.
            `detachment_points` specifies expected tranche detachment points as decimals in `(0, 1]` (e.g. `0.03` for 3%), the same unit as `CdsTrancheQuote.detachment`. When non-empty, every quote must match one point and every point must be quoted.
            `date_regime` specifies contractual date regime of the synthetic tranches (record CALIBRATION-FRAMEWORK-007, O7): `standard` CDS roll dates (20th of Mar/Jun/Sep/Dec) or `bespoke` for a schedule from the convention frequency and stub. Required; there is no default regime.

        Raises
        ------
        ValueError
            If a field is unknown, missing, or has an invalid type or wire value.
        """
        ...

    def to_dict(self) -> _BaseCorrelationParamsFields:
        """Export an independent snapshot of the native parameters.

        Returns
        -------
        _BaseCorrelationParamsFields
            Effective parameter values as a plain mapping, including defaults.

        Raises
        ------
        ValueError
            If a native parameter cannot be exported.
        """
        ...

class _StudentTParamsFields(TypedDict):
    """Named inputs validated by the canonical student t calibration schema."""

    tranche_instrument_id: str
    base_correlation_curve_id: str
    discount_curve_id: NotRequired[str | None]
    initial_df: NotRequired[float]
    df_bounds: NotRequired[tuple[float, float]]
    correlation: NotRequired[float]

class StudentTParams:
    """Immutable parameters for student t calibration.

    Examples
    --------
    >>> from finstack_quant.calibration import StudentTParams
    >>> params = StudentTParams(
    ...     tranche_instrument_id="TRANCHE-1",
    ...     base_correlation_curve_id="CDX.NA.IG_CORR",
    ... )
    >>> params.to_dict()["tranche_instrument_id"]
    'TRANCHE-1'
    """

    def __init__(self, **fields: Unpack[_StudentTParamsFields]) -> None:
        """Read typed fields and supply the native defaults for omitted inputs.

        Parameters
        ----------
        fields : _StudentTParamsFields
            `tranche_instrument_id` specifies identifier for the reference tranche instrument.
            `base_correlation_curve_id` specifies identifier for the pre-calibrated base correlation curve.
            `discount_curve_id` specifies discount curve identifier used to price the tranche. When omitted, calibration falls back to the only discount curve present in the market context as a convenience default.
            `initial_df` specifies starting guess for degrees of freedom (typically 4-10).
            `df_bounds` specifies feasible domain for `df` as `(lower_bound, upper_bound)`. `df` must be > 2 for finite variance. Typical range: `(2.1, 50.0)`.
            `correlation` specifies market-implied flat correlation for the tranche.

        Raises
        ------
        ValueError
            If a field is unknown, missing, or has an invalid type or wire value.
        """
        ...

    def to_dict(self) -> _StudentTParamsFields:
        """Export an independent snapshot of the native parameters.

        Returns
        -------
        _StudentTParamsFields
            Effective parameter values as a plain mapping, including defaults.

        Raises
        ------
        ValueError
            If a native parameter cannot be exported.
        """
        ...

class _HullWhiteStepParamsFields(TypedDict):
    """Named inputs validated by the canonical hull white step calibration schema."""

    fit_tolerance: float
    curve_id: str
    currency: Currency | str
    base_date: date | str
    initial_kappa: NotRequired[float | None]
    initial_sigma: NotRequired[float | None]

class HullWhiteStepParams:
    """Immutable parameters for hull white step calibration.

    Examples
    --------
    >>> from finstack_quant.calibration import HullWhiteStepParams
    >>> params = HullWhiteStepParams(
    ...     curve_id="USD-OIS",
    ...     currency="USD",
    ...     base_date="2026-05-08",
    ...     fit_tolerance=0.0001,
    ... )
    >>> params.to_dict()["curve_id"]
    'USD-OIS'
    """

    def __init__(self, **fields: Unpack[_HullWhiteStepParamsFields]) -> None:
        """Read typed fields and supply the native defaults for omitted inputs.

        Parameters
        ----------
        fields : _HullWhiteStepParamsFields
            `fit_tolerance` specifies required positive maximum implied-quote error in quoted volatility units. Normal quotes use decimal rate volatility; Black quotes use relative volatility. This acceptance budget is independent of the numerical solver tolerance.
            `curve_id` specifies discount curve ID (must already exist in market context).
            `currency` specifies currency for conventions.
            `base_date` specifies base date for the calibration.
            `initial_kappa` specifies optional initial guess for mean reversion κ.
            `initial_sigma` specifies optional initial guess for short rate vol σ.

        Raises
        ------
        ValueError
            If a field is unknown, missing, or has an invalid type or wire value.
        """
        ...

    def to_dict(self) -> _HullWhiteStepParamsFields:
        """Export an independent snapshot of the native parameters.

        Returns
        -------
        _HullWhiteStepParamsFields
            Effective parameter values as a plain mapping, including defaults.

        Raises
        ------
        ValueError
            If a native parameter cannot be exported.
        """
        ...

class _CapFloorHullWhiteStepParamsFields(TypedDict):
    """Named inputs validated by the canonical cap floor hull white step calibration schema."""

    fit_tolerance: float
    discount_curve_id: str
    forward_curve_id: str
    currency: Currency | str
    base_date: date | str
    fixed_kappa: NotRequired[float | None]
    initial_kappa: NotRequired[float | None]
    initial_sigma: NotRequired[float | None]
    index_id: str
    volatility_mode: NotRequired[str]

class CapFloorHullWhiteStepParams:
    """Immutable parameters for cap floor hull white step calibration.

    Examples
    --------
    >>> from finstack_quant.calibration import CapFloorHullWhiteStepParams
    >>> params = CapFloorHullWhiteStepParams(
    ...     discount_curve_id="USD-OIS",
    ...     forward_curve_id="USD-SOFR-3M",
    ...     currency="USD",
    ...     base_date="2026-05-08",
    ...     index_id="USD-SOFR-3M",
    ...     fit_tolerance=0.0001,
    ... )
    >>> params.to_dict()["discount_curve_id"]
    'USD-OIS'
    """

    def __init__(self, **fields: Unpack[_CapFloorHullWhiteStepParamsFields]) -> None:
        """Read typed fields and supply the native defaults for omitted inputs.

        Parameters
        ----------
        fields : _CapFloorHullWhiteStepParamsFields
            `fit_tolerance` specifies required positive maximum implied-quote error in quoted volatility units. Normal quotes use decimal rate volatility; Black quotes use relative volatility. This acceptance budget is independent of the numerical solver tolerance.
            `discount_curve_id` specifies discount curve ID (must already exist in market context).
            `forward_curve_id` specifies forward/projection curve ID. If equal to `discount_curve_id`, the discount curve is used as the single-curve projection proxy.
            `currency` specifies currency for conventions.
            `base_date` specifies base date for the calibration.
            `fixed_kappa` specifies optional source mean reversion κ. Required for one-quote calibration.
            `initial_kappa` specifies optional initial guess for mean reversion κ when solving both κ and σ.
            `initial_sigma` specifies optional initial guess for short-rate volatility σ when solving both κ and σ.
            `index_id` specifies term-rate index defining settlement, reset tenor, accrual day count, calendar, business-day adjustments, and payment lag for caplets. Its currency must match `currency`; overnight indices are unsupported.
            `volatility_mode` specifies scalar or expiry-bootstraped piecewise short-rate volatility calibration.

        Raises
        ------
        ValueError
            If a field is unknown, missing, or has an invalid type or wire value.
        """
        ...

    def to_dict(self) -> _CapFloorHullWhiteStepParamsFields:
        """Export an independent snapshot of the native parameters.

        Returns
        -------
        _CapFloorHullWhiteStepParamsFields
            Effective parameter values as a plain mapping, including defaults.

        Raises
        ------
        ValueError
            If a native parameter cannot be exported.
        """
        ...

class _SviSurfaceParamsFields(TypedDict):
    """Named inputs validated by the canonical svi surface calibration schema."""

    vol_surface_id: str
    base_date: date | str
    underlying_ticker: str
    discount_curve_id: NotRequired[str | None]
    target_expiries: NotRequired[Sequence[float]]
    target_strikes: NotRequired[Sequence[float]]
    spot_override: NotRequired[float | None]
    dividend_yield_override: NotRequired[float | None]

class SviSurfaceParams:
    """Immutable parameters for svi surface calibration.

    Examples
    --------
    >>> from finstack_quant.calibration import SviSurfaceParams
    >>> params = SviSurfaceParams(
    ...     vol_surface_id="AAPL-SVI",
    ...     base_date="2026-05-08",
    ...     underlying_ticker="AAPL",
    ... )
    >>> params.to_dict()["vol_surface_id"]
    'AAPL-SVI'
    """

    def __init__(self, **fields: Unpack[_SviSurfaceParamsFields]) -> None:
        """Read typed fields and supply the native defaults for omitted inputs.

        Parameters
        ----------
        fields : _SviSurfaceParamsFields
            `vol_surface_id` specifies identifier for the volatility surface being built.
            `base_date` specifies base date for the surface.
            `underlying_ticker` specifies underlying instrument ticker.
            `discount_curve_id` specifies discount curve ID (optional).
            `target_expiries` specifies target expiries for calibration.
            `target_strikes` specifies target strikes for calibration.
            `spot_override` specifies optional spot price override.
            `dividend_yield_override` specifies optional continuous dividend yield in decimal units. Without an override, uses the unitless market scalar `"<underlying_ticker>-DIVYIELD"`. When a discount curve is supplied, a matching cash-dividend schedule may supply carry instead; otherwise an explicit yield is required.

        Raises
        ------
        ValueError
            If a field is unknown, missing, or has an invalid type or wire value.
        """
        ...

    def to_dict(self) -> _SviSurfaceParamsFields:
        """Export an independent snapshot of the native parameters.

        Returns
        -------
        _SviSurfaceParamsFields
            Effective parameter values as a plain mapping, including defaults.

        Raises
        ------
        ValueError
            If a native parameter cannot be exported.
        """
        ...

class _XccyBasisParamsFields(TypedDict):
    """Named inputs validated by the canonical xccy basis calibration schema."""

    curve_id: str
    currency: Currency | str
    base_date: date | str
    fx_spot: float
    domestic_discount_id: str
    method: NotRequired[str | dict[str, object]]
    interpolation: NotRequired[str]
    extrapolation: NotRequired[str]
    conventions: NotRequired[dict[str, object]]

class XccyBasisParams:
    """Immutable parameters for xccy basis calibration.

    Examples
    --------
    >>> from finstack_quant.calibration import XccyBasisParams
    >>> params = XccyBasisParams(
    ...     curve_id="EUR-XCCY",
    ...     currency="EUR",
    ...     base_date="2026-05-08",
    ...     fx_spot=1.1,
    ...     domestic_discount_id="USD-OIS",
    ... )
    >>> params.to_dict()["curve_id"]
    'EUR-XCCY'
    """

    def __init__(self, **fields: Unpack[_XccyBasisParamsFields]) -> None:
        """Read typed fields and supply the native defaults for omitted inputs.

        Parameters
        ----------
        fields : _XccyBasisParamsFields
            `curve_id` specifies identifier for the foreign discount curve being built.
            `currency` specifies foreign currency being calibrated.
            `base_date` specifies base date for the curve.
            `fx_spot` specifies t+0 cash FX rate (domestic per foreign), used when a quote omits `spot_fx`. Covered-interest parity from today needs the cash FX, not the screen "spot". Market spot is T+2 for most G10 pairs and T+1 for USD/CAD; convert to T+0 using ON/TN points before passing the rate here. Mixing T+2 screen spot with T+0 discounting biases long-tenor basis by roughly 1–2 bp.
            `domestic_discount_id` specifies identifier for the pre-calibrated domestic discount curve.
            `method` specifies sequential bootstrap method. `GlobalSolve` is unsupported and rejected.
            `interpolation` specifies interpolation style for the constructed foreign discount curve. Defaults to log-linear discount factors. `Linear` remains available only when explicitly requested.
            `extrapolation` specifies extrapolation policy for the foreign curve.
            `conventions` specifies step-level conventions for pricing and curve time axis.

        Raises
        ------
        ValueError
            If a field is unknown, missing, or has an invalid type or wire value.
        """
        ...

    def to_dict(self) -> _XccyBasisParamsFields:
        """Export an independent snapshot of the native parameters.

        Returns
        -------
        _XccyBasisParamsFields
            Effective parameter values as a plain mapping, including defaults.

        Raises
        ------
        ValueError
            If a native parameter cannot be exported.
        """
        ...

class _ParametricCurveParamsFields(TypedDict):
    """Named inputs validated by the canonical parametric curve calibration schema."""

    curve_id: str
    base_date: date | str
    model: str
    initial_params: NotRequired[dict[str, object] | None]

class ParametricCurveParams:
    """Immutable parameters for parametric curve calibration.

    Examples
    --------
    >>> from finstack_quant.calibration import ParametricCurveParams
    >>> params = ParametricCurveParams(
    ...     curve_id="USD-NS",
    ...     base_date="2026-05-08",
    ...     model="ns",
    ... )
    >>> params.to_dict()["curve_id"]
    'USD-NS'
    """

    def __init__(self, **fields: Unpack[_ParametricCurveParamsFields]) -> None:
        """Read typed fields and supply the native defaults for omitted inputs.

        Parameters
        ----------
        fields : _ParametricCurveParamsFields
            `curve_id` specifies identifier for the single discount curve being fitted. All calibration instruments use this curve for discounting and implied projection.
            `base_date` specifies base date for the curve.
            `model` specifies nelson-Siegel variant (NS or NSS).
            `initial_params` specifies optional initial parameter guesses.

        Raises
        ------
        ValueError
            If a field is unknown, missing, or has an invalid type or wire value.
        """
        ...

    def to_dict(self) -> _ParametricCurveParamsFields:
        """Export an independent snapshot of the native parameters.

        Returns
        -------
        _ParametricCurveParamsFields
            Effective parameter values as a plain mapping, including defaults.

        Raises
        ------
        ValueError
            If a native parameter cannot be exported.
        """
        ...

StepParams: TypeAlias = (
    DiscountCurveParams
    | ForwardCurveParams
    | HazardCurveParams
    | InflationCurveParams
    | VolSurfaceParams
    | SwaptionVolParams
    | BaseCorrelationParams
    | StudentTParams
    | HullWhiteStepParams
    | CapFloorHullWhiteStepParams
    | SviSurfaceParams
    | XccyBasisParams
    | ParametricCurveParams
)
