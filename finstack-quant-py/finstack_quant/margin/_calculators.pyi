"""Stub declarations mirroring ``finstack-quant-py/src/bindings/margin/calculators.rs``."""

from __future__ import annotations

import datetime
from typing import Any
import pandas as pd
from finstack_quant.margin._types import CsaSpec, ImMethodology

class VmResult:
    """
    Variation margin calculation result.

    Sign convention: ``gross_exposure`` is the signed mark-to-market from our
    side (positive = the counterparty owes us). ``post_amount`` is what we
    post and ``collect_amount`` what we receive back; at most one is non-zero.
    Amounts are floats in the CSA base currency (``currency``).

    Parameters
    ----------
    (Returned by ``VmCalculator.calculate``; also loadable via ``from_json``.)

    Returns
    -------
    VmResult
        VM amounts and call flag.

    Examples
    --------
    >>> r = VmCalculator(CsaSpec.usd_regulatory()).calculate(1e6, 0.0, "USD", "2024-06-15")
    >>> isinstance(r.net_margin, float)
    True
    """

    @staticmethod
    def from_json(json: str) -> VmResult:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            Canonical JSON for a ``VmResult``.

        Returns
        -------
        VmResult
            Parsed value.

        Raises
        ------
        ValueError
            If the payload is malformed.

        Examples
        --------
        >>> original = VmCalculator(CsaSpec.usd_regulatory()).calculate(1e6, 0.0, "USD", "2024-06-15")
        >>> VmResult.from_json(original.to_json()).to_json() == original.to_json()
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON accepted by ``from_json``.

        Returns
        -------
        str
            JSON string.

        Raises
        ------
        ValueError
            If serialization fails.

        Examples
        --------
        >>> isinstance(VmCalculator(CsaSpec.usd_regulatory()).calculate(1e6, 0.0, "USD", "2024-06-15").to_json(), str)
        True
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Support ``pickle`` via the ``to_json`` / ``from_json`` round-trip."""
        ...

    @property
    def date(self) -> datetime.date:
        """
        Calculation date.

        Returns
        -------
        datetime.date
            The ``as_of`` passed to ``VmCalculator.calculate``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> r = VmCalculator(CsaSpec.usd_regulatory()).calculate(1e6, 0.0, "USD", "2024-06-17")
        >>> r.date
        datetime.date(2024, 6, 17)
        """
        ...

    @property
    def settlement_date(self) -> datetime.date:
        """
        Settlement date of the margin transfer: the calculation date plus the
        CSA settlement lag, adjusted on the CSA calendar.

        Returns
        -------
        datetime.date
            Settlement date, strictly after ``date`` for a T+1 CSA.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> r = VmCalculator(CsaSpec.usd_regulatory()).calculate(1e6, 0.0, "USD", "2024-06-17")
        >>> r.settlement_date
        datetime.date(2024, 6, 18)
        """
        ...

    @property
    def currency(self) -> str:
        """
        CSA base currency of every amount.

        Returns
        -------
        str
            ISO-4217 code.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> VmCalculator(CsaSpec.usd_regulatory()).calculate(1e6, 0.0, "USD", "2024-06-15").currency
        'USD'
        """
        ...

    @property
    def gross_exposure(self) -> float:
        """
        Gross mark-to-market exposure (positive = counterparty owes us).

        Returns
        -------
        float
            Gross exposure.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> r = VmCalculator(CsaSpec.usd_regulatory()).calculate(1e6, 0.0, "USD", "2024-06-15")
        >>> r.gross_exposure >= 0
        True
        """
        ...

    @property
    def net_exposure(self) -> float:
        """
        Net exposure after threshold and independent amount.

        Returns
        -------
        float
            Net exposure.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> r = VmCalculator(CsaSpec.usd_regulatory()).calculate(1e6, 0.0, "USD", "2024-06-15")
        >>> isinstance(r.net_exposure, float)
        True
        """
        ...

    @property
    def post_amount(self) -> float:
        """
        Amount paid by the desk, including new postings and collateral returns.

        Returns
        -------
        float
            Delivery amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> r = VmCalculator(CsaSpec.usd_regulatory()).calculate(1e6, 0.0, "USD", "2024-06-15")
        >>> r.post_amount >= 0
        True
        """
        ...

    @property
    def collect_amount(self) -> float:
        """
        Amount received by the desk, including new collections and collateral returns.

        Returns
        -------
        float
            Return amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> r = VmCalculator(CsaSpec.usd_regulatory()).calculate(1e6, 0.0, "USD", "2024-06-15")
        >>> r.collect_amount >= 0
        True
        """
        ...

    @property
    def net_margin(self) -> float:
        """
        Net desk cash outflow (post minus collect).

        Returns
        -------
        float
            Net margin.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> r = VmCalculator(CsaSpec.usd_regulatory()).calculate(1e6, 0.0, "USD", "2024-06-15")
        >>> isinstance(r.net_margin, float)
        True
        """
        ...

    @property
    def requires_call(self) -> bool:
        """
        Whether a margin call is required.

        Returns
        -------
        bool
            Call required flag.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> r = VmCalculator(CsaSpec.usd_regulatory()).calculate(1e6, 0.0, "USD", "2024-06-15")
        >>> isinstance(r.requires_call, bool)
        True
        """
        ...

    @property
    def threshold(self) -> float:
        """
        CSA threshold applied symmetrically to ``abs(gross_exposure)``.

        Returns
        -------
        float
            Non-negative threshold in the CSA base currency. Exposure inside
            the threshold needs no collateral.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> r = VmCalculator(CsaSpec.usd_regulatory()).calculate(1e6, 0.0, "USD", "2024-06-15")
        >>> r.threshold >= 0
        True
        """
        ...

    @property
    def independent_amount(self) -> float:
        """
        CSA independent amount added to the threshold-adjusted exposure.

        Returns
        -------
        float
            Independent amount in the CSA base currency;
            ``net_exposure`` is the threshold-adjusted exposure plus this.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> r = VmCalculator(CsaSpec.usd_regulatory()).calculate(1e6, 0.0, "USD", "2024-06-15")
        >>> r.independent_amount >= 0
        True
        """
        ...

    @property
    def collateral_balance(self) -> float:
        """
        Signed collateral balance netted against ``net_exposure``.

        Returns
        -------
        float
            The balance passed to the calculator, in the CSA base currency:
            positive when we hold collateral, negative when we have posted it.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> r = VmCalculator(CsaSpec.usd_regulatory()).calculate(1e6, 0.0, "USD", "2024-06-15")
        >>> r.collateral_balance
        0.0
        """
        ...

    @property
    def unrounded_call(self) -> float:
        """
        Signed call before the minimum-transfer test and rounding.

        Returns
        -------
        float
            ``net_exposure - collateral_balance`` in the CSA base currency;
            positive means collect, negative means we pay (post or return).

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> r = VmCalculator(CsaSpec.usd_regulatory()).calculate(1e6, 0.0, "USD", "2024-06-15")
        >>> r.unrounded_call == r.net_exposure - r.collateral_balance
        True
        """
        ...

    @property
    def mta(self) -> float:
        """
        CSA minimum transfer amount.

        Returns
        -------
        float
            Minimum transfer amount in the CSA base currency. When
            ``abs(unrounded_call) < mta`` no transfer is made.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> r = VmCalculator(CsaSpec.usd_regulatory()).calculate(1e6, 0.0, "USD", "2024-06-15")
        >>> r.mta >= 0
        True
        """
        ...

    @property
    def rounding_increment(self) -> float:
        """
        CSA rounding increment applied to the transfer.

        Returns
        -------
        float
            Increment in the CSA base currency. A transfer that passes the MTA
            test is rounded to a multiple of it: up for a delivery, down for
            a return. Zero disables rounding.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> r = VmCalculator(CsaSpec.usd_regulatory()).calculate(1e6, 0.0, "USD", "2024-06-15")
        >>> r.rounding_increment >= 0
        True
        """
        ...

    def __repr__(self) -> str: ...
    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the result as a single-row pandas ``DataFrame``.

        Columns: ``date``, ``settlement_date`` (ISO 8601 strings),
        ``gross_exposure``, ``threshold``, ``independent_amount``,
        ``net_exposure``, ``collateral_balance``, ``unrounded_call``, ``mta``,
        ``rounding_increment``, ``post_amount``, ``collect_amount``,
        ``net_margin``, ``requires_call``, ``currency``.

        All amount columns are floats in the single CSA currency reported by
        ``currency``; positive ``post_amount`` means we post margin and
        positive ``collect_amount`` means cash received by the desk. The
        columns from ``gross_exposure`` to ``rounding_increment`` are in
        calculation order: threshold and independent amount turn the gross
        exposure into ``net_exposure``, subtracting ``collateral_balance``
        gives ``unrounded_call``, and the MTA test and rounding give the
        transfer.

        Returns
        -------
        pd.DataFrame
            One row describing the variation-margin result.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

class VmCalculator:
    """
    Variation margin calculator following ISDA CSA rules.

    Parameters
    ----------
    csa : CsaSpec
        Credit Support Annex specification.

    Returns
    -------
    VmCalculator
        Calculator bound to ``csa``.

    Examples
    --------
    >>> calc = VmCalculator(CsaSpec.usd_regulatory())
    >>> out = calc.calculate(1e6, 0.0, "USD", "2024-06-15")
    >>> isinstance(out, VmResult)
    True
    """

    def __init__(self, csa: CsaSpec) -> None:
        """
        Bind a variation-margin calculator to one CSA specification.

        Parameters
        ----------
        csa : CsaSpec
            Thresholds, transfer minimums, rounding rules, eligible
            currencies, and calendar terms applied to each margin call.

        Notes
        -----
        Construction does not raise; arguments are stored as supplied.
        """
        ...

    @property
    def csa(self) -> CsaSpec:
        """
        CSA specification this calculator applies.

        Returns
        -------
        CsaSpec
            The spec passed to the constructor.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> VmCalculator(CsaSpec.usd_regulatory()).csa.base_currency
        'USD'
        """
        ...

    def calculate(
        self,
        exposure: float,
        posted_collateral: float,
        currency: str,
        as_of: datetime.date | str,
    ) -> VmResult:
        """
        Calculate variation margin.

        Parameters
        ----------
        exposure : float
            Signed mark-to-market in ``currency``: positive means the
            counterparty owes us, negative means we owe them.
        posted_collateral : float
            Signed collateral balance in ``currency``: positive held, negative
            posted, including pending agreed calls.
        currency : str
            ISO currency code; must equal the CSA base currency.
        as_of : datetime.date | str
            Calculation date as ``datetime.date``, ``datetime.datetime``,
            ``pandas.Timestamp`` or ISO ``YYYY-MM-DD`` string; the
            settlement date is derived from it on the CSA calendar.

        Returns
        -------
        VmResult
            VM breakdown with delivery/return amounts and settlement date.

        Raises
        ------
        ValueError
            If the currency is unknown or differs from the CSA base
            currency, an amount is non-finite, or a date string is not ISO
            8601.
        TypeError
            If ``as_of`` is neither a string nor date-like.

        Examples
        --------
        >>> VmCalculator(CsaSpec.usd_regulatory()).calculate(1e6, 0.0, "USD", "2024-06-17")
        VmResult(date=2024-06-17, post=0.00, collect=1000000.00, requires_call=True, settlement_date=2024-06-18)
        """
        ...

    def generate_margin_calls(
        self,
        exposures: list[tuple[datetime.date | str, float]] | pd.Series,
        initial_collateral: float,
    ) -> pd.DataFrame:
        """
        Run an exposure time series into a margin-call schedule.

        Parameters
        ----------
        exposures : list[tuple[datetime.date | str, float]] | pd.Series
            Dated signed exposures in the CSA base currency (positive = the
            counterparty owes us), processed in the order given. A ``Series``
            contributes its index as the dates.
        initial_collateral : float
            Collateral posted before the first date, in the CSA base currency.

        Returns
        -------
        pd.DataFrame
            One row per call: ``call_date``, ``settlement_date`` (ISO 8601
            strings), ``call_type`` (``"variation_margin_post"`` or
            ``"variation_margin_collect"``), ``amount``, ``mtm_trigger``,
            ``threshold``, ``mta_applied`` (floats in ``currency``) and
            ``currency``. Dates without a call produce no row.

        Raises
        ------
        ValueError
            If an amount is non-finite or a date string is not ISO 8601.

        Examples
        --------
        >>> calc = VmCalculator(CsaSpec.usd_regulatory())
        >>> calls = calc.generate_margin_calls([("2024-06-17", 1e6), ("2024-06-18", 4e5)], 0.0)
        >>> calls["call_type"].tolist()
        ['variation_margin_collect', 'variation_margin_post']
        """
        ...

    def margin_call_dates(self, start: datetime.date | str, end: datetime.date | str) -> list[datetime.date]:
        """
        Contractual margin-call dates between ``start`` and ``end``.

        Follows the CSA VM frequency on the CSA calendar: daily lists every
        business day, weekly/monthly roll from ``start`` with each date
        adjusted forward, on-demand returns just the adjusted endpoints.

        Parameters
        ----------
        start : datetime.date | str
            First date of the window (inclusive).
        end : datetime.date | str
            Last date of the window (inclusive).

        Returns
        -------
        list[datetime.date]
            Call dates in ascending order.

        Raises
        ------
        ValueError
            If a date string is not ISO 8601 or the CSA calendar is not
            registered.

        Examples
        --------
        >>> calc = VmCalculator(CsaSpec.usd_regulatory())
        >>> calc.margin_call_dates("2024-06-14", "2024-06-17")
        [datetime.date(2024, 6, 14), datetime.date(2024, 6, 17)]
        """
        ...

    def __repr__(self) -> str: ...

class ImResult:
    """
    Initial margin calculation result.

    ``amount`` is a float in ``currency``. ``breakdown_keys`` are
    methodology-specific component labels: SIMM publishes ``IR_Delta``,
    ``IR_Vega``, ``Credit_Qualifying_Delta``, ``Credit_Qualifying_Vega``,
    ``Credit_NonQualifying_Delta``, ``Credit_NonQualifying_Vega``,
    ``Equity_Delta``, ``Equity_Vega``, ``FX_Delta``, ``FX_Vega``,
    ``Commodity_Delta``, ``Commodity_Vega`` and ``Curvature``; the schedule
    calculator publishes the normalised asset class (e.g. ``interest_rate``,
    or ``interest_rate_ngr`` for the NGR path); the haircut calculator the
    collateral asset class.

    Parameters
    ----------
    (Produced by the IM calculators; also loadable via ``from_json``.)

    Returns
    -------
    ImResult
        IM amount and metadata.

    Examples
    --------
    >>> calc = ScheduleImCalculator.bcbs_standard()
    >>> result = calc.calculate_for_notional(1_000_000, "USD", "interest_rate", 5.0, "2025-01-15")
    >>> (result.amount, result.breakdown_keys())
    (40000.0, ['interest_rate'])
    """

    @staticmethod
    def from_json(json: str) -> ImResult:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            Canonical JSON for a ``ImResult``.

        Returns
        -------
        ImResult
            Parsed value.

        Raises
        ------
        ValueError
            If the payload is malformed.

        Examples
        --------
        >>> original = ScheduleImCalculator.bcbs_standard().calculate_for_notional(
        ...     1_000_000, "USD", "interest_rate", 5.0, "2025-01-15"
        ... )
        >>> ImResult.from_json(original.to_json()).to_json() == original.to_json()
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON accepted by ``from_json``.

        Returns
        -------
        str
            JSON string.

        Raises
        ------
        ValueError
            If serialization fails.

        Examples
        --------
        >>> isinstance(
        ...     ScheduleImCalculator
        ...     .bcbs_standard()
        ...     .calculate_for_notional(1_000_000, "USD", "interest_rate", 5.0, "2025-01-15")
        ...     .to_json(),
        ...     str,
        ... )
        True
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Support ``pickle`` via the ``to_json`` / ``from_json`` round-trip."""
        ...

    @property
    def amount(self) -> float:
        """
        Calculated initial margin amount.

        Returns
        -------
        float
            IM notional.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def currency(self) -> str:
        """
        Currency of the IM amount.

        Returns
        -------
        str
            ISO currency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def methodology(self) -> ImMethodology:
        """
        Methodology used for calculation.

        Returns
        -------
        ImMethodology
            IM methodology.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def mpor_days(self) -> int:
        """
        Margin period of risk in business days.

        Returns
        -------
        int
            MPOR in business days (10 for SIMM/schedule, 2 for haircut IM).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def approximation(self) -> bool:
        """
        True when the amount is a conservative proxy, not an exact methodology result.

        Returns
        -------
        bool
            Whether the amount is an approximation (which need not be conservative) rather
            than an exact computation under the named methodology.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def as_of(self) -> datetime.date:
        """
        Calculation date.

        Returns
        -------
        datetime.date
            The ``as_of`` passed to the calculator.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> calc = ScheduleImCalculator.bcbs_standard()
        >>> calc.calculate_for_notional(1_000_000, "USD", "interest_rate", 5.0, "2025-01-15").as_of
        datetime.date(2025, 1, 15)
        """
        ...

    @property
    def simm_detail(self) -> dict[str, Any] | None:
        """
        SIMM aggregation detail behind ``breakdown_amount`` and ``amount``.

        Returns
        -------
        dict[str, Any] | None
            ``None`` unless the result came from the SIMM calculator. Otherwise
            ``components`` (one per breakdown key, each with ``component``,
            ``risk_class``, ``historical_volatility_ratio``, ``margin`` and
            ``buckets``), ``risk_class_margins`` and ``mpor_scale``. A bucket
            has ``bucket``, ``k``, ``signed_sum`` and
            ``weighted_sensitivities`` rows with ``risk_factor``, ``tenor``,
            ``sensitivity``, ``risk_weight``, ``concentration_factor`` and
            ``weighted_sensitivity``. Margins are in the result currency
            before the MPOR scale: ``margin * mpor_scale`` is the breakdown
            amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> calc = ScheduleImCalculator.bcbs_standard()
        >>> im = calc.calculate_for_notional(1_000_000, "USD", "interest_rate", 5.0, "2025-01-15")
        >>> im.simm_detail is None
        True
        """
        ...

    def breakdown_keys(self) -> list[str]:
        """
        Breakdown component labels present, in canonical sorted order (see
        the class docstring for the SIMM and schedule label sets).

        Returns
        -------
        list[str]
            Keys present in the breakdown map.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """
        ...

    def breakdown_amount(self, key: str) -> float | None:
        """
        Get breakdown amount for a risk class.

        Parameters
        ----------
        key : str
            Component label such as ``"IR_Delta"`` (SIMM) or
            ``"interest_rate"`` (schedule).

        Returns
        -------
        float or None
            Amount in ``currency`` if present.

        Notes
        -----
        This method does not raise; a missing result is ``None`` rather than an exception.
        """
        ...

    def __repr__(self) -> str: ...
    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the headline result as a single-row pandas ``DataFrame``.

        Columns: ``amount``, ``currency``, ``methodology``, ``mpor_days``,
        ``as_of``, ``approximation``.

        ``amount`` is a float in ``currency``; ``mpor_days`` is the margin
        period of risk in business days; ``as_of`` is an ISO 8601 date string.
        ``approximation`` is ``True`` when the amount uses a proxy methodology
        rather than an exact computation under the named methodology - do not
        reconcile an approximated figure against an actual margin call.
        Per-risk-class detail lives in ``to_breakdown_dataframe``.

        Returns
        -------
        pd.DataFrame
            One row describing the initial-margin result.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_breakdown_dataframe(self) -> pd.DataFrame:
        """
        Export the per-component breakdown as a pandas ``DataFrame``.

        Columns: ``risk_class``, ``amount``, ``currency``. One row per
        component label (SIMM: ``IR_Delta``, ``IR_Vega``, ``FX_Delta``,
        ``IR_Curvature``, ``Equity_Curvature``, ...; schedule: the asset class such as
        ``interest_rate``), sorted by ``risk_class`` so repeated runs are
        byte-identical. Methodologies that publish no breakdown yield a
        zero-row frame that still carries all three columns.

        Breakdown components do not generally sum to ``amount``: SIMM and
        other methodologies aggregate risk classes with correlations.

        Returns
        -------
        pd.DataFrame
            One row per risk class, sorted by ``risk_class``.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
