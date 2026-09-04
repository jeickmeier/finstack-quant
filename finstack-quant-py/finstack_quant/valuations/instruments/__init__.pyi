"""
Python bindings for the corresponding finstack-quant Rust API.

Every typed instrument ``from_json`` classmethod accepts either canonical bare
``{"type": ..., "spec": ...}`` JSON or a versioned
``{"schema": "finstack_quant.instrument/1", "instrument": ...}`` envelope.
Inputs larger than 16 MiB raise ``ValueError`` before parsing.

Examples
--------
>>> from finstack_quant.valuations.instruments import list_models, list_models_grouped
>>> models = list_models_grouped()["bond"]
>>> all(model in models for model in ("discounting", "hazard_rate", "tree", "rates_credit"))
True

"""

from __future__ import annotations

import datetime
from typing import Any, Literal

import pandas as pd

from finstack_quant.core.currency import Currency
from finstack_quant.core.dates import BusinessDayConvention, DayCount, StubKind, Tenor
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.money import Money
from finstack_quant.core.types import Attributes, Bps, Rate
from finstack_quant.valuations import ValuationResult
from finstack_quant.valuations.composite import CompositeInstrument
from finstack_quant.models.credit import (
    DynamicRecoverySpec,
    EndogenousHazardSpec,
    MertonModel,
    ToggleExerciseModel,
)

__all__ = [
    "AssetPool",
    "BarrierCrossing",
    "Bond",
    "BondBuilder",
    "CDSIndex",
    "CDSIndexBuilder",
    "CDSIndexConstituent",
    "CDSIndexParams",
    "CDSTranche",
    "CDSTrancheBuilder",
    "CDSTrancheParams",
    "CallPutSchedule",
    "CapFloor",
    "CapFloorBuilder",
    "ConversionSpec",
    "ConvertibleBond",
    "ConvertibleBondBuilder",
    "CreditDefaultSwap",
    "CreditDefaultSwapBuilder",
    "EquityOption",
    "EquityOptionBuilder",
    "FixedLegSpec",
    "FloatLegSpec",
    "FxForward",
    "FxForwardBuilder",
    "FxOption",
    "FxOptionBuilder",
    "InterestRateSwap",
    "InterestRateSwapBuilder",
    "MarketHistory",
    "MertonMcConfig",
    "MertonMcResult",
    "MetricPricingOverrides",
    "OasResult",
    "PathStatistics",
    "PikMode",
    "PikSchedule",
    "PremiumLegSpec",
    "ProtectionLegSpec",
    "RepLine",
    "ScenarioTable",
    "StructuredCredit",
    "StructuredCreditBuilder",
    "Swaption",
    "SwaptionBuilder",
    "TermLoan",
    "TermLoanBuilder",
    "Tranche",
    "TrancheBuilder",
    "TrancheMetrics",
    "TrancheStructure",
    "bond_from_cashflows_json",
    "instrument_cashflows_json",
    "list_models",
    "list_models_grouped",
    "list_standard_metrics",
    "list_standard_metrics_grouped",
    "pretty_instrument_json",
    "price_instrument",
    "structured_credit_tranche_breakeven_cdr",
    "structured_credit_tranche_discount_margin",
    "structured_credit_tranche_metrics",
    "structured_credit_tranche_oas",
    "structured_credit_tranche_scenario_table",
    "validate_instrument_json",
    "validate_typed_instrument_json",
]

class Bond:
    """
    Typed wrapper for the canonical Rust ``Bond`` instrument.

    Construct via :meth:`Bond.fixed` (US-corporate or a named convention
    preset), :meth:`Bond.with_convention`, :meth:`Bond.floating` /
    :meth:`Bond.floating_with_convention`, :meth:`Bond.zero_coupon`, the
    :meth:`Bond.builder` fluent builder (callable schedules, credit curve,
    custom cashflow specs and settlement conventions), the ``Bond.example*``
    presets or :meth:`Bond.from_json`. Every public Rust field is readable
    as a property; :meth:`Bond.price` / :meth:`Bond.metric` run the same
    pricer as :func:`price_instrument`. Instances are accepted directly by
    :func:`price_instrument` and :func:`instrument_cashflows_json`.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import Bond
    >>> bond = Bond.fixed(
    ...     "BOND-1",
    ...     1_000_000.0,
    ...     0.05,
    ...     "2024-01-01",
    ...     "2034-01-01",
    ...     "none",
    ...     "USD-OIS",
    ...     currency="USD",
    ... )
    >>> bond.id
    'BOND-1'
    >>> bond.notional.amount
    1000000.0
    """

    @staticmethod
    def builder() -> BondBuilder:
        """
        Create a fluent builder (mirrors Rust ``Bond::builder()``).

        Returns
        -------
        BondBuilder
            A builder with fluent, consuming setter methods.

        Notes
        -----
        This factory does not raise; it returns an empty builder.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Bond
        >>> builder = Bond.builder()
        >>> builder.id("EXAMPLE") is builder
        True
        """
        ...
    @staticmethod
    def fixed(
        id: str,
        notional: Money | float,
        coupon_rate: float | Rate,
        issue: datetime.date | datetime.datetime | pd.Timestamp | str,
        maturity: datetime.date | datetime.datetime | pd.Timestamp | str,
        stub: StubKind | Literal["none", "short_front", "long_front", "short_back", "long_back"],
        discount_curve_id: str,
        *,
        convention: Literal[
            "us_treasury", "us_agency", "german_bund", "uk_gilt", "french_oat", "jgb", "us_corporate", "eur_corporate"
        ]
        | None = None,
        currency: str | None = None,
    ) -> Bond:
        """
        Create a fixed-rate bond from a settlement/day-count convention preset.

        Mirrors Rust ``Bond::fixed`` when ``convention`` is ``None`` (US corporate:
        semi-annual, 30/360, T+1) and ``Bond::with_convention`` followed by
        ``with_stub`` when a preset is named.

        Parameters
        ----------
        id : str
            Unique instrument identifier.
        notional : Money | float
            Principal amount; a bare number is tagged with ``currency``.
        coupon_rate : float | Rate
            Annual coupon as a decimal (``0.05`` = 5%) or a ``Rate``.
        issue : datetime.date | datetime.datetime | pd.Timestamp | str
            Issue date (ISO 8601 strings accepted).
        maturity : datetime.date | datetime.datetime | pd.Timestamp | str
            Maturity date.
        stub : StubKind | str
            Placement and length policy for an irregular coupon period, as a
            ``StubKind`` or its serde name (``"none"``, ``"short_front"``, ...).
        discount_curve_id : str
            Discount curve identifier used for pricing.
        convention : str, optional
            Bond convention preset controlling coupon frequency, day count,
            calendar, business-day convention and settlement lag. ``None`` is
            ``"us_corporate"``.
        currency : str, optional
            ISO-4217 code applied when ``notional`` is a bare number.

        Returns
        -------
        Bond
            A validated fixed-rate bond.

        Raises
        ------
        ValueError
            If ``convention``/``stub`` is not a recognized name, a bare
            ``notional`` has no ``currency``, or validation fails (e.g. maturity
            not after issue).
        TypeError
            If ``coupon_rate`` or ``notional`` has an unsupported type or a date
            cannot be interpreted.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Bond
        >>> bund = Bond.fixed(
        ...     "BUND",
        ...     1_000_000.0,
        ...     0.025,
        ...     "2024-01-15",
        ...     "2034-01-15",
        ...     "none",
        ...     "EUR-OIS",
        ...     convention="german_bund",
        ...     currency="EUR",
        ... )
        >>> bund.settlement_days
        2
        """
        ...
    @staticmethod
    def with_convention(
        id: str,
        notional: Money | float,
        coupon_rate: float | Rate,
        issue: datetime.date | datetime.datetime | pd.Timestamp | str,
        maturity: datetime.date | datetime.datetime | pd.Timestamp | str,
        convention: Literal[
            "us_treasury", "us_agency", "german_bund", "uk_gilt", "french_oat", "jgb", "us_corporate", "eur_corporate"
        ],
        discount_curve_id: str,
        *,
        currency: str | None = None,
    ) -> Bond:
        """
        Create a fixed-rate bond from a named market convention.

        Mirrors Rust ``Bond::with_convention``; the stub rule is the preset's own
        (use :meth:`Bond.fixed` with ``convention=`` to override it).

        Parameters
        ----------
        id : str
            Unique instrument identifier.
        notional : Money | float
            Principal amount; a bare number is tagged with ``currency``.
        coupon_rate : float | Rate
            Annual coupon as a decimal (``0.05`` = 5%) or a ``Rate``.
        issue : datetime.date | datetime.datetime | pd.Timestamp | str
            Issue date.
        maturity : datetime.date | datetime.datetime | pd.Timestamp | str
            Maturity date.
        convention : str
            Bond convention preset (``"us_treasury"``, ``"us_agency"``,
            ``"german_bund"``, ``"uk_gilt"``, ``"french_oat"``, ``"jgb"``,
            ``"us_corporate"``, ``"eur_corporate"``).
        discount_curve_id : str
            Discount curve identifier used for pricing.
        currency : str, optional
            ISO-4217 code applied when ``notional`` is a bare number.

        Returns
        -------
        Bond
            A validated fixed-rate bond.

        Raises
        ------
        ValueError
            If ``convention`` is unknown, a bare ``notional`` has no ``currency``,
            or validation fails.
        TypeError
            If ``coupon_rate``/``notional`` has an unsupported type or a date
            cannot be interpreted.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Bond
        >>> gilt = Bond.with_convention(
        ...     "GILT",
        ...     1_000_000.0,
        ...     0.04,
        ...     "2024-01-01",
        ...     "2034-01-01",
        ...     "uk_gilt",
        ...     "GBP-OIS",
        ...     currency="GBP",
        ... )
        >>> gilt.settlement_days
        1
        """
        ...
    @staticmethod
    def floating(
        id: str,
        notional: Money | float,
        index_id: str,
        margin_bp: float | Bps,
        issue: datetime.date | datetime.datetime | pd.Timestamp | str,
        maturity: datetime.date | datetime.datetime | pd.Timestamp | str,
        frequency: Tenor,
        day_count: DayCount,
        discount_curve_id: str,
        *,
        currency: str | None = None,
    ) -> Bond:
        """
        Create a floating-rate bond (FRN) linked to a forward index.

        Mirrors Rust ``Bond::floating``. Settlement, calendar, and business-day
        convention come from the notional currency: USD ``us_corporate`` (T+1,
        ``usny``), EUR ``eur_corporate`` (T+2, ``target2``), GBP ``uk_gilt``
        (T+1), JPY ``jgb`` (T+2). Other currencies raise ``ValueError``; use
        :meth:`Bond.floating_with_convention` to name the preset explicitly.

        Parameters
        ----------
        id : str
            Unique instrument identifier.
        notional : Money | float
            Principal amount; a bare number is tagged with ``currency``.
        index_id : str
            Forward curve identifier (e.g. ``"USD-SOFR-3M"``).
        margin_bp : float | Bps
            Spread over the index in whole basis points (fractions are rounded).
        issue : datetime.date | datetime.datetime | pd.Timestamp | str
            Issue date.
        maturity : datetime.date | datetime.datetime | pd.Timestamp | str
            Maturity date.
        frequency : Tenor
            Payment frequency (e.g. ``Tenor.quarterly()``).
        day_count : DayCount
            Day count convention (e.g. ``DayCount.ACT_360``).
        discount_curve_id : str
            Discount curve identifier used for pricing.
        currency : str, optional
            ISO-4217 code applied when ``notional`` is a bare number.

        Returns
        -------
        Bond
            A validated floating-rate note.

        Raises
        ------
        ValueError
            If the notional currency has no mapped settlement convention,
            ``notional`` is not finite and positive, or ``issue`` is not strictly
            before ``maturity``.
        TypeError
            If ``margin_bp``/``notional`` has an unsupported type or a date cannot
            be interpreted.

        Examples
        --------
        >>> from finstack_quant.core.dates import DayCount, Tenor
        >>> from finstack_quant.valuations.instruments import Bond
        >>> frn = Bond.floating(
        ...     "FRN",
        ...     1000.0,
        ...     "USD-SOFR-3M",
        ...     125.0,
        ...     "2024-01-01",
        ...     "2029-01-01",
        ...     Tenor.quarterly(),
        ...     DayCount.ACT_360,
        ...     "USD-OIS",
        ...     currency="USD",
        ... )
        >>> frn.has_floating_coupons
        True
        """
        ...
    @staticmethod
    def floating_with_convention(
        id: str,
        notional: Money | float,
        index_id: str,
        margin_bp: float | Bps,
        issue: datetime.date | datetime.datetime | pd.Timestamp | str,
        maturity: datetime.date | datetime.datetime | pd.Timestamp | str,
        frequency: Tenor,
        day_count: DayCount,
        convention: Literal[
            "us_treasury", "us_agency", "german_bund", "uk_gilt", "french_oat", "jgb", "us_corporate", "eur_corporate"
        ],
        discount_curve_id: str,
        *,
        currency: str | None = None,
    ) -> Bond:
        """
        Create a floating-rate bond with an explicit convention preset.

        Mirrors Rust ``Bond::floating_with_convention``.

        Parameters
        ----------
        id : str
            Unique instrument identifier.
        notional : Money | float
            Principal amount; a bare number is tagged with ``currency``.
        index_id : str
            Forward curve identifier (e.g. ``"EUR-EURIBOR-3M"``).
        margin_bp : float | Bps
            Spread over the index in whole basis points (fractions are rounded).
        issue : datetime.date | datetime.datetime | pd.Timestamp | str
            Issue date.
        maturity : datetime.date | datetime.datetime | pd.Timestamp | str
            Maturity date.
        frequency : Tenor
            Payment frequency.
        day_count : DayCount
            Day count convention.
        convention : str
            Bond convention preset (see :meth:`Bond.with_convention`).
        discount_curve_id : str
            Discount curve identifier used for pricing.
        currency : str, optional
            ISO-4217 code applied when ``notional`` is a bare number.

        Returns
        -------
        Bond
            A validated floating-rate note.

        Raises
        ------
        ValueError
            If ``convention`` is unknown, a bare ``notional`` has no ``currency``,
            or validation fails.
        TypeError
            If ``margin_bp``/``notional`` has an unsupported type or a date cannot
            be interpreted.

        Examples
        --------
        >>> from finstack_quant.core.dates import DayCount, Tenor
        >>> from finstack_quant.valuations.instruments import Bond
        >>> frn = Bond.floating_with_convention(
        ...     "FRN-EUR",
        ...     1000.0,
        ...     "EUR-EURIBOR-3M",
        ...     80.0,
        ...     "2024-01-01",
        ...     "2029-01-01",
        ...     Tenor.quarterly(),
        ...     DayCount.ACT_360,
        ...     "eur_corporate",
        ...     "EUR-OIS",
        ...     currency="EUR",
        ... )
        >>> frn.settlement_days
        2
        """
        ...
    @staticmethod
    def zero_coupon(
        id: str,
        notional: Money | float,
        issue: datetime.date | datetime.datetime | pd.Timestamp | str,
        maturity: datetime.date | datetime.datetime | pd.Timestamp | str,
        discount_curve_id: str,
        *,
        currency: str | None = None,
    ) -> Bond:
        """
        Create a zero-coupon bond (single principal redemption at maturity).

        Mirrors Rust ``Bond::zero_coupon``.

        Parameters
        ----------
        id : str
            Unique instrument identifier.
        notional : Money | float
            Redemption amount; a bare number is tagged with ``currency``.
        issue : datetime.date | datetime.datetime | pd.Timestamp | str
            Issue date.
        maturity : datetime.date | datetime.datetime | pd.Timestamp | str
            Maturity (redemption) date.
        discount_curve_id : str
            Discount curve identifier used for pricing.
        currency : str, optional
            ISO-4217 code applied when ``notional`` is a bare number.

        Returns
        -------
        Bond
            A validated zero-coupon bond.

        Raises
        ------
        ValueError
            If a bare ``notional`` has no ``currency`` or ``maturity`` is not after
            ``issue``.
        TypeError
            If ``notional`` has an unsupported type or a date cannot be
            interpreted.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Bond
        >>> zc = Bond.zero_coupon("ZC", 1_000_000.0, "2024-01-01", "2029-01-01", "USD-OIS", currency="USD")
        >>> zc.has_floating_coupons
        False
        """
        ...
    @staticmethod
    def example() -> Bond:
        """
        Canonical example: 5-year USD 5% semi-annual fixed-rate bond discounted on ``USD-OIS`` (mirrors Rust ``Bond::example``).

        Returns
        -------
        Bond
            The example bond.

        Raises
        ------
        ValueError
            If construction fails (should not occur).

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Bond
        >>> Bond.example().discount_curve_id
        'USD-TREASURY'
        """
        ...
    @staticmethod
    def example_floating() -> Bond:
        """
        Canonical example: USD SOFR-linked floating-rate note (mirrors Rust ``Bond::example_floating``).

        Returns
        -------
        Bond
            The example bond.

        Raises
        ------
        ValueError
            If construction fails (should not occur).

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Bond
        >>> Bond.example_floating().has_floating_coupons
        True
        """
        ...
    @staticmethod
    def example_callable() -> Bond:
        """
        Canonical example: fixed-rate bond carrying a call schedule (mirrors Rust ``Bond::example_callable``).

        Returns
        -------
        Bond
            The example bond.

        Raises
        ------
        ValueError
            If construction fails (should not occur).

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Bond
        >>> Bond.example_callable().call_put is not None
        True
        """
        ...
    @staticmethod
    def example_amortizing() -> Bond:
        """
        Canonical example: fixed-rate bond with a principal amortization schedule (mirrors Rust ``Bond::example_amortizing``).

        Returns
        -------
        Bond
            The example bond.

        Raises
        ------
        ValueError
            If construction fails (should not occur).

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Bond
        >>> Bond.example_amortizing().cashflow_spec.keys() >= {"amortizing"}
        True
        """
        ...
    @classmethod
    def from_json(cls, json: str) -> Bond:
        """
        Deserialize a validated Bond from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"bond"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        Bond
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Bond
        >>> Bond.from_json(Bond.example().to_json()).id == Bond.example().id
        True
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to a canonical ``finstack_quant.instrument/1`` envelope.

        Returns
        -------
        str
            Canonical instrument envelope accepted by :func:`price_instrument`
            and :meth:`Bond.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def to_dict(self) -> dict[str, object]:
        """
        Serde form of the bond spec as a plain Python ``dict``.

        Returns
        -------
        dict[str, object]
            The ``spec`` object of the instrument envelope (JSON-compatible
            values: ``str``, ``float``, ``dict``, ``list``, ``None``).

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def price(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        model: str = "default",
        metrics: list[str] | None = None,
        pricing_options: dict[str, object] | str | None = None,
        market_history: str | None = None,
    ) -> ValuationResult:
        """
        Price this bond and return a :class:`~finstack_quant.valuations.ValuationResult`.

        Same pipeline and keyword surface as :func:`price_instrument`.

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date (ISO 8601 strings accepted).
        model : str, default "default"
            Model key (``"discounting"``, ``"hazard_rate"``, ``"tree"``,
            ``"rates_credit"``, ...). For bonds, ``"discounting"`` and
            ``"hazard_rate"`` are non-callable rates-only and fractional-
            recovery-of-par models;
            ``"tree"`` values rates-only rights; ``"rates_credit"`` values
            call, put, and return-floor rights jointly with credit risk.
        metrics : list[str], optional
            Metric identifiers to compute (see :func:`list_standard_metrics`).
        pricing_options : dict[str, object] | str, optional
            ``MetricPricingOverrides`` merged into the instrument's own overrides
            (e.g. ``{"theta_period": "1D"}``).
        market_history : str, optional
            JSON ``MarketHistory`` scenarios required by ``hvar`` /
            ``expected_shortfall``.

        Returns
        -------
        ValuationResult
            Typed valuation envelope with price, currency and metrics.
            Stochastic ``"rates_credit"`` pricing includes Monte Carlo
            convergence and reproducibility diagnostics in ``details``.

        Raises
        ------
        ValueError
            If an input cannot be interpreted or the instrument fails validation.
        KeyError
            If a required curve, surface or metric is missing from ``market``.
        RuntimeError
            If pricing or a metric computation fails.
        """
        ...
    def metric(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        metric_id: str,
        model: str = "default",
    ) -> float:
        """
        Compute one scalar metric for this bond (e.g. ``"ytm"``).

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.
        metric_id : str
            Registered metric identifier (see :func:`list_standard_metrics`).
        model : str, default "default"
            ``"default"`` uses the bond-native selection. Explicit keys are
            ``"discounting"`` for non-callable rates-only PV,
            ``"hazard_rate"`` for non-callable fractional recovery of par,
            ``"tree"`` for rates-only exercise rights, and ``"rates_credit"``
            for joint rates-credit valuation including call, put, and
            return-floor rights.

        Returns
        -------
        float
            The metric value in the metric's native unit (decimal for rates and
            yields, currency units for DV01-style sensitivities, basis points for
            spreads).

        Raises
        ------
        ValueError
            If ``metric_id`` is unknown or an input cannot be interpreted.
        KeyError
            If a required curve or surface is missing from ``market``.
        RuntimeError
            If the metric computation fails.
        """
        ...
    def market_dependencies(self) -> dict[str, object]:
        """
        Market-data dependencies declared by the Rust ``Instrument`` trait.

        Returns
        -------
        dict[str, object]
            Serde form of ``MarketDependencies`` (``curves`` grouped by role,
            fixings, volatility surfaces, FX pairs).

        Raises
        ------
        ValueError
            If the instrument cannot enumerate its dependencies.
        """
        ...
    def min_moic(self, multiple: float) -> Bond:
        """
        Return a copy with a minimum MOIC return floor on early redemption
        (mirrors Rust ``Bond::min_moic``).

        Parameters
        ----------
        multiple : float
            Minimum multiple of invested capital (e.g. ``1.25``).

        Returns
        -------
        Bond
            A new bond with ``return_floor`` set; ``self`` is unchanged.

        Notes
        -----
        This method does not raise; the floor is validated at pricing time.
        """
        ...
    def min_xirr(self, rate: float | Rate) -> Bond:
        """
        Return a copy with a minimum XIRR return floor on early redemption
        (mirrors Rust ``Bond::min_xirr``).

        Parameters
        ----------
        rate : float | Rate
            Target annualized IRR as a decimal (``0.12`` = 12%) or a ``Rate``.

        Returns
        -------
        Bond
            A new bond with ``return_floor`` set; ``self`` is unchanged.

        Raises
        ------
        TypeError
            If ``rate`` is neither a number nor a ``Rate``.
        """
        ...
    @property
    def id(self) -> str:
        """
        Stable instrument identifier used in market lookup and results.

        Returns
        -------
        str
            The unique instrument identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def notional(self) -> Money:
        """
        Principal amount of the bond.

        Returns
        -------
        Money
            Currency-tagged principal.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def issue_date(self) -> datetime.date:
        """
        Issue date of the bond.

        Returns
        -------
        datetime.date
            The contractual issue date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def maturity(self) -> datetime.date:
        """
        Maturity (final redemption) date.

        Returns
        -------
        datetime.date
            The contractual maturity date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def cashflow_spec(self) -> dict[str, object]:
        """
        Coupon/cashflow specification in serde form.

        Returns
        -------
        dict[str, object]
            One-key dict: ``{"fixed": {...}}``, ``{"floating": {...}}``, ``{"step_up": {...}}`` or ``{"amortizing": {...}}``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def discount_curve_id(self) -> str:
        """
        Discount curve identifier.

        Returns
        -------
        str
            Curve id used for discounting.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def forward_curve_id(self) -> str | None:
        """
        Forward curve identifier for floating coupons.

        Returns
        -------
        str | None
            Curve id, or ``None`` for fixed coupons.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def credit_curve_id(self) -> str | None:
        """
        Hazard curve identifier for ``hazard_rate`` and ``rates_credit`` pricing.

        Returns
        -------
        str | None
            Curve id, or ``None`` when no credit-consuming model is configured.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def funding_curve_id(self) -> str | None:
        """
        Funding curve identifier.

        Returns
        -------
        str | None
            Curve id, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def call_put(self) -> dict[str, object] | None:
        """
        Call/put schedule in serde form.

        Returns
        -------
        dict[str, object] | None
            ``{"calls": [...], "puts": [...]}`` or ``None`` for a bullet bond.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def return_floor(self) -> dict[str, object] | None:
        """
        Return-floor specification (minimum MOIC / XIRR) in serde form.

        Returns
        -------
        dict[str, object] | None
            The spec dict, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def custom_cashflows(self) -> dict[str, object] | None:
        """
        Explicit cashflow schedule overriding generated coupons, in serde form.

        Returns
        -------
        dict[str, object] | None
            The schedule dict, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def accrual_method(self) -> str:
        """
        Accrual method (serde string).

        Returns
        -------
        str
            ``"linear"`` unless overridden.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def settlement_convention(self) -> dict[str, object] | None:
        """
        Settlement convention (settlement lag, ex-coupon period) in serde form.

        Returns
        -------
        dict[str, object] | None
            ``{"settlement_days": ..., "ex_coupon_days": ..., "ex_coupon_calendar_id": ...}`` or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def settlement_days(self) -> int | None:
        """
        Settlement lag in business days.

        Returns
        -------
        int | None
            The lag, or ``None`` when no settlement convention is set.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def ex_coupon_days(self) -> int | None:
        """
        Ex-coupon period in business days.

        Returns
        -------
        int | None
            The period, or ``None`` when no settlement convention is set.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def has_floating_coupons(self) -> bool:
        """
        Whether coupons depend on forward-curve projection (FRNs).

        Returns
        -------
        bool
            ``True`` for floating (or amortizing-floating) cashflow specs.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def default_model(self) -> str:
        """
        Canonical model key used when ``model="default"`` is passed to ``price``.

        Returns
        -------
        str
            Registered model key such as ``"discounting"`` or ``"black76"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def attributes(self) -> Attributes:
        """
        Instrument attributes (tags and metadata) used for scenario selection.

        Returns
        -------
        Attributes
            The attribute bag; empty when none were set.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def expiry(self) -> datetime.date | None:
        """
        Expiry date exposed by the Rust ``Instrument`` trait.

        Returns
        -------
        datetime.date | None
            The expiry/maturity date, or ``None`` when the instrument type reports none.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    def price_merton_mc(
        self,
        config: MertonMcConfig,
        discount_rate: float,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
    ) -> MertonMcResult:
        """
        Price this bond with the Merton Monte Carlo structural credit engine.

        Uses geometric Brownian motion asset dynamics only. Floating-rate and
        amortizing cashflow specs raise ``ValueError``. When the config's PIK
        schedule is the default uniform cash mode, the bond's ``CouponType``
        overrides the schedule; otherwise the config schedule takes precedence.

        Parameters
        ----------
        config : MertonMcConfig
            Merton MC simulation configuration including the structural model.
        discount_rate : float
            Flat continuously compounded risk-free rate as a decimal used to
            discount simulated cashflows (unless term-structure discount factors
            are set on the config).
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.

        Returns
        -------
        MertonMcResult
            Monte Carlo pricing result with clean/dirty prices and path stats.

        Raises
        ------
        ValueError
            If ``as_of`` is invalid, the bond has floating or amortizing
            cashflows, or simulation parameters fail validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Bond, MertonMcConfig, PikMode, PikSchedule
        >>> from finstack_quant.models.credit import MertonModel
        >>> bond = Bond.fixed(
        ...     "BOND-MC",
        ...     1_000_000.0,
        ...     0.08,
        ...     "2024-01-01",
        ...     "2029-01-01",
        ...     "none",
        ...     "USD-OIS",
        ...     currency="USD",
        ... )
        >>> merton = MertonModel(100.0, 0.25, 80.0, 0.04)
        >>> config = (
        ...     MertonMcConfig(merton, 0.40).pik_schedule(PikSchedule.uniform(PikMode.pik())).num_paths(256).seed(42)
        ... )
        >>> bond.price_merton_mc(config, 0.04, "2024-01-01").num_paths
        256
        """
        ...

class BondBuilder:
    """
    Fluent builder for :class:`Bond`; wraps the Rust ``FinancialBuilder``
    output one setter for one setter.

    Builders are consumed by ``build()``; create a new builder per
    instrument. Required fields: ``id``, ``notional``, ``maturity``,
    ``cashflow_spec``, ``discount_curve_id`` (``issue_date`` defaults to
    ``maturity - 365 days``). Nested specs accept a ``dict`` or JSON ``str``
    in the Rust serde shape; ``Bond.example().to_dict()`` shows the exact
    field names.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import Bond
    >>> base = Bond.example().to_dict()
    >>> callable_bond = (
    ...     Bond
    ...     .builder()
    ...     .id("CALLABLE")
    ...     .notional(1_000_000.0, currency="USD")
    ...     .issue_date("2024-01-15")
    ...     .maturity("2034-01-15")
    ...     .cashflow_spec(base["cashflow_spec"])
    ...     .discount_curve_id("USD-OIS")
    ...     .credit_curve_id("ACME-HZD")
    ...     .call_put({
    ...         "calls": [{"start_date": "2029-01-15", "end_date": "2034-01-15", "price_pct_of_par": 100.0}],
    ...         "puts": [],
    ...     })
    ...     .build()
    ... )
    >>> callable_bond.credit_curve_id
    'ACME-HZD'
    """

    def id(self, value: str) -> BondBuilder:
        """
        Set the instrument identifier.

        Parameters
        ----------
        value : str
            Instrument identifier.

        Returns
        -------
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def notional(self, value: Money | float, currency: str | None = None) -> BondBuilder:
        """
        Set the principal amount.

        Parameters
        ----------
        value : Money | float
            Principal amount; a bare number is tagged with ``currency``.
        currency : str, optional
            ISO-4217 code applied when ``value`` is a bare number.

        Returns
        -------
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or a bare number is given without ``currency``.
        """
        ...
    def issue_date(self, value: datetime.date | datetime.datetime | pd.Timestamp | str) -> BondBuilder:
        """
        Set the issue date (defaults to ``maturity - 365 days`` when unset).

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pd.Timestamp | str
            Issue date (defaults to ``maturity - 365 days`` when unset) (ISO 8601 strings accepted).

        Returns
        -------
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or the date cannot be interpreted.
        """
        ...
    def maturity(self, value: datetime.date | datetime.datetime | pd.Timestamp | str) -> BondBuilder:
        """
        Set the maturity date.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pd.Timestamp | str
            Maturity date (ISO 8601 strings accepted).

        Returns
        -------
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or the date cannot be interpreted.
        """
        ...
    def cashflow_spec(self, value: dict[str, object] | str) -> BondBuilder:
        """
        Set the coupon/cashflow specification.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``CashflowSpec`` in serde form (``dict`` or JSON string), e.g. ``{"fixed": {"coupon_type": "cash", "rate": "0.05", "schedule": {...}}}`` (copy ``Bond.example().cashflow_spec``).

        Returns
        -------
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does not deserialize as a ``CashflowSpec``.
        """
        ...
    def discount_curve_id(self, value: str) -> BondBuilder:
        """
        Set the discount curve identifier.

        Parameters
        ----------
        value : str
            Discount curve identifier.

        Returns
        -------
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def forward_curve_id(self, value: str) -> BondBuilder:
        """
        Set the forward curve identifier used by floating coupons.

        Parameters
        ----------
        value : str
            Forward curve identifier used by floating coupons.

        Returns
        -------
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def credit_curve_id(self, value: str) -> BondBuilder:
        """
        Set the hazard curve identifier for ``hazard_rate`` and ``rates_credit`` pricing.

        Parameters
        ----------
        value : str
            Hazard curve identifier for scalar or joint rates-credit pricing.

        Returns
        -------
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def funding_curve_id(self, value: str) -> BondBuilder:
        """
        Set the funding curve identifier.

        Parameters
        ----------
        value : str
            Funding curve identifier.

        Returns
        -------
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def call_put(self, value: dict[str, object] | str) -> BondBuilder:
        """
        Set the call/put schedule.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``CallPutSchedule`` in serde form (``dict`` or JSON string), e.g. ``{"calls": [{"start_date": "2027-01-15", "end_date": "2029-01-15", "price_pct_of_par": 100.0}], "puts": []}``.

        Returns
        -------
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does not deserialize as a ``CallPutSchedule``.
        """
        ...
    def return_floor(self, value: dict[str, object] | str) -> BondBuilder:
        """
        Set the return-floor specification (minimum MOIC / XIRR on early redemption).

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``ReturnFloorSpec`` in serde form (``dict`` or JSON string), e.g. ``Bond.example().min_moic(1.25).return_floor``.

        Returns
        -------
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does not deserialize as a ``ReturnFloorSpec``.
        """
        ...
    def custom_cashflows(self, value: dict[str, object] | str) -> BondBuilder:
        """
        Set the explicit cashflow schedule that overrides generated coupons.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``CashFlowSchedule`` in serde form (``dict`` or JSON string), e.g. the ``custom_cashflows`` value of a bond built from cashflows.

        Returns
        -------
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does not deserialize as a ``CashFlowSchedule``.
        """
        ...
    def accrual_method(self, value: str) -> BondBuilder:
        """
        Set the accrual method.

        Parameters
        ----------
        value : str
            Accrual method (serde string). ``"linear"`` is the default.

        Returns
        -------
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized name.
        """
        ...
    def settlement_convention(self, value: dict[str, object] | str) -> BondBuilder:
        """
        Set the settlement convention (settlement lag and ex-coupon period).

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``BondSettlementConvention`` in serde form (``dict`` or JSON string), e.g. ``{"settlement_days": 2, "ex_coupon_days": 0, "ex_coupon_calendar_id": None}``.

        Returns
        -------
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does not deserialize as a ``BondSettlementConvention``.
        """
        ...
    def attributes(self, value: Attributes | dict[str, str]) -> BondBuilder:
        """
        Set instrument attributes (tags and metadata).

        Parameters
        ----------
        value : Attributes | dict[str, str]
            Attribute bag; a ``dict`` populates ``meta`` and an optional
            ``"tags"`` entry holding a list of strings populates ``tags``.

        Returns
        -------
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        TypeError
            If ``value`` is neither ``Attributes`` nor a ``dict``.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style rendering of the fields set so far, e.g.
        ``BondBuilder(id='X', notional=Money(1000000.0, 'USD'))``.

        Returns
        -------
        str
            ``BondBuilder(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise; it renders the recorded setter calls.
        """
        ...
    def build(self) -> Bond:
        """
        Build the validated bond.

        Runs the same validation as the Rust ``BondBuilder::build`` (structural
        invariants only); pricing-time checks run in ``Bond.price``.

        Returns
        -------
        Bond
            The validated instrument.

        Raises
        ------
        ValueError
            If the builder was already consumed, a required field is missing
            (the message names the builder and the field, e.g.
            ``BondBuilder: missing required field 'id'``), or the instrument
            fails validation.
        """
        ...

class BarrierCrossing:
    """
    Barrier-crossing detection policy for first-passage default simulation.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import BarrierCrossing
    >>> BarrierCrossing.brownian_bridge().to_json()
    '"brownian_bridge"'
    """

    @staticmethod
    def discrete() -> BarrierCrossing:
        """
        Discrete monitoring at simulation grid points.

        Returns
        -------
        BarrierCrossing
            Discrete policy: default is declared only when an asset value
            sampled on the simulation grid sits below the barrier. Fast, but it
            misses excursions between steps and so understates default risk on
            coarse grids. The default for terminal-barrier models.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import BarrierCrossing
        >>> BarrierCrossing.discrete().to_json()
        '"discrete"'
        """
        ...

    @staticmethod
    def brownian_bridge() -> BarrierCrossing:
        """
        Brownian-bridge correction for continuous monitoring.

        Returns
        -------
        BarrierCrossing
            Brownian-bridge policy: between two surviving grid points it draws
            against the analytic crossing probability, which removes the
            discretisation bias. The default for first-passage models and the
            more expensive of the two.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import BarrierCrossing
        >>> BarrierCrossing.brownian_bridge().to_json()
        '"brownian_bridge"'
        """
        ...

    @staticmethod
    def from_json(json: str) -> BarrierCrossing:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON-encoded barrier-crossing policy.

        Returns
        -------
        BarrierCrossing
            The decoded policy.

        Raises
        ------
        ValueError
            If ``json`` is not valid JSON for a barrier-crossing policy.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import BarrierCrossing
        >>> BarrierCrossing.from_json('"discrete"').to_json()
        '"discrete"'
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            JSON-encoded barrier-crossing policy.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def __repr__(self) -> str:
        """Return a Python-style constructor form of this value.

        Returns
        -------
        str
            ``BarrierCrossing.brownian_bridge()`` text.
        """
        ...

class MertonMcConfig:
    """
    Configuration for Merton Monte Carlo PIK bond pricing.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import MertonMcConfig, PikMode, PikSchedule
    >>> from finstack_quant.models.credit import MertonModel
    >>> config = MertonMcConfig(MertonModel(100.0, 0.25, 80.0, 0.04), 0.40).num_paths(1000)
    >>> isinstance(config.seed(1).pik_schedule(PikSchedule.uniform(PikMode.cash())), MertonMcConfig)
    True
    """

    def __init__(self, merton: MertonModel, recovery_rate: float) -> None:
        """
        Create a configuration with registry-sourced simulation defaults.

        Parameters
        ----------
        merton : MertonModel
            Structural credit model driving asset dynamics and default.
        recovery_rate : float
            Required recovery on default as a decimal fraction in ``[0, 1]``.

        Raises
        ------
        ValueError
            If ``recovery_rate`` is non-finite or outside ``[0, 1]``.
        """
        ...

    def pik_schedule(self, s: PikSchedule) -> MertonMcConfig:
        """
        Set the payment-in-kind schedule for the Merton MC config.

        Parameters
        ----------
        s : PikSchedule
            PIK schedule applied across coupon dates.

        Returns
        -------
        MertonMcConfig
            Updated configuration (fluent).

        Raises
        ------
        ValueError
            If ``r`` is non-finite or outside ``[0, 1]``.
        """
        ...

    def num_paths(self, n: int) -> MertonMcConfig:
        """
        Set the number of Monte Carlo paths.

        Parameters
        ----------
        n : int
            Total paths to retain. With antithetic variates on, the mirrors
            count toward ``n`` rather than doubling it, so the engine draws
            only ``ceil(n / 2)`` independent normals.

        Returns
        -------
        MertonMcConfig
            Updated configuration (fluent).

        Notes
        -----
        This method does not raise; it returns the same instance for chaining.
        """
        ...

    def seed(self, s: int) -> MertonMcConfig:
        """
        Set the Monte Carlo RNG seed for reproducible paths.

        Parameters
        ----------
        s : int
            Unsigned 64-bit seed.

        Returns
        -------
        MertonMcConfig
            Updated configuration (fluent).

        Notes
        -----
        This method does not raise; it returns the same instance for chaining.
        """
        ...

    def antithetic(self, a: bool) -> MertonMcConfig:
        """
        Enable or disable antithetic variates.

        Parameters
        ----------
        a : bool
            When ``True``, pair each path with its antithetic counterpart.

        Returns
        -------
        MertonMcConfig
            Updated configuration (fluent).

        Notes
        -----
        This method does not raise; it returns the same instance for chaining.
        """
        ...

    def time_steps_per_year(self, n: int) -> MertonMcConfig:
        """
        Set simulation grid density.

        Parameters
        ----------
        n : int
            Time steps per year.

        Returns
        -------
        MertonMcConfig
            Updated configuration (fluent).

        Notes
        -----
        This method does not raise; it returns the same instance for chaining.
        """
        ...

    def barrier_crossing(self, p: BarrierCrossing) -> MertonMcConfig:
        """
        Set barrier-crossing policy for first-passage monitoring.

        Parameters
        ----------
        p : BarrierCrossing
            Discrete or Brownian-bridge policy.

        Returns
        -------
        MertonMcConfig
            Updated configuration (fluent).

        Notes
        -----
        This method does not raise; it returns the same instance for chaining.
        """
        ...

    def default_recovery_rate(self, r: float) -> MertonMcConfig:
        """
        Set flat recovery when no dynamic recovery model is configured.

        Parameters
        ----------
        r : float
            Recovery rate as a decimal in ``[0, 1]``.

        Returns
        -------
        MertonMcConfig
            Updated configuration (fluent).

        Notes
        -----
        This method does not raise; it returns the same instance for chaining.
        """
        ...

    def endogenous_hazard(self, h: EndogenousHazardSpec) -> MertonMcConfig:
        """
        Set an endogenous hazard model.

        Parameters
        ----------
        h : EndogenousHazardSpec
            Endogenous hazard specification.

        Returns
        -------
        MertonMcConfig
            Updated configuration (fluent).

        Notes
        -----
        This method does not raise; it returns the same instance for chaining.
        """
        ...

    def dynamic_recovery(self, r: DynamicRecoverySpec) -> MertonMcConfig:
        """
        Set a dynamic recovery model.

        Parameters
        ----------
        r : DynamicRecoverySpec
            Dynamic recovery specification.

        Returns
        -------
        MertonMcConfig
            Updated configuration (fluent).

        Notes
        -----
        This method does not raise; it returns the same instance for chaining.
        """
        ...

    def toggle_model(self, t: ToggleExerciseModel) -> MertonMcConfig:
        """
        Set toggle exercise model for PIK/cash decisions.

        Parameters
        ----------
        t : ToggleExerciseModel
            Toggle exercise model.

        Returns
        -------
        MertonMcConfig
            Updated configuration (fluent).

        Notes
        -----
        This method does not raise; it returns the same instance for chaining.
        """
        ...

    @staticmethod
    def from_json(json: str) -> MertonMcConfig:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON-encoded configuration.

        Returns
        -------
        MertonMcConfig
            The decoded configuration.

        Raises
        ------
        ValueError
            If ``json`` is not valid JSON for a Merton MC configuration.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import MertonMcConfig
        >>> from finstack_quant.models.credit import MertonModel
        >>> config = MertonMcConfig(MertonModel(100.0, 0.25, 80.0, 0.04), 0.40).num_paths(256).seed(42)
        >>> MertonMcConfig.from_json(config.to_json()).to_json() == config.to_json()
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            JSON-encoded configuration.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def __repr__(self) -> str:
        """Return a Python-style constructor form of this value.

        Returns
        -------
        str
            ``MertonMcConfig(num_paths=10000, seed=42, antithetic=True, ...)`` text.
        """
        ...

class MertonMcResult:
    """
    Result from Merton Monte Carlo PIK bond pricing.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.dates import StubKind
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.core.types import Rate
    >>> from finstack_quant.valuations.instruments import (
    ...     BarrierCrossing,
    ...     Bond,
    ...     MertonMcConfig,
    ...     PikMode,
    ...     PikSchedule,
    ... )
    >>> from finstack_quant.models.credit import MertonModel
    >>> config = (
    ...     MertonMcConfig(MertonModel(100.0, 0.25, 60.0, 0.04), 0.40)
    ...     .num_paths(64)
    ...     .seed(7)
    ...     .pik_schedule(PikSchedule.uniform(PikMode.pik()))
    ...     .barrier_crossing(BarrierCrossing.discrete())
    ... )
    >>> bond = Bond.fixed(
    ...     "PIK-1",
    ...     Money(100.0, Currency("USD")),
    ...     Rate(0.08),
    ...     datetime.date(2024, 1, 15),
    ...     datetime.date(2029, 1, 15),
    ...     StubKind.NONE,
    ...     "USD-OIS",
    ... )
    >>> bond.price_merton_mc(config, 0.04, datetime.date(2024, 1, 15)).clean_price_pct > 0.0
    True
    """

    @property
    def clean_price_pct(self) -> float:
        """
        Clean price as a percentage of par.

        Returns
        -------
        float
            Mean discounted path value divided by notional, times 100, so
            ``98.7`` means 98.7% of par. Quoted on the same discount basis the
            configuration supplied.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def dirty_price_pct(self) -> float:
        """
        Dirty price as a percentage of par.

        Returns
        -------
        float
            Always equal to :attr:`clean_price_pct`: the Monte Carlo engine
            works in continuous time and never separates accrued interest. Use
            the pricer's metrics pipeline for a genuine clean/dirty split.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def expected_loss(self) -> float:
        """
        Expected loss as a fraction of PIK-aware risk-free PV.

        Returns
        -------
        float
            ``1 - mean_mc_pv / risk_free_pv``, so ``0.03`` is a 3% credit
            haircut. The benchmark PV accretes notional under the configured
            PIK schedule, and the value turns negative when the simulated PV
            exceeds that benchmark.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def unexpected_loss(self) -> float:
        """
        Unexpected loss (std dev of path PVs / notional).

        Returns
        -------
        float
            Dispersion of the loss distribution as a fraction of par, not a
            percentage: ``0.05`` here is comparable to 5 points on
            :attr:`clean_price_pct`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def expected_shortfall_95(self) -> float:
        """
        Expected shortfall at the 95% confidence level.

        Returns
        -------
        float
            Mean of the worst 5% of path PVs, expressed as a percentage of par
            like :attr:`clean_price_pct`. It is a price level rather than a
            loss, so lower is worse and it never exceeds the clean price.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def average_pik_fraction(self) -> float:
        """
        Average PIK fraction across coupon dates and paths.

        Returns
        -------
        float
            PIK elections divided by simulated coupon periods, in ``[0, 1]``.
            Counts whole elections, so a 50/50 split coupon still registers as
            one election. Identical to
            ``path_statistics.pik_exercise_rate``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def effective_spread_bp(self) -> float:
        """
        Effective spread in basis points versus risk-free PV.

        Returns
        -------
        float
            Constant continuous spread ``s`` solving ``risk_free_pv`` with each
            discount factor scaled by ``exp(-s * t)`` equal to the mean
            simulated PV. Solved on whichever discount basis priced the bond,
            so curve shape stays out of the spread.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def path_statistics(self) -> PathStatistics:
        """
        Path-level simulation statistics.

        Returns
        -------
        PathStatistics
            Default frequency, timing, recovery, and PIK-election diagnostics
            for the same run that produced these prices.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def num_paths(self) -> int:
        """
        Number of Monte Carlo paths used.

        Returns
        -------
        int
            Paths actually retained, matching the configured
            ``MertonMcConfig.num_paths``. Antithetic mirrors count toward this
            total instead of doubling it.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def standard_error(self) -> float:
        """
        Standard error of the clean price (percentage of par).

        Returns
        -------
        float
            Sampling error of :attr:`clean_price_pct` in the same
            percent-of-par units, so 1.96 of these brackets a 95% interval
            either side of the price. With antithetic variates the estimate
            comes from pair averages, which keeps the negatively correlated
            legs from understating it.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the headline results as a single-row pandas ``DataFrame``.

        Returns
        -------
        pd.DataFrame
            Columns ``clean_price_pct``, ``dirty_price_pct``, ``expected_loss``,
            ``unexpected_loss``, ``expected_shortfall_95``,
            ``average_pik_fraction``, ``effective_spread_bp``, ``num_paths``,
            ``standard_error``, ``default_rate``, ``avg_default_time``,
            ``avg_terminal_notional``, ``avg_recovery_pct`` and
            ``pik_exercise_rate``.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def _repr_html_(self) -> str:
        """
        Jupyter rich display: the :meth:`to_dataframe` table as HTML.

        Returns
        -------
        str
            HTML table rendered by pandas.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

class PathStatistics:
    """
    Path-level statistics from a Merton Monte Carlo simulation.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.dates import StubKind
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.core.types import Rate
    >>> from finstack_quant.valuations.instruments import (
    ...     BarrierCrossing,
    ...     Bond,
    ...     MertonMcConfig,
    ...     PikMode,
    ...     PikSchedule,
    ... )
    >>> from finstack_quant.models.credit import MertonModel
    >>> config = (
    ...     MertonMcConfig(MertonModel(100.0, 0.25, 60.0, 0.04), 0.40)
    ...     .num_paths(64)
    ...     .seed(7)
    ...     .pik_schedule(PikSchedule.uniform(PikMode.pik()))
    ...     .barrier_crossing(BarrierCrossing.discrete())
    ... )
    >>> bond = Bond.fixed(
    ...     "PIK-1",
    ...     Money(100.0, Currency("USD")),
    ...     Rate(0.08),
    ...     datetime.date(2024, 1, 15),
    ...     datetime.date(2029, 1, 15),
    ...     StubKind.NONE,
    ...     "USD-OIS",
    ... )
    >>> 0.0 <= bond.price_merton_mc(config, 0.04, datetime.date(2024, 1, 15)).path_statistics.default_rate <= 1.0
    True
    """

    @property
    def default_rate(self) -> float:
        """
        Fraction of paths that defaulted.

        Returns
        -------
        float
            Defaulted paths divided by simulated paths, in ``[0, 1]``. This is
            a cumulative default probability to maturity, not an annualised
            hazard rate.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def avg_default_time(self) -> float:
        """
        Average default time in years among defaulted paths.

        Returns
        -------
        float
            Mean time from the valuation date to the barrier crossing,
            averaged over defaulted paths only. Exactly ``0.0`` when no path
            defaulted, so check :attr:`default_rate` before reading it.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def avg_terminal_notional(self) -> float:
        """
        Average terminal notional reflecting PIK accretion.

        Returns
        -------
        float
            Currency-unit notional at maturity averaged over surviving paths,
            so it exceeds the issued notional whenever coupons were PIKed.
            Falls back to the issued notional when every path defaulted.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def avg_recovery_pct(self) -> float:
        """
        Average recovery percentage among defaulted paths.

        Returns
        -------
        float
            Decimal fraction despite the ``_pct`` name: ``0.40`` means 40%
            recovery on the notional accreted up to the default time,
            averaged over defaulted paths. Exactly ``0.0`` when no path
            defaulted.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def pik_exercise_rate(self) -> float:
        """
        Fraction of coupon dates where PIK was elected.

        Returns
        -------
        float
            Same quantity as ``MertonMcResult.average_pik_fraction``, in
            ``[0, 1]``: a uniform cash schedule pins it to ``0.0`` and a
            uniform PIK schedule to ``1.0``, so only toggle and split
            schedules put it in between.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class PikMode:
    """
    Per-coupon PIK behavior for the Merton Monte Carlo engine.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import PikMode
    >>> PikMode.cash().to_json()
    '"cash"'
    """

    @staticmethod
    def cash() -> PikMode:
        """
        Coupon paid entirely in cash.

        Returns
        -------
        PikMode
            Cash mode, the default a schedule falls back to before its first
            step and whenever a toggle model is missing.

        Notes
        -----
        This factory does not raise; it returns a new instance with the documented defaults.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import PikMode
        >>> PikMode.cash().to_json()
        '"cash"'
        """
        ...

    @staticmethod
    def pik() -> PikMode:
        """
        Coupon accreted to notional.

        Returns
        -------
        PikMode
            Payment-in-kind mode: the coupon pays no cash and instead raises
            the notional that later coupons and recovery are computed on.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import PikMode
        >>> PikMode.pik().to_json()
        '"pik"'
        """
        ...

    @staticmethod
    def split(cash_fraction: float, pik_fraction: float) -> PikMode:
        """
        Coupon split between cash and PIK.

        Parameters
        ----------
        cash_fraction : float
            Fraction paid in cash as a decimal.
        pik_fraction : float
            Fraction accreted to notional as a decimal.

        Returns
        -------
        PikMode
            Split PIK mode. The two fractions must be non-negative and sum to
            one; the engine rejects anything else when the bond is priced, not
            when this mode is built.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import PikMode
        >>> PikMode.split(0.5, 0.5).to_json()
        '{"split":{"cash_fraction":0.5,"pik_fraction":0.5}}'
        """
        ...

    @staticmethod
    def toggle() -> PikMode:
        """
        Defer to the toggle exercise model on the config.

        Returns
        -------
        PikMode
            Toggle mode, which decides cash versus PIK per path from
            ``MertonMcConfig.toggle_model``. Without that model set the coupon
            silently falls back to cash.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import PikMode
        >>> PikMode.toggle().to_json()
        '"toggle"'
        """
        ...

    @staticmethod
    def from_json(json: str) -> PikMode:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON-encoded PIK mode.

        Returns
        -------
        PikMode
            The decoded mode.

        Raises
        ------
        ValueError
            If ``json`` is not valid JSON for a PIK mode.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import PikMode
        >>> PikMode.from_json('"pik"').to_json()
        '"pik"'
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            JSON-encoded PIK mode.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def __repr__(self) -> str:
        """Return a Python-style constructor form of this value.

        Returns
        -------
        str
            ``PikMode.split(cash_fraction=0.5, pik_fraction=0.5)`` text.
        """
        ...

class PikSchedule:
    """
    Time-varying PIK schedule for the Merton Monte Carlo engine.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import PikMode, PikSchedule
    >>> PikSchedule.uniform(PikMode.pik()).mode_at(1.0).to_json()
    '"pik"'
    """

    @staticmethod
    def uniform(mode: PikMode) -> PikSchedule:
        """
        Apply the same PIK mode at every coupon date.

        Parameters
        ----------
        mode : PikMode
            PIK mode applied uniformly.

        Returns
        -------
        PikSchedule
            Uniform schedule; every coupon date resolves to ``mode`` for the
            whole life of the bond.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import PikMode, PikSchedule
        >>> PikSchedule.uniform(PikMode.pik()).mode_at(1.0).to_json()
        '"pik"'
        """
        ...

    @staticmethod
    def stepped(steps: list[tuple[float, PikMode]]) -> PikSchedule:
        """
        Step-function PIK schedule keyed by year fraction.

        Parameters
        ----------
        steps : list[tuple[float, PikMode]]
            ``(year_fraction, mode)`` pairs sorted by time ascending.

        Returns
        -------
        PikSchedule
            Stepped schedule in which each entry stays in force from its year
            fraction until the next one. Coupons before the first step fall
            back to cash, so start the list at ``0.0`` to control them.

        Raises
        ------
        ValueError
            If ``steps`` cannot be parsed or fails validation at pricing time.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import PikMode, PikSchedule
        >>> schedule = PikSchedule.stepped([(0.0, PikMode.pik()), (2.0, PikMode.cash())])
        >>> schedule.mode_at(2.5).to_json()
        '"cash"'
        """
        ...

    def mode_at(self, t: float) -> PikMode:
        """
        Look up the active PIK mode at time ``t``.

        Parameters
        ----------
        t : float
            Time in years from the valuation date.

        Returns
        -------
        PikMode
            Active mode at ``t``.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """
        ...

    @staticmethod
    def from_json(json: str) -> PikSchedule:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON-encoded PIK schedule.

        Returns
        -------
        PikSchedule
            The decoded schedule.

        Raises
        ------
        ValueError
            If ``json`` is not valid JSON for a PIK schedule.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import PikMode, PikSchedule
        >>> PikSchedule.from_json('{"stepped":[[0.0,"pik"],[2.0,"cash"]]}').mode_at(0.5).to_json()
        '"pik"'
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to compact JSON.

        Returns
        -------
        str
            JSON-encoded PIK schedule.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def __repr__(self) -> str:
        """Return a Python-style constructor form of this value.

        Returns
        -------
        str
            ``PikSchedule.uniform(PikMode.pik())`` text.
        """
        ...

class TermLoan:
    """
    Typed wrapper for the canonical Rust ``TermLoan`` instrument.

    Construct via :meth:`TermLoan.builder` (a bare decimal ``rate`` builds a
    fixed-rate loan; a serde ``dict`` builds a floating one), the
    ``TermLoan.example*`` presets or :meth:`TermLoan.from_json`. Every
    public Rust field is readable as a property; :meth:`TermLoan.price` /
    :meth:`TermLoan.metric` run the same pricer as :func:`price_instrument`.
    Instances are accepted directly by :func:`price_instrument` and
    :func:`instrument_cashflows_json`.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import TermLoan
    >>> loan = TermLoan.example()
    >>> (loan.id, loan.rate)
    ('TERM-LOAN-USD-5Y', {'fixed': {'rate_bp': 600}})
    """

    @staticmethod
    def builder() -> TermLoanBuilder:
        """
        Create a fluent builder (mirrors Rust ``TermLoan::builder()``).

        Returns
        -------
        TermLoanBuilder
            A builder with fluent, consuming setter methods.

        Notes
        -----
        This factory does not raise; it returns an empty builder.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import TermLoan
        >>> builder = TermLoan.builder()
        >>> builder.id("EXAMPLE") is builder
        True
        """
        ...
    @staticmethod
    def example() -> TermLoan:
        """
        Canonical example: 5-year USD 6% quarterly Act/360 loan with 2.5% per-period amortization (mirrors Rust ``TermLoan::example``).

        Returns
        -------
        TermLoan
            The example loan.

        Raises
        ------
        ValueError
            If construction fails (should not occur).

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import TermLoan
        >>> TermLoan.example().id
        'TERM-LOAN-USD-5Y'
        """
        ...
    @staticmethod
    def example_floating_with_ddtl() -> TermLoan:
        """
        Canonical example: 7-year USD SOFR + 400bp leveraged loan with a delayed-draw commitment and a 0% floor (mirrors Rust ``TermLoan::example_floating_with_ddtl``).

        Returns
        -------
        TermLoan
            The example loan.

        Raises
        ------
        ValueError
            If construction fails (should not occur).

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import TermLoan
        >>> TermLoan.example_floating_with_ddtl().ddtl is not None
        True
        """
        ...
    @staticmethod
    def example_callable() -> TermLoan:
        """
        Canonical example: loan carrying a prepayment (call) schedule (mirrors Rust ``TermLoan::example_callable``).

        Returns
        -------
        TermLoan
            The example loan.

        Raises
        ------
        ValueError
            If construction fails (should not occur).

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import TermLoan
        >>> TermLoan.example_callable().call_schedule is not None
        True
        """
        ...
    @classmethod
    def from_json(cls, json: str) -> TermLoan:
        """
        Deserialize a validated TermLoan from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"term_loan"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        TermLoan
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import TermLoan
        >>> TermLoan.from_json(TermLoan.example().to_json()).id
        'TERM-LOAN-USD-5Y'
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to a canonical ``finstack_quant.instrument/1`` envelope.

        Returns
        -------
        str
            Canonical instrument envelope accepted by :func:`price_instrument`
            and :meth:`TermLoan.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def to_dict(self) -> dict[str, object]:
        """
        Serde form of the loan spec as a plain Python ``dict``.

        Returns
        -------
        dict[str, object]
            The ``spec`` object of the instrument envelope (JSON-compatible
            values: ``str``, ``float``, ``dict``, ``list``, ``None``).

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def price(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        model: str = "default",
        metrics: list[str] | None = None,
        pricing_options: dict[str, object] | str | None = None,
        market_history: str | None = None,
    ) -> ValuationResult:
        """
        Price this term loan and return a :class:`~finstack_quant.valuations.ValuationResult`.

        Same pipeline and keyword surface as :func:`price_instrument`.

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date (ISO 8601 strings accepted).
        model : str, default "default"
            Model key (``"discounting"``, ``"hazard_rate"``, ``"tree"``).
        metrics : list[str], optional
            Metric identifiers to compute (see :func:`list_standard_metrics`).
        pricing_options : dict[str, object] | str, optional
            ``MetricPricingOverrides`` merged into the instrument's own overrides
            (e.g. ``{"theta_period": "1D"}``).
        market_history : str, optional
            JSON ``MarketHistory`` scenarios required by ``hvar`` /
            ``expected_shortfall``.

        Returns
        -------
        ValuationResult
            Typed valuation envelope with price, currency and metrics.

        Raises
        ------
        ValueError
            If an input cannot be interpreted or the instrument fails validation.
        KeyError
            If a required curve, surface or metric is missing from ``market``.
        RuntimeError
            If pricing or a metric computation fails.
        """
        ...
    def metric(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        metric_id: str,
        model: str = "default",
    ) -> float:
        """
        Compute one scalar metric for this term loan (e.g. ``"dv01"``).

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.
        metric_id : str
            Registered metric identifier (see :func:`list_standard_metrics`).
        model : str, default "default"
            Model key.

        Returns
        -------
        float
            The metric value in the metric's native unit (decimal for rates and
            yields, currency units for DV01-style sensitivities, basis points for
            spreads).

        Raises
        ------
        ValueError
            If ``metric_id`` is unknown or an input cannot be interpreted.
        KeyError
            If a required curve or surface is missing from ``market``.
        RuntimeError
            If the metric computation fails.
        """
        ...
    def market_dependencies(self) -> dict[str, object]:
        """
        Market-data dependencies declared by the Rust ``Instrument`` trait.

        Returns
        -------
        dict[str, object]
            Serde form of ``MarketDependencies`` (``curves`` grouped by role,
            fixings, volatility surfaces, FX pairs).

        Raises
        ------
        ValueError
            If the instrument cannot enumerate its dependencies.
        """
        ...
    @property
    def id(self) -> str:
        """
        Stable instrument identifier used in market lookup and results.

        Returns
        -------
        str
            The unique instrument identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def currency(self) -> str:
        """
        Currency the loan is denominated in.

        Returns
        -------
        str
            ISO-4217 currency code such as ``"USD"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def notional_limit(self) -> Money:
        """
        Committed notional (facility limit).

        Returns
        -------
        Money
            Currency-tagged commitment.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def issue_date(self) -> datetime.date:
        """
        Issue / funding date.

        Returns
        -------
        datetime.date
            The issue date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def maturity(self) -> datetime.date:
        """
        Maturity (final redemption) date.

        Returns
        -------
        datetime.date
            The contractual maturity date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def rate(self) -> dict[str, object]:
        """
        Rate specification in serde form.

        Returns
        -------
        dict[str, object]
            ``{"fixed": {"rate_bp": 600}}`` or ``{"floating": {...}}``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def frequency(self) -> Tenor:
        """
        Payment frequency.

        Returns
        -------
        Tenor
            The payment tenor.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def day_count(self) -> DayCount:
        """
        Accrual day-count convention.

        Returns
        -------
        DayCount
            The day count.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def business_day_convention(self) -> str:
        """
        Business day convention (serde string).

        Returns
        -------
        str
            ``"modified_following"`` unless overridden.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def calendar_id(self) -> str | None:
        """
        Holiday calendar identifier.

        Returns
        -------
        str | None
            Calendar id, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def stub(self) -> StubKind:
        """
        Stub-period handling rule for the schedule.

        Returns
        -------
        StubKind
            The ``StubKind`` variant (``NONE``, ``SHORT_FRONT``, ...).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def discount_curve_id(self) -> str:
        """
        Discount curve identifier.

        Returns
        -------
        str
            Curve id used for discounting.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def credit_curve_id(self) -> str | None:
        """
        Hazard curve identifier.

        Returns
        -------
        str | None
            Curve id, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def amortization(self) -> str | dict[str, object]:
        """
        Amortization specification in serde form.

        Returns
        -------
        str | dict[str, object]
            ``"none"``, ``{"percent_per_period": {"bp": 250}}``, ``{"linear": {...}}``, ...

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def coupon_type(self) -> str:
        """
        Coupon type (serde string).

        Returns
        -------
        str
            ``"cash"``, ``"pik"``, ...

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def upfront_fee(self) -> Money | None:
        """
        Upfront fee paid at funding.

        Returns
        -------
        Money | None
            Currency-tagged fee amount, or ``None`` when the loan has no upfront fee.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def ddtl(self) -> dict[str, object] | None:
        """
        Delayed-draw term loan specification in serde form.

        Returns
        -------
        dict[str, object] | None
            The spec dict, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def covenants(self) -> dict[str, object] | None:
        """
        Covenant event schedule in serde form.

        Returns
        -------
        dict[str, object] | None
            The events dict, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def oid_eir(self) -> dict[str, object] | None:
        """
        OID / effective-interest-rate specification in serde form.

        Returns
        -------
        dict[str, object] | None
            The spec dict, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def call_schedule(self) -> dict[str, object] | None:
        """
        Prepayment (call) schedule in serde form.

        Returns
        -------
        dict[str, object] | None
            The schedule dict, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def settlement_days(self) -> int:
        """
        Settlement lag in business days.

        Returns
        -------
        int
            The lag (Rust default 2).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def default_model(self) -> str:
        """
        Canonical model key used when ``model="default"`` is passed to ``price``.

        Returns
        -------
        str
            Registered model key such as ``"discounting"`` or ``"black76"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def attributes(self) -> Attributes:
        """
        Instrument attributes (tags and metadata) used for scenario selection.

        Returns
        -------
        Attributes
            The attribute bag; empty when none were set.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def expiry(self) -> datetime.date | None:
        """
        Expiry date exposed by the Rust ``Instrument`` trait.

        Returns
        -------
        datetime.date | None
            The expiry/maturity date, or ``None`` when the instrument type reports none.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class TermLoanBuilder:
    """
    Fluent builder for :class:`TermLoan`; wraps the Rust ``FinancialBuilder``
    output one setter for one setter.

    Builders are consumed by ``build()``; create a new builder per
    instrument. Required fields: ``id``, ``currency``, ``notional_limit``,
    ``maturity``, ``rate``, ``frequency``, ``day_count``,
    ``discount_curve_id``, ``amortization``. Nested specs accept a ``dict``
    or JSON ``str`` in the Rust serde shape.

    Examples
    --------
    >>> from finstack_quant.core.dates import DayCount, Tenor
    >>> from finstack_quant.valuations.instruments import TermLoan
    >>> loan = (
    ...     TermLoan
    ...     .builder()
    ...     .id("TL-1")
    ...     .currency("USD")
    ...     .notional_limit(10_000_000.0, currency="USD")
    ...     .issue_date("2024-01-01")
    ...     .maturity("2029-01-01")
    ...     .rate(0.06)
    ...     .frequency(Tenor.quarterly())
    ...     .day_count(DayCount.ACT_360)
    ...     .discount_curve_id("USD-OIS")
    ...     .amortization({"percent_per_period": {"bp": 250}})
    ...     .build()
    ... )
    >>> loan.rate
    {'fixed': {'rate_bp': 600}}
    """

    def id(self, value: str) -> TermLoanBuilder:
        """
        Set the instrument identifier.

        Parameters
        ----------
        value : str
            Instrument identifier.

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def currency(self, value: str) -> TermLoanBuilder:
        """
        Set the loan currency.

        Parameters
        ----------
        value : str
            ISO-4217 currency code (e.g. ``"USD"``).

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized currency code.
        """
        ...
    def notional_limit(self, value: Money | float, currency: str | None = None) -> TermLoanBuilder:
        """
        Set the committed notional (facility limit).

        Parameters
        ----------
        value : Money | float
            Committed notional (facility limit); a bare number is tagged with ``currency``.
        currency : str, optional
            ISO-4217 code applied when ``value`` is a bare number.

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or a bare number is given without ``currency``.
        """
        ...
    def issue_date(self, value: datetime.date | datetime.datetime | pd.Timestamp | str) -> TermLoanBuilder:
        """
        Set the issue / funding date (defaults to ``maturity - 365 days`` when unset).

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pd.Timestamp | str
            Issue / funding date (defaults to ``maturity - 365 days`` when unset) (ISO 8601 strings accepted).

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or the date cannot be interpreted.
        """
        ...
    def maturity(self, value: datetime.date | datetime.datetime | pd.Timestamp | str) -> TermLoanBuilder:
        """
        Set the maturity date.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pd.Timestamp | str
            Maturity date (ISO 8601 strings accepted).

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or the date cannot be interpreted.
        """
        ...
    def rate(self, value: float | Rate | dict[str, object] | str) -> TermLoanBuilder:
        """
        Set the interest rate specification.

        Parameters
        ----------
        value : float | Rate | dict[str, object] | str
            A bare decimal (``0.06`` = 6%) or ``Rate`` sets a fixed rate (mirrors
            Rust ``RateSpec::fixed_rate``; rounded to whole basis points). A
            ``dict`` / JSON ``str`` is the Rust ``RateSpec`` in serde form, e.g.
            ``{"floating": {"index_id": "USD-SOFR-3M", "spread_bp": 400, ...}}``.

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed or a dict/str does not
            deserialize as a ``RateSpec``.
        TypeError
            If ``value`` has an unsupported type.
        """
        ...
    def frequency(self, value: Tenor) -> TermLoanBuilder:
        """
        Set the payment frequency.

        Parameters
        ----------
        value : Tenor
            Payment frequency (e.g. ``Tenor.quarterly()``).

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def day_count(self, value: DayCount) -> TermLoanBuilder:
        """
        Set the accrual day-count convention.

        Parameters
        ----------
        value : DayCount
            Day count convention (e.g. ``DayCount.ACT_360``).

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def business_day_convention(self, value: str) -> TermLoanBuilder:
        """
        Set the business day convention.

        Parameters
        ----------
        value : str
            Business day convention (serde string). Default ``"modified_following"``.

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized name.
        """
        ...
    def calendar_id(self, value: str) -> TermLoanBuilder:
        """
        Set the holiday calendar identifier (e.g. ``"usny"``).

        Parameters
        ----------
        value : str
            Holiday calendar identifier (e.g. ``"usny"``).

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def stub(
        self, value: StubKind | Literal["none", "short_front", "long_front", "short_back", "long_back"]
    ) -> TermLoanBuilder:
        """
        Set the stub rule.

        Parameters
        ----------
        value : StubKind | Literal["none", "short_front", "long_front", "short_back", "long_back"]
            Stub rule (serde string). Default ``"short_front"``.

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized name.
        """
        ...
    def discount_curve_id(self, value: str) -> TermLoanBuilder:
        """
        Set the discount curve identifier.

        Parameters
        ----------
        value : str
            Discount curve identifier.

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def credit_curve_id(self, value: str) -> TermLoanBuilder:
        """
        Set the hazard curve identifier for credit-risky pricing.

        Parameters
        ----------
        value : str
            Hazard curve identifier for credit-risky pricing.

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def amortization(self, value: dict[str, object] | str) -> TermLoanBuilder:
        """
        Set the amortization schedule.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``AmortizationSpec`` in serde form (``dict`` or JSON string), e.g. ``"none"``, ``{"percent_per_period": {"bp": 250}}`` or ``{"linear": {"start": "2025-01-01", "end": "2029-01-01"}}``.

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does not deserialize as a ``AmortizationSpec``.
        """
        ...
    def coupon_type(self, value: str) -> TermLoanBuilder:
        """
        Set the coupon type.

        Parameters
        ----------
        value : str
            Coupon type (serde string). ``"cash"`` (default), ``"pik"``, ...

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized name.
        """
        ...
    def upfront_fee(self, value: Money | float, currency: str | None = None) -> TermLoanBuilder:
        """
        Set the upfront fee.

        Parameters
        ----------
        value : Money | float
            Upfront fee; a bare number is tagged with ``currency``.
        currency : str, optional
            ISO-4217 code applied when ``value`` is a bare number.

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or a bare number is given without ``currency``.
        """
        ...
    def ddtl(self, value: dict[str, object] | str) -> TermLoanBuilder:
        """
        Set the delayed-draw (DDTL) specification.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``DdtlSpec`` in serde form (``dict`` or JSON string), e.g. ``TermLoan.example_floating_with_ddtl().ddtl``.

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does not deserialize as a ``DdtlSpec``.
        """
        ...
    def covenants(self, value: dict[str, object] | str) -> TermLoanBuilder:
        """
        Set the covenant event schedule.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``TermLoanCovenantEvents`` in serde form (``dict`` or JSON string), e.g. ``{"events": [...]}``.

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does not deserialize as a ``TermLoanCovenantEvents``.
        """
        ...
    def oid_eir(self, value: dict[str, object] | str) -> TermLoanBuilder:
        """
        Set the OID / effective-interest-rate specification.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``OidEirSpec`` in serde form (``dict`` or JSON string), e.g. ``{"issue_price_pct": 99.0, ...}``.

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does not deserialize as a ``OidEirSpec``.
        """
        ...
    def call_schedule(self, value: dict[str, object] | str) -> TermLoanBuilder:
        """
        Set the prepayment (call) schedule.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``LoanCallSchedule`` in serde form (``dict`` or JSON string), e.g. ``TermLoan.example_callable().call_schedule``.

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does not deserialize as a ``LoanCallSchedule``.
        """
        ...
    def settlement_days(self, value: int) -> TermLoanBuilder:
        """
        Set the settlement lag in business days (default 2).

        Parameters
        ----------
        value : int
            Settlement lag.

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def attributes(self, value: Attributes | dict[str, str]) -> TermLoanBuilder:
        """
        Set instrument attributes (tags and metadata).

        Parameters
        ----------
        value : Attributes | dict[str, str]
            Attribute bag; a ``dict`` populates ``meta`` and an optional
            ``"tags"`` entry holding a list of strings populates ``tags``.

        Returns
        -------
        TermLoanBuilder
            ``self``, for chaining.

        Raises
        ------
        TypeError
            If ``value`` is neither ``Attributes`` nor a ``dict``.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style rendering of the fields set so far, e.g.
        ``TermLoanBuilder(id='X', notional=Money(1000000.0, 'USD'))``.

        Returns
        -------
        str
            ``TermLoanBuilder(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise; it renders the recorded setter calls.
        """
        ...
    def build(self) -> TermLoan:
        """
        Build the validated term loan.

        Runs the same validation as the Rust ``TermLoanBuilder::build`` (structural
        invariants only); pricing-time checks run in ``TermLoan.price``.

        Returns
        -------
        TermLoan
            The validated instrument.

        Raises
        ------
        ValueError
            If the builder was already consumed, a required field is missing
            (the message names the builder and the field, e.g.
            ``TermLoanBuilder: missing required field 'id'``), or the instrument
            fails validation.
        """
        ...

class FixedLegSpec:
    """
    Fixed leg of an interest-rate swap.

    Thin typed wrapper for the canonical Rust ``FixedLegSpec``. Used to build
    :class:`InterestRateSwap` and :class:`Swaption` instruments. Immutable:
    every constructor argument is readable back through a property of the
    same name; ``to_json`` / ``from_json`` (and therefore ``pickle``) round-trip
    the wire form.

    Examples
    --------
    >>> from finstack_quant.core.dates import DayCount, Tenor
    >>> from finstack_quant.valuations.instruments import FixedLegSpec
    >>> leg = FixedLegSpec(
    ...     "USD-OIS",
    ...     0.04,
    ...     Tenor.semi_annual(),
    ...     DayCount.THIRTY_360,
    ...     "2024-01-15",
    ...     "2029-01-15",
    ...     compounding_simple=False,
    ... )
    >>> leg.rate
    0.04
    """

    def __init__(
        self,
        discount_curve_id: str,
        rate: float | Rate,
        frequency: Tenor,
        day_count: DayCount,
        start: datetime.date | datetime.datetime | pd.Timestamp | str,
        end: datetime.date | datetime.datetime | pd.Timestamp | str,
        *,
        compounding_simple: bool,
        business_day_convention: str = "modified_following",
        calendar_id: str | None = None,
        stub: StubKind | Literal["none", "short_front", "long_front", "short_back", "long_back"] = "short_front",
        par_method: Literal["forward_based", "discount_ratio"] | None = None,
        payment_lag_days: int = 0,
        end_of_month: bool = False,
    ) -> None:
        """
        Create a fixed leg.

        Parameters
        ----------
        discount_curve_id : str
            Discount curve identifier for pricing this leg.
        rate : float | Rate
            Fixed rate as a decimal (``0.04`` = 4%) or a ``Rate``.
        frequency : Tenor
            Payment frequency.
        day_count : DayCount
            Day count convention for accrual.
        start : datetime.date | datetime.datetime | pd.Timestamp | str
            Start date of the fixed leg (ISO 8601 strings accepted).
        end : datetime.date | datetime.datetime | pd.Timestamp | str
            End date of the fixed leg.
        compounding_simple : bool
            If true, use simple interest on the accrual fraction. Required:
            the canonical Rust ``FixedLegSpec`` field has no default.
        business_day_convention : str, default "modified_following"
            Business day convention for payment dates.
        calendar_id : str, optional
            Calendar used for business day adjustments.
        stub : StubKind | str, default "short_front"
            Stub period handling rule.
        par_method : str, optional
            Par-rate method override (``"forward_based"`` or
            ``"discount_ratio"``); ``None`` keeps the pricer default.
        payment_lag_days : int, default 0
            Payment lag in business days after period end.
        end_of_month : bool, default False
            End-of-month roll convention.

        Raises
        ------
        ValueError
            If an enum value is invalid, ``rate`` is not finite, or the
            accrual period is malformed (``start >= end``).
        TypeError
            If ``rate`` is neither a number nor a ``Rate`` or a date cannot
            be interpreted.

        Examples
        --------
        >>> from finstack_quant.core.dates import DayCount, Tenor
        >>> from finstack_quant.valuations.instruments import FixedLegSpec
        >>> leg = FixedLegSpec(
        ...     "USD-OIS",
        ...     0.04,
        ...     Tenor.semi_annual(),
        ...     DayCount.THIRTY_360,
        ...     "2024-01-15",
        ...     "2029-01-15",
        ...     compounding_simple=False,
        ... )
        >>> leg.compounding_simple
        False
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> FixedLegSpec:
        """
        Deserialize a fixed-leg spec from its serde JSON object.

        Parameters
        ----------
        json : str
            JSON object with the same fields as the Rust ``FixedLegSpec`` (the value
            ``FixedLegSpec.to_json`` returns).

        Returns
        -------
        FixedLegSpec
            The validated leg.

        Raises
        ------
        ValueError
            If the JSON is malformed, has unknown fields, or fails validation.

        Examples
        --------
        >>> from finstack_quant.core.dates import DayCount, Tenor
        >>> from finstack_quant.valuations.instruments import FixedLegSpec
        >>> leg = FixedLegSpec(
        ...     "USD-OIS",
        ...     0.04,
        ...     Tenor.semi_annual(),
        ...     DayCount.THIRTY_360,
        ...     "2024-01-15",
        ...     "2029-01-15",
        ...     compounding_simple=False,
        ... )
        >>> FixedLegSpec.from_json(leg.to_json()).rate
        0.04
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to the serde JSON object accepted by :meth:`FixedLegSpec.from_json`.

        Returns
        -------
        str
            JSON object (a leg is not an instrument envelope).

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def to_dict(self) -> dict[str, object]:
        """
        Serde form of the fixed-leg spec as a plain Python ``dict``.

        Returns
        -------
        dict[str, object]
            JSON-compatible dict with one key per Rust field.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style rendering of every field, e.g.
        ``FixedLegSpec(discount_curve_id='USD-OIS', ...)``.

        Returns
        -------
        str
            ``FixedLegSpec(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise.
        """
        ...
    @property
    def discount_curve_id(self) -> str:
        """
        Discount curve identifier.

        Returns
        -------
        str
            Curve id used to discount this leg.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def frequency(self) -> Tenor:
        """
        Payment frequency.

        Returns
        -------
        Tenor
            The payment tenor.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def day_count(self) -> DayCount:
        """
        Accrual day-count convention.

        Returns
        -------
        DayCount
            The day count.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def business_day_convention(self) -> str:
        """
        Business day convention (serde string).

        Returns
        -------
        str
            ``"modified_following"`` unless overridden.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def calendar_id(self) -> str | None:
        """
        Payment calendar identifier.

        Returns
        -------
        str | None
            Calendar id, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def stub(self) -> StubKind:
        """
        Stub-period handling rule for the schedule.

        Returns
        -------
        StubKind
            The ``StubKind`` variant (``NONE``, ``SHORT_FRONT``, ...).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def start(self) -> datetime.date:
        """
        Accrual start date.

        Returns
        -------
        datetime.date
            The start date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def end(self) -> datetime.date:
        """
        Accrual end date.

        Returns
        -------
        datetime.date
            The end date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def rate(self) -> float:
        """
        Fixed rate as a decimal.

        Returns
        -------
        float
            ``0.04`` for 4%.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def par_method(self) -> str | None:
        """
        Par-rate method override.

        Returns
        -------
        str | None
            ``"forward_based"``, ``"discount_ratio"`` or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def compounding_simple(self) -> bool:
        """
        Whether simple interest is used on the accrual fraction.

        Returns
        -------
        bool
            ``True`` for simple accrual, ``False`` for compounded accrual.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def payment_lag_days(self) -> int:
        """
        Payment lag in business days after period end.

        Returns
        -------
        int
            Number of business days between accrual end and payment (``0`` = pay on period end).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def end_of_month(self) -> bool:
        """
        End-of-month roll convention flag.

        Returns
        -------
        bool
            ``True`` when roll dates stick to month ends, ``False`` otherwise.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class FloatLegSpec:
    """
    Floating leg of an interest-rate swap.

    Thin typed wrapper for the canonical Rust ``FloatLegSpec``. Used to build
    :class:`InterestRateSwap` and :class:`Swaption` instruments. Immutable:
    every constructor argument is readable back through a property of the
    same name; ``to_json`` / ``from_json`` (and therefore ``pickle``) round-trip
    the wire form. ``reset_lag_days`` defaults to ``0`` (fixing on the accrual
    start), so a swap whose first period starts on or after the valuation date
    prices off the forward curve without historical fixings.

    Examples
    --------
    >>> from finstack_quant.core.dates import DayCount, Tenor
    >>> from finstack_quant.valuations.instruments import FloatLegSpec
    >>> leg = FloatLegSpec(
    ...     "USD-OIS",
    ...     "USD-SOFR-3M",
    ...     0.0,
    ...     Tenor.quarterly(),
    ...     DayCount.ACT_360,
    ...     "2024-01-15",
    ...     "2029-01-15",
    ... )
    >>> (leg.reset_lag_days, leg.compounding)
    (0, 'simple')
    """

    def __init__(
        self,
        discount_curve_id: str,
        forward_curve_id: str,
        spread_bp: float | Bps,
        frequency: Tenor,
        day_count: DayCount,
        start: datetime.date | datetime.datetime | pd.Timestamp | str,
        end: datetime.date | datetime.datetime | pd.Timestamp | str,
        *,
        business_day_convention: str = "modified_following",
        calendar_id: str | None = None,
        stub: StubKind | Literal["none", "short_front", "long_front", "short_back", "long_back"] = "short_front",
        reset_lag_days: int = 0,
        fixing_calendar_id: str | None = None,
        compounding: Literal["simple"] | dict[str, dict[str, int]] | None = None,
        payment_lag_days: int = 0,
        end_of_month: bool = False,
    ) -> None:
        """
        Create a floating leg.

        Parameters
        ----------
        discount_curve_id : str
            Discount curve identifier for pricing this leg.
        forward_curve_id : str
            Forward curve identifier for rate projections.
        spread_bp : float | Bps
            Spread over the index in basis points (``25.0`` = 25bp) or a
            ``Bps``.
        frequency : Tenor
            Payment frequency.
        day_count : DayCount
            Day count convention for accrual.
        start : datetime.date | datetime.datetime | pd.Timestamp | str
            Start date of the floating leg (ISO 8601 strings accepted).
        end : datetime.date | datetime.datetime | pd.Timestamp | str
            End date of the floating leg.
        business_day_convention : str, default "modified_following"
            Business day convention for payment dates.
        calendar_id : str, optional
            Calendar used for business day adjustments.
        stub : StubKind | str, default "short_front"
            Stub period handling rule.
        reset_lag_days : int, default 0
            Reset lag in business days before each accrual start. ``0``
            (the Rust default) fixes on the accrual start date; use ``2``
            for a T-2 term index. :meth:`InterestRateSwap.from_conventions`
            applies the registered market default for an index. A fixing
            date before the valuation date requires a
            ``ScalarTimeSeries("FIXING:<forward_curve_id>", ...)`` in the
            market context.
        fixing_calendar_id : str, optional
            Calendar used for rate fixing (reset lag).
        compounding : str | dict, optional
            Coupon compounding; ``None`` means ``"simple"`` (term indices).
            Overnight RFR legs pass a struct variant such as
            ``{"compounded_in_arrears": {"lookback_days": 0}}``,
            ``{"compounded_with_observation_shift": {"shift_days": 0}}`` or
            ``{"compounded_with_rate_cutoff": {"cutoff_days": 0}}``.
        payment_lag_days : int, default 0
            Payment lag in business days after period end.
        end_of_month : bool, default False
            End-of-month roll convention.

        Raises
        ------
        ValueError
            If an enum value is invalid, ``compounding`` does not name a
            variant, ``spread_bp`` is not finite, or the accrual period is
            malformed (``start >= end``).
        TypeError
            If ``spread_bp`` is neither a number nor a ``Bps`` or a date
            cannot be interpreted.

        Examples
        --------
        >>> from finstack_quant.core.dates import DayCount, Tenor
        >>> from finstack_quant.valuations.instruments import FloatLegSpec
        >>> ois = FloatLegSpec(
        ...     "USD-OIS",
        ...     "USD-SOFR",
        ...     0.0,
        ...     Tenor.annual(),
        ...     DayCount.ACT_360,
        ...     "2024-01-15",
        ...     "2029-01-15",
        ...     compounding={"compounded_in_arrears": {"lookback_days": 0}},
        ... )
        >>> ois.compounding
        {'compounded_in_arrears': {'lookback_days': 0}}
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> FloatLegSpec:
        """
        Deserialize a floating-leg spec from its serde JSON object.

        Parameters
        ----------
        json : str
            JSON object with the same fields as the Rust ``FloatLegSpec`` (the value
            ``FloatLegSpec.to_json`` returns).

        Returns
        -------
        FloatLegSpec
            The validated leg.

        Raises
        ------
        ValueError
            If the JSON is malformed, has unknown fields, or fails validation.

        Examples
        --------
        >>> from finstack_quant.core.dates import DayCount, Tenor
        >>> from finstack_quant.valuations.instruments import FloatLegSpec
        >>> leg = FloatLegSpec(
        ...     "USD-OIS",
        ...     "USD-SOFR-3M",
        ...     0.0,
        ...     Tenor.quarterly(),
        ...     DayCount.ACT_360,
        ...     "2024-01-15",
        ...     "2029-01-15",
        ... )
        >>> FloatLegSpec.from_json(leg.to_json()).forward_curve_id
        'USD-SOFR-3M'
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to the serde JSON object accepted by :meth:`FloatLegSpec.from_json`.

        Returns
        -------
        str
            JSON object (a leg is not an instrument envelope).

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def to_dict(self) -> dict[str, object]:
        """
        Serde form of the floating-leg spec as a plain Python ``dict``.

        Returns
        -------
        dict[str, object]
            JSON-compatible dict with one key per Rust field.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style rendering of every field, e.g.
        ``FloatLegSpec(discount_curve_id='USD-OIS', ...)``.

        Returns
        -------
        str
            ``FloatLegSpec(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise.
        """
        ...
    @property
    def discount_curve_id(self) -> str:
        """
        Discount curve identifier.

        Returns
        -------
        str
            Curve id used to discount this leg.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def frequency(self) -> Tenor:
        """
        Payment frequency.

        Returns
        -------
        Tenor
            The payment tenor.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def day_count(self) -> DayCount:
        """
        Accrual day-count convention.

        Returns
        -------
        DayCount
            The day count.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def business_day_convention(self) -> str:
        """
        Business day convention (serde string).

        Returns
        -------
        str
            ``"modified_following"`` unless overridden.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def calendar_id(self) -> str | None:
        """
        Payment calendar identifier.

        Returns
        -------
        str | None
            Calendar id, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def stub(self) -> StubKind:
        """
        Stub-period handling rule for the schedule.

        Returns
        -------
        StubKind
            The ``StubKind`` variant (``NONE``, ``SHORT_FRONT``, ...).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def start(self) -> datetime.date:
        """
        Accrual start date.

        Returns
        -------
        datetime.date
            The start date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def end(self) -> datetime.date:
        """
        Accrual end date.

        Returns
        -------
        datetime.date
            The end date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def forward_curve_id(self) -> str:
        """
        Forward (projection) curve identifier.

        Returns
        -------
        str
            Curve id used to project the index.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def spread_bp(self) -> float:
        """
        Spread over the index in basis points.

        Returns
        -------
        float
            ``25.0`` for 25bp.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def reset_lag_days(self) -> int:
        """
        Reset lag in business days before each accrual start.

        Returns
        -------
        int
            ``0`` fixes on the accrual start.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def fixing_calendar_id(self) -> str | None:
        """
        Fixing calendar identifier.

        Returns
        -------
        str | None
            Calendar id, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def compounding(self) -> str | dict[str, dict[str, int]]:
        """
        Coupon compounding in serde form.

        Returns
        -------
        str | dict[str, dict[str, int]]
            ``"simple"`` or a one-key dict for the compounded variants.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def payment_lag_days(self) -> int:
        """
        Payment lag in business days after period end.

        Returns
        -------
        int
            Number of business days between accrual end and payment (``0`` = pay on period end).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def end_of_month(self) -> bool:
        """
        End-of-month roll convention flag.

        Returns
        -------
        bool
            ``True`` when roll dates stick to month ends, ``False`` otherwise.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class PremiumLegSpec:
    """
    Premium (fixed coupon) leg of a CDS or CDS index.

    Thin typed wrapper for the canonical Rust ``PremiumLegSpec``. Used by
    :class:`CreditDefaultSwap` and :class:`CDSIndex` builders. Immutable;
    fields are readable through properties and ``to_json`` / ``from_json``
    (and therefore ``pickle``) round-trip the wire form.

    Examples
    --------
    >>> from finstack_quant.core.dates import DayCount, Tenor
    >>> from finstack_quant.valuations.instruments import PremiumLegSpec
    >>> leg = PremiumLegSpec(
    ...     "2024-03-20",
    ...     "2029-06-20",
    ...     Tenor.quarterly(),
    ...     DayCount.ACT_360,
    ...     100.0,
    ...     "USD-OIS",
    ... )
    >>> leg.spread_bp
    100.0
    """

    def __init__(
        self,
        start: datetime.date | datetime.datetime | pd.Timestamp | str,
        end: datetime.date | datetime.datetime | pd.Timestamp | str,
        frequency: Tenor,
        day_count: DayCount,
        spread_bp: float | Bps,
        discount_curve_id: str,
        *,
        stub: StubKind | Literal["none", "short_front", "long_front", "short_back", "long_back"] = "short_front",
        business_day_convention: str = "modified_following",
        calendar_id: str | None = None,
    ) -> None:
        """
        Create a premium leg.

        Parameters
        ----------
        start : datetime.date | datetime.datetime | pd.Timestamp | str
            Start date of protection / premium accrual (ISO strings accepted).
        end : datetime.date | datetime.datetime | pd.Timestamp | str
            End date of protection / premium accrual.
        frequency : Tenor
            Payment frequency.
        day_count : DayCount
            Day count convention for accrual.
        spread_bp : float | Bps
            Fixed running spread in basis points (``100.0`` = 100bp = 1%).
        discount_curve_id : str
            Discount curve identifier for pricing this leg.
        stub : StubKind | str, default "short_front"
            Stub period handling rule.
        business_day_convention : str, default "modified_following"
            Business day convention for payment dates.
        calendar_id : str, optional
            Calendar used for business day adjustments.

        Raises
        ------
        ValueError
            If an enum value is invalid or ``spread_bp`` is not finite.
        TypeError
            If ``spread_bp`` is neither a number nor a ``Bps`` or a date
            cannot be interpreted.

        Examples
        --------
        >>> from finstack_quant.core.dates import DayCount, Tenor
        >>> from finstack_quant.valuations.instruments import PremiumLegSpec
        >>> leg = PremiumLegSpec(
        ...     "2024-03-20",
        ...     "2029-06-20",
        ...     Tenor.quarterly(),
        ...     DayCount.ACT_360,
        ...     100.0,
        ...     "USD-OIS",
        ... )
        >>> leg.discount_curve_id
        'USD-OIS'
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> PremiumLegSpec:
        """
        Deserialize a premium-leg spec from its serde JSON object.

        Parameters
        ----------
        json : str
            JSON object with the same fields as the Rust ``PremiumLegSpec`` (the value
            ``PremiumLegSpec.to_json`` returns).

        Returns
        -------
        PremiumLegSpec
            The validated leg.

        Raises
        ------
        ValueError
            If the JSON is malformed, has unknown fields, or fails validation.

        Examples
        --------
        >>> from finstack_quant.core.dates import DayCount, Tenor
        >>> from finstack_quant.valuations.instruments import PremiumLegSpec
        >>> leg = PremiumLegSpec(
        ...     "2024-03-20",
        ...     "2029-06-20",
        ...     Tenor.quarterly(),
        ...     DayCount.ACT_360,
        ...     100.0,
        ...     "USD-OIS",
        ... )
        >>> PremiumLegSpec.from_json(leg.to_json()).spread_bp
        100.0
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to the serde JSON object accepted by :meth:`PremiumLegSpec.from_json`.

        Returns
        -------
        str
            JSON object (a leg is not an instrument envelope).

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def to_dict(self) -> dict[str, object]:
        """
        Serde form of the premium-leg spec as a plain Python ``dict``.

        Returns
        -------
        dict[str, object]
            JSON-compatible dict with one key per Rust field.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style rendering of every field, e.g.
        ``PremiumLegSpec(discount_curve_id='USD-OIS', ...)``.

        Returns
        -------
        str
            ``PremiumLegSpec(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise.
        """
        ...
    @property
    def discount_curve_id(self) -> str:
        """
        Discount curve identifier.

        Returns
        -------
        str
            Curve id used to discount this leg.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def frequency(self) -> Tenor:
        """
        Payment frequency.

        Returns
        -------
        Tenor
            The payment tenor.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def day_count(self) -> DayCount:
        """
        Accrual day-count convention.

        Returns
        -------
        DayCount
            The day count.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def business_day_convention(self) -> str:
        """
        Business day convention (serde string).

        Returns
        -------
        str
            ``"modified_following"`` unless overridden.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def calendar_id(self) -> str | None:
        """
        Payment calendar identifier.

        Returns
        -------
        str | None
            Calendar id, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def stub(self) -> StubKind:
        """
        Stub-period handling rule for the schedule.

        Returns
        -------
        StubKind
            The ``StubKind`` variant (``NONE``, ``SHORT_FRONT``, ...).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def start(self) -> datetime.date:
        """
        Accrual start date.

        Returns
        -------
        datetime.date
            The start date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def end(self) -> datetime.date:
        """
        Accrual end date.

        Returns
        -------
        datetime.date
            The end date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def spread_bp(self) -> float:
        """
        Running spread in basis points.

        Returns
        -------
        float
            ``100.0`` for 100bp.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class ProtectionLegSpec:
    """
    Protection (default-contingent) leg of a CDS or CDS index.

    Thin typed wrapper for the canonical Rust ``ProtectionLegSpec``. Used by
    :class:`CreditDefaultSwap` and :class:`CDSIndex` builders. Immutable;
    fields are readable through properties and ``to_json`` / ``from_json``
    (and therefore ``pickle``) round-trip the wire form.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import ProtectionLegSpec
    >>> leg = ProtectionLegSpec("ACME-CDS", 0.4, 3)
    >>> leg.recovery_rate
    0.4
    """

    def __init__(self, credit_curve_id: str, recovery_rate: float, settlement_delay: int = 3) -> None:
        """
        Create a protection leg.

        Parameters
        ----------
        credit_curve_id : str
            Hazard/credit curve identifier for default probabilities.
        recovery_rate : float
            Recovery rate in ``[0.0, 1.0]`` (e.g. 0.4 = 40%).
        settlement_delay : int, default 3
            Settlement delay in business days.

        Raises
        ------
        ValueError
            If ``recovery_rate`` is outside ``[0.0, 1.0]``.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import ProtectionLegSpec
        >>> leg = ProtectionLegSpec("ACME-CDS", 0.4, 3)
        >>> leg.settlement_delay
        3
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> ProtectionLegSpec:
        """
        Deserialize a protection-leg spec from its serde JSON object.

        Parameters
        ----------
        json : str
            JSON object with the same fields as the Rust ``ProtectionLegSpec`` (the value
            ``ProtectionLegSpec.to_json`` returns).

        Returns
        -------
        ProtectionLegSpec
            The validated leg.

        Raises
        ------
        ValueError
            If the JSON is malformed, has unknown fields, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import ProtectionLegSpec
        >>> leg = ProtectionLegSpec("ACME-CDS", 0.4, 3)
        >>> ProtectionLegSpec.from_json(leg.to_json()).credit_curve_id
        'ACME-CDS'
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to the serde JSON object accepted by :meth:`ProtectionLegSpec.from_json`.

        Returns
        -------
        str
            JSON object (a leg is not an instrument envelope).

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def to_dict(self) -> dict[str, object]:
        """
        Serde form of the protection-leg spec as a plain Python ``dict``.

        Returns
        -------
        dict[str, object]
            JSON-compatible dict with one key per Rust field.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style rendering of every field, e.g.
        ``ProtectionLegSpec(discount_curve_id='USD-OIS', ...)``.

        Returns
        -------
        str
            ``ProtectionLegSpec(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise.
        """
        ...
    @property
    def credit_curve_id(self) -> str:
        """
        Hazard / credit curve identifier.

        Returns
        -------
        str
            Curve id used for survival probabilities.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def recovery_rate(self) -> float:
        """
        Recovery rate as a decimal.

        Returns
        -------
        float
            Value in ``[0.0, 1.0]``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def settlement_delay(self) -> int:
        """
        Settlement delay in business days after a credit event.

        Returns
        -------
        int
            Number of business days between default and protection settlement.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class InterestRateSwap:
    """
    Typed wrapper for the canonical Rust ``InterestRateSwap`` instrument.

    Construct via :meth:`InterestRateSwap.from_conventions` (market
    conventions resolved from the rate-index registry, the preferred way
    to build standard swaps), :meth:`InterestRateSwap.builder` with explicit
    :class:`FixedLegSpec` / :class:`FloatLegSpec` legs,
    :meth:`InterestRateSwap.example_standard` or
    :meth:`InterestRateSwap.from_json`. Every public Rust field is readable
    as a property; :meth:`InterestRateSwap.price` /
    :meth:`InterestRateSwap.metric` run the same pricer as
    :func:`price_instrument`.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import InterestRateSwap
    >>> swap = InterestRateSwap.from_conventions(
    ...     "IRS-5Y",
    ...     10_000_000.0,
    ...     "pay",
    ...     0.035,
    ...     "2025-01-15",
    ...     "2030-01-15",
    ...     "USD-SOFR",
    ...     "USD-OIS",
    ...     "USD-SOFR",
    ...     currency="USD",
    ... )
    >>> (swap.side, swap.float.reset_lag_days)
    ('pay', 0)
    """

    @staticmethod
    def builder() -> InterestRateSwapBuilder:
        """
        Create a fluent builder (mirrors Rust ``InterestRateSwap::builder()``).

        Returns
        -------
        InterestRateSwapBuilder
            A builder with fluent, consuming setter methods.

        Notes
        -----
        This factory does not raise; it returns an empty builder.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import InterestRateSwap
        >>> builder = InterestRateSwap.builder()
        >>> builder.id("EXAMPLE") is builder
        True
        """
        ...
    @staticmethod
    def from_conventions(
        id: str,
        notional: Money | float,
        side: Literal["pay", "receive"],
        fixed_rate: float | Rate,
        start: datetime.date | datetime.datetime | pd.Timestamp | str,
        end: datetime.date | datetime.datetime | pd.Timestamp | str,
        index_id: str,
        discount_curve_id: str,
        forward_curve_id: str,
        *,
        currency: str | None = None,
    ) -> InterestRateSwap:
        """
        Create a vanilla swap from registered rate-index conventions.

        Mirrors Rust ``InterestRateSwap::from_conventions`` (QuantLib
        ``MakeVanillaSwap`` ergonomics): day counts, frequencies, calendars,
        reset/payment lags and overnight compounding are resolved from the
        convention registry entry for ``index_id``.

        Parameters
        ----------
        id : str
            Unique instrument identifier.
        notional : Money | float
            Notional shared by both legs; a bare number is tagged with ``currency``.
        side : str
            ``"pay"`` pays fixed / receives floating; ``"receive"`` the opposite.
        fixed_rate : float | Rate
            Fixed coupon as a decimal (``0.03`` = 3%) or a ``Rate``.
        start : datetime.date | datetime.datetime | pd.Timestamp | str
            Effective date (ISO 8601 strings accepted).
        end : datetime.date | datetime.datetime | pd.Timestamp | str
            Maturity date.
        index_id : str
            Registered rate index (e.g. ``"USD-SOFR"``, ``"USD-SOFR-3M"``,
            ``"EUR-EURIBOR-6M"``).
        discount_curve_id : str
            Discount curve identifier for both legs.
        forward_curve_id : str
            Projection curve identifier for the floating leg.
        currency : str, optional
            ISO-4217 code applied when ``notional`` is a bare number.

        Returns
        -------
        InterestRateSwap
            The validated swap.

        Raises
        ------
        ValueError
            If ``side`` is unknown, ``index_id`` is not registered, a bare
            ``notional`` has no ``currency``, or validation fails.
        TypeError
            If ``fixed_rate``/``notional`` has an unsupported type or a date cannot
            be interpreted.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import InterestRateSwap
        >>> swap = InterestRateSwap.from_conventions(
        ...     "IRS-5Y",
        ...     10_000_000.0,
        ...     "pay",
        ...     0.035,
        ...     "2025-01-15",
        ...     "2030-01-15",
        ...     "USD-SOFR",
        ...     "USD-OIS",
        ...     "USD-SOFR",
        ...     currency="USD",
        ... )
        >>> (swap.side, swap.float.reset_lag_days)
        ('pay', 0)
        """
        ...
    @staticmethod
    def example_standard() -> InterestRateSwap:
        """
        Canonical 5-year USD pay-fixed swap (mirrors Rust
        ``InterestRateSwap::example_standard``): semi-annual 30/360 fixed vs
        quarterly ACT/360 ``USD-SOFR-3M``, T-2 reset lag, ``usny`` calendar.

        Returns
        -------
        InterestRateSwap
            The example swap.

        Raises
        ------
        ValueError
            If construction fails (should not occur).

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import InterestRateSwap
        >>> InterestRateSwap.example_standard().float.reset_lag_days
        2
        """
        ...
    @classmethod
    def from_json(cls, json: str) -> InterestRateSwap:
        """
        Deserialize a validated InterestRateSwap from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"interest_rate_swap"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        InterestRateSwap
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import InterestRateSwap
        >>> swap = InterestRateSwap.example_standard()
        >>> InterestRateSwap.from_json(swap.to_json()).id == swap.id
        True
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to a canonical ``finstack_quant.instrument/1`` envelope.

        Returns
        -------
        str
            Canonical instrument envelope accepted by :func:`price_instrument`
            and :meth:`InterestRateSwap.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def to_dict(self) -> dict[str, object]:
        """
        Serde form of the swap spec as a plain Python ``dict``.

        Returns
        -------
        dict[str, object]
            The ``spec`` object of the instrument envelope (JSON-compatible
            values: ``str``, ``float``, ``dict``, ``list``, ``None``).

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def price(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        model: str = "default",
        metrics: list[str] | None = None,
        pricing_options: dict[str, object] | str | None = None,
        market_history: str | None = None,
    ) -> ValuationResult:
        """
        Price this swap and return a :class:`~finstack_quant.valuations.ValuationResult`.

        Same pipeline and keyword surface as :func:`price_instrument`.

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date (ISO 8601 strings accepted).
        model : str, default "default"
            Model key (``"discounting"``, ``"hull_white_1f"``, ...).
        metrics : list[str], optional
            Metric identifiers to compute (see :func:`list_standard_metrics`).
        pricing_options : dict[str, object] | str, optional
            ``MetricPricingOverrides`` merged into the instrument's own overrides
            (e.g. ``{"theta_period": "1D"}``).
        market_history : str, optional
            JSON ``MarketHistory`` scenarios required by ``hvar`` /
            ``expected_shortfall``.

        Returns
        -------
        ValuationResult
            Typed valuation envelope with price, currency and metrics.

        Raises
        ------
        ValueError
            If an input cannot be interpreted or the instrument fails validation.
            A seasoned floating period whose fixing date precedes ``as_of`` needs a
            ``ScalarTimeSeries("FIXING:<forward_curve_id>", ...)`` in ``market``; the
            message names the series id.
        KeyError
            If a required curve, surface or metric is missing from ``market``.
        RuntimeError
            If pricing or a metric computation fails.
        """
        ...
    def metric(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        metric_id: str,
        model: str = "default",
    ) -> float:
        """
        Compute one scalar metric for this swap (e.g. ``"dv01"``).

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.
        metric_id : str
            Registered metric identifier (see :func:`list_standard_metrics`).
        model : str, default "default"
            Model key.

        Returns
        -------
        float
            The metric value in the metric's native unit (decimal for rates and
            yields, currency units for DV01-style sensitivities, basis points for
            spreads).

        Raises
        ------
        ValueError
            If ``metric_id`` is unknown or an input cannot be interpreted.
        KeyError
            If a required curve or surface is missing from ``market``.
        RuntimeError
            If the metric computation fails.
        """
        ...
    def market_dependencies(self) -> dict[str, object]:
        """
        Market-data dependencies declared by the Rust ``Instrument`` trait.

        Returns
        -------
        dict[str, object]
            Serde form of ``MarketDependencies`` (``curves`` grouped by role,
            fixings, volatility surfaces, FX pairs).

        Raises
        ------
        ValueError
            If the instrument cannot enumerate its dependencies.
        """
        ...
    @property
    def id(self) -> str:
        """
        Stable instrument identifier used in market lookup and results.

        Returns
        -------
        str
            The unique instrument identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def notional(self) -> Money:
        """
        Notional shared by both legs.

        Returns
        -------
        Money
            Currency-tagged notional.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def side(self) -> str:
        """
        Swap direction for the fixed leg.

        Returns
        -------
        str
            ``"pay"`` or ``"receive"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def fixed(self) -> FixedLegSpec:
        """
        Fixed leg specification.

        Returns
        -------
        FixedLegSpec
            The fixed leg.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def float(self) -> FloatLegSpec:
        """
        Floating leg specification.

        Returns
        -------
        FloatLegSpec
            The floating leg.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def margin_spec(self) -> dict[str, object] | None:
        """
        OTC margin (CSA / initial-margin) specification in serde form.

        Returns
        -------
        dict[str, object] | None
            The spec dict, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def default_model(self) -> str:
        """
        Canonical model key used when ``model="default"`` is passed to ``price``.

        Returns
        -------
        str
            Registered model key such as ``"discounting"`` or ``"black76"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def attributes(self) -> Attributes:
        """
        Instrument attributes (tags and metadata) used for scenario selection.

        Returns
        -------
        Attributes
            The attribute bag; empty when none were set.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def expiry(self) -> datetime.date | None:
        """
        Expiry date exposed by the Rust ``Instrument`` trait.

        Returns
        -------
        datetime.date | None
            The expiry/maturity date, or ``None`` when the instrument type reports none.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class InterestRateSwapBuilder:
    """
    Fluent builder for :class:`InterestRateSwap`; wraps the Rust
    ``FinancialBuilder`` output one setter for one setter.

    Builders are consumed by ``build()``; create a new builder per
    instrument. Required fields: ``id``, ``notional``, ``side``, ``fixed``,
    ``float``.

    Examples
    --------
    >>> from finstack_quant.core.dates import DayCount, Tenor
    >>> from finstack_quant.valuations.instruments import FixedLegSpec, FloatLegSpec, InterestRateSwap
    >>> fixed = FixedLegSpec(
    ...     "USD-OIS",
    ...     0.04,
    ...     Tenor.semi_annual(),
    ...     DayCount.THIRTY_360,
    ...     "2025-01-15",
    ...     "2030-01-15",
    ...     compounding_simple=True,
    ... )
    >>> floating = FloatLegSpec(
    ...     "USD-OIS",
    ...     "USD-SOFR-3M",
    ...     0.0,
    ...     Tenor.quarterly(),
    ...     DayCount.ACT_360,
    ...     "2025-01-15",
    ...     "2030-01-15",
    ... )
    >>> swap = (
    ...     InterestRateSwap
    ...     .builder()
    ...     .id("IRS-1")
    ...     .notional(10_000_000.0, currency="USD")
    ...     .side("pay")
    ...     .fixed(fixed)
    ...     .float(floating)
    ...     .build()
    ... )
    >>> swap.notional.amount
    10000000.0
    """

    def id(self, value: str) -> InterestRateSwapBuilder:
        """
        Set the instrument identifier.

        Parameters
        ----------
        value : str
            Instrument identifier.

        Returns
        -------
        InterestRateSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def notional(self, value: Money | float, currency: str | None = None) -> InterestRateSwapBuilder:
        """
        Set the notional shared by both legs.

        Parameters
        ----------
        value : Money | float
            Notional shared by both legs; a bare number is tagged with ``currency``.
        currency : str, optional
            ISO-4217 code applied when ``value`` is a bare number.

        Returns
        -------
        InterestRateSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or a bare number is given without ``currency``.
        """
        ...
    def side(self, value: Literal["pay", "receive"]) -> InterestRateSwapBuilder:
        """
        Set the swap direction for the fixed leg.

        Parameters
        ----------
        value : Literal["pay", "receive"]
            Swap direction for the fixed leg (serde string). ``"pay"`` pays fixed / receives floating.

        Returns
        -------
        InterestRateSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized name.
        """
        ...
    def fixed(self, value: FixedLegSpec) -> InterestRateSwapBuilder:
        """
        Set the fixed leg specification.

        Parameters
        ----------
        value : FixedLegSpec
            Fixed leg specification.

        Returns
        -------
        InterestRateSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def float(self, value: FloatLegSpec) -> InterestRateSwapBuilder:
        """
        Set the floating leg specification.

        Parameters
        ----------
        value : FloatLegSpec
            Floating leg specification.

        Returns
        -------
        InterestRateSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def margin_spec(self, value: dict[str, object] | str) -> InterestRateSwapBuilder:
        """
        Set the OTC margin (CSA / initial-margin) specification.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``OtcMarginSpec`` in serde form (``dict`` or JSON string), e.g. the ``margin_spec`` value of a margined swap's ``to_dict()``.

        Returns
        -------
        InterestRateSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does not deserialize as a ``OtcMarginSpec``.
        """
        ...
    def attributes(self, value: Attributes | dict[str, str]) -> InterestRateSwapBuilder:
        """
        Set instrument attributes (tags and metadata).

        Parameters
        ----------
        value : Attributes | dict[str, str]
            Attribute bag; a ``dict`` populates ``meta`` and an optional
            ``"tags"`` entry holding a list of strings populates ``tags``.

        Returns
        -------
        InterestRateSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        TypeError
            If ``value`` is neither ``Attributes`` nor a ``dict``.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style rendering of the fields set so far, e.g.
        ``InterestRateSwapBuilder(id='X', notional=Money(1000000.0, 'USD'))``.

        Returns
        -------
        str
            ``InterestRateSwapBuilder(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise; it renders the recorded setter calls.
        """
        ...
    def build(self) -> InterestRateSwap:
        """
        Build the validated swap.

        Runs the same validation as the Rust ``InterestRateSwapBuilder::build`` (structural
        invariants only); pricing-time checks run in ``InterestRateSwap.price``.

        Returns
        -------
        InterestRateSwap
            The validated instrument.

        Raises
        ------
        ValueError
            If the builder was already consumed, a required field is missing
            (the message names the builder and the field, e.g.
            ``InterestRateSwapBuilder: missing required field 'id'``), or the instrument
            fails validation.
        """
        ...

class Swaption:
    """
    Typed wrapper for the canonical Rust ``Swaption`` instrument.

    Construct via :meth:`Swaption.builder`, :meth:`Swaption.example` /
    :meth:`Swaption.example_bermudan` or :meth:`Swaption.from_json`. Every
    public Rust field is readable as a property; ``get_strike`` /
    ``get_swap_start`` / ``get_swap_end`` / ``forward_swap_rate`` mirror the
    Rust accessors and :meth:`Swaption.price` / :meth:`Swaption.metric` run
    the same pricer as :func:`price_instrument`.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import Swaption
    >>> swpn = Swaption.example()
    >>> (swpn.option_type, swpn.get_strike(), swpn.vol_surface_id)
    ('call', 0.03, 'USD-SWPNVOL')
    """

    @staticmethod
    def builder() -> SwaptionBuilder:
        """
        Create a fluent builder (mirrors Rust ``Swaption::builder()``).

        Returns
        -------
        SwaptionBuilder
            A builder with fluent, consuming setter methods.

        Notes
        -----
        This factory does not raise; it returns an empty builder.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Swaption
        >>> builder = Swaption.builder()
        >>> builder.id("EXAMPLE") is builder
        True
        """
        ...
    @staticmethod
    def example() -> Swaption:
        """
        Canonical European 1Yx5Y USD payer swaption (mirrors Rust
        ``Swaption::example``): cash-settled, Black vol, 3% strike on a 5-year
        swap, vol surface ``USD-SWPNVOL``.

        Returns
        -------
        Swaption
            The example swaption.

        Notes
        -----
        This factory does not raise; the example is built from constants.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Swaption
        >>> Swaption.example().settlement
        'cash'
        """
        ...
    @staticmethod
    def example_bermudan() -> Swaption:
        """
        Bermudan-exercise variant of the example (mirrors Rust
        ``Swaption::example_bermudan``).

        Returns
        -------
        Swaption
            The example swaption with ``exercise_style == "bermudan"``.

        Notes
        -----
        This factory does not raise; the example is built from constants.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Swaption
        >>> Swaption.example_bermudan().exercise_style
        'bermudan'
        """
        ...
    @classmethod
    def from_json(cls, json: str) -> Swaption:
        """
        Deserialize a validated Swaption from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"swaption"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        Swaption
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Swaption
        >>> Swaption.from_json(Swaption.example().to_json()).id
        'SWPN-1Yx5Y-USD'
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to a canonical ``finstack_quant.instrument/1`` envelope.

        Returns
        -------
        str
            Canonical instrument envelope accepted by :func:`price_instrument`
            and :meth:`Swaption.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def to_dict(self) -> dict[str, object]:
        """
        Serde form of the swaption spec as a plain Python ``dict``.

        Returns
        -------
        dict[str, object]
            The ``spec`` object of the instrument envelope (JSON-compatible
            values: ``str``, ``float``, ``dict``, ``list``, ``None``).

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def price(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        model: str = "default",
        metrics: list[str] | None = None,
        pricing_options: dict[str, object] | str | None = None,
        market_history: str | None = None,
    ) -> ValuationResult:
        """
        Price this swaption and return a :class:`~finstack_quant.valuations.ValuationResult`.

        Same pipeline and keyword surface as :func:`price_instrument`.

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date (ISO 8601 strings accepted).
        model : str, default "default"
            Model key (``"black76"``, ``"normal"``, ``"hull_white_1f"``, ...).
        metrics : list[str], optional
            Metric identifiers to compute (see :func:`list_standard_metrics`).
        pricing_options : dict[str, object] | str, optional
            ``MetricPricingOverrides`` merged into the instrument's own overrides
            (e.g. ``{"theta_period": "1D"}``).
        market_history : str, optional
            JSON ``MarketHistory`` scenarios required by ``hvar`` /
            ``expected_shortfall``.

        Returns
        -------
        ValuationResult
            Typed valuation envelope with price, currency and metrics.

        Raises
        ------
        ValueError
            If an input cannot be interpreted or the instrument fails validation.
        KeyError
            If a required curve, surface or metric is missing from ``market``.
        RuntimeError
            If pricing or a metric computation fails.
        """
        ...
    def metric(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        metric_id: str,
        model: str = "default",
    ) -> float:
        """
        Compute one scalar metric for this swaption (e.g. ``"vega"``).

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.
        metric_id : str
            Registered metric identifier (see :func:`list_standard_metrics`).
        model : str, default "default"
            Model key.

        Returns
        -------
        float
            The metric value in the metric's native unit (decimal for rates and
            yields, currency units for DV01-style sensitivities, basis points for
            spreads).

        Raises
        ------
        ValueError
            If ``metric_id`` is unknown or an input cannot be interpreted.
        KeyError
            If a required curve or surface is missing from ``market``.
        RuntimeError
            If the metric computation fails.
        """
        ...
    def market_dependencies(self) -> dict[str, object]:
        """
        Market-data dependencies declared by the Rust ``Instrument`` trait.

        Returns
        -------
        dict[str, object]
            Serde form of ``MarketDependencies`` (``curves`` grouped by role,
            fixings, volatility surfaces, FX pairs).

        Raises
        ------
        ValueError
            If the instrument cannot enumerate its dependencies.
        """
        ...
    def forward_swap_rate(
        self, market: MarketContext | str, as_of: datetime.date | datetime.datetime | pd.Timestamp | str
    ) -> float:
        """
        Forward swap rate of the underlying (mirrors Rust ``Swaption::forward_swap_rate``).

        Parameters
        ----------
        market : MarketContext | str
            Market context holding the discount and forward curves.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.

        Returns
        -------
        float
            Par swap rate of the underlying as a decimal.

        Raises
        ------
        KeyError
            If a required curve is missing from ``market``.
        RuntimeError
            If the annuity or floating PV cannot be computed.
        """
        ...
    def get_strike(self) -> float:
        """
        Fixed strike of the underlying swap (mirrors Rust ``get_strike``).

        Returns
        -------
        float
            Strike as a decimal rate.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    def get_swap_start(self) -> datetime.date:
        """
        Effective date of the underlying swap (mirrors Rust ``get_swap_start``).

        Returns
        -------
        datetime.date
            The underlying start date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    def get_swap_end(self) -> datetime.date:
        """
        Maturity of the underlying swap (mirrors Rust ``get_swap_end``).

        Returns
        -------
        datetime.date
            The underlying end date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def id(self) -> str:
        """
        Stable instrument identifier used in market lookup and results.

        Returns
        -------
        str
            The unique instrument identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def option_type(self) -> str:
        """
        Option type of the swaption.

        Returns
        -------
        str
            ``"call"`` (payer) or ``"put"`` (receiver).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def notional(self) -> Money:
        """
        Notional of the underlying swap.

        Returns
        -------
        Money
            Currency-tagged notional.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def expiry(self) -> datetime.date:
        """
        Option expiry date.

        Returns
        -------
        datetime.date
            The expiry date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def exercise_style(self) -> str:
        """
        Exercise style of the swaption.

        Returns
        -------
        str
            ``"european"``, ``"bermudan"`` or ``"american"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def settlement(self) -> str:
        """
        Settlement method.

        Returns
        -------
        str
            ``"physical"`` or ``"cash"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def cash_settlement_method(self) -> str:
        """
        Cash settlement annuity method (serde string).

        Returns
        -------
        str
            ``"collateralized_cash_price"``, ``"par_yield"``, ``"isda_par_par"`` or ``"zero_coupon"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def vol_model(self) -> str:
        """
        Volatility model.

        Returns
        -------
        str
            ``"black"`` or ``"normal"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def vol_surface_id(self) -> str:
        """
        Volatility surface identifier.

        Returns
        -------
        str
            Surface id looked up in the market context.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def underlying_fixed_leg(self) -> FixedLegSpec:
        """
        Fixed leg of the underlying swap.

        Returns
        -------
        FixedLegSpec
            The fixed leg.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def underlying_float_leg(self) -> FloatLegSpec:
        """
        Floating leg of the underlying swap.

        Returns
        -------
        FloatLegSpec
            The floating leg.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def sabr_params(self) -> dict[str, object] | None:
        """
        SABR parameters (``alpha``, ``beta``, ``nu``, ``rho``, ``shift``).

        Returns
        -------
        dict[str, object] | None
            The parameter dict, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def default_model(self) -> str:
        """
        Canonical model key used when ``model="default"`` is passed to ``price``.

        Returns
        -------
        str
            Registered model key such as ``"discounting"`` or ``"black76"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def attributes(self) -> Attributes:
        """
        Instrument attributes (tags and metadata) used for scenario selection.

        Returns
        -------
        Attributes
            The attribute bag; empty when none were set.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class SwaptionBuilder:
    """
    Fluent builder for :class:`Swaption`; wraps the Rust ``FinancialBuilder``
    output one setter for one setter.

    Builders are consumed by ``build()``; create a new builder per
    instrument. Required fields: ``id``, ``option_type``, ``notional``,
    ``expiry``, ``settlement``, ``cash_settlement_method``, ``vol_model``,
    ``vol_surface_id``, ``underlying_fixed_leg``, ``underlying_float_leg``
    (``exercise_style`` defaults to ``"european"``).

    Examples
    --------
    >>> from finstack_quant.core.dates import DayCount, Tenor
    >>> from finstack_quant.valuations.instruments import FixedLegSpec, FloatLegSpec, Swaption
    >>> fixed = FixedLegSpec(
    ...     "USD-OIS",
    ...     0.04,
    ...     Tenor.semi_annual(),
    ...     DayCount.THIRTY_360,
    ...     "2025-01-15",
    ...     "2030-01-15",
    ...     compounding_simple=False,
    ... )
    >>> floating = FloatLegSpec(
    ...     "USD-OIS",
    ...     "USD-SOFR-3M",
    ...     0.0,
    ...     Tenor.quarterly(),
    ...     DayCount.ACT_360,
    ...     "2025-01-15",
    ...     "2030-01-15",
    ... )
    >>> swaption = (
    ...     Swaption
    ...     .builder()
    ...     .id("SWPT-1")
    ...     .option_type("call")
    ...     .notional(10_000_000.0, currency="USD")
    ...     .expiry("2025-01-13")
    ...     .settlement("cash")
    ...     .cash_settlement_method("collateralized_cash_price")
    ...     .vol_model("black")
    ...     .vol_surface_id("USD-SWPT-VOL")
    ...     .underlying_fixed_leg(fixed)
    ...     .underlying_float_leg(floating)
    ...     .build()
    ... )
    >>> swaption.get_strike()
    0.04
    """

    def id(self, value: str) -> SwaptionBuilder:
        """
        Set the instrument identifier.

        Parameters
        ----------
        value : str
            Instrument identifier.

        Returns
        -------
        SwaptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def option_type(self, value: Literal["call", "put"]) -> SwaptionBuilder:
        """
        Set the option type.

        Parameters
        ----------
        value : Literal["call", "put"]
            Option type (serde string). ``"call"`` is a payer, ``"put"`` a receiver swaption.

        Returns
        -------
        SwaptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized name.
        """
        ...
    def notional(self, value: Money | float, currency: str | None = None) -> SwaptionBuilder:
        """
        Set the notional of the underlying swap.

        Parameters
        ----------
        value : Money | float
            Notional of the underlying swap; a bare number is tagged with ``currency``.
        currency : str, optional
            ISO-4217 code applied when ``value`` is a bare number.

        Returns
        -------
        SwaptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or a bare number is given without ``currency``.
        """
        ...
    def expiry(self, value: datetime.date | datetime.datetime | pd.Timestamp | str) -> SwaptionBuilder:
        """
        Set the option expiry date.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pd.Timestamp | str
            Option expiry date (ISO 8601 strings accepted).

        Returns
        -------
        SwaptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or the date cannot be interpreted.
        """
        ...
    def exercise_style(self, value: Literal["european", "bermudan", "american"]) -> SwaptionBuilder:
        """
        Set the exercise style.

        Parameters
        ----------
        value : Literal["european", "bermudan", "american"]
            Exercise style (serde string). Default ``"european"``.

        Returns
        -------
        SwaptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized name.
        """
        ...
    def settlement(self, value: Literal["physical", "cash"]) -> SwaptionBuilder:
        """
        Set the settlement method.

        Parameters
        ----------
        value : Literal["physical", "cash"]
            Settlement method (serde string).

        Returns
        -------
        SwaptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized name.
        """
        ...
    def cash_settlement_method(
        self, value: Literal["collateralized_cash_price", "par_yield", "isda_par_par", "zero_coupon"]
    ) -> SwaptionBuilder:
        """
        Set the cash settlement annuity method (only used when ``settlement`` is ``"cash"``).

        Parameters
        ----------
        value : Literal["collateralized_cash_price", "par_yield", "isda_par_par", "zero_coupon"]
            Cash settlement annuity method (only used when ``settlement`` is ``"cash"``) (serde string). ``"collateralized_cash_price"`` discounts the physical fixed-leg annuity.

        Returns
        -------
        SwaptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized name.
        """
        ...
    def vol_model(self, value: Literal["black", "normal"]) -> SwaptionBuilder:
        """
        Set the volatility model used for pricing.

        Parameters
        ----------
        value : Literal["black", "normal"]
            Volatility model used for pricing (serde string).

        Returns
        -------
        SwaptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized name.
        """
        ...
    def vol_surface_id(self, value: str) -> SwaptionBuilder:
        """
        Set the volatility surface identifier.

        Parameters
        ----------
        value : str
            Volatility surface identifier.

        Returns
        -------
        SwaptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def underlying_fixed_leg(self, value: FixedLegSpec) -> SwaptionBuilder:
        """
        Set the complete fixed leg of the underlying swap.

        Parameters
        ----------
        value : FixedLegSpec
            Fixed leg of the underlying swap.

        Returns
        -------
        SwaptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def underlying_float_leg(self, value: FloatLegSpec) -> SwaptionBuilder:
        """
        Set the complete floating leg of the underlying swap.

        Parameters
        ----------
        value : FloatLegSpec
            Floating leg of the underlying swap.

        Returns
        -------
        SwaptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def sabr_params(self, value: dict[str, object] | str) -> SwaptionBuilder:
        """
        Set the SABR volatility model parameters.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``SabrParameters`` in serde form (``dict`` or JSON string), e.g. ``{"alpha": 0.025, "beta": 0.5, "nu": 0.4, "rho": -0.3, "shift": None}``.

        Returns
        -------
        SwaptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does not deserialize as a ``SabrParameters``.
        """
        ...
    def sabr_params_json(self, value: str) -> SwaptionBuilder:
        """
        Set the SABR volatility model parameters from a JSON string.

        Parameters
        ----------
        value : str
            JSON object with fields ``alpha``, ``beta``, ``nu``, ``rho`` and optional ``shift``.

        Returns
        -------
        SwaptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not valid JSON for the SABR parameters shape.
        """
        ...
    def attributes(self, value: Attributes | dict[str, str]) -> SwaptionBuilder:
        """
        Set instrument attributes (tags and metadata).

        Parameters
        ----------
        value : Attributes | dict[str, str]
            Attribute bag; a ``dict`` populates ``meta`` and an optional
            ``"tags"`` entry holding a list of strings populates ``tags``.

        Returns
        -------
        SwaptionBuilder
            ``self``, for chaining.

        Raises
        ------
        TypeError
            If ``value`` is neither ``Attributes`` nor a ``dict``.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style rendering of the fields set so far, e.g.
        ``SwaptionBuilder(id='X', notional=Money(1000000.0, 'USD'))``.

        Returns
        -------
        str
            ``SwaptionBuilder(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise; it renders the recorded setter calls.
        """
        ...
    def build(self) -> Swaption:
        """
        Build the validated swaption.

        Runs the same validation as the Rust ``SwaptionBuilder::build`` (structural
        invariants only); pricing-time checks run in ``Swaption.price``.

        Returns
        -------
        Swaption
            The validated instrument.

        Raises
        ------
        ValueError
            If the builder was already consumed, a required field is missing
            (the message names the builder and the field, e.g.
            ``SwaptionBuilder: missing required field 'id'``), or the instrument
            fails validation.
        """
        ...

class CapFloor:
    """
    Typed wrapper for the canonical Rust ``CapFloor`` instrument.

    Construct via :meth:`CapFloor.builder`, :meth:`CapFloor.example` or
    :meth:`CapFloor.from_json`. Every public Rust field is readable as a
    property; :meth:`CapFloor.price` / :meth:`CapFloor.metric` run the same
    pricer as :func:`price_instrument`.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import CapFloor
    >>> cap = CapFloor.example()
    >>> (cap.rate_option_type, cap.strike, cap.vol_type)
    ('cap', 0.03, 'auto')
    """

    @staticmethod
    def builder() -> CapFloorBuilder:
        """
        Create a fluent builder (mirrors Rust ``CapFloor::builder()``).

        Returns
        -------
        CapFloorBuilder
            A builder with fluent, consuming setter methods.

        Notes
        -----
        This factory does not raise; it returns a new instance with the documented
        defaults. Unset ``vol_type`` defaults to ``"auto"``: the surface is treated
        as a lognormal quote and each caplet uses Black-76 when forward and strike
        are positive, otherwise an equivalent Bachelier price. A normal-vol surface
        must set ``vol_type`` to ``"normal"``.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CapFloor
        >>> builder = CapFloor.builder()
        >>> builder.id("EXAMPLE") is builder
        True
        """
        ...
    @staticmethod
    def example() -> CapFloor:
        """
        Canonical 5-year USD 3% cap (mirrors Rust ``CapFloor::example``): quarterly
        ACT/360 on ``USD-SOFR-3M`` discounted on ``USD-OIS`` with vol surface
        ``USD-CAPFLOOR-VOL``.

        Returns
        -------
        CapFloor
            The example cap.

        Raises
        ------
        ValueError
            If construction fails (should not occur).

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CapFloor
        >>> CapFloor.example().forward_curve_id
        'USD-SOFR-3M'
        """
        ...
    @classmethod
    def from_json(cls, json: str) -> CapFloor:
        """
        Deserialize a validated CapFloor from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"cap_floor"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        CapFloor
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CapFloor
        >>> CapFloor.from_json(CapFloor.example().to_json()).id
        'IRCAP-USD-5Y-3PCT'
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to a canonical ``finstack_quant.instrument/1`` envelope.

        Returns
        -------
        str
            Canonical instrument envelope accepted by :func:`price_instrument`
            and :meth:`CapFloor.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def to_dict(self) -> dict[str, object]:
        """
        Serde form of the cap/floor spec as a plain Python ``dict``.

        Returns
        -------
        dict[str, object]
            The ``spec`` object of the instrument envelope (JSON-compatible
            values: ``str``, ``float``, ``dict``, ``list``, ``None``).

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def price(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        model: str = "default",
        metrics: list[str] | None = None,
        pricing_options: dict[str, object] | str | None = None,
        market_history: str | None = None,
    ) -> ValuationResult:
        """
        Price this cap/floor and return a :class:`~finstack_quant.valuations.ValuationResult`.

        Same pipeline and keyword surface as :func:`price_instrument`.

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date (ISO 8601 strings accepted).
        model : str, default "default"
            Model key (``"black76"``, ``"normal"``, ``"hull_white_1f"``, ...).
        metrics : list[str], optional
            Metric identifiers to compute (see :func:`list_standard_metrics`).
        pricing_options : dict[str, object] | str, optional
            ``MetricPricingOverrides`` merged into the instrument's own overrides
            (e.g. ``{"theta_period": "1D"}``).
        market_history : str, optional
            JSON ``MarketHistory`` scenarios required by ``hvar`` /
            ``expected_shortfall``.

        Returns
        -------
        ValuationResult
            Typed valuation envelope with price, currency and metrics.

        Raises
        ------
        ValueError
            If an input cannot be interpreted or the instrument fails validation.
        KeyError
            If a required curve, surface or metric is missing from ``market``.
        RuntimeError
            If pricing or a metric computation fails.
        """
        ...
    def metric(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        metric_id: str,
        model: str = "default",
    ) -> float:
        """
        Compute one scalar metric for this cap/floor (e.g. ``"vega"``).

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.
        metric_id : str
            Registered metric identifier (see :func:`list_standard_metrics`).
        model : str, default "default"
            Model key.

        Returns
        -------
        float
            The metric value in the metric's native unit (decimal for rates and
            yields, currency units for DV01-style sensitivities, basis points for
            spreads).

        Raises
        ------
        ValueError
            If ``metric_id`` is unknown or an input cannot be interpreted.
        KeyError
            If a required curve or surface is missing from ``market``.
        RuntimeError
            If the metric computation fails.
        """
        ...
    def market_dependencies(self) -> dict[str, object]:
        """
        Market-data dependencies declared by the Rust ``Instrument`` trait.

        Returns
        -------
        dict[str, object]
            Serde form of ``MarketDependencies`` (``curves`` grouped by role,
            fixings, volatility surfaces, FX pairs).

        Raises
        ------
        ValueError
            If the instrument cannot enumerate its dependencies.
        """
        ...
    @property
    def id(self) -> str:
        """
        Stable instrument identifier used in market lookup and results.

        Returns
        -------
        str
            The unique instrument identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def rate_option_type(self) -> str:
        """
        Option type of the cap/floor.

        Returns
        -------
        str
            ``"cap"``, ``"floor"``, ``"caplet"`` or ``"floorlet"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def notional(self) -> Money:
        """
        Notional amount.

        Returns
        -------
        Money
            Currency-tagged notional.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def strike(self) -> float:
        """
        Strike as a decimal rate.

        Returns
        -------
        float
            ``0.03`` for 3%.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def spread(self) -> float:
        """
        Contractual spread added to the index, as a decimal rate.

        Returns
        -------
        float
            ``0.0`` when unset.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def start_date(self) -> datetime.date:
        """
        Start date of the underlying period.

        Returns
        -------
        datetime.date
            The start date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def maturity(self) -> datetime.date:
        """
        End date of the underlying period.

        Returns
        -------
        datetime.date
            The maturity date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def frequency(self) -> Tenor:
        """
        Payment frequency.

        Returns
        -------
        Tenor
            The payment tenor.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def day_count(self) -> DayCount:
        """
        Accrual day-count convention.

        Returns
        -------
        DayCount
            The day count.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def stub(self) -> StubKind:
        """
        Stub-period handling rule for the schedule.

        Returns
        -------
        StubKind
            The ``StubKind`` variant (``NONE``, ``SHORT_FRONT``, ...).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def business_day_convention(self) -> str:
        """
        Business day convention (serde string).

        Returns
        -------
        str
            ``"modified_following"`` unless overridden.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def calendar_id(self) -> str | None:
        """
        Holiday calendar identifier.

        Returns
        -------
        str | None
            Calendar id, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def exercise_style(self) -> str:
        """
        Exercise style (serde string).

        Returns
        -------
        str
            ``"european"`` unless overridden.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def settlement(self) -> str:
        """
        Settlement type (serde string).

        Returns
        -------
        str
            ``"cash"`` unless overridden.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def discount_curve_id(self) -> str:
        """
        Discount curve identifier.

        Returns
        -------
        str
            Curve id used for discounting.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def forward_curve_id(self) -> str:
        """
        Forward curve identifier.

        Returns
        -------
        str
            Curve id used to project the index.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def vol_surface_id(self) -> str:
        """
        Volatility surface identifier.

        Returns
        -------
        str
            Surface id looked up in the market context.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def vol_type(self) -> str:
        """
        Volatility convention.

        Returns
        -------
        str
            ``"lognormal"``, ``"shifted_lognormal"``, ``"normal"`` or ``"auto"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def vol_shift(self) -> float:
        """
        Displacement shift for shifted-lognormal pricing.

        Returns
        -------
        float
            Non-negative shift; ``0.0`` when unset.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def overnight_coupon(self) -> dict[str, object] | None:
        """
        Overnight (RFR) coupon convention in serde form.

        Returns
        -------
        dict[str, object] | None
            The convention dict, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def premium(self) -> tuple[datetime.date, Money] | None:
        """
        Dated premium paid by the holder.

        Returns
        -------
        tuple[datetime.date, Money] | None
            ``(payment_date, amount)`` or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def default_model(self) -> str:
        """
        Canonical model key used when ``model="default"`` is passed to ``price``.

        Returns
        -------
        str
            Registered model key such as ``"discounting"`` or ``"black76"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def attributes(self) -> Attributes:
        """
        Instrument attributes (tags and metadata) used for scenario selection.

        Returns
        -------
        Attributes
            The attribute bag; empty when none were set.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def expiry(self) -> datetime.date | None:
        """
        Expiry date exposed by the Rust ``Instrument`` trait.

        Returns
        -------
        datetime.date | None
            The expiry/maturity date, or ``None`` when the instrument type reports none.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class CapFloorBuilder:
    """
    Fluent builder for :class:`CapFloor`; wraps the Rust ``FinancialBuilder``
    output one setter for one setter.

    Builders are consumed by ``build()``; create a new builder per
    instrument. Required fields: ``id``, ``rate_option_type``, ``notional``,
    ``strike``, ``start_date``, ``maturity``, ``frequency``, ``day_count``,
    ``discount_curve_id``, ``forward_curve_id``, ``vol_surface_id``.

    Examples
    --------
    >>> from finstack_quant.core.dates import DayCount, Tenor
    >>> from finstack_quant.valuations.instruments import CapFloor
    >>> cap = (
    ...     CapFloor
    ...     .builder()
    ...     .id("CAP-1")
    ...     .rate_option_type("cap")
    ...     .notional(5_000_000.0, currency="USD")
    ...     .strike(0.05)
    ...     .start_date("2024-01-15")
    ...     .maturity("2027-01-15")
    ...     .frequency(Tenor.quarterly())
    ...     .day_count(DayCount.ACT_360)
    ...     .discount_curve_id("USD-OIS")
    ...     .forward_curve_id("USD-SOFR-3M")
    ...     .vol_surface_id("USD-CAP-VOL")
    ...     .build()
    ... )
    >>> cap.vol_type
    'auto'
    """

    def id(self, value: str) -> CapFloorBuilder:
        """
        Set the instrument identifier.

        Parameters
        ----------
        value : str
            Instrument identifier.

        Returns
        -------
        CapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def rate_option_type(self, value: Literal["cap", "floor", "caplet", "floorlet"]) -> CapFloorBuilder:
        """
        Set the option type.

        Parameters
        ----------
        value : Literal["cap", "floor", "caplet", "floorlet"]
            Option type (serde string). ``"cap"``/``"floor"`` price a series of caplets/floorlets, ``"caplet"``/``"floorlet"`` a single period.

        Returns
        -------
        CapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized name.
        """
        ...
    def notional(self, value: Money | float, currency: str | None = None) -> CapFloorBuilder:
        """
        Set the notional amount.

        Parameters
        ----------
        value : Money | float
            Notional amount; a bare number is tagged with ``currency``.
        currency : str, optional
            ISO-4217 code applied when ``value`` is a bare number.

        Returns
        -------
        CapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or a bare number is given without ``currency``.
        """
        ...
    def strike(self, value: float | Rate) -> CapFloorBuilder:
        """
        Set the strike rate of every caplet/floorlet.

        Parameters
        ----------
        value : float | Rate
            Strike as a decimal (``0.05`` = 5%) or a ``Rate``.

        Returns
        -------
        CapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not finite.
        """
        ...
    def spread(self, value: float | Rate) -> CapFloorBuilder:
        """
        Set the contractual spread added to the referenced rate.

        Parameters
        ----------
        value : float | Rate
            Spread in decimal rate units (``0.001`` = 10bp) or a ``Rate``, added after projecting the index.

        Returns
        -------
        CapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not finite.
        """
        ...
    def premium(
        self,
        payment_date: datetime.date | datetime.datetime | pd.Timestamp | str,
        amount: Money | float,
        currency: str | None = None,
    ) -> CapFloorBuilder:
        """
        Set the dated premium paid by the cap/floor holder.

        Parameters
        ----------
        payment_date : datetime.date | datetime.datetime | pd.Timestamp | str
            Contractual premium payment date. Payments on or before the valuation
            date are treated as settled and excluded from NPV.
        amount : Money | float
            Non-negative premium outflow in the notional currency; a bare number is
            tagged with ``currency``.
        currency : str, optional
            ISO-4217 code applied when ``amount`` is a bare number.

        Returns
        -------
        CapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``payment_date`` cannot be interpreted, a bare amount has no
            ``currency``, or the builder was already consumed. Premium amount and
            currency validation occurs in ``build``.
        """
        ...
    def start_date(self, value: datetime.date | datetime.datetime | pd.Timestamp | str) -> CapFloorBuilder:
        """
        Set the start date of the underlying period.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pd.Timestamp | str
            Start date of the underlying period (ISO 8601 strings accepted).

        Returns
        -------
        CapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or the date cannot be interpreted.
        """
        ...
    def maturity(self, value: datetime.date | datetime.datetime | pd.Timestamp | str) -> CapFloorBuilder:
        """
        Set the end date of the underlying period.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pd.Timestamp | str
            End date of the underlying period (ISO 8601 strings accepted).

        Returns
        -------
        CapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or the date cannot be interpreted.
        """
        ...
    def frequency(self, value: Tenor) -> CapFloorBuilder:
        """
        Set the payment frequency.

        Parameters
        ----------
        value : Tenor
            Payment frequency for caps/floors.

        Returns
        -------
        CapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def day_count(self, value: DayCount) -> CapFloorBuilder:
        """
        Set the day count convention.

        Parameters
        ----------
        value : DayCount
            Day count convention.

        Returns
        -------
        CapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def stub(
        self, value: StubKind | Literal["none", "short_front", "long_front", "short_back", "long_back"]
    ) -> CapFloorBuilder:
        """
        Set the stub rule.

        Parameters
        ----------
        value : StubKind | Literal["none", "short_front", "long_front", "short_back", "long_back"]
            Stub rule (serde string). Default ``"short_front"``.

        Returns
        -------
        CapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized name.
        """
        ...
    def business_day_convention(self, value: str) -> CapFloorBuilder:
        """
        Set the business day convention.

        Parameters
        ----------
        value : str
            Business day convention (serde string). Default ``"modified_following"``.

        Returns
        -------
        CapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized name.
        """
        ...
    def calendar_id(self, value: str) -> CapFloorBuilder:
        """
        Set the holiday calendar identifier for schedule and roll conventions.

        Parameters
        ----------
        value : str
            Holiday calendar identifier for schedule and roll conventions.

        Returns
        -------
        CapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def exercise_style(self, value: str) -> CapFloorBuilder:
        """
        Set the exercise style.

        Parameters
        ----------
        value : str
            Exercise style (serde string). Default ``"european"``.

        Returns
        -------
        CapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized name.
        """
        ...
    def settlement(self, value: str) -> CapFloorBuilder:
        """
        Set the settlement type.

        Parameters
        ----------
        value : str
            Settlement type (serde string). Default ``"cash"``.

        Returns
        -------
        CapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized name.
        """
        ...
    def discount_curve_id(self, value: str) -> CapFloorBuilder:
        """
        Set the discount curve identifier.

        Parameters
        ----------
        value : str
            Discount curve identifier.

        Returns
        -------
        CapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def forward_curve_id(self, value: str) -> CapFloorBuilder:
        """
        Set the forward curve identifier.

        Parameters
        ----------
        value : str
            Forward curve identifier.

        Returns
        -------
        CapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def vol_surface_id(self, value: str) -> CapFloorBuilder:
        """
        Set the volatility surface identifier.

        Parameters
        ----------
        value : str
            Volatility surface identifier.

        Returns
        -------
        CapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def vol_type(self, value: Literal["lognormal", "shifted_lognormal", "normal", "auto"]) -> CapFloorBuilder:
        """
        Set the volatility type convention.

        Parameters
        ----------
        value : Literal["lognormal", "shifted_lognormal", "normal", "auto"]
            Volatility type convention (serde string). Must match the configured surface; ``"auto"`` (the default when unset) resolves to ``"lognormal"`` with a Bachelier fallback where Black-76 is undefined.

        Returns
        -------
        CapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized name.
        """
        ...
    def vol_shift(self, value: float) -> CapFloorBuilder:
        """
        Set the displacement shift used for shifted-lognormal pricing.

        Parameters
        ----------
        value : float
            Displacement added to forward and strike; must be non-negative.

        Returns
        -------
        CapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def overnight_coupon(self, value: dict[str, object] | str) -> CapFloorBuilder:
        """
        Set the overnight (RFR) coupon convention for compounded caplets.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``OvernightCouponConvention`` in serde form (``dict`` or JSON string), e.g. ``{"compounding": {"compounded_in_arrears": {"lookback_days": 0}}, "payment_delay_days": 2}``.

        Returns
        -------
        CapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does not deserialize as a ``OvernightCouponConvention``.
        """
        ...
    def attributes(self, value: Attributes | dict[str, str]) -> CapFloorBuilder:
        """
        Set instrument attributes (tags and metadata).

        Parameters
        ----------
        value : Attributes | dict[str, str]
            Attribute bag; a ``dict`` populates ``meta`` and an optional
            ``"tags"`` entry holding a list of strings populates ``tags``.

        Returns
        -------
        CapFloorBuilder
            ``self``, for chaining.

        Raises
        ------
        TypeError
            If ``value`` is neither ``Attributes`` nor a ``dict``.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style rendering of the fields set so far, e.g.
        ``CapFloorBuilder(id='X', notional=Money(1000000.0, 'USD'))``.

        Returns
        -------
        str
            ``CapFloorBuilder(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise; it renders the recorded setter calls.
        """
        ...
    def build(self) -> CapFloor:
        """
        Build the validated cap/floor.

        Runs the same validation as the Rust ``CapFloorBuilder::build`` (structural
        invariants only); pricing-time checks run in ``CapFloor.price``.

        Returns
        -------
        CapFloor
            The validated instrument.

        Raises
        ------
        ValueError
            If the builder was already consumed, a required field is missing
            (the message names the builder and the field, e.g.
            ``CapFloorBuilder: missing required field 'id'``), or the instrument
            fails validation.
        """
        ...

class CreditDefaultSwap:
    """
    Single-name credit default swap (typed wrapper for the canonical Rust
    ``CreditDefaultSwap``).

    Follows the ISDA CDS Standard Model conventions: quarterly IMM premium
    dates, ACT/360, accrual-on-default and points-upfront quoting via
    :meth:`CreditDefaultSwapBuilder.upfront`. ``convention="isda_na"`` is the
    SNAC / post-Big-Bang standard (and the Rust default);
    ``valuation_convention`` defaults to Bloomberg CDSW clean principal.
    Construct via :meth:`CreditDefaultSwap.builder`,
    :meth:`CreditDefaultSwap.example` or :meth:`CreditDefaultSwap.from_json`.
    Every public Rust field is readable as a property and
    :meth:`CreditDefaultSwap.price` / :meth:`CreditDefaultSwap.metric` run the
    same pricer as :func:`price_instrument`. The desk CS01 on a hand-built
    hazard curve is the ``"cs01_hazard"`` metric (``"cs01"`` needs a
    calibration recipe on the curve).

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import CreditDefaultSwap
    >>> cds = CreditDefaultSwap.example()
    >>> (cds.id, cds.side, cds.convention, cds.doc_clause_effective)
    ('CDS-CORP-5Y', 'pay', 'isda_na', 'xr14')
    """

    @staticmethod
    def builder() -> CreditDefaultSwapBuilder:
        """
        Create a fluent builder (mirrors Rust ``CreditDefaultSwap::builder()``).

        Returns
        -------
        CreditDefaultSwapBuilder
            A builder with fluent, consuming setter methods.

        Notes
        -----
        This factory does not raise; it returns an empty builder.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CreditDefaultSwap
        >>> builder = CreditDefaultSwap.builder()
        >>> builder.id("EXAMPLE") is builder
        True
        """
        ...
    @classmethod
    def from_json(cls, json: str) -> CreditDefaultSwap:
        """
        Deserialize a validated CreditDefaultSwap from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"credit_default_swap"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        CreditDefaultSwap
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CreditDefaultSwap
        >>> CreditDefaultSwap.from_json(CreditDefaultSwap.example().to_json()).id
        'CDS-CORP-5Y'
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to a canonical ``finstack_quant.instrument/1`` envelope.

        Returns
        -------
        str
            Canonical instrument envelope accepted by :func:`price_instrument`
            and :meth:`CreditDefaultSwap.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def to_dict(self) -> dict[str, object]:
        """
        Serde form of the instrument spec as a plain Python ``dict``.

        Returns
        -------
        dict[str, object]
            The ``spec`` object of the instrument envelope (JSON-compatible
            values: ``str``, ``float``, ``dict``, ``list``, ``None``).

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def price(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        model: str = "default",
        metrics: list[str] | None = None,
        pricing_options: dict[str, object] | str | None = None,
        market_history: str | None = None,
    ) -> ValuationResult:
        """
        Price this instrument and return a :class:`~finstack_quant.valuations.ValuationResult`.

        Same pipeline and keyword surface as :func:`price_instrument`.

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date (ISO 8601 strings accepted).
        model : str, default "default"
            Model key (``"hazard_rate"`` is the native model).
        metrics : list[str], optional
            Metric identifiers to compute (see :func:`list_standard_metrics`).
        pricing_options : dict[str, object] | str, optional
            ``MetricPricingOverrides`` merged into the instrument's own overrides
            (e.g. ``{"theta_period": "1D"}``).
        market_history : str, optional
            JSON ``MarketHistory`` scenarios required by ``hvar`` /
            ``expected_shortfall``.

        Returns
        -------
        ValuationResult
            Typed valuation envelope with price, currency and metrics.

        Raises
        ------
        ValueError
            If an input cannot be interpreted or the instrument fails validation.
        KeyError
            If a required curve, surface or metric is missing from ``market``.
        RuntimeError
            If pricing or a metric computation fails.
        """
        ...
    def metric(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        metric_id: str,
        model: str = "default",
    ) -> float:
        """
        Compute one scalar metric for this instrument (e.g. ``"cs01_hazard"`` or ``"par_spread"``).

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.
        metric_id : str
            Registered metric identifier (see :func:`list_standard_metrics`).
        model : str, default "default"
            Model key.

        Returns
        -------
        float
            The metric value in the metric's native unit (decimal for rates and
            yields, currency units for DV01/CS01-style sensitivities, basis
            points for spreads).

        Raises
        ------
        ValueError
            If ``metric_id`` is unknown or an input cannot be interpreted.
        KeyError
            If a required curve or surface is missing from ``market``.
        RuntimeError
            If the metric computation fails.
        """
        ...
    def market_dependencies(self) -> dict[str, object]:
        """
        Market-data dependencies declared by the Rust ``Instrument`` trait.

        Returns
        -------
        dict[str, object]
            Serde form of ``MarketDependencies`` (``curves`` grouped by role,
            ``credit_index_ids``, ``market_scalar_ids``,
            ``volatility_dependencies``, ``fx_pairs``, ``series_ids``).

        Raises
        ------
        ValueError
            If the instrument cannot enumerate its dependencies.
        """
        ...
    @property
    def id(self) -> str:
        """
        Stable instrument identifier used in market lookup and results.

        Returns
        -------
        str
            The unique instrument identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def default_model(self) -> str:
        """
        Canonical model key used when ``model="default"`` is passed to ``price``.

        Returns
        -------
        str
            Registered model key such as ``"hazard_rate"`` or ``"black76"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def attributes(self) -> Attributes:
        """
        Instrument attributes (tags and metadata) used for scenario selection.

        Returns
        -------
        Attributes
            The attribute bag; empty when none were set.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @staticmethod
    def example() -> CreditDefaultSwap:
        """
        Canonical 5-year USD 10,000,000 investment-grade payer CDS (mirrors Rust
        ``CreditDefaultSwap::example``): ``isda_na`` convention, 100bp running
        spread, 40% recovery, curves ``USD-OIS`` / ``CORP-HAZARD``, premium
        2024-03-20 to 2029-03-20.

        Returns
        -------
        CreditDefaultSwap
            The example CDS.

        Notes
        -----
        This factory does not raise; the example is built from constants.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CreditDefaultSwap
        >>> CreditDefaultSwap.example().protection.credit_curve_id
        'CORP-HAZARD'
        """
        ...
    def get_par_spread(
        self, market: MarketContext | str, as_of: datetime.date | datetime.datetime | pd.Timestamp | str
    ) -> float:
        """
        Par spread implied by the market, in basis points (mirrors Rust
        ``CreditDefaultSwap::get_par_spread``): the running spread at which the
        contract is worth zero under this CDS's valuation convention, premium
        schedule, discount curve, hazard curve and recovery assumption.

        Parameters
        ----------
        market : MarketContext | str
            Market carrying the discount and hazard curves named by the CDS.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.

        Returns
        -------
        float
            Par spread in basis points.

        Raises
        ------
        KeyError
            If a curve is missing from ``market``.
        ValueError
            If the curve recovery metadata conflicts with the contract recovery.
        """
        ...
    @property
    def notional(self) -> Money:
        """
        Notional amount of protection.

        Returns
        -------
        Money
            Currency-tagged notional.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def side(self) -> str:
        """
        Protection perspective.

        Returns
        -------
        str
            ``"pay"`` (buy protection) or ``"receive"`` (sell protection).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def convention(self) -> str:
        """
        ISDA regional convention (serde name).

        Returns
        -------
        str
            ``"isda_na"``, ``"isda_eu"``, ``"isda_as"`` or ``"custom"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def premium(self) -> PremiumLegSpec:
        """
        Premium (fixed coupon) leg specification.

        Returns
        -------
        PremiumLegSpec
            The premium leg.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def protection(self) -> ProtectionLegSpec:
        """
        Protection (default-contingent) leg specification.

        Returns
        -------
        ProtectionLegSpec
            The protection leg.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def valuation_convention(self) -> str:
        """
        Valuation presentation convention (serde name).

        Returns
        -------
        str
            ``"bloomberg_cdsw_clean"`` (default), ``"bloomberg_cdsw_clean_full_premium"``, ``"isda_dirty"`` or ``"quant_lib_isda_parity"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def upfront(self) -> tuple[datetime.date, Money] | None:
        """
        Points-upfront payment as ``(payment_date, amount)``; positive means the protection buyer pays.

        Returns
        -------
        tuple[datetime.date, Money] | None
            The upfront pair, or ``None`` when the trade has no upfront.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def doc_clause(self) -> str | None:
        """
        Explicit ISDA documentation clause (serde name).

        Returns
        -------
        str | None
            The clause, or ``None`` when derived from the convention.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def doc_clause_effective(self) -> str:
        """
        Effective documentation clause after convention-based resolution (mirrors Rust ``doc_clause_effective``).

        Returns
        -------
        str
            ``"xr14"`` for ``isda_na`` / ``isda_as`` / ``custom``, ``"mm14"`` for ``isda_eu``, or the explicit clause resolved to its 2014 variant.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def protection_effective_date(self) -> datetime.date | None:
        """
        Protection effective date for a forward-starting CDS.

        Returns
        -------
        datetime.date | None
            The date, or ``None`` when protection starts with the premium leg.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def protection_start(self) -> datetime.date:
        """
        Date protection starts (mirrors Rust ``protection_start``).

        Returns
        -------
        datetime.date
            ``protection_effective_date`` when set, else the premium start date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def margin_spec(self) -> dict[str, object] | None:
        """
        OTC margin specification in serde form.

        Returns
        -------
        dict[str, object] | None
            The ``OtcMarginSpec`` dict, or ``None`` for unmargined trades.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def expiry(self) -> datetime.date | None:
        """
        Premium-leg end date as seen by the pricer.

        Returns
        -------
        datetime.date | None
            The scheduled maturity, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style rendering of the key economics, e.g. ``CreditDefaultSwap(id='CDS-CORP-5Y', side='pay', notional=Money(10000000.0, 'USD'), spread_bp=100, ...)``.

        Returns
        -------
        str
            ``CreditDefaultSwap(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise; it renders stored values.
        """
        ...

class CreditDefaultSwapBuilder:
    """
    Fluent builder for :class:`CreditDefaultSwap`; wraps the Rust
    ``FinancialBuilder`` output one setter for one setter.

    Builders are consumed by ``build()``; create a new builder per
    instrument. Required fields: ``id``, ``notional``, ``side``,
    ``convention``, ``premium``, ``protection``. ``valuation_convention``
    defaults to ``"bloomberg_cdsw_clean"``; ``upfront``, ``doc_clause``,
    ``protection_effective_date`` and ``margin_spec`` are optional.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.dates import DayCount, Tenor
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.valuations.instruments import (
    ...     CreditDefaultSwap,
    ...     PremiumLegSpec,
    ...     ProtectionLegSpec,
    ... )
    >>> premium = PremiumLegSpec(
    ...     datetime.date(2024, 3, 20),
    ...     datetime.date(2029, 6, 20),
    ...     Tenor.quarterly(),
    ...     DayCount.ACT_360,
    ...     100.0,
    ...     "USD-OIS",
    ... )
    >>> protection = ProtectionLegSpec("ACME-CDS", 0.4, 3)
    >>> cds = (
    ...     CreditDefaultSwap
    ...     .builder()
    ...     .id("CDS-1")
    ...     .notional(Money(10_000_000.0, Currency("USD")))
    ...     .side("pay")
    ...     .convention("isda_na")
    ...     .premium(premium)
    ...     .protection(protection)
    ...     .upfront((datetime.date(2024, 6, 25), Money(-250_000.0, Currency("USD"))))
    ...     .build()
    ... )
    >>> cds.upfront[1].amount
    -250000.0
    """

    def id(self, value: str) -> CreditDefaultSwapBuilder:
        """
        Set the instrument identifier.

        Parameters
        ----------
        value : str
            Unique identifier for the CDS.

        Returns
        -------
        CreditDefaultSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def notional(self, value: Money) -> CreditDefaultSwapBuilder:
        """
        Set the notional amount.

        Parameters
        ----------
        value : Money
            Notional amount of protection.

        Returns
        -------
        CreditDefaultSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def side(self, value: Literal["pay", "receive"]) -> CreditDefaultSwapBuilder:
        """
        Set the protection buyer/seller perspective.

        Parameters
        ----------
        value : Literal["pay", "receive"]
            ``"pay"`` to buy protection (pay premium), ``"receive"`` to sell protection (receive premium).

        Returns
        -------
        CreditDefaultSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized side.
        """
        ...
    def convention(self, value: Literal["isda_na", "isda_eu", "isda_as", "custom"]) -> CreditDefaultSwapBuilder:
        """
        Set the ISDA regional convention.

        Parameters
        ----------
        value : Literal["isda_na", "isda_eu", "isda_as", "custom"]
            ``"isda_na"`` is the SNAC / post-Big-Bang North American standard (ACT/360, quarterly IMM, T+3); ``"isda_eu"`` the European standard (T+1, TARGET2); ``"isda_as"`` Asian (ACT/365F, Tokyo); ``"custom"`` for a manually configured convention.

        Returns
        -------
        CreditDefaultSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not one of the accepted strings (the message lists them).
        """
        ...
    def premium(self, value: PremiumLegSpec) -> CreditDefaultSwapBuilder:
        """
        Set the premium leg specification.

        Parameters
        ----------
        value : PremiumLegSpec
            Premium leg specification.

        Returns
        -------
        CreditDefaultSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def protection(self, value: ProtectionLegSpec) -> CreditDefaultSwapBuilder:
        """
        Set the protection leg specification.

        Parameters
        ----------
        value : ProtectionLegSpec
            Protection leg specification.

        Returns
        -------
        CreditDefaultSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def valuation_convention(
        self,
        value: Literal[
            "bloomberg_cdsw_clean", "bloomberg_cdsw_clean_full_premium", "isda_dirty", "quant_lib_isda_parity"
        ],
    ) -> CreditDefaultSwapBuilder:
        """
        Set the valuation presentation convention.

        Parameters
        ----------
        value : Literal["bloomberg_cdsw_clean", "bloomberg_cdsw_clean_full_premium", "isda_dirty", "quant_lib_isda_parity"]
            ``"bloomberg_cdsw_clean"`` (default) reports Bloomberg CDSW clean principal; ``"isda_dirty"`` the academic ISDA dirty PV; ``"quant_lib_isda_parity"`` reproduces QuantLib ``IsdaCdsEngine``.

        Returns
        -------
        CreditDefaultSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized convention.
        """
        ...
    def upfront(
        self, value: tuple[datetime.date | datetime.datetime | pd.Timestamp | str, Money]
    ) -> CreditDefaultSwapBuilder:
        """
        Set the points-upfront payment (the standard post-Big-Bang quote).

        Parameters
        ----------
        value : tuple[datetime.date | datetime.datetime | pd.Timestamp | str, Money]
            ``(payment_date, amount)``; a payment from protection buyer to seller (positive: buyer pays, negative: seller pays). The currency must match the notional.

        Returns
        -------
        CreditDefaultSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        TypeError
            If ``value`` is not a ``(date, Money)`` pair.
        """
        ...
    def doc_clause(
        self,
        value: Literal["cr14", "mr14", "mm14", "xr14", "isda_na", "isda_eu", "isda_as", "isda_au", "isda_nz", "custom"],
    ) -> CreditDefaultSwapBuilder:
        """
        Set the ISDA documentation clause for restructuring credit events.

        Parameters
        ----------
        value : Literal["cr14", "mr14", "mm14", "xr14", "isda_na", "isda_eu", "isda_as", "isda_au", "isda_nz", "custom"]
            One of the four 2014 ISDA restructuring elections, a regional ISDA corporate default, or ``"custom"``. If never set, the effective clause is derived from the CDS convention (see :attr:`CreditDefaultSwap.doc_clause_effective`).

        Returns
        -------
        CreditDefaultSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized documentation clause.
        """
        ...
    def protection_effective_date(
        self, value: datetime.date | datetime.datetime | pd.Timestamp | str
    ) -> CreditDefaultSwapBuilder:
        """
        Set the protection effective date for a forward-starting CDS.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pd.Timestamp | str
            Date on which credit protection begins; must satisfy ``premium.start <= value <= premium.end`` (ISO 8601 strings accepted).

        Returns
        -------
        CreditDefaultSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or the date cannot be interpreted.
        """
        ...
    def margin_spec(self, value: dict[str, object] | str) -> CreditDefaultSwapBuilder:
        """
        Set the OTC margin (CSA / initial-margin) specification.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``OtcMarginSpec`` in serde form (``dict`` or JSON string); cleared CDS use the ``cleared`` form, bilateral CDS need a SIMM credit classification.

        Returns
        -------
        CreditDefaultSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does not deserialize as an ``OtcMarginSpec``.
        """
        ...
    def attributes(self, value: Attributes | dict[str, str] | None) -> CreditDefaultSwapBuilder:
        """
        Set instrument attributes (tags and metadata).

        Parameters
        ----------
        value : Attributes | dict[str, str] | None
            Attribute bag; a ``dict`` populates ``meta`` and an optional ``"tags"`` entry holding a list of strings populates ``tags``.

        Returns
        -------
        CreditDefaultSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        TypeError
            If ``value`` is neither ``Attributes``, a ``dict`` nor ``None``.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style rendering of the fields set so far, e.g.
        ``CreditDefaultSwapBuilder(id='X', notional=Money(1000000.0, 'USD'))``.

        Returns
        -------
        str
            ``CreditDefaultSwapBuilder(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise; it renders the recorded setter calls.
        """
        ...
    def build(self) -> CreditDefaultSwap:
        """
        Build the validated CDS.

        Runs only the Rust ``CreditDefaultSwapBuilder::build`` validation (structural
        invariants); pricing-time checks run in :meth:`CreditDefaultSwap.price`.

        Returns
        -------
        CreditDefaultSwap
            The validated instrument.

        Raises
        ------
        ValueError
            If the builder was already consumed, a required field is missing
            (the message names the builder and the field, e.g.
            ``CreditDefaultSwapBuilder: missing required field 'id'``), or the instrument
            fails validation (recovery outside ``[0, 1]``, upfront currency mismatch, protection date outside the premium period).
        """
        ...

class CDSIndex:
    """
    Credit index (CDX / iTraxx) trade (typed wrapper for the canonical Rust
    ``CDSIndex``).

    Priced against a single index hazard curve (``pricing="single_curve"``, a
    synthetic CDS) or by expanding into weighted constituents
    (``pricing="constituents"``); ``index_factor`` scales the surviving
    notional after defaults. Construct via :meth:`CDSIndex.from_preset` (the
    preferred way for standardized indices), :meth:`CDSIndex.builder`,
    :meth:`CDSIndex.example` or :meth:`CDSIndex.from_json`. Every public Rust
    field is readable as a property; ``par_spread`` / ``risky_pv01`` / ``cs01``
    mirror the Rust accessors and :meth:`CDSIndex.price` /
    :meth:`CDSIndex.metric` run the same pricer as :func:`price_instrument`.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import CDSIndex
    >>> idx = CDSIndex.example()
    >>> (idx.index_name, idx.series, idx.pricing, idx.num_constituents)
    ('CDX.NA.IG', 42, 'single_curve', 125)
    """

    @staticmethod
    def builder() -> CDSIndexBuilder:
        """
        Create a fluent builder (mirrors Rust ``CDSIndex::builder()``).

        Returns
        -------
        CDSIndexBuilder
            A builder with fluent, consuming setter methods.

        Notes
        -----
        This factory does not raise; it returns an empty builder.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CDSIndex
        >>> builder = CDSIndex.builder()
        >>> builder.id("EXAMPLE") is builder
        True
        """
        ...
    @classmethod
    def from_json(cls, json: str) -> CDSIndex:
        """
        Deserialize a validated CDSIndex from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"cds_index"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        CDSIndex
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CDSIndex
        >>> CDSIndex.from_json(CDSIndex.example().to_json()).id
        'CDX-IG-42'
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to a canonical ``finstack_quant.instrument/1`` envelope.

        Returns
        -------
        str
            Canonical instrument envelope accepted by :func:`price_instrument`
            and :meth:`CDSIndex.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def to_dict(self) -> dict[str, object]:
        """
        Serde form of the instrument spec as a plain Python ``dict``.

        Returns
        -------
        dict[str, object]
            The ``spec`` object of the instrument envelope (JSON-compatible
            values: ``str``, ``float``, ``dict``, ``list``, ``None``).

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def price(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        model: str = "default",
        metrics: list[str] | None = None,
        pricing_options: dict[str, object] | str | None = None,
        market_history: str | None = None,
    ) -> ValuationResult:
        """
        Price this instrument and return a :class:`~finstack_quant.valuations.ValuationResult`.

        Same pipeline and keyword surface as :func:`price_instrument`.

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date (ISO 8601 strings accepted).
        model : str, default "default"
            Model key (``"hazard_rate"`` is the native model).
        metrics : list[str], optional
            Metric identifiers to compute (see :func:`list_standard_metrics`).
        pricing_options : dict[str, object] | str, optional
            ``MetricPricingOverrides`` merged into the instrument's own overrides
            (e.g. ``{"theta_period": "1D"}``).
        market_history : str, optional
            JSON ``MarketHistory`` scenarios required by ``hvar`` /
            ``expected_shortfall``.

        Returns
        -------
        ValuationResult
            Typed valuation envelope with price, currency and metrics.

        Raises
        ------
        ValueError
            If an input cannot be interpreted or the instrument fails validation.
        KeyError
            If a required curve, surface or metric is missing from ``market``.
        RuntimeError
            If pricing or a metric computation fails.
        """
        ...
    def metric(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        metric_id: str,
        model: str = "default",
    ) -> float:
        """
        Compute one scalar metric for this instrument (e.g. ``"cs01_hazard"``).

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.
        metric_id : str
            Registered metric identifier (see :func:`list_standard_metrics`).
        model : str, default "default"
            Model key.

        Returns
        -------
        float
            The metric value in the metric's native unit (decimal for rates and
            yields, currency units for DV01/CS01-style sensitivities, basis
            points for spreads).

        Raises
        ------
        ValueError
            If ``metric_id`` is unknown or an input cannot be interpreted.
        KeyError
            If a required curve or surface is missing from ``market``.
        RuntimeError
            If the metric computation fails.
        """
        ...
    def market_dependencies(self) -> dict[str, object]:
        """
        Market-data dependencies declared by the Rust ``Instrument`` trait.

        Returns
        -------
        dict[str, object]
            Serde form of ``MarketDependencies`` (``curves`` grouped by role,
            ``credit_index_ids``, ``market_scalar_ids``,
            ``volatility_dependencies``, ``fx_pairs``, ``series_ids``).

        Raises
        ------
        ValueError
            If the instrument cannot enumerate its dependencies.
        """
        ...
    @property
    def id(self) -> str:
        """
        Stable instrument identifier used in market lookup and results.

        Returns
        -------
        str
            The unique instrument identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def default_model(self) -> str:
        """
        Canonical model key used when ``model="default"`` is passed to ``price``.

        Returns
        -------
        str
            Registered model key such as ``"hazard_rate"`` or ``"black76"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def attributes(self) -> Attributes:
        """
        Instrument attributes (tags and metadata) used for scenario selection.

        Returns
        -------
        Attributes
            The attribute bag; empty when none were set.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @staticmethod
    def example() -> CDSIndex:
        """
        Canonical CDX.NA.IG series 42 USD 10,000,000 payer (mirrors Rust
        ``CDSIndex::example``): 60bp running spread, ``single_curve`` pricing off
        ``CDX.NA.IG.HAZARD`` discounted on ``USD-OIS``, premium 2024-03-20 to
        2029-12-20, 125 names.

        Returns
        -------
        CDSIndex
            The example index trade.

        Notes
        -----
        This factory does not raise; the example is built from constants.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CDSIndex
        >>> CDSIndex.example().protection.credit_curve_id
        'CDX.NA.IG.HAZARD'
        """
        ...
    @staticmethod
    def from_preset(
        preset: CDSIndexParams,
        id: str,
        notional: Money,
        side: Literal["pay", "receive"],
        start: datetime.date | datetime.datetime | pd.Timestamp | str,
        end: datetime.date | datetime.datetime | pd.Timestamp | str,
        recovery_rate: float,
        discount_curve_id: str,
        credit_curve_id: str,
    ) -> CDSIndex:
        """
        Build an index trade from a standardized preset (mirrors Rust
        ``CDSIndex::from_preset``): the premium leg takes the preset's fixed
        coupon and regional convention (day count, frequency, business-day rule,
        calendar, stub), pricing is ``"single_curve"``, ``index_factor`` is
        ``1.0`` and the constituent list is empty.

        Parameters
        ----------
        preset : CDSIndexParams
            Index identity, coupon and convention (e.g. :meth:`CDSIndexParams.cdx_na_ig`).
        id : str
            Unique instrument identifier for the trade.
        notional : Money
            Index notional.
        side : {"pay", "receive"}
            ``"pay"`` buys protection, ``"receive"`` sells protection.
        start : datetime.date | datetime.datetime | pd.Timestamp | str
            Premium accrual start (typically the last IMM roll).
        end : datetime.date | datetime.datetime | pd.Timestamp | str
            Scheduled maturity (an IMM date).
        recovery_rate : float
            Assumed recovery as a fraction (``0.4`` = 40%).
        discount_curve_id : str
            Discount curve identifier.
        credit_curve_id : str
            Index hazard curve identifier.

        Returns
        -------
        CDSIndex
            The index trade.

        Raises
        ------
        ValueError
            If ``side`` is unknown, a date cannot be interpreted, or the preset
            coupon is not representable.

        Examples
        --------
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.instruments import CDSIndex, CDSIndexParams
        >>> idx = CDSIndex.from_preset(
        ...     CDSIndexParams.cdx_na_ig(42, 1, 100.0),
        ...     "CDX-42-5Y",
        ...     Money(10_000_000.0, Currency("USD")),
        ...     "pay",
        ...     "2024-03-20",
        ...     "2029-06-20",
        ...     0.4,
        ...     "USD-OIS",
        ...     "CDX.NA.IG.HAZARD",
        ... )
        >>> (idx.convention, idx.num_constituents, idx.index_factor)
        ('isda_na', 125, 1.0)
        """
        ...
    def par_spread(
        self, market: MarketContext | str, as_of: datetime.date | datetime.datetime | pd.Timestamp | str
    ) -> float:
        """
        Par spread of the index in basis points (mirrors Rust
        ``CDSIndex::par_spread``; risky-annuity denominator in ``single_curve``
        mode, weighted constituents otherwise).

        Parameters
        ----------
        market : MarketContext | str
            Market carrying the discount and hazard curves the index names.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.

        Returns
        -------
        float
            Par spread in basis points.

        Raises
        ------
        KeyError
            If a curve is missing from ``market``.
        ValueError
            If ``as_of`` or the market JSON is invalid.
        """
        ...
    def risky_pv01(
        self, market: MarketContext | str, as_of: datetime.date | datetime.datetime | pd.Timestamp | str
    ) -> float:
        """
        Risky PV01 (risky annuity) of the premium leg (mirrors Rust
        ``CDSIndex::risky_pv01``): PV of 1bp running on the surviving notional.

        Parameters
        ----------
        market : MarketContext | str
            Market carrying the discount and hazard curves the index names.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.

        Returns
        -------
        float
            Risky PV01 in notional currency units per basis point.

        Raises
        ------
        KeyError
            If a curve is missing from ``market``.
        ValueError
            If ``as_of`` or the market JSON is invalid.
        """
        ...
    def cs01(self, market: MarketContext | str, as_of: datetime.date | datetime.datetime | pd.Timestamp | str) -> float:
        """
        Credit spread sensitivity (mirrors Rust ``CDSIndex::cs01`` with the cached
        recalibration provider): the hazard curve(s) are rebootstrapped after a
        1bp parallel spread bump. Hazard curves built by hand (no calibration
        recipe) raise; use ``metric(market, as_of, "cs01_hazard")`` for a direct
        hazard-rate bump instead.

        Parameters
        ----------
        market : MarketContext | str
            Market carrying the discount and hazard curves the index names.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.

        Returns
        -------
        float
            PV change for a +1bp spread move, in notional currency units.

        Raises
        ------
        KeyError
            If a curve is missing from ``market``.
        ValueError
            If the hazard curve carries no lossless calibration recipe.
        RuntimeError
            If the recalibration fails.
        """
        ...
    @property
    def index_name(self) -> str:
        """
        Ticker of the credit index family this contract references.

        Returns
        -------
        str
            Index family ticker as supplied at construction, for example
            ``"CDX.NA.IG"`` or ``"iTraxx Europe"``. The value is stored
            verbatim and is not normalised or validated against a registry.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def series(self) -> int:
        """
        Roll series of the credit index, incremented each semi-annual roll.

        Returns
        -------
        int
            Series number as an unsigned integer (for example ``41`` for
            CDX.NA.IG series 41). Higher numbers denote more recent
            on-the-run rolls.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def version(self) -> int:
        """
        Version within the series.

        Returns
        -------
        int
            The version number.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def notional(self) -> Money:
        """
        Traded notional of the index position, carrying its own currency.

        Returns
        -------
        Money
            Currency-tagged notional in the index deal currency (USD for
            CDX, EUR for iTraxx). It is the full original notional and is
            not scaled by the index factor; apply
            :attr:`index_factor` to obtain the current outstanding amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def index_factor(self) -> float:
        """
        Fraction of surviving notional.

        Returns
        -------
        float
            ``1.0`` when no constituent has defaulted since inception.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def side(self) -> str:
        """
        Protection perspective.

        Returns
        -------
        str
            ``"pay"`` (buy protection) or ``"receive"`` (sell protection).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def convention(self) -> str:
        """
        Regional ISDA convention (serde name).

        Returns
        -------
        str
            ``"isda_na"``, ``"isda_eu"``, ``"isda_as"`` or ``"custom"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def premium(self) -> PremiumLegSpec:
        """
        Premium leg specification.

        Returns
        -------
        PremiumLegSpec
            The premium leg.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def protection(self) -> ProtectionLegSpec:
        """
        Protection leg specification.

        Returns
        -------
        ProtectionLegSpec
            The protection leg.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def pricing(self) -> str:
        """
        Pricing aggregation mode.

        Returns
        -------
        str
            ``"single_curve"`` or ``"constituents"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def constituents(self) -> list[CDSIndexConstituent]:
        """
        Constituent rows.

        Returns
        -------
        list[CDSIndexConstituent]
            Typed rows; empty in ``single_curve`` mode.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def num_constituents(self) -> int | None:
        """
        Number of names in the pool.

        Returns
        -------
        int | None
            The count, or ``None`` when unset.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def margin_spec(self) -> dict[str, object] | None:
        """
        OTC margin specification in serde form.

        Returns
        -------
        dict[str, object] | None
            The ``OtcMarginSpec`` dict, or ``None`` for unmargined trades.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def expiry(self) -> datetime.date | None:
        """
        Premium-leg end date as seen by the pricer.

        Returns
        -------
        datetime.date | None
            The scheduled maturity, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style rendering of the key economics, e.g. ``CDSIndex(id='CDX-IG-42', index_name='CDX.NA.IG', series=42, side='pay', notional=Money(10000000.0, 'USD'), spread_bp=60, ...)``.

        Returns
        -------
        str
            ``CDSIndex(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise; it renders stored values.
        """
        ...

class CDSIndexBuilder:
    """
    Fluent builder for :class:`CDSIndex`; wraps the Rust ``FinancialBuilder``
    output one setter for one setter.

    The builder pre-seeds an empty ``constituents`` list so ``build()``
    succeeds without calling :meth:`constituents` in ``"single_curve"`` mode.
    Builders are consumed by ``build()``; create a new builder per
    instrument. Required fields: ``id``, ``index_name``, ``series``,
    ``version``, ``notional``, ``index_factor``, ``side``, ``convention``,
    ``premium``, ``protection``, ``pricing``.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.dates import DayCount, Tenor
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.valuations.instruments import (
    ...     CDSIndex,
    ...     PremiumLegSpec,
    ...     ProtectionLegSpec,
    ... )
    >>> premium = PremiumLegSpec(
    ...     datetime.date(2024, 3, 20),
    ...     datetime.date(2029, 6, 20),
    ...     Tenor.quarterly(),
    ...     DayCount.ACT_360,
    ...     100.0,
    ...     "USD-OIS",
    ... )
    >>> index = (
    ...     CDSIndex
    ...     .builder()
    ...     .id("CDX-IG-42")
    ...     .index_name("CDX.NA.IG")
    ...     .series(42)
    ...     .version(1)
    ...     .notional(Money(10_000_000.0, Currency("USD")))
    ...     .index_factor(1.0)
    ...     .side("pay")
    ...     .convention("isda_na")
    ...     .premium(premium)
    ...     .protection(ProtectionLegSpec("CDX-IG-42-HZD", 0.4, 3))
    ...     .pricing("single_curve")
    ...     .num_constituents(125)
    ...     .build()
    ... )
    >>> index.pricing
    'single_curve'
    """

    def id(self, value: str) -> CDSIndexBuilder:
        """
        Set the instrument identifier.

        Parameters
        ----------
        value : str
            Unique identifier for the index trade.

        Returns
        -------
        CDSIndexBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def index_name(self, value: str) -> CDSIndexBuilder:
        """
        Set the index name.

        Parameters
        ----------
        value : str
            Index name, e.g. ``"CDX.NA.IG"``, ``"CDX.NA.HY"``, ``"iTraxx Europe"``.

        Returns
        -------
        CDSIndexBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def series(self, value: int) -> CDSIndexBuilder:
        """
        Set the series number.

        Parameters
        ----------
        value : int
            Series number, e.g. ``42``.

        Returns
        -------
        CDSIndexBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def version(self, value: int) -> CDSIndexBuilder:
        """
        Set the version number within the series.

        Parameters
        ----------
        value : int
            Version number, e.g. ``1``.

        Returns
        -------
        CDSIndexBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def notional(self, value: Money) -> CDSIndexBuilder:
        """
        Set the notional amount of the index.

        Parameters
        ----------
        value : Money
            Notional amount of the index.

        Returns
        -------
        CDSIndexBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def index_factor(self, value: float) -> CDSIndexBuilder:
        """
        Set the index factor (fraction of surviving notional).

        Parameters
        ----------
        value : float
            Index factor in ``[0.0, 1.0]``; ``1.0`` means no constituent has defaulted since series inception.

        Returns
        -------
        CDSIndexBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def side(self, value: Literal["pay", "receive"]) -> CDSIndexBuilder:
        """
        Set the protection buyer/seller perspective.

        Parameters
        ----------
        value : Literal["pay", "receive"]
            ``"pay"`` to buy protection (pay premium), ``"receive"`` to sell protection (receive premium).

        Returns
        -------
        CDSIndexBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized side.
        """
        ...
    def convention(self, value: Literal["isda_na", "isda_eu", "isda_as", "custom"]) -> CDSIndexBuilder:
        """
        Set the ISDA regional convention.

        Parameters
        ----------
        value : Literal["isda_na", "isda_eu", "isda_as", "custom"]
            ``"isda_na"`` is the SNAC / post-Big-Bang North American standard; ``"isda_eu"`` European; ``"isda_as"`` Asian; ``"custom"`` for a manually configured convention.

        Returns
        -------
        CDSIndexBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not one of the accepted strings (the message lists them).
        """
        ...
    def premium(self, value: PremiumLegSpec) -> CDSIndexBuilder:
        """
        Set the premium leg specification.

        Parameters
        ----------
        value : PremiumLegSpec
            Premium leg specification (coupon schedule and discounting).

        Returns
        -------
        CDSIndexBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def protection(self, value: ProtectionLegSpec) -> CDSIndexBuilder:
        """
        Set the protection leg specification.

        Parameters
        ----------
        value : ProtectionLegSpec
            Protection leg specification (credit curve and settlement).

        Returns
        -------
        CDSIndexBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def pricing(self, value: Literal["single_curve", "constituents"]) -> CDSIndexBuilder:
        """
        Set the pricing aggregation mode.

        Parameters
        ----------
        value : Literal["single_curve", "constituents"]
            ``"single_curve"`` prices the index against a single index hazard curve (synthetic CDS). ``"constituents"`` prices each issuer separately and aggregates by weight; requires :meth:`CDSIndexBuilder.constituents` to be set.

        Returns
        -------
        CDSIndexBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized pricing mode.
        """
        ...
    def constituents(self, value: list[CDSIndexConstituent | dict[str, object]] | str) -> CDSIndexBuilder:
        """
        Set the index constituents.

        Parameters
        ----------
        value : list[CDSIndexConstituent | dict[str, object]] | str
            Constituent rows as typed :class:`CDSIndexConstituent` objects, dicts with ``credit`` (``reference_entity``, ``recovery_rate``, ``credit_curve_id``), ``weight`` and optional ``defaulted``, or a JSON array of the same shape.

        Returns
        -------
        CDSIndexBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or a dict/JSON entry does not match the constituent shape.
        TypeError
            If ``value`` is neither a list nor a string.
        """
        ...
    def num_constituents(self, value: int) -> CDSIndexBuilder:
        """
        Set the number of reference entities in the index pool.

        Parameters
        ----------
        value : int
            Number of names in the index pool, e.g. ``125`` for CDX.NA.IG; required for portfolio-level analytics (e.g. jump-to-default) when ``constituents`` is empty.

        Returns
        -------
        CDSIndexBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def margin_spec(self, value: dict[str, object] | str) -> CDSIndexBuilder:
        """
        Set the OTC margin (CSA / initial-margin) specification.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``OtcMarginSpec`` in serde form (``dict`` or JSON string).

        Returns
        -------
        CDSIndexBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does not deserialize as an ``OtcMarginSpec``.
        """
        ...
    def attributes(self, value: Attributes | dict[str, str] | None) -> CDSIndexBuilder:
        """
        Set instrument attributes (tags and metadata).

        Parameters
        ----------
        value : Attributes | dict[str, str] | None
            Attribute bag; a ``dict`` populates ``meta`` and an optional ``"tags"`` entry holding a list of strings populates ``tags``.

        Returns
        -------
        CDSIndexBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        TypeError
            If ``value`` is neither ``Attributes``, a ``dict`` nor ``None``.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style rendering of the fields set so far, e.g.
        ``CDSIndexBuilder(id='X', notional=Money(1000000.0, 'USD'))``.

        Returns
        -------
        str
            ``CDSIndexBuilder(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise; it renders the recorded setter calls.
        """
        ...
    def build(self) -> CDSIndex:
        """
        Build the validated CDS index.

        Runs only the Rust ``CDSIndexBuilder::build`` validation (structural
        invariants); pricing-time checks run in :meth:`CDSIndex.price`.

        Returns
        -------
        CDSIndex
            The validated instrument.

        Raises
        ------
        ValueError
            If the builder was already consumed, a required field is missing
            (the message names the builder and the field, e.g.
            ``CDSIndexBuilder: missing required field 'id'``), or the instrument
            fails validation (index factor outside ``[0, 1]``, empty constituents in ``constituents`` mode).
        """
        ...

class CDSTranche:
    """
    Synthetic CDO / index tranche (typed wrapper for the canonical Rust
    ``CDSTranche``): protection on portfolio losses between ``attach_pct``
    and ``detach_pct`` (percent points), paying ``running_coupon_bp`` on the
    surviving tranche notional, priced with the one-factor Gaussian copula
    against the ``credit_index_id`` loss distribution.

    Construct via :meth:`CDSTranche.standard` (standard quarterly ACT/360
    schedule), :meth:`CDSTranche.builder`, :meth:`CDSTranche.example` or
    :meth:`CDSTranche.from_json`. Every public Rust field is readable as a
    property; ``expected_loss`` / ``jump_to_default`` mirror the Rust
    accessors and :meth:`CDSTranche.price` / :meth:`CDSTranche.metric` run
    the same pricer as :func:`price_instrument`.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import CDSTranche
    >>> tranche = CDSTranche.example()
    >>> (tranche.attach_pct, tranche.detach_pct, tranche.side)
    (0.0, 3.0, 'buy_protection')
    """

    @staticmethod
    def builder() -> CDSTrancheBuilder:
        """
        Create a fluent builder (mirrors Rust ``CDSTranche::builder()``).

        Returns
        -------
        CDSTrancheBuilder
            A builder with fluent, consuming setter methods.

        Notes
        -----
        This factory does not raise; it returns an empty builder.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CDSTranche
        >>> builder = CDSTranche.builder()
        >>> builder.id("EXAMPLE") is builder
        True
        """
        ...
    @classmethod
    def from_json(cls, json: str) -> CDSTranche:
        """
        Deserialize a validated CDSTranche from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"cds_tranche"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        CDSTranche
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CDSTranche
        >>> CDSTranche.from_json(CDSTranche.example().to_json()).id
        'CDXIG-42-0X3'
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to a canonical ``finstack_quant.instrument/1`` envelope.

        Returns
        -------
        str
            Canonical instrument envelope accepted by :func:`price_instrument`
            and :meth:`CDSTranche.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def to_dict(self) -> dict[str, object]:
        """
        Serde form of the instrument spec as a plain Python ``dict``.

        Returns
        -------
        dict[str, object]
            The ``spec`` object of the instrument envelope (JSON-compatible
            values: ``str``, ``float``, ``dict``, ``list``, ``None``).

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def price(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        model: str = "default",
        metrics: list[str] | None = None,
        pricing_options: dict[str, object] | str | None = None,
        market_history: str | None = None,
    ) -> ValuationResult:
        """
        Price this instrument and return a :class:`~finstack_quant.valuations.ValuationResult`.

        Same pipeline and keyword surface as :func:`price_instrument`.

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date (ISO 8601 strings accepted).
        model : str, default "default"
            Model key (the copula tranche pricer is the native model).
        metrics : list[str], optional
            Metric identifiers to compute (see :func:`list_standard_metrics`).
        pricing_options : dict[str, object] | str, optional
            ``MetricPricingOverrides`` merged into the instrument's own overrides
            (e.g. ``{"theta_period": "1D"}``).
        market_history : str, optional
            JSON ``MarketHistory`` scenarios required by ``hvar`` /
            ``expected_shortfall``.

        Returns
        -------
        ValuationResult
            Typed valuation envelope with price, currency and metrics.

        Raises
        ------
        ValueError
            If an input cannot be interpreted or the instrument fails validation.
        KeyError
            If a required curve, surface or metric is missing from ``market``.
        RuntimeError
            If pricing or a metric computation fails.
        """
        ...
    def metric(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        metric_id: str,
        model: str = "default",
    ) -> float:
        """
        Compute one scalar metric for this instrument (e.g. ``"expected_loss"``).

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.
        metric_id : str
            Registered metric identifier (see :func:`list_standard_metrics`).
        model : str, default "default"
            Model key.

        Returns
        -------
        float
            The metric value in the metric's native unit (decimal for rates and
            yields, currency units for DV01/CS01-style sensitivities, basis
            points for spreads).

        Raises
        ------
        ValueError
            If ``metric_id`` is unknown or an input cannot be interpreted.
        KeyError
            If a required curve or surface is missing from ``market``.
        RuntimeError
            If the metric computation fails.
        """
        ...
    def market_dependencies(self) -> dict[str, object]:
        """
        Market-data dependencies declared by the Rust ``Instrument`` trait.

        Returns
        -------
        dict[str, object]
            Serde form of ``MarketDependencies`` (``curves`` grouped by role,
            ``credit_index_ids``, ``market_scalar_ids``,
            ``volatility_dependencies``, ``fx_pairs``, ``series_ids``).

        Raises
        ------
        ValueError
            If the instrument cannot enumerate its dependencies.
        """
        ...
    @property
    def id(self) -> str:
        """
        Stable instrument identifier used in market lookup and results.

        Returns
        -------
        str
            The unique instrument identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def default_model(self) -> str:
        """
        Canonical model key used when ``model="default"`` is passed to ``price``.

        Returns
        -------
        str
            Registered model key such as ``"hazard_rate"`` or ``"black76"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def attributes(self) -> Attributes:
        """
        Instrument attributes (tags and metadata) used for scenario selection.

        Returns
        -------
        Attributes
            The attribute bag; empty when none were set.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @staticmethod
    def example() -> CDSTranche:
        """
        Canonical CDX.NA.IG 42 equity (0–3%) tranche, USD 10,000,000 (mirrors
        Rust ``CDSTranche::example``): buy protection, 100bp running, maturity
        2029-12-20, curves ``USD-OIS`` / ``CDX.NA.IG.HAZARD``.

        Returns
        -------
        CDSTranche
            The example tranche.

        Notes
        -----
        This factory does not raise; the example is built from constants.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CDSTranche
        >>> CDSTranche.example().credit_index_id
        'CDX.NA.IG.HAZARD'
        """
        ...
    @staticmethod
    def standard(
        id: str,
        params: CDSTrancheParams,
        discount_curve_id: str,
        credit_index_id: str,
        side: Literal["buy_protection", "sell_protection"],
    ) -> CDSTranche:
        """
        Build a tranche on the standard schedule (mirrors Rust
        ``CDSTranche::standard``): quarterly, ACT/360, Following, weekends-only
        calendar, short-front stub.

        Parameters
        ----------
        id : str
            Unique instrument identifier.
        params : CDSTrancheParams
            Economic terms (attach/detach, notional, maturity, coupon).
        discount_curve_id : str
            Discount curve identifier.
        credit_index_id : str
            Credit index identifier for the loss distribution.
        side : {"buy_protection", "sell_protection"}
            Tranche side.

        Returns
        -------
        CDSTranche
            The validated tranche.

        Raises
        ------
        ValueError
            If ``side`` is unknown or the parameters fail validation
            (``attach_pct >= detach_pct``, fractional attach/detach, ...).

        Examples
        --------
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.instruments import CDSTranche, CDSTrancheParams
        >>> params = CDSTrancheParams.mezzanine_tranche(
        ...     "CDX.NA.IG", 42, Money(1e7, Currency("USD")), "2029-12-20", 100.0
        ... )
        >>> tranche = CDSTranche.standard("CDX-42-3X7", params, "USD-OIS", "CDX.NA.IG.HAZARD", "buy_protection")
        >>> (tranche.day_count, tranche.business_day_convention)
        ('act_360', 'following')
        """
        ...
    def expected_loss(self, market: MarketContext | str) -> float:
        """
        Expected tranche loss as a fraction of tranche notional (mirrors Rust
        ``CDSTranche::expected_loss``).

        Parameters
        ----------
        market : MarketContext | str
            Market carrying the credit index and discount curve.

        Returns
        -------
        float
            Expected loss fraction in ``[0, 1]``.

        Raises
        ------
        KeyError
            If the credit index is missing from ``market``.
        RuntimeError
            If the loss-distribution integration fails.
        """
        ...
    def jump_to_default(
        self, market: MarketContext | str, as_of: datetime.date | datetime.datetime | pd.Timestamp | str
    ) -> float:
        """
        Jump-to-default exposure (mirrors Rust ``CDSTranche::jump_to_default``):
        PV impact of one constituent defaulting immediately.

        Parameters
        ----------
        market : MarketContext | str
            Market carrying the credit index and discount curve.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.

        Returns
        -------
        float
            Jump-to-default PV change in notional currency units.

        Raises
        ------
        KeyError
            If the credit index is missing from ``market``.
        RuntimeError
            If the loss-distribution integration fails.
        """
        ...
    @property
    def index_name(self) -> str:
        """
        Underlying index name.

        Returns
        -------
        str
            The index name.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def series(self) -> int:
        """
        Index series number.

        Returns
        -------
        int
            The series number.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def attach_pct(self) -> float:
        """
        Attachment point in percent.

        Returns
        -------
        float
            Attachment (``3.0`` = 3%).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def detach_pct(self) -> float:
        """
        Detachment point in percent.

        Returns
        -------
        float
            Detachment (``7.0`` = 7%).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def notional(self) -> Money:
        """
        Tranche notional.

        Returns
        -------
        Money
            Currency-tagged notional.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def maturity(self) -> datetime.date:
        """
        Scheduled maturity.

        Returns
        -------
        datetime.date
            The maturity date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def running_coupon_bp(self) -> float:
        """
        Fixed running spread paid on the tranche premium leg.

        Returns
        -------
        float
            Coupon quoted in basis points per annum on the outstanding
            tranche notional (for example ``100.0`` for a 100 bp coupon),
            not as a decimal rate. Accrues on the premium-leg day count.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def frequency(self) -> Tenor:
        """
        Payment frequency.

        Returns
        -------
        Tenor
            The coupon tenor (typically quarterly).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def day_count(self) -> str:
        """
        Day count convention (serde name).

        Returns
        -------
        str
            ``"act_360"`` for standard tranches.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def business_day_convention(self) -> str:
        """
        Business day convention (serde name).

        Returns
        -------
        str
            ``"modified_following"`` unless set otherwise.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def calendar_id(self) -> str | None:
        """
        Holiday calendar identifier.

        Returns
        -------
        str | None
            The calendar id, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def discount_curve_id(self) -> str:
        """
        Discount curve identifier.

        Returns
        -------
        str
            The curve id.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def credit_index_id(self) -> str:
        """
        Credit index identifier for the loss distribution.

        Returns
        -------
        str
            The credit index id.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def side(self) -> str:
        """
        Direction of the tranche position from the holder's perspective.

        Returns
        -------
        str
            Serde string, either ``"buy_protection"`` (pays the running
            coupon and receives tranche loss payments) or
            ``"sell_protection"`` (receives the coupon and pays losses).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def effective_date(self) -> datetime.date | None:
        """
        Explicit effective date for schedule anchoring.

        Returns
        -------
        datetime.date | None
            The date, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def accumulated_loss(self) -> float:
        """
        Realized portfolio loss so far.

        Returns
        -------
        float
            Fraction of the original portfolio notional.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def standard_imm_dates(self) -> bool:
        """
        Whether coupon dates are forced onto standard IMM dates.

        Returns
        -------
        bool
            ``True`` when IMM rolling is enforced.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def upfront(self) -> tuple[datetime.date, Money] | None:
        """
        Upfront payment as ``(payment_date, amount)``.

        Returns
        -------
        tuple[datetime.date, Money] | None
            The pair, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def expiry(self) -> datetime.date | None:
        """
        Maturity as seen by the pricer.

        Returns
        -------
        datetime.date | None
            The maturity, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style rendering of the key economics, e.g. ``CDSTranche(id='CDXIG-42-0X3', index_name='CDX.NA.IG', series=42, attach_pct=0.0, detach_pct=3.0, side='buy_protection', ...)``.

        Returns
        -------
        str
            ``CDSTranche(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise; it renders stored values.
        """
        ...

class CDSTrancheBuilder:
    """
    Fluent builder for :class:`CDSTranche`; wraps the Rust
    ``FinancialBuilder`` output one setter for one setter.

    The builder pre-seeds ``accumulated_loss(0.0)``; ``standard_imm_dates``
    defaults to ``False`` and ``business_day_convention`` to
    ``"modified_following"``. Builders are consumed by ``build()``; create a
    new builder per instrument. Required fields: ``id``, ``index_name``,
    ``series``, ``attach_pct``, ``detach_pct``, ``notional``, ``maturity``,
    ``running_coupon_bp``, ``frequency``, ``day_count``,
    ``discount_curve_id``, ``credit_index_id``, ``side``.

    Examples
    --------
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.dates import DayCount, Tenor
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.valuations.instruments import CDSTranche
    >>> tranche = (
    ...     CDSTranche
    ...     .builder()
    ...     .id("CDX-IG-42-3-7")
    ...     .index_name("CDX.NA.IG")
    ...     .series(42)
    ...     .attach_pct(3.0)
    ...     .detach_pct(7.0)
    ...     .notional(Money(10_000_000.0, Currency("USD")))
    ...     .maturity("2029-06-20")
    ...     .running_coupon_bp(100.0)
    ...     .frequency(Tenor.quarterly())
    ...     .day_count(DayCount.ACT_360)
    ...     .discount_curve_id("USD-OIS")
    ...     .credit_index_id("CDX-IG-42-CURVE")
    ...     .side("buy_protection")
    ...     .build()
    ... )
    >>> tranche.running_coupon_bp
    100.0
    """

    def id(self, value: str) -> CDSTrancheBuilder:
        """
        Set the instrument identifier.

        Parameters
        ----------
        value : str
            Unique identifier for the tranche trade.

        Returns
        -------
        CDSTrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def index_name(self, value: str) -> CDSTrancheBuilder:
        """
        Set the underlying index name.

        Parameters
        ----------
        value : str
            Index name, e.g. ``"CDX.NA.IG"``, ``"CDX.NA.HY"``, ``"iTraxx EUR"``.

        Returns
        -------
        CDSTrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def series(self, value: int) -> CDSTrancheBuilder:
        """
        Set the series number.

        Parameters
        ----------
        value : int
            Series number, e.g. ``42``.

        Returns
        -------
        CDSTrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def attach_pct(self, value: float) -> CDSTrancheBuilder:
        """
        Set the attachment point.

        Parameters
        ----------
        value : float
            Attachment point quoted in percent (``0.0`` for equity; ``3.0`` for a tranche attaching at 3%).

        Returns
        -------
        CDSTrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def detach_pct(self, value: float) -> CDSTrancheBuilder:
        """
        Set the detachment point.

        Parameters
        ----------
        value : float
            Detachment point quoted in percent (``3.0`` for a 0-3% tranche).

        Returns
        -------
        CDSTrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def notional(self, value: Money) -> CDSTrancheBuilder:
        """
        Set the notional amount of the tranche.

        Parameters
        ----------
        value : Money
            Notional amount of the tranche.

        Returns
        -------
        CDSTrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def maturity(self, value: datetime.date | datetime.datetime | pd.Timestamp | str) -> CDSTrancheBuilder:
        """
        Set the maturity date of the tranche.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pd.Timestamp | str
            Maturity date (ISO 8601 strings accepted).

        Returns
        -------
        CDSTrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or the date cannot be interpreted.
        """
        ...
    def running_coupon_bp(self, value: float | Bps) -> CDSTrancheBuilder:
        """
        Set the running coupon.

        Parameters
        ----------
        value : float | Bps
            Running coupon in basis points (``100.0`` = 1.00%).

        Returns
        -------
        CDSTrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        TypeError
            If ``value`` is neither a number nor ``Bps``.
        """
        ...
    def frequency(self, value: Tenor | str) -> CDSTrancheBuilder:
        """
        Set the payment frequency.

        Parameters
        ----------
        value : Tenor | str
            Payment frequency (typically quarterly, ``"3M"``).

        Returns
        -------
        CDSTrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or a tenor string cannot be parsed.
        """
        ...
    def day_count(self, value: DayCount | str) -> CDSTrancheBuilder:
        """
        Set the day count convention.

        Parameters
        ----------
        value : DayCount | str
            Day count convention (typically ``DayCount.ACT_360`` / ``"act_360"``).

        Returns
        -------
        CDSTrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or a string name is not a recognized day count.
        """
        ...
    def business_day_convention(self, value: BusinessDayConvention | str) -> CDSTrancheBuilder:
        """
        Set the business day convention for coupon dates.

        Parameters
        ----------
        value : BusinessDayConvention | str
            Roll rule (``"modified_following"`` when never set).

        Returns
        -------
        CDSTrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or a string name is not a recognized convention.
        """
        ...
    def calendar_id(self, value: str) -> CDSTrancheBuilder:
        """
        Set the holiday calendar identifier.

        Parameters
        ----------
        value : str
            Holiday calendar identifier.

        Returns
        -------
        CDSTrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def discount_curve_id(self, value: str) -> CDSTrancheBuilder:
        """
        Set the discount curve identifier (by quote currency).

        Parameters
        ----------
        value : str
            Discount curve identifier.

        Returns
        -------
        CDSTrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def credit_index_id(self, value: str) -> CDSTrancheBuilder:
        """
        Set the credit index identifier for survival/loss modeling.

        Parameters
        ----------
        value : str
            Credit index identifier.

        Returns
        -------
        CDSTrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def side(self, value: Literal["buy_protection", "sell_protection"]) -> CDSTrancheBuilder:
        """
        Set the tranche side (buy/sell protection).

        Parameters
        ----------
        value : Literal["buy_protection", "sell_protection"]
            Tranche side.

        Returns
        -------
        CDSTrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized side.
        """
        ...
    def effective_date(self, value: datetime.date | datetime.datetime | pd.Timestamp | str) -> CDSTrancheBuilder:
        """
        Set the effective date for schedule anchoring.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pd.Timestamp | str
            Effective date; if never set, uses the as-of date (or standard IMM-date rolling, if ``standard_imm_dates`` is true).

        Returns
        -------
        CDSTrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or the date cannot be interpreted.
        """
        ...
    def accumulated_loss(self, value: float) -> CDSTrancheBuilder:
        """
        Set the accumulated realized loss.

        Parameters
        ----------
        value : float
            Accumulated realized loss as a fraction of the original portfolio notional; ``0.0`` when never set.

        Returns
        -------
        CDSTrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def standard_imm_dates(self, value: bool) -> CDSTrancheBuilder:
        """
        Set whether to enforce standard IMM dates.

        Parameters
        ----------
        value : bool
            Whether to enforce standard IMM dates (20th of Mar, Jun, Sep, Dec); ``False`` when never set.

        Returns
        -------
        CDSTrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def upfront(self, value: tuple[datetime.date | datetime.datetime | pd.Timestamp | str, Money]) -> CDSTrancheBuilder:
        """
        Set the upfront payment.

        Parameters
        ----------
        value : tuple[datetime.date | datetime.datetime | pd.Timestamp | str, Money]
            ``(payment_date, amount)``; the amount currency must match the tranche notional.

        Returns
        -------
        CDSTrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        TypeError
            If ``value`` is not a ``(date, Money)`` pair.
        """
        ...
    def attributes(self, value: Attributes | dict[str, str] | None) -> CDSTrancheBuilder:
        """
        Set instrument attributes (tags and metadata).

        Parameters
        ----------
        value : Attributes | dict[str, str] | None
            Attribute bag; a ``dict`` populates ``meta`` and an optional ``"tags"`` entry holding a list of strings populates ``tags``.

        Returns
        -------
        CDSTrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        TypeError
            If ``value`` is neither ``Attributes``, a ``dict`` nor ``None``.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style rendering of the fields set so far, e.g.
        ``CDSTrancheBuilder(id='X', notional=Money(1000000.0, 'USD'))``.

        Returns
        -------
        str
            ``CDSTrancheBuilder(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise; it renders the recorded setter calls.
        """
        ...
    def build(self) -> CDSTranche:
        """
        Build the validated CDS tranche.

        Runs only the Rust ``CDSTrancheBuilder::build`` validation (structural
        invariants); pricing-time checks run in :meth:`CDSTranche.price`.

        Returns
        -------
        CDSTranche
            The validated instrument.

        Raises
        ------
        ValueError
            If the builder was already consumed, a required field is missing
            (the message names the builder and the field, e.g.
            ``CDSTrancheBuilder: missing required field 'id'``), or the instrument
            fails validation (``attach_pct >= detach_pct``, fractional attach/detach, loss outside ``[0, 1]``).
        """
        ...

class ConvertibleBond:
    """
    Convertible bond (typed wrapper for the canonical Rust
    ``ConvertibleBond``): debt with an embedded equity conversion option,
    priced on a Tsiveriotis–Fernandes style tree. The bond floor discounts on
    ``credit_curve_id`` (falling back to ``discount_curve_id``), the equity
    component on the risk-free curve.

    Construct via :meth:`ConvertibleBond.builder`,
    :meth:`ConvertibleBond.example` / :meth:`ConvertibleBond.example_mandatory`
    or :meth:`ConvertibleBond.from_json`. Every public Rust field is readable
    as a property (typed :class:`ConversionSpec` / :class:`CallPutSchedule`
    where Rust has a struct); ``conversion_ratio`` / ``parity`` /
    ``conversion_premium`` / ``greeks`` mirror the Rust accessors and
    :meth:`ConvertibleBond.price` / :meth:`ConvertibleBond.metric` run the
    same pricer as :func:`price_instrument`.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import ConvertibleBond
    >>> cb = ConvertibleBond.example()
    >>> (cb.id, cb.conversion_ratio, cb.underlying_equity_id)
    ('CB-TECH-5Y', 25.0, 'TECH')
    """

    @staticmethod
    def builder() -> ConvertibleBondBuilder:
        """
        Create a fluent builder (mirrors Rust ``ConvertibleBond::builder()``).

        Returns
        -------
        ConvertibleBondBuilder
            A builder with fluent, consuming setter methods.

        Notes
        -----
        This factory does not raise; it returns an empty builder.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import ConvertibleBond
        >>> builder = ConvertibleBond.builder()
        >>> builder.id("EXAMPLE") is builder
        True
        """
        ...
    @classmethod
    def from_json(cls, json: str) -> ConvertibleBond:
        """
        Deserialize a validated ConvertibleBond from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"convertible_bond"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        ConvertibleBond
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import ConvertibleBond
        >>> ConvertibleBond.from_json(ConvertibleBond.example().to_json()).id
        'CB-TECH-5Y'
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to a canonical ``finstack_quant.instrument/1`` envelope.

        Returns
        -------
        str
            Canonical instrument envelope accepted by :func:`price_instrument`
            and :meth:`ConvertibleBond.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def to_dict(self) -> dict[str, object]:
        """
        Serde form of the instrument spec as a plain Python ``dict``.

        Returns
        -------
        dict[str, object]
            The ``spec`` object of the instrument envelope (JSON-compatible
            values: ``str``, ``float``, ``dict``, ``list``, ``None``).

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def price(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        model: str = "default",
        metrics: list[str] | None = None,
        pricing_options: dict[str, object] | str | None = None,
        market_history: str | None = None,
    ) -> ValuationResult:
        """
        Price this instrument and return a :class:`~finstack_quant.valuations.ValuationResult`.

        Same pipeline and keyword surface as :func:`price_instrument`.

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date (ISO 8601 strings accepted).
        model : str, default "default"
            Model key (the convertible tree pricer is the native model).
        metrics : list[str], optional
            Metric identifiers to compute (see :func:`list_standard_metrics`).
        pricing_options : dict[str, object] | str, optional
            ``MetricPricingOverrides`` merged into the instrument's own overrides
            (e.g. ``{"theta_period": "1D"}``).
        market_history : str, optional
            JSON ``MarketHistory`` scenarios required by ``hvar`` /
            ``expected_shortfall``.

        Returns
        -------
        ValuationResult
            Typed valuation envelope with price, currency and metrics.

        Raises
        ------
        ValueError
            If an input cannot be interpreted or the instrument fails validation.
        KeyError
            If a required curve, surface or metric is missing from ``market``.
        RuntimeError
            If pricing or a metric computation fails.
        """
        ...
    def metric(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        metric_id: str,
        model: str = "default",
    ) -> float:
        """
        Compute one scalar metric for this instrument (e.g. ``"delta"`` or ``"bond_floor"``).

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.
        metric_id : str
            Registered metric identifier (see :func:`list_standard_metrics`).
        model : str, default "default"
            Model key.

        Returns
        -------
        float
            The metric value in the metric's native unit (decimal for rates and
            yields, currency units for DV01/CS01-style sensitivities, basis
            points for spreads).

        Raises
        ------
        ValueError
            If ``metric_id`` is unknown or an input cannot be interpreted.
        KeyError
            If a required curve or surface is missing from ``market``.
        RuntimeError
            If the metric computation fails.
        """
        ...
    def market_dependencies(self) -> dict[str, object]:
        """
        Market-data dependencies declared by the Rust ``Instrument`` trait.

        Returns
        -------
        dict[str, object]
            Serde form of ``MarketDependencies`` (``curves`` grouped by role,
            ``credit_index_ids``, ``market_scalar_ids``,
            ``volatility_dependencies``, ``fx_pairs``, ``series_ids``).

        Raises
        ------
        ValueError
            If the instrument cannot enumerate its dependencies.
        """
        ...
    @property
    def id(self) -> str:
        """
        Stable instrument identifier used in market lookup and results.

        Returns
        -------
        str
            The unique instrument identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def default_model(self) -> str:
        """
        Canonical model key used when ``model="default"`` is passed to ``price``.

        Returns
        -------
        str
            Registered model key such as ``"hazard_rate"`` or ``"black76"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def attributes(self) -> Attributes:
        """
        Instrument attributes (tags and metadata) used for scenario selection.

        Returns
        -------
        Attributes
            The attribute bag; empty when none were set.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @staticmethod
    def example() -> ConvertibleBond:
        """
        Canonical 5-year USD 1,000,000 2% semi-annual convertible (mirrors Rust
        ``ConvertibleBond::example``): ratio 25 shares per bond, voluntary
        conversion, underlying ``"TECH"``, curves ``USD-IG`` / ``USD-CREDIT-BBB``,
        issue 2024-01-15, maturity 2029-01-15.

        Returns
        -------
        ConvertibleBond
            The example bond.

        Raises
        ------
        ValueError
            If the canonical example fails validation (never for a released build).

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import ConvertibleBond
        >>> ConvertibleBond.example().credit_curve_id
        'USD-CREDIT-BBB'
        """
        ...
    @staticmethod
    def example_mandatory() -> ConvertibleBond:
        """
        Mandatory (PERCS/DECS-style) convertible example (mirrors Rust
        ``ConvertibleBond::example_mandatory``): 3-year 5% semi-annual,
        mandatory-variable conversion at maturity (upper conversion price 60,
        lower 40), 130% soft call, call at 101% after year 2 and put at 100%
        after year 1.

        Returns
        -------
        ConvertibleBond
            The example bond.

        Raises
        ------
        ValueError
            If the canonical example fails validation (never for a released build).

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import ConvertibleBond
        >>> ConvertibleBond.example_mandatory().soft_call_trigger["threshold_pct"]
        130.0
        """
        ...
    def parity(self, market: MarketContext | str) -> float:
        """
        Conversion value (parity) of the bond (mirrors Rust
        ``ConvertibleBond::parity``): ``effective_conversion_ratio * spot`` where
        spot is the market price of ``underlying_equity_id``.

        Parameters
        ----------
        market : MarketContext | str
            Market carrying the underlying equity price.

        Returns
        -------
        float
            Parity in notional currency units per bond.

        Raises
        ------
        KeyError
            If the underlying price is missing from ``market``.
        ValueError
            If the bond has no ``underlying_equity_id``.
        """
        ...
    def conversion_premium(self, market: MarketContext | str, bond_price: float) -> float:
        """
        Conversion premium over parity (mirrors Rust
        ``ConvertibleBond::conversion_premium``): ``bond_price / parity - 1``.

        Parameters
        ----------
        market : MarketContext | str
            Market carrying the underlying equity price.
        bond_price : float
            Observed bond price in notional currency units per bond.

        Returns
        -------
        float
            Conversion premium as a decimal fraction (``0.15`` = 15%).

        Raises
        ------
        KeyError
            If the underlying price is missing from ``market``.
        ValueError
            If the bond has no ``underlying_equity_id`` or parity is zero.
        """
        ...
    def greeks(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        bump_size: float | None = None,
    ) -> dict[str, float]:
        """
        Tree Greeks of the convertible (mirrors Rust ``ConvertibleBond::greeks``
        with the default tree).

        Parameters
        ----------
        market : MarketContext | str
            Market carrying the curves, underlying price and volatility.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.
        bump_size : float | None
            Finite-difference bump for delta/gamma as a fraction of spot;
            ``None`` uses the pricer default.

        Returns
        -------
        dict[str, float]
            ``price``, ``delta``, ``gamma``, ``vega``, ``theta``, ``rho``.

        Raises
        ------
        KeyError
            If required market data is missing.
        RuntimeError
            If the tree pricer fails.
        """
        ...
    @property
    def notional(self) -> Money:
        """
        Principal amount.

        Returns
        -------
        Money
            Currency-tagged principal.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def issue_date(self) -> datetime.date:
        """
        Dated date from which the bond starts accruing interest.

        Returns
        -------
        datetime.date
            Calendar date, unadjusted for business days. It anchors the
            coupon schedule and the first accrual period.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def maturity(self) -> datetime.date:
        """
        Scheduled redemption date on which principal is repaid.

        Returns
        -------
        datetime.date
            Unadjusted calendar maturity; payment dates derived from it are
            rolled by the instrument's business-day convention.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def discount_curve_id(self) -> str:
        """
        Discount curve identifier for the debt component.

        Returns
        -------
        str
            The curve id.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def credit_curve_id(self) -> str | None:
        """
        Credit curve identifier for risky discounting.

        Returns
        -------
        str | None
            The curve id, or ``None`` (falls back to ``discount_curve_id``).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def conversion(self) -> ConversionSpec:
        """
        Conversion terms.

        Returns
        -------
        ConversionSpec
            The typed conversion spec.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def conversion_ratio(self) -> float | None:
        """
        Base conversion ratio (shares per bond), derived from ratio or price (mirrors Rust ``conversion_ratio``).

        Returns
        -------
        float | None
            The ratio, or ``None`` when neither ratio nor price is set.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def effective_conversion_ratio(self) -> float | None:
        """
        Conversion ratio after anti-dilution adjustments (mirrors Rust ``effective_conversion_ratio``).

        Returns
        -------
        float | None
            The adjusted ratio, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def underlying_equity_id(self) -> str | None:
        """
        Underlying equity identifier.

        Returns
        -------
        str | None
            The id, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def call_put(self) -> CallPutSchedule | None:
        """
        Call/put schedule.

        Returns
        -------
        CallPutSchedule | None
            The typed schedule, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def soft_call_trigger(self) -> dict[str, object] | None:
        """
        Soft-call trigger (``threshold_pct``, ``observation_days``, ``required_days_above``).

        Returns
        -------
        dict[str, object] | None
            The trigger dict, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def settlement_days(self) -> int | None:
        """
        Settlement lag in business days.

        Returns
        -------
        int | None
            The lag, or ``None`` for same-day.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def recovery_rate(self) -> float | None:
        """
        Assumed recovery rate on default as a fraction.

        Returns
        -------
        float | None
            The recovery, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def fixed_coupon(self) -> dict[str, object] | None:
        """
        Fixed coupon specification in serde form.

        Returns
        -------
        dict[str, object] | None
            The ``FixedCouponSpec`` dict, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def floating_coupon(self) -> dict[str, object] | None:
        """
        Floating coupon specification in serde form.

        Returns
        -------
        dict[str, object] | None
            The ``FloatingCouponSpec`` dict, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def expiry(self) -> datetime.date | None:
        """
        Maturity as seen by the pricer.

        Returns
        -------
        datetime.date | None
            The maturity, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style rendering of the key economics, e.g. ``ConvertibleBond(id='CB-TECH-5Y', notional=Money(1000000.0, 'USD'), issue_date=datetime.date(2024, 1, 15), ...)``.

        Returns
        -------
        str
            ``ConvertibleBond(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise; it renders stored values.
        """
        ...

class ConvertibleBondBuilder:
    """
    Fluent builder for :class:`ConvertibleBond`; wraps the Rust
    ``FinancialBuilder`` output one setter for one setter.

    Builders are consumed by ``build()``; create a new builder per
    instrument. Required fields: ``id``, ``notional``, ``issue_date``,
    ``maturity``, ``discount_curve_id``, ``conversion``.

    Examples
    --------
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.valuations.instruments import ConversionSpec, ConvertibleBond
    >>> bond = (
    ...     ConvertibleBond
    ...     .builder()
    ...     .id("CONV-1")
    ...     .notional(Money(1_000.0, Currency("USD")))
    ...     .issue_date("2024-01-15")
    ...     .maturity("2029-01-15")
    ...     .discount_curve_id("USD-OIS")
    ...     .conversion(ConversionSpec(ratio=20.0, anti_dilution="full_ratchet"))
    ...     .underlying_equity_id("ACME")
    ...     .build()
    ... )
    >>> bond.conversion_ratio
    20.0
    """

    def id(self, value: str) -> ConvertibleBondBuilder:
        """
        Set the instrument identifier.

        Parameters
        ----------
        value : str
            Unique identifier for the convertible bond.

        Returns
        -------
        ConvertibleBondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def notional(self, value: Money) -> ConvertibleBondBuilder:
        """
        Set the principal amount.

        Parameters
        ----------
        value : Money
            Principal amount.

        Returns
        -------
        ConvertibleBondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def issue_date(self, value: datetime.date | datetime.datetime | pd.Timestamp | str) -> ConvertibleBondBuilder:
        """
        Set the issue date.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pd.Timestamp | str
            Issue date (ISO 8601 strings accepted).

        Returns
        -------
        ConvertibleBondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or the date cannot be interpreted.
        """
        ...
    def maturity(self, value: datetime.date | datetime.datetime | pd.Timestamp | str) -> ConvertibleBondBuilder:
        """
        Set the maturity date.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pd.Timestamp | str
            Maturity date (ISO 8601 strings accepted).

        Returns
        -------
        ConvertibleBondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or the date cannot be interpreted.
        """
        ...
    def discount_curve_id(self, value: str) -> ConvertibleBondBuilder:
        """
        Set the discount curve identifier for the debt component.

        Parameters
        ----------
        value : str
            Discount curve identifier (risk-free or funding).

        Returns
        -------
        ConvertibleBondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def credit_curve_id(self, value: str) -> ConvertibleBondBuilder:
        """
        Set the credit curve identifier for risky discounting (bond floor).

        Parameters
        ----------
        value : str
            Credit curve identifier; if not provided, falls back to ``discount_curve_id`` (no credit spread). Must represent zero-recovery (pure hazard) risky discounting.

        Returns
        -------
        ConvertibleBondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def conversion(self, value: ConversionSpec | dict[str, object] | str) -> ConvertibleBondBuilder:
        """
        Set the conversion terms.

        Parameters
        ----------
        value : ConversionSpec | dict[str, object] | str
            Typed :class:`ConversionSpec`, a dict, or a JSON object string with ``ratio``, ``price``, ``policy``, ``anti_dilution``, ``dividend_adjustment`` and ``dilution_events``; at least one of ``ratio`` / ``price`` must be set.

        Returns
        -------
        ConvertibleBondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does not match the ``ConversionSpec`` shape.
        """
        ...
    def underlying_equity_id(self, value: str) -> ConvertibleBondBuilder:
        """
        Set the underlying equity identifier.

        Parameters
        ----------
        value : str
            Underlying equity identifier (ticker or instrument id).

        Returns
        -------
        ConvertibleBondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def call_put(self, value: CallPutSchedule | dict[str, object] | str) -> ConvertibleBondBuilder:
        """
        Set the call/put schedule.

        Parameters
        ----------
        value : CallPutSchedule | dict[str, object] | str
            Typed :class:`CallPutSchedule`, a dict, or a JSON object string with ``calls`` and ``puts`` arrays of windows.

        Returns
        -------
        ConvertibleBondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does not match the ``CallPutSchedule`` shape.
        """
        ...
    def soft_call_trigger(self, value: dict[str, object] | str) -> ConvertibleBondBuilder:
        """
        Set the soft-call trigger condition.

        Parameters
        ----------
        value : dict[str, object] | str
            ``SoftCallTrigger`` as a dict or JSON object string with ``threshold_pct`` (percent of conversion price, e.g. ``130.0``), ``observation_days`` and ``required_days_above``.

        Returns
        -------
        ConvertibleBondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does not match the ``SoftCallTrigger`` shape.
        """
        ...
    def settlement_days(self, value: int) -> ConvertibleBondBuilder:
        """
        Set the settlement lag.

        Parameters
        ----------
        value : int
            Business days from trade date to settlement (e.g. ``2`` for US corporate convertibles); same-day when never set.

        Returns
        -------
        ConvertibleBondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def recovery_rate(self, value: float) -> ConvertibleBondBuilder:
        """
        Set the assumed recovery rate on default.

        Parameters
        ----------
        value : float
            Recovery rate as a fraction (``0.40`` = 40%); only relevant when ``credit_curve_id`` is set.

        Returns
        -------
        ConvertibleBondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def fixed_coupon(self, value: dict[str, object] | str) -> ConvertibleBondBuilder:
        """
        Set the fixed coupon specification.

        Parameters
        ----------
        value : dict[str, object] | str
            ``FixedCouponSpec`` as a dict or JSON object string (``coupon_type``, decimal ``rate`` and a ``schedule`` block).

        Returns
        -------
        ConvertibleBondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does not match the ``FixedCouponSpec`` shape.
        """
        ...
    def floating_coupon(self, value: dict[str, object] | str) -> ConvertibleBondBuilder:
        """
        Set the floating coupon specification.

        Parameters
        ----------
        value : dict[str, object] | str
            ``FloatingCouponSpec`` as a dict or JSON object string.

        Returns
        -------
        ConvertibleBondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does not match the ``FloatingCouponSpec`` shape.
        """
        ...
    def attributes(self, value: Attributes | dict[str, str] | None) -> ConvertibleBondBuilder:
        """
        Set instrument attributes (tags and metadata).

        Parameters
        ----------
        value : Attributes | dict[str, str] | None
            Attribute bag; a ``dict`` populates ``meta`` and an optional ``"tags"`` entry holding a list of strings populates ``tags``.

        Returns
        -------
        ConvertibleBondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        TypeError
            If ``value`` is neither ``Attributes``, a ``dict`` nor ``None``.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style rendering of the fields set so far, e.g.
        ``ConvertibleBondBuilder(id='X', notional=Money(1000000.0, 'USD'))``.

        Returns
        -------
        str
            ``ConvertibleBondBuilder(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise; it renders the recorded setter calls.
        """
        ...
    def build(self) -> ConvertibleBond:
        """
        Build the validated convertible bond.

        Runs only the Rust ``ConvertibleBondBuilder::build`` validation (structural
        invariants); pricing-time checks run in :meth:`ConvertibleBond.price`.

        Returns
        -------
        ConvertibleBond
            The validated instrument.

        Raises
        ------
        ValueError
            If the builder was already consumed, a required field is missing
            (the message names the builder and the field, e.g.
            ``ConvertibleBondBuilder: missing required field 'id'``), or the instrument
            fails validation (conversion terms set neither ``ratio`` nor ``price``, maturity not after issue).
        """
        ...

class FxForward:
    """
    Outright FX forward on a currency pair (typed wrapper for the canonical
    Rust ``FxForward``). The notional is denominated in ``base_currency``; PV
    is reported in ``quote_currency`` via covered interest parity. A missing
    ``contract_rate`` values the forward at-market (zero PV at inception).

    Construct via :meth:`FxForward.builder`, :meth:`FxForward.from_trade_date`
    (spot-lag and tenor roll from a trade date), :meth:`FxForward.example` or
    :meth:`FxForward.from_json`; fix the rate with
    :meth:`FxForward.with_forward_points` / :meth:`FxForward.with_forward_pips`.
    Every public Rust field is readable as a property and
    :meth:`FxForward.price` / :meth:`FxForward.metric` run the same pricer as
    :func:`price_instrument`.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import FxForward
    >>> fwd = FxForward.example()
    >>> (fwd.base_currency.code, fwd.quote_currency.code, fwd.contract_rate)
    ('EUR', 'USD', 1.12)
    """

    @staticmethod
    def builder() -> FxForwardBuilder:
        """
        Create a fluent builder (mirrors Rust ``FxForward::builder()``).

        Returns
        -------
        FxForwardBuilder
            A builder with fluent, consuming setter methods.

        Notes
        -----
        This factory does not raise; it returns an empty builder.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import FxForward
        >>> builder = FxForward.builder()
        >>> builder.id("EXAMPLE") is builder
        True
        """
        ...
    @classmethod
    def from_json(cls, json: str) -> FxForward:
        """
        Deserialize a validated FxForward from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"fx_forward"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        FxForward
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import FxForward
        >>> FxForward.from_json(FxForward.example().to_json()).id
        'EURUSD-FWD-6M'
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to a canonical ``finstack_quant.instrument/1`` envelope.

        Returns
        -------
        str
            Canonical instrument envelope accepted by :func:`price_instrument`
            and :meth:`FxForward.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def to_dict(self) -> dict[str, object]:
        """
        Serde form of the instrument spec as a plain Python ``dict``.

        Returns
        -------
        dict[str, object]
            The ``spec`` object of the instrument envelope (JSON-compatible
            values: ``str``, ``float``, ``dict``, ``list``, ``None``).

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def price(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        model: str = "default",
        metrics: list[str] | None = None,
        pricing_options: dict[str, object] | str | None = None,
        market_history: str | None = None,
    ) -> ValuationResult:
        """
        Price this instrument and return a :class:`~finstack_quant.valuations.ValuationResult`.

        Same pipeline and keyword surface as :func:`price_instrument`.

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date (ISO 8601 strings accepted).
        model : str, default "default"
            Model key (``"discounting"`` is the native model).
        metrics : list[str], optional
            Metric identifiers to compute (see :func:`list_standard_metrics`).
        pricing_options : dict[str, object] | str, optional
            ``MetricPricingOverrides`` merged into the instrument's own overrides
            (e.g. ``{"theta_period": "1D"}``).
        market_history : str, optional
            JSON ``MarketHistory`` scenarios required by ``hvar`` /
            ``expected_shortfall``.

        Returns
        -------
        ValuationResult
            Typed valuation envelope with price, currency and metrics.

        Raises
        ------
        ValueError
            If an input cannot be interpreted or the instrument fails validation.
        KeyError
            If a required curve, surface or metric is missing from ``market``.
        RuntimeError
            If pricing or a metric computation fails.
        """
        ...
    def metric(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        metric_id: str,
        model: str = "default",
    ) -> float:
        """
        Compute one scalar metric for this instrument (e.g. ``"dv01"`` or ``"fx_delta"``).

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.
        metric_id : str
            Registered metric identifier (see :func:`list_standard_metrics`).
        model : str, default "default"
            Model key.

        Returns
        -------
        float
            The metric value in the metric's native unit (decimal for rates and
            yields, currency units for DV01/CS01-style sensitivities, basis
            points for spreads).

        Raises
        ------
        ValueError
            If ``metric_id`` is unknown or an input cannot be interpreted.
        KeyError
            If a required curve or surface is missing from ``market``.
        RuntimeError
            If the metric computation fails.
        """
        ...
    def market_dependencies(self) -> dict[str, object]:
        """
        Market-data dependencies declared by the Rust ``Instrument`` trait.

        Returns
        -------
        dict[str, object]
            Serde form of ``MarketDependencies`` (``curves`` grouped by role,
            ``credit_index_ids``, ``market_scalar_ids``,
            ``volatility_dependencies``, ``fx_pairs``, ``series_ids``).

        Raises
        ------
        ValueError
            If the instrument cannot enumerate its dependencies.
        """
        ...
    @property
    def id(self) -> str:
        """
        Stable instrument identifier used in market lookup and results.

        Returns
        -------
        str
            The unique instrument identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def default_model(self) -> str:
        """
        Canonical model key used when ``model="default"`` is passed to ``price``.

        Returns
        -------
        str
            Registered model key such as ``"hazard_rate"`` or ``"black76"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def attributes(self) -> Attributes:
        """
        Instrument attributes (tags and metadata) used for scenario selection.

        Returns
        -------
        Attributes
            The attribute bag; empty when none were set.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @staticmethod
    def example() -> FxForward:
        """
        Canonical 6-month EUR/USD forward, EUR 1,000,000 at 1.12 (mirrors Rust
        ``FxForward::example``): curves ``USD-OIS`` / ``EUR-OIS``, maturity
        2025-06-15.

        Returns
        -------
        FxForward
            The example forward.

        Raises
        ------
        ValueError
            If the canonical example fails validation (never for a released build).

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import FxForward
        >>> FxForward.example().maturity
        datetime.date(2025, 6, 15)
        """
        ...
    @staticmethod
    def from_trade_date(
        id: str,
        base_currency: Currency | str,
        quote_currency: Currency | str,
        trade_date: datetime.date | datetime.datetime | pd.Timestamp | str,
        tenor: Tenor | str,
        notional: Money | float,
        domestic_discount_curve_id: str,
        foreign_discount_curve_id: str,
        *,
        base_calendar_id: str | None = None,
        quote_calendar_id: str | None = None,
        spot_lag_days: int | None = None,
        business_day_convention: BusinessDayConvention | str | None = None,
        end_of_month: bool = False,
    ) -> FxForward:
        """
        Build a forward from a trade date and a standard FX tenor (mirrors Rust
        ``FxForward::from_trade_date``): the spot date is rolled from
        ``trade_date`` by ``spot_lag_days`` business days (CLS-consistent pair
        roll), then ``tenor`` is added with the FX end-of-month rule and
        ``business_day_convention``.

        Parameters
        ----------
        id : str
            Unique instrument identifier.
        base_currency : Currency | str
            Base (foreign) currency; notional currency.
        quote_currency : Currency | str
            Quote (domestic) currency; PV currency.
        trade_date : datetime.date | datetime.datetime | pd.Timestamp | str
            Trade date from which spot is rolled.
        tenor : Tenor | str
            Standard FX tenor from spot, e.g. ``"3M"``.
        notional : Money | float
            Notional in ``base_currency``; a bare float is tagged with that currency.
        domestic_discount_curve_id : str
            Quote-currency discount curve identifier.
        foreign_discount_curve_id : str
            Base-currency discount curve identifier.
        base_calendar_id : str | None
            Base-currency holiday calendar; ``None`` uses weekends only.
        quote_calendar_id : str | None
            Quote-currency holiday calendar; ``None`` uses weekends only.
        spot_lag_days : int | None
            Spot lag in business days; ``None`` uses
            :meth:`FxForward.standard_spot_days` for the pair.
        business_day_convention : BusinessDayConvention | str | None
            Roll rule applied to the maturity; ``None`` means ``"modified_following"``.
        end_of_month : bool, default False
            Apply the FX end-of-month rule when spot falls on month end.

        Returns
        -------
        FxForward
            Validated at-market forward (no ``contract_rate``).

        Raises
        ------
        ValueError
            If the currencies coincide, the tenor/date is invalid, or the
            notional currency differs from ``base_currency``.
        KeyError
            If a calendar identifier is unknown.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import FxForward
        >>> fwd = FxForward.from_trade_date(
        ...     "EURUSD-3M", "EUR", "USD", "2025-01-15", "3M", 1_000_000.0, "USD-OIS", "EUR-OIS"
        ... )
        >>> fwd.contract_rate is None
        True
        """
        ...
    @staticmethod
    def standard_spot_days(base: Currency | str, quote: Currency | str) -> int:
        """
        Market-standard spot lag (business days) for a currency pair (mirrors
        Rust ``FxForward::standard_spot_days``).

        Parameters
        ----------
        base : Currency | str
            Base currency of the pair.
        quote : Currency | str
            Quote currency of the pair.

        Returns
        -------
        int
            ``1`` for USD/CAD, USD/TRY, USD/RUB (either order); ``2`` otherwise.

        Raises
        ------
        ValueError
            If a currency code is not ISO-4217.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import FxForward
        >>> FxForward.standard_spot_days("EUR", "USD")
        2
        """
        ...
    def with_forward_points(self, spot_rate: float, forward_points: float) -> FxForward:
        """
        Return a copy whose contract rate is ``spot_rate + forward_points``
        (mirrors Rust ``FxForward::with_forward_points``). Forward points are in
        rate units (``0.0025`` for 25 pips on EUR/USD).

        Parameters
        ----------
        spot_rate : float
            Spot rate, quote currency per unit of base currency; must be positive.
        forward_points : float
            Forward points in rate units, added to ``spot_rate``.

        Returns
        -------
        FxForward
            New forward with ``contract_rate`` set.

        Raises
        ------
        ValueError
            If ``spot_rate`` is not positive/finite, or the resulting contract
            rate is not positive.
        """
        ...
    def with_forward_pips(self, spot_rate: float, pips: float) -> FxForward:
        """
        Return a copy whose contract rate is ``spot_rate + pips * pip_size``
        (mirrors Rust ``FxForward::with_forward_pips``); the pip size follows
        market convention (``0.01`` for JPY/KRW/HUF pairs, ``0.0001`` otherwise).

        Parameters
        ----------
        spot_rate : float
            Spot rate, quote currency per unit of base currency; must be positive.
        pips : float
            Forward points quoted in pips.

        Returns
        -------
        FxForward
            New forward with ``contract_rate`` set.

        Raises
        ------
        ValueError
            If ``pips`` or ``spot_rate`` is not finite, or the resulting
            contract rate is not positive.
        """
        ...
    def market_forward_rate(
        self, market: MarketContext | str, as_of: datetime.date | datetime.datetime | pd.Timestamp | str
    ) -> float:
        """
        Covered-interest-parity forward rate implied by the market (mirrors Rust
        ``FxForward::market_forward_rate``): ``F = S * DF_foreign(T) / DF_domestic(T)``.

        Parameters
        ----------
        market : MarketContext | str
            Market carrying both discount curves and the FX matrix (or an
            explicit ``spot_rate_override`` on the instrument).
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.

        Returns
        -------
        float
            Forward rate, quote currency per unit of base currency.

        Raises
        ------
        KeyError
            If a discount curve or the FX spot is missing from ``market``.
        ValueError
            If the market JSON or ``as_of`` is invalid.
        """
        ...
    @property
    def base_currency(self) -> Currency:
        """
        Base (foreign) currency; the notional currency.

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
    def quote_currency(self) -> Currency:
        """
        Quote (domestic) currency; the PV currency.

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
    def maturity(self) -> datetime.date:
        """
        Maturity / settlement date.

        Returns
        -------
        datetime.date
            The maturity date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def notional(self) -> Money:
        """
        Notional amount in the base currency.

        Returns
        -------
        Money
            Currency-tagged notional.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def contract_rate(self) -> float | None:
        """
        Contract forward rate (quote per base).

        Returns
        -------
        float | None
            The rate, or ``None`` when at-market.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def domestic_discount_curve_id(self) -> str:
        """
        Domestic (quote-currency) discount curve identifier.

        Returns
        -------
        str
            The curve id.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def foreign_discount_curve_id(self) -> str:
        """
        Foreign (base-currency) discount curve identifier.

        Returns
        -------
        str
            The curve id.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def spot_rate_override(self) -> float | None:
        """
        Explicit spot override (quote per base).

        Returns
        -------
        float | None
            The override, or ``None`` to use the FX matrix.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def base_calendar_id(self) -> str | None:
        """
        Base-currency holiday calendar identifier.

        Returns
        -------
        str | None
            The calendar id, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def quote_calendar_id(self) -> str | None:
        """
        Quote-currency holiday calendar identifier.

        Returns
        -------
        str | None
            The calendar id, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def expiry(self) -> datetime.date | None:
        """
        Expiry as seen by the pricer.

        Returns
        -------
        datetime.date | None
            ``None``: FX forwards carry no option expiry.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style rendering of the key economics, e.g. ``FxForward(id='EURUSD-FWD-6M', pair='EURUSD', notional=Money(1000000.0, 'EUR'), maturity=datetime.date(2025, 6, 15), contract_rate=1.12)``.

        Returns
        -------
        str
            ``FxForward(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise; it renders stored values.
        """
        ...

class FxForwardBuilder:
    """
    Fluent builder for :class:`FxForward`; wraps the Rust ``FinancialBuilder``
    output one setter for one setter.

    Builders are consumed by ``build()``; create a new builder per
    instrument. Required fields: ``id``, ``base_currency``,
    ``quote_currency``, ``maturity``, ``notional``,
    ``domestic_discount_curve_id``, ``foreign_discount_curve_id``.

    Examples
    --------
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.valuations.instruments import FxForward
    >>> fwd = (
    ...     FxForward
    ...     .builder()
    ...     .id("EURUSD-FWD")
    ...     .base_currency("EUR")
    ...     .quote_currency(Currency("USD"))
    ...     .maturity("2025-06-15")
    ...     .notional(Money(1_000_000.0, Currency("EUR")))
    ...     .contract_rate(1.12)
    ...     .domestic_discount_curve_id("USD-OIS")
    ...     .foreign_discount_curve_id("EUR-OIS")
    ...     .build()
    ... )
    >>> fwd.contract_rate
    1.12
    """

    def id(self, value: str) -> FxForwardBuilder:
        """
        Set the instrument identifier.

        Parameters
        ----------
        value : str
            Unique identifier for the FX forward.

        Returns
        -------
        FxForwardBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def base_currency(self, value: Currency | str) -> FxForwardBuilder:
        """
        Set the base currency (foreign currency, numerator of the pair).

        Parameters
        ----------
        value : Currency | str
            Base (foreign) currency, as a ``Currency`` or ISO-4217 code.

        Returns
        -------
        FxForwardBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or a string code is not ISO-4217.
        """
        ...
    def quote_currency(self, value: Currency | str) -> FxForwardBuilder:
        """
        Set the quote currency (domestic currency, denominator of the pair).

        Parameters
        ----------
        value : Currency | str
            Quote (domestic) currency; also the PV currency.

        Returns
        -------
        FxForwardBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or a string code is not ISO-4217.
        """
        ...
    def maturity(self, value: datetime.date | datetime.datetime | pd.Timestamp | str) -> FxForwardBuilder:
        """
        Set the maturity/settlement date.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pd.Timestamp | str
            Maturity/settlement date (ISO 8601 strings accepted).

        Returns
        -------
        FxForwardBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or the date cannot be interpreted.
        """
        ...
    def notional(self, value: Money) -> FxForwardBuilder:
        """
        Set the notional amount in base currency.

        Parameters
        ----------
        value : Money
            Notional amount, denominated in the base currency.

        Returns
        -------
        FxForwardBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def contract_rate(self, value: float) -> FxForwardBuilder:
        """
        Set the contract forward rate (quote per base).

        Parameters
        ----------
        value : float
            Contract forward rate; when never set the forward is valued at-market (zero PV at inception).

        Returns
        -------
        FxForwardBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def domestic_discount_curve_id(self, value: str) -> FxForwardBuilder:
        """
        Set the domestic (quote currency) discount curve identifier.

        Parameters
        ----------
        value : str
            Domestic (quote currency) discount curve identifier.

        Returns
        -------
        FxForwardBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def foreign_discount_curve_id(self, value: str) -> FxForwardBuilder:
        """
        Set the foreign (base currency) discount curve identifier.

        Parameters
        ----------
        value : str
            Foreign (base currency) discount curve identifier.

        Returns
        -------
        FxForwardBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def spot_rate_override(self, value: float) -> FxForwardBuilder:
        """
        Set an explicit spot rate override (quote per base).

        Parameters
        ----------
        value : float
            Spot FX rate; when never set the spot is sourced from the market's FX matrix.

        Returns
        -------
        FxForwardBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def base_calendar_id(self, value: str) -> FxForwardBuilder:
        """
        Set the base currency calendar identifier for business day adjustment.

        Parameters
        ----------
        value : str
            Base currency holiday calendar identifier.

        Returns
        -------
        FxForwardBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def quote_calendar_id(self, value: str) -> FxForwardBuilder:
        """
        Set the quote currency calendar identifier for business day adjustment.

        Parameters
        ----------
        value : str
            Quote currency holiday calendar identifier.

        Returns
        -------
        FxForwardBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def attributes(self, value: Attributes | dict[str, str] | None) -> FxForwardBuilder:
        """
        Set instrument attributes (tags and metadata).

        Parameters
        ----------
        value : Attributes | dict[str, str] | None
            Attribute bag; a ``dict`` populates ``meta`` and an optional ``"tags"`` entry holding a list of strings populates ``tags``.

        Returns
        -------
        FxForwardBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        TypeError
            If ``value`` is neither ``Attributes``, a ``dict`` nor ``None``.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style rendering of the fields set so far, e.g.
        ``FxForwardBuilder(id='X', notional=Money(1000000.0, 'USD'))``.

        Returns
        -------
        str
            ``FxForwardBuilder(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise; it renders the recorded setter calls.
        """
        ...
    def build(self) -> FxForward:
        """
        Build the validated FX forward.

        Runs only the Rust ``FxForwardBuilder::build`` validation (structural
        invariants); pricing-time checks run in :meth:`FxForward.price`.

        Returns
        -------
        FxForward
            The validated instrument.

        Raises
        ------
        ValueError
            If the builder was already consumed, a required field is missing
            (the message names the builder and the field, e.g.
            ``FxForwardBuilder: missing required field 'id'``), or the instrument
            fails validation (``base_currency`` equals ``quote_currency``, notional not in the base currency).
        """
        ...

class FxOption:
    """
    Vanilla FX option priced with Garman–Kohlhagen (typed wrapper for the
    canonical Rust ``FxOption``). ``strike`` is quoted as quote currency per
    unit of base currency; the notional is in ``base_currency``. The option
    carries its pair/venue delta convention so Greeks are reported the way
    the desk quotes them.

    Construct via :meth:`FxOption.builder`, :meth:`FxOption.european`,
    :meth:`FxOption.example` or :meth:`FxOption.from_json`. Every public Rust
    field is readable as a property; ``greeks`` / ``delta`` / ``gamma`` /
    ``vega`` / ``theta`` / ``rho`` / ``foreign_rho`` / ``vanna`` / ``volga`` /
    ``implied_vol`` mirror the Rust accessors and :meth:`FxOption.price` /
    :meth:`FxOption.metric` run the same pricer as :func:`price_instrument`.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import FxOption
    >>> opt = FxOption.example()
    >>> (opt.option_type, opt.strike, opt.delta_convention["kind"])
    ('call', 1.12, 'forward')
    """

    @staticmethod
    def builder() -> FxOptionBuilder:
        """
        Create a fluent builder (mirrors Rust ``FxOption::builder()``).

        Returns
        -------
        FxOptionBuilder
            A builder with fluent, consuming setter methods.

        Notes
        -----
        This factory does not raise; it returns an empty builder.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import FxOption
        >>> builder = FxOption.builder()
        >>> builder.id("EXAMPLE") is builder
        True
        """
        ...
    @classmethod
    def from_json(cls, json: str) -> FxOption:
        """
        Deserialize a validated FxOption from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"fx_option"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        FxOption
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import FxOption
        >>> FxOption.from_json(FxOption.example().to_json()).id
        'FXOPT-EURUSD-CALL'
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to a canonical ``finstack_quant.instrument/1`` envelope.

        Returns
        -------
        str
            Canonical instrument envelope accepted by :func:`price_instrument`
            and :meth:`FxOption.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def to_dict(self) -> dict[str, object]:
        """
        Serde form of the instrument spec as a plain Python ``dict``.

        Returns
        -------
        dict[str, object]
            The ``spec`` object of the instrument envelope (JSON-compatible
            values: ``str``, ``float``, ``dict``, ``list``, ``None``).

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def price(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        model: str = "default",
        metrics: list[str] | None = None,
        pricing_options: dict[str, object] | str | None = None,
        market_history: str | None = None,
    ) -> ValuationResult:
        """
        Price this instrument and return a :class:`~finstack_quant.valuations.ValuationResult`.

        Same pipeline and keyword surface as :func:`price_instrument`.

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date (ISO 8601 strings accepted).
        model : str, default "default"
            Model key (``"black76"`` (Garman–Kohlhagen) is the native model).
        metrics : list[str], optional
            Metric identifiers to compute (see :func:`list_standard_metrics`).
        pricing_options : dict[str, object] | str, optional
            ``MetricPricingOverrides`` merged into the instrument's own overrides
            (e.g. ``{"theta_period": "1D"}``).
        market_history : str, optional
            JSON ``MarketHistory`` scenarios required by ``hvar`` /
            ``expected_shortfall``.

        Returns
        -------
        ValuationResult
            Typed valuation envelope with price, currency and metrics.

        Raises
        ------
        ValueError
            If an input cannot be interpreted or the instrument fails validation.
        KeyError
            If a required curve, surface or metric is missing from ``market``.
        RuntimeError
            If pricing or a metric computation fails.
        """
        ...
    def metric(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        metric_id: str,
        model: str = "default",
    ) -> float:
        """
        Compute one scalar metric for this instrument (e.g. ``"delta"`` or ``"vega"``).

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.
        metric_id : str
            Registered metric identifier (see :func:`list_standard_metrics`).
        model : str, default "default"
            Model key.

        Returns
        -------
        float
            The metric value in the metric's native unit (decimal for rates and
            yields, currency units for DV01/CS01-style sensitivities, basis
            points for spreads).

        Raises
        ------
        ValueError
            If ``metric_id`` is unknown or an input cannot be interpreted.
        KeyError
            If a required curve or surface is missing from ``market``.
        RuntimeError
            If the metric computation fails.
        """
        ...
    def market_dependencies(self) -> dict[str, object]:
        """
        Market-data dependencies declared by the Rust ``Instrument`` trait.

        Returns
        -------
        dict[str, object]
            Serde form of ``MarketDependencies`` (``curves`` grouped by role,
            ``credit_index_ids``, ``market_scalar_ids``,
            ``volatility_dependencies``, ``fx_pairs``, ``series_ids``).

        Raises
        ------
        ValueError
            If the instrument cannot enumerate its dependencies.
        """
        ...
    @property
    def id(self) -> str:
        """
        Stable instrument identifier used in market lookup and results.

        Returns
        -------
        str
            The unique instrument identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def default_model(self) -> str:
        """
        Canonical model key used when ``model="default"`` is passed to ``price``.

        Returns
        -------
        str
            Registered model key such as ``"hazard_rate"`` or ``"black76"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def attributes(self) -> Attributes:
        """
        Instrument attributes (tags and metadata) used for scenario selection.

        Returns
        -------
        Attributes
            The attribute bag; empty when none were set.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @staticmethod
    def example() -> FxOption:
        """
        Canonical EUR/USD call, strike 1.12, EUR 1,000,000 (mirrors Rust
        ``FxOption::example``): forward-delta convention, premium in USD, curves
        ``USD-OIS`` / ``EUR-OIS``, surface ``EURUSD-VOL``.

        Returns
        -------
        FxOption
            The example option.

        Raises
        ------
        ValueError
            If the canonical example fails validation (never for a released build).

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import FxOption
        >>> FxOption.example().vol_surface_id
        'EURUSD-VOL'
        """
        ...
    @staticmethod
    def european(
        id: str,
        base_currency: Currency | str,
        quote_currency: Currency | str,
        strike: float,
        expiry: datetime.date | datetime.datetime | pd.Timestamp | str,
        notional: Money | float,
        vol_surface_id: str,
        option_type: Literal["call", "put"],
        delta_convention_kind: Literal["spot", "forward", "premium_adjusted_spot", "premium_adjusted_forward"],
        premium_currency: Currency | str,
        venue: str,
    ) -> FxOption:
        """
        Build a European FX option with currency-derived OIS curves (mirrors Rust
        ``FxOption::european``): discount curves default to ``"<QUOTE>-OIS"``
        (domestic) and ``"<BASE>-OIS"`` (foreign), with the pre-configured
        EUR/USD and GBP/USD underlying presets when applicable.

        Parameters
        ----------
        id : str
            Unique instrument identifier.
        base_currency : Currency | str
            Base (foreign) currency; notional currency.
        quote_currency : Currency | str
            Quote (domestic) currency.
        strike : float
            Strike, quote currency per unit of base currency.
        expiry : datetime.date | datetime.datetime | pd.Timestamp | str
            Expiry date.
        notional : Money | float
            Notional in ``base_currency``; a bare float is tagged with that currency.
        vol_surface_id : str
            FX volatility surface identifier.
        option_type : {"call", "put"}
            Call or put on the base currency.
        delta_convention_kind : {"spot", "forward", "premium_adjusted_spot", "premium_adjusted_forward"}
            Delta convention quoted by the venue.
        premium_currency : Currency | str
            Currency in which the premium is paid (base or quote).
        venue : str
            Non-empty market venue / quoting-source identifier.

        Returns
        -------
        FxOption
            The validated option.

        Raises
        ------
        ValueError
            If the currencies coincide, ``premium_currency`` is neither leg,
            ``venue`` is blank, or the notional is not positive.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import FxOption
        >>> opt = FxOption.european(
        ...     "EURUSD-CALL",
        ...     "EUR",
        ...     "USD",
        ...     1.12,
        ...     "2025-06-15",
        ...     1_000_000.0,
        ...     "EURUSD-VOL",
        ...     "call",
        ...     "spot",
        ...     "USD",
        ...     "desk",
        ... )
        >>> (opt.domestic_discount_curve_id, opt.foreign_discount_curve_id)
        ('USD-OIS', 'EUR-OIS')
        """
        ...
    def implied_vol(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        target_price: float,
    ) -> float:
        """
        Implied volatility that reproduces ``target_price`` (mirrors Rust
        ``FxOption::implied_vol``, Garman–Kohlhagen inversion).

        Parameters
        ----------
        market : MarketContext | str
            Market carrying both discount curves and the FX spot.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.
        target_price : float
            Observed option PV in quote currency (same scaling as ``price``).

        Returns
        -------
        float
            Annualized lognormal volatility as a decimal (``0.10`` = 10%).

        Raises
        ------
        KeyError
            If a curve or the spot is missing from ``market``.
        RuntimeError
            If the root search does not converge (price outside no-arbitrage bounds).
        """
        ...
    def delta(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        model: str = "default",
    ) -> float:
        """
        Spot delta of the option under the selected model.

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.
        model : str, default "default"
            Model key.

        Returns
        -------
        float
            Spot delta produced by the selected model.

        Raises
        ------
        ValueError
            If an input is invalid, required market data is missing, pricing
            fails, or the model does not produce delta.
        """
        ...
    def gamma(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        model: str = "default",
    ) -> float:
        """
        Spot gamma of the option under the selected model.

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.
        model : str, default "default"
            Model key.

        Returns
        -------
        float
            Spot gamma produced by the selected model.

        Raises
        ------
        ValueError
            If an input is invalid, required market data is missing, pricing
            fails, or the model does not produce gamma.
        """
        ...
    def vega(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        model: str = "default",
    ) -> float:
        """
        Vega of the option under the selected model.

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.
        model : str, default "default"
            Model key.

        Returns
        -------
        float
            Vega produced by the selected model.

        Raises
        ------
        ValueError
            If an input is invalid, required market data is missing, pricing
            fails, or the model does not produce vega.
        """
        ...
    def theta(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        model: str = "default",
    ) -> float:
        """
        Theta of the option under the selected model.

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.
        model : str, default "default"
            Model key.

        Returns
        -------
        float
            Theta produced by the selected model.

        Raises
        ------
        ValueError
            If an input is invalid, required market data is missing, pricing
            fails, or the model does not produce theta.
        """
        ...
    def rho(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        model: str = "default",
    ) -> float:
        """
        Domestic-rate rho of the option under the selected model.

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.
        model : str, default "default"
            Model key.

        Returns
        -------
        float
            Domestic-rate rho produced by the selected model.

        Raises
        ------
        ValueError
            If an input is invalid, required market data is missing, pricing
            fails, or the model does not produce rho.
        """
        ...
    def foreign_rho(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        model: str = "default",
    ) -> float:
        """
        Foreign-rate rho of the option under the selected model.

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.
        model : str, default "default"
            Model key.

        Returns
        -------
        float
            Foreign-rate rho produced by the selected model.

        Raises
        ------
        ValueError
            If an input is invalid, required market data is missing, pricing
            fails, or the model does not produce foreign_rho.
        """
        ...
    def vanna(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        model: str = "default",
    ) -> float:
        """
        Vanna of the option under the selected model.

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.
        model : str, default "default"
            Model key.

        Returns
        -------
        float
            Vanna produced by the selected model.

        Raises
        ------
        ValueError
            If an input is invalid, required market data is missing, pricing
            fails, or the model does not produce vanna.
        """
        ...
    def volga(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        model: str = "default",
    ) -> float:
        """
        Volga of the option under the selected model.

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.
        model : str, default "default"
            Model key.

        Returns
        -------
        float
            Volga produced by the selected model.

        Raises
        ------
        ValueError
            If an input is invalid, required market data is missing, pricing
            fails, or the model does not produce volga.
        """
        ...
    def greeks(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        model: str = "default",
    ) -> dict[str, float]:
        """
        Compute the standard option Greek set as a dict (mirrors the WASM
        ``greeks`` method): Greeks the selected model cannot produce are
        omitted, and any non-finite Greek raises rather than being returned.

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.
        model : str, default "default"
            Model key.

        Returns
        -------
        dict[str, float]
            Mapping of Greek name to value for every Greek the model produced.

        Raises
        ------
        ValueError
            If an input is invalid, required market data is missing, pricing
            fails, or a returned Greek is non-finite.
        """
        ...
    @property
    def base_currency(self) -> Currency:
        """
        Base (foreign) currency; the notional currency.

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
    def quote_currency(self) -> Currency:
        """
        Quote (domestic) currency.

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
    def strike(self) -> float:
        """
        Strike, quote currency per unit of base currency.

        Returns
        -------
        float
            The strike rate.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def option_type(self) -> str:
        """
        Option type on the base currency.

        Returns
        -------
        str
            ``"call"`` or ``"put"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def delta_convention(self) -> dict[str, str]:
        """
        Delta convention.

        Returns
        -------
        dict[str, str]
            ``{"kind", "premium_currency", "venue"}``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def expiry(self) -> datetime.date:
        """
        Last date on which the option may be exercised.

        Returns
        -------
        datetime.date
            Unadjusted calendar expiry date. Time to expiry used in pricing
            is measured from the valuation date to this date under the
            pricing day-count convention.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def day_count(self) -> str:
        """
        Day count for the time-to-expiry year fraction (serde name).

        Returns
        -------
        str
            ``"act_365f"`` unless set otherwise.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def notional(self) -> Money:
        """
        Notional amount in the base currency.

        Returns
        -------
        Money
            Currency-tagged notional.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def domestic_discount_curve_id(self) -> str:
        """
        Domestic (quote-currency) discount curve identifier.

        Returns
        -------
        str
            The curve id.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def foreign_discount_curve_id(self) -> str:
        """
        Foreign (base-currency) discount curve identifier.

        Returns
        -------
        str
            The curve id.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def vol_surface_id(self) -> str:
        """
        FX volatility surface identifier.

        Returns
        -------
        str
            The surface id.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style rendering of the key economics, e.g. ``FxOption(id='FXOPT-EURUSD-CALL', pair='EURUSD', option_type='call', strike=1.12, expiry=datetime.date(2030, 1, 15), notional=Money(1000000.0, 'EUR'))``.

        Returns
        -------
        str
            ``FxOption(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise; it renders stored values.
        """
        ...

class FxOptionBuilder:
    """
    Fluent builder for :class:`FxOption`; wraps the Rust ``FinancialBuilder``
    output one setter for one setter.

    Builders are consumed by ``build()``; create a new builder per
    instrument. Required fields: ``id``, ``base_currency``,
    ``quote_currency``, ``strike``, ``option_type``, ``delta_convention``,
    ``expiry``, ``notional``, ``domestic_discount_curve_id``,
    ``foreign_discount_curve_id``, ``vol_surface_id`` (``day_count``
    defaults to ACT/365F).

    Examples
    --------
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.valuations.instruments import FxOption
    >>> opt = (
    ...     FxOption
    ...     .builder()
    ...     .id("EURUSD-CALL")
    ...     .base_currency("EUR")
    ...     .quote_currency("USD")
    ...     .strike(1.12)
    ...     .option_type("call")
    ...     .delta_convention("spot", "USD", "desk")
    ...     .expiry("2025-06-15")
    ...     .notional(Money(1_000_000.0, Currency("EUR")))
    ...     .domestic_discount_curve_id("USD-OIS")
    ...     .foreign_discount_curve_id("EUR-OIS")
    ...     .vol_surface_id("EURUSD-VOL")
    ...     .build()
    ... )
    >>> opt.delta_convention["venue"]
    'desk'
    """

    def id(self, value: str) -> FxOptionBuilder:
        """
        Set the instrument identifier.

        Parameters
        ----------
        value : str
            Unique identifier for the FX option.

        Returns
        -------
        FxOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def base_currency(self, value: Currency | str) -> FxOptionBuilder:
        """
        Set the base currency (foreign currency).

        Parameters
        ----------
        value : Currency | str
            Base (foreign) currency, as a ``Currency`` or ISO-4217 code.

        Returns
        -------
        FxOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or a string code is not ISO-4217.
        """
        ...
    def quote_currency(self, value: Currency | str) -> FxOptionBuilder:
        """
        Set the quote currency (domestic currency).

        Parameters
        ----------
        value : Currency | str
            Quote (domestic) currency, as a ``Currency`` or ISO-4217 code.

        Returns
        -------
        FxOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or a string code is not ISO-4217.
        """
        ...
    def strike(self, value: float) -> FxOptionBuilder:
        """
        Set the strike exchange rate (quote per base).

        Parameters
        ----------
        value : float
            Strike exchange rate, quote currency per unit of base currency.

        Returns
        -------
        FxOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def option_type(self, value: Literal["call", "put"]) -> FxOptionBuilder:
        """
        Set the option type: ``"call"`` or ``"put"`` on base currency.

        Parameters
        ----------
        value : Literal["call", "put"]
            Option type of the FX option.

        Returns
        -------
        FxOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized option type.
        """
        ...
    def delta_convention(
        self,
        kind: Literal["spot", "forward", "premium_adjusted_spot", "premium_adjusted_forward"],
        premium_currency: Currency | str,
        venue: str,
    ) -> FxOptionBuilder:
        """
        Set the pair/venue delta convention and premium currency.

        Parameters
        ----------
        kind : {"spot", "forward", "premium_adjusted_spot", "premium_adjusted_forward"}
            Delta convention quoted by the venue.
        premium_currency : Currency | str
            Currency in which the FX option premium is paid.
        venue : str
            Non-empty market venue or quoting-source identifier.

        Returns
        -------
        FxOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``, ``kind`` is unknown or ``venue`` is blank.
        """
        ...
    def expiry(self, value: datetime.date | datetime.datetime | pd.Timestamp | str) -> FxOptionBuilder:
        """
        Set the option expiry date.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pd.Timestamp | str
            Option expiry date (ISO 8601 strings accepted).

        Returns
        -------
        FxOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or the date cannot be interpreted.
        """
        ...
    def day_count(self, value: DayCount | str) -> FxOptionBuilder:
        """
        Set the day count for the time-to-expiry year fraction.

        Parameters
        ----------
        value : DayCount | str
            Day count convention; ACT/365F when never set.

        Returns
        -------
        FxOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or a string name is not a recognized day count.
        """
        ...
    def notional(self, value: Money) -> FxOptionBuilder:
        """
        Set the notional amount in base currency.

        Parameters
        ----------
        value : Money
            Notional amount, denominated in the base currency.

        Returns
        -------
        FxOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def domestic_discount_curve_id(self, value: str) -> FxOptionBuilder:
        """
        Set the domestic currency discount curve identifier.

        Parameters
        ----------
        value : str
            Domestic currency discount curve identifier.

        Returns
        -------
        FxOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def foreign_discount_curve_id(self, value: str) -> FxOptionBuilder:
        """
        Set the foreign currency discount curve identifier.

        Parameters
        ----------
        value : str
            Foreign currency discount curve identifier.

        Returns
        -------
        FxOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def vol_surface_id(self, value: str) -> FxOptionBuilder:
        """
        Set the FX volatility surface identifier.

        Parameters
        ----------
        value : str
            FX volatility surface identifier for option pricing.

        Returns
        -------
        FxOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def attributes(self, value: Attributes | dict[str, str] | None) -> FxOptionBuilder:
        """
        Set instrument attributes (tags and metadata).

        Parameters
        ----------
        value : Attributes | dict[str, str] | None
            Attribute bag; a ``dict`` populates ``meta`` and an optional ``"tags"`` entry holding a list of strings populates ``tags``.

        Returns
        -------
        FxOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        TypeError
            If ``value`` is neither ``Attributes``, a ``dict`` nor ``None``.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style rendering of the fields set so far, e.g.
        ``FxOptionBuilder(id='X', notional=Money(1000000.0, 'USD'))``.

        Returns
        -------
        str
            ``FxOptionBuilder(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise; it renders the recorded setter calls.
        """
        ...
    def build(self) -> FxOption:
        """
        Build the validated FX option.

        Runs only the Rust ``FxOptionBuilder::build`` validation (structural
        invariants); pricing-time checks run in :meth:`FxOption.price`.

        Returns
        -------
        FxOption
            The validated instrument.

        Raises
        ------
        ValueError
            If the builder was already consumed, a required field is missing
            (the message names the builder and the field, e.g.
            ``FxOptionBuilder: missing required field 'id'``), or the instrument
            fails validation (``base_currency`` equals ``quote_currency``, premium currency is neither leg, notional not in the base currency).
        """
        ...

class EquityOption:
    """
    Vanilla equity option (typed wrapper for the canonical Rust
    ``EquityOption``). European options price with Black–Scholes–Merton
    (``"black76"`` on the forward); American and Bermudan styles use the tree
    pricer. ``notional`` scales the per-share value; discrete dividends and a
    continuous ``div_yield_id`` are both supported.

    Construct via :meth:`EquityOption.builder`,
    :meth:`EquityOption.european_call`, :meth:`EquityOption.example` or
    :meth:`EquityOption.from_json`. Every public Rust field is readable as a
    property; ``greeks`` / ``delta`` / ``gamma`` / ``vega`` / ``theta`` /
    ``rho`` / ``implied_vol`` mirror the Rust accessors and
    :meth:`EquityOption.price` / :meth:`EquityOption.metric` run the same
    pricer as :func:`price_instrument`.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import EquityOption
    >>> opt = EquityOption.example()
    >>> (opt.underlying_ticker, opt.strike, opt.option_type, opt.exercise_style)
    ('SPX', 4500.0, 'call', 'european')
    """

    @staticmethod
    def builder() -> EquityOptionBuilder:
        """
        Create a fluent builder (mirrors Rust ``EquityOption::builder()``).

        Returns
        -------
        EquityOptionBuilder
            A builder with fluent, consuming setter methods.

        Notes
        -----
        This factory does not raise; it returns an empty builder.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import EquityOption
        >>> builder = EquityOption.builder()
        >>> builder.id("EXAMPLE") is builder
        True
        """
        ...
    @classmethod
    def from_json(cls, json: str) -> EquityOption:
        """
        Deserialize a validated EquityOption from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"equity_option"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        EquityOption
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import EquityOption
        >>> EquityOption.from_json(EquityOption.example().to_json()).id
        'SPX-CALL-4500'
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to a canonical ``finstack_quant.instrument/1`` envelope.

        Returns
        -------
        str
            Canonical instrument envelope accepted by :func:`price_instrument`
            and :meth:`EquityOption.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def to_dict(self) -> dict[str, object]:
        """
        Serde form of the instrument spec as a plain Python ``dict``.

        Returns
        -------
        dict[str, object]
            The ``spec`` object of the instrument envelope (JSON-compatible
            values: ``str``, ``float``, ``dict``, ``list``, ``None``).

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def price(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        model: str = "default",
        metrics: list[str] | None = None,
        pricing_options: dict[str, object] | str | None = None,
        market_history: str | None = None,
    ) -> ValuationResult:
        """
        Price this instrument and return a :class:`~finstack_quant.valuations.ValuationResult`.

        Same pipeline and keyword surface as :func:`price_instrument`.

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date (ISO 8601 strings accepted).
        model : str, default "default"
            Model key (``"black76"`` (European) or ``"tree"``).
        metrics : list[str], optional
            Metric identifiers to compute (see :func:`list_standard_metrics`).
        pricing_options : dict[str, object] | str, optional
            ``MetricPricingOverrides`` merged into the instrument's own overrides
            (e.g. ``{"theta_period": "1D"}``).
        market_history : str, optional
            JSON ``MarketHistory`` scenarios required by ``hvar`` /
            ``expected_shortfall``.

        Returns
        -------
        ValuationResult
            Typed valuation envelope with price, currency and metrics.

        Raises
        ------
        ValueError
            If an input cannot be interpreted or the instrument fails validation.
        KeyError
            If a required curve, surface or metric is missing from ``market``.
        RuntimeError
            If pricing or a metric computation fails.
        """
        ...
    def metric(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        metric_id: str,
        model: str = "default",
    ) -> float:
        """
        Compute one scalar metric for this instrument (e.g. ``"delta"`` or ``"vega"``).

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.
        metric_id : str
            Registered metric identifier (see :func:`list_standard_metrics`).
        model : str, default "default"
            Model key.

        Returns
        -------
        float
            The metric value in the metric's native unit (decimal for rates and
            yields, currency units for DV01/CS01-style sensitivities, basis
            points for spreads).

        Raises
        ------
        ValueError
            If ``metric_id`` is unknown or an input cannot be interpreted.
        KeyError
            If a required curve or surface is missing from ``market``.
        RuntimeError
            If the metric computation fails.
        """
        ...
    def market_dependencies(self) -> dict[str, object]:
        """
        Market-data dependencies declared by the Rust ``Instrument`` trait.

        Returns
        -------
        dict[str, object]
            Serde form of ``MarketDependencies`` (``curves`` grouped by role,
            ``credit_index_ids``, ``market_scalar_ids``,
            ``volatility_dependencies``, ``fx_pairs``, ``series_ids``).

        Raises
        ------
        ValueError
            If the instrument cannot enumerate its dependencies.
        """
        ...
    @property
    def id(self) -> str:
        """
        Stable instrument identifier used in market lookup and results.

        Returns
        -------
        str
            The unique instrument identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def default_model(self) -> str:
        """
        Canonical model key used when ``model="default"`` is passed to ``price``.

        Returns
        -------
        str
            Registered model key such as ``"hazard_rate"`` or ``"black76"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def attributes(self) -> Attributes:
        """
        Instrument attributes (tags and metadata) used for scenario selection.

        Returns
        -------
        Attributes
            The attribute bag; empty when none were set.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @staticmethod
    def example() -> EquityOption:
        """
        Canonical SPX 4500 European call expiring 2024-06-21 (mirrors Rust
        ``EquityOption::example``): USD 100 notional, curve ``USD-OIS``, spot
        ``EQUITY-SPOT``, surface ``EQUITY-VOL``, dividend yield ``EQUITY-DIVYIELD``.

        Returns
        -------
        EquityOption
            The example option.

        Raises
        ------
        ValueError
            If the canonical example fails validation (never for a released build).

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import EquityOption
        >>> EquityOption.example().vol_surface_id
        'EQUITY-VOL'
        """
        ...
    @staticmethod
    def european_call(
        id: str,
        ticker: str,
        strike: float,
        expiry: datetime.date | datetime.datetime | pd.Timestamp | str,
        notional: Money | float,
        *,
        discount_curve_id: str = "USD-OIS",
        spot_id: str = "EQUITY-SPOT",
        vol_surface_id: str = "EQUITY-VOL",
        div_yield_id: str | None = "EQUITY-DIVYIELD",
    ) -> EquityOption:
        """
        Build a cash-settled European call (mirrors Rust
        ``EquityOption::european_call`` / ``european_call_with_market_data``).
        The market-data identifiers default to the same generic ids the Rust
        constructor uses; pass your own to bind the option to real market
        objects.

        Parameters
        ----------
        id : str
            Unique instrument identifier.
        ticker : str
            Underlying equity ticker.
        strike : float
            Strike price; must be finite and positive.
        expiry : datetime.date | datetime.datetime | pd.Timestamp | str
            Expiry date.
        notional : Money | float
            Notional for valuation scaling; a bare float is USD.
        discount_curve_id : str, default "USD-OIS"
            Discount curve identifier.
        spot_id : str, default "EQUITY-SPOT"
            Equity spot price identifier.
        vol_surface_id : str, default "EQUITY-VOL"
            Volatility surface identifier.
        div_yield_id : str | None, default "EQUITY-DIVYIELD"
            Continuous dividend yield identifier; ``None`` for no yield.

        Returns
        -------
        EquityOption
            The validated option.

        Raises
        ------
        ValueError
            If ``strike`` is not positive, the notional is zero, or ``expiry``
            cannot be interpreted.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import EquityOption
        >>> opt = EquityOption.european_call("AAPL-C-200", "AAPL", 200.0, "2025-06-20", 100.0, spot_id="AAPL")
        >>> (opt.option_type, opt.settlement, opt.spot_id)
        ('call', 'cash', 'AAPL')
        """
        ...
    def implied_vol(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        market_price: float,
    ) -> float:
        """
        Implied volatility that reproduces ``market_price`` (mirrors Rust
        ``EquityOption::implied_vol``, Black–Scholes inversion on the option's
        day count).

        Parameters
        ----------
        market : MarketContext | str
            Market carrying the discount curve, spot and (optional) dividend yield.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.
        market_price : float
            Observed option value in the same scaling as ``price``.

        Returns
        -------
        float
            Annualized lognormal volatility as a decimal (``0.20`` = 20%).

        Raises
        ------
        KeyError
            If required market data is missing from ``market``.
        RuntimeError
            If the root search does not converge.
        """
        ...
    def delta(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        model: str = "default",
    ) -> float:
        """
        Spot delta of the option under the selected model.

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.
        model : str, default "default"
            Model key.

        Returns
        -------
        float
            Spot delta produced by the selected model.

        Raises
        ------
        ValueError
            If an input is invalid, required market data is missing, pricing
            fails, or the model does not produce delta.
        """
        ...
    def gamma(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        model: str = "default",
    ) -> float:
        """
        Gamma of the option under the selected model.

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.
        model : str, default "default"
            Model key.

        Returns
        -------
        float
            Gamma produced by the selected model.

        Raises
        ------
        ValueError
            If an input is invalid, required market data is missing, pricing
            fails, or the model does not produce gamma.
        """
        ...
    def vega(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        model: str = "default",
    ) -> float:
        """
        Vega (per 1% vol) of the option under the selected model.

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.
        model : str, default "default"
            Model key.

        Returns
        -------
        float
            Vega (per 1% vol) produced by the selected model.

        Raises
        ------
        ValueError
            If an input is invalid, required market data is missing, pricing
            fails, or the model does not produce vega.
        """
        ...
    def theta(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        model: str = "default",
    ) -> float:
        """
        Theta (per day on ``theta_day_basis``) of the option under the selected model.

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.
        model : str, default "default"
            Model key.

        Returns
        -------
        float
            Theta (per day on ``theta_day_basis``) produced by the selected model.

        Raises
        ------
        ValueError
            If an input is invalid, required market data is missing, pricing
            fails, or the model does not produce theta.
        """
        ...
    def rho(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        model: str = "default",
    ) -> float:
        """
        Rho of the option under the selected model.

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.
        model : str, default "default"
            Model key.

        Returns
        -------
        float
            Rho produced by the selected model.

        Raises
        ------
        ValueError
            If an input is invalid, required market data is missing, pricing
            fails, or the model does not produce rho.
        """
        ...
    def greeks(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        model: str = "default",
    ) -> dict[str, float]:
        """
        Compute the standard option Greek set as a dict (mirrors the WASM
        ``greeks`` method): Greeks the selected model cannot produce are
        omitted, and any non-finite Greek raises rather than being returned.

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.
        model : str, default "default"
            Model key.

        Returns
        -------
        dict[str, float]
            Mapping of Greek name to value for every Greek the model produced.

        Raises
        ------
        ValueError
            If an input is invalid, required market data is missing, pricing
            fails, or a returned Greek is non-finite.
        """
        ...
    @property
    def underlying_ticker(self) -> str:
        """
        Identifier of the underlying equity referenced by the option.

        Returns
        -------
        str
            Ticker string exactly as supplied at construction; it is the key
            used to look up the spot price and volatility surface in the
            market context, so it must match the market-data identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def strike(self) -> float:
        """
        Contractual exercise price of the option.

        Returns
        -------
        float
            Strike expressed in the same price units and currency as the
            underlying spot quote, not as a percentage of spot or as
            moneyness.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def option_type(self) -> str:
        """
        Payoff direction of the option contract.

        Returns
        -------
        str
            Serde string, either ``"call"`` (payoff ``max(S - K, 0)``) or
            ``"put"`` (payoff ``max(K - S, 0)``).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def exercise_style(self) -> str:
        """
        Exercise rights attached to the option, which select the pricing
        engine used.

        Returns
        -------
        str
            Serde string: ``"european"`` (exercise only at expiry),
            ``"american"`` (any time up to expiry) or ``"bermudan"``
            (on a discrete set of scheduled exercise dates).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def expiry(self) -> datetime.date:
        """
        Last date on which the option may be exercised.

        Returns
        -------
        datetime.date
            Unadjusted calendar expiry date. Time to expiry used in pricing
            is measured from the valuation date to this date under the
            pricing day-count convention.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def notional(self) -> Money:
        """
        Notional for valuation scaling.

        Returns
        -------
        Money
            Currency-tagged notional.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def day_count(self) -> str:
        """
        Day count for the time-to-expiry year fraction (serde name).

        Returns
        -------
        str
            ``"act_365f"`` unless set otherwise.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def theta_day_basis(self) -> str:
        """
        Per-day theta basis.

        Returns
        -------
        str
            ``"calendar_365"`` or ``"trading_252"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def settlement(self) -> str:
        """
        Settlement method.

        Returns
        -------
        str
            ``"physical"`` or ``"cash"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def exercise(self) -> dict[str, object] | None:
        """
        Observed exercise state (``date``, ``spot``, ``settlement_date``, ``exercised``).

        Returns
        -------
        dict[str, object] | None
            The lifecycle dict, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def discount_curve_id(self) -> str:
        """
        Discount curve identifier.

        Returns
        -------
        str
            The curve id.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def spot_id(self) -> str:
        """
        Equity spot price identifier.

        Returns
        -------
        str
            The price id.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def vol_surface_id(self) -> str:
        """
        Volatility surface identifier.

        Returns
        -------
        str
            The surface id.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def div_yield_id(self) -> str | None:
        """
        Continuous dividend yield identifier.

        Returns
        -------
        str | None
            The id, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def discrete_dividends(self) -> list[tuple[datetime.date, float]]:
        """
        Discrete dividend schedule.

        Returns
        -------
        list[tuple[datetime.date, float]]
            ``(ex_date, amount)`` pairs in date order.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def exercise_schedule(self) -> list[datetime.date] | None:
        """
        Bermudan exercise dates.

        Returns
        -------
        list[datetime.date] | None
            The dates, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style rendering of the key economics, e.g. ``EquityOption(id='SPX-CALL-4500', underlying_ticker='SPX', option_type='call', strike=4500.0, expiry=datetime.date(2024, 6, 21), ...)``.

        Returns
        -------
        str
            ``EquityOption(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise; it renders stored values.
        """
        ...

class EquityOptionBuilder:
    """
    Fluent builder for :class:`EquityOption`; wraps the Rust
    ``FinancialBuilder`` output one setter for one setter.

    Builders are consumed by ``build()``; create a new builder per
    instrument. Required fields: ``id``, ``underlying_ticker``, ``strike``,
    ``option_type``, ``expiry``, ``notional``, ``discount_curve_id``,
    ``spot_id``, ``vol_surface_id`` (``exercise_style`` defaults to
    ``"european"``, ``day_count`` to ACT/365F, ``settlement`` to ``"cash"``).

    Examples
    --------
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.valuations.instruments import EquityOption
    >>> option = (
    ...     EquityOption
    ...     .builder()
    ...     .id("AAPL-C-200")
    ...     .underlying_ticker("AAPL")
    ...     .strike(200.0)
    ...     .option_type("call")
    ...     .expiry("2025-06-20")
    ...     .notional(Money(100.0, Currency("USD")))
    ...     .discount_curve_id("USD-OIS")
    ...     .spot_id("AAPL")
    ...     .vol_surface_id("AAPL-VOL")
    ...     .build()
    ... )
    >>> option.exercise_style
    'european'
    """

    def id(self, value: str) -> EquityOptionBuilder:
        """
        Set the instrument identifier.

        Parameters
        ----------
        value : str
            Unique identifier for the equity option.

        Returns
        -------
        EquityOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def underlying_ticker(self, value: str) -> EquityOptionBuilder:
        """
        Set the underlying equity ticker symbol.

        Parameters
        ----------
        value : str
            Underlying equity ticker symbol.

        Returns
        -------
        EquityOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def strike(self, value: float) -> EquityOptionBuilder:
        """
        Set the strike price.

        Parameters
        ----------
        value : float
            Strike price; must be finite and positive.

        Returns
        -------
        EquityOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def option_type(self, value: Literal["call", "put"]) -> EquityOptionBuilder:
        """
        Set the option type.

        Parameters
        ----------
        value : Literal["call", "put"]
            Option type of the equity option.

        Returns
        -------
        EquityOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized option type.
        """
        ...
    def exercise_style(self, value: Literal["european", "american", "bermudan"]) -> EquityOptionBuilder:
        """
        Set the exercise style.

        Parameters
        ----------
        value : Literal["european", "american", "bermudan"]
            Exercise style; defaults to ``"european"`` when never set.

        Returns
        -------
        EquityOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized exercise style.
        """
        ...
    def theta_day_basis(self, value: Literal["calendar_365", "trading_252"]) -> EquityOptionBuilder:
        """
        Set the day basis for per-day theta.

        Parameters
        ----------
        value : Literal["calendar_365", "trading_252"]
            Calendar-day theta is the default; trading-day theta must be selected explicitly.

        Returns
        -------
        EquityOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized theta day basis.
        """
        ...
    def expiry(self, value: datetime.date | datetime.datetime | pd.Timestamp | str) -> EquityOptionBuilder:
        """
        Set the option expiry date.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pd.Timestamp | str
            Option expiry date (ISO 8601 strings accepted).

        Returns
        -------
        EquityOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or the date cannot be interpreted.
        """
        ...
    def day_count(self, value: DayCount | str) -> EquityOptionBuilder:
        """
        Set the day count for the time-to-expiry year fraction.

        Parameters
        ----------
        value : DayCount | str
            Day count convention; ACT/365F when never set.

        Returns
        -------
        EquityOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or a string name is not a recognized day count.
        """
        ...
    def settlement(self, value: Literal["physical", "cash"]) -> EquityOptionBuilder:
        """
        Set the settlement method.

        Parameters
        ----------
        value : Literal["physical", "cash"]
            Physical delivery or fixed cash settlement.

        Returns
        -------
        EquityOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized settlement method.
        """
        ...
    def exercise(
        self,
        date: datetime.date | datetime.datetime | pd.Timestamp | str,
        spot: float,
        settlement_date: datetime.date | datetime.datetime | pd.Timestamp | str,
        exercised: bool,
    ) -> EquityOptionBuilder:
        """
        Set the observed exercise or expiry lifecycle state.

        Parameters
        ----------
        date : datetime.date | datetime.datetime | pd.Timestamp | str
            Exercise date, or expiry date for an unexercised observation.
        spot : float
            Positive observed underlying level in strike-price units.
        settlement_date : datetime.date | datetime.datetime | pd.Timestamp | str
            Contractual cash-payment or physical-delivery date.
        exercised : bool
            Whether exercise or assignment occurred.

        Returns
        -------
        EquityOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or a date cannot be interpreted.
        """
        ...
    def notional(self, value: Money) -> EquityOptionBuilder:
        """
        Set the notional amount for valuation scaling.

        Parameters
        ----------
        value : Money
            Notional amount for valuation scaling.

        Returns
        -------
        EquityOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def discount_curve_id(self, value: str) -> EquityOptionBuilder:
        """
        Set the discount curve identifier for present value calculations.

        Parameters
        ----------
        value : str
            Discount curve identifier.

        Returns
        -------
        EquityOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def spot_id(self, value: str) -> EquityOptionBuilder:
        """
        Set the equity spot price identifier.

        Parameters
        ----------
        value : str
            Equity spot price identifier.

        Returns
        -------
        EquityOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def vol_surface_id(self, value: str) -> EquityOptionBuilder:
        """
        Set the equity volatility surface identifier.

        Parameters
        ----------
        value : str
            Equity volatility surface identifier.

        Returns
        -------
        EquityOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def div_yield_id(self, value: str) -> EquityOptionBuilder:
        """
        Set the continuous dividend yield identifier.

        Parameters
        ----------
        value : str
            Continuous dividend yield identifier; zero yield when never set.

        Returns
        -------
        EquityOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def discrete_dividends(
        self, value: list[tuple[datetime.date | datetime.datetime | pd.Timestamp | str, float]]
    ) -> EquityOptionBuilder:
        """
        Set the discrete dividend schedule.

        Parameters
        ----------
        value : list[tuple[datetime.date | datetime.datetime | pd.Timestamp | str, float]]
            Positive ``(ex_date, dividend_amount)`` pairs in strictly increasing date order. European pricing uses escrowed spot adjustment; tree pricing restores remaining dividend value at exercise nodes.

        Returns
        -------
        EquityOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or a date cannot be interpreted.
        """
        ...
    def exercise_schedule(
        self, value: list[datetime.date | datetime.datetime | pd.Timestamp | str]
    ) -> EquityOptionBuilder:
        """
        Set the exercise schedule for Bermudan options.

        Parameters
        ----------
        value : list[datetime.date | datetime.datetime | pd.Timestamp | str]
            Dates on which early exercise is permitted; required when ``exercise_style`` is ``"bermudan"``.

        Returns
        -------
        EquityOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or a date cannot be interpreted.
        """
        ...
    def attributes(self, value: Attributes | dict[str, str] | None) -> EquityOptionBuilder:
        """
        Set instrument attributes (tags and metadata).

        Parameters
        ----------
        value : Attributes | dict[str, str] | None
            Attribute bag; a ``dict`` populates ``meta`` and an optional ``"tags"`` entry holding a list of strings populates ``tags``.

        Returns
        -------
        EquityOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        TypeError
            If ``value`` is neither ``Attributes``, a ``dict`` nor ``None``.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style rendering of the fields set so far, e.g.
        ``EquityOptionBuilder(id='X', notional=Money(1000000.0, 'USD'))``.

        Returns
        -------
        str
            ``EquityOptionBuilder(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise; it renders the recorded setter calls.
        """
        ...
    def build(self) -> EquityOption:
        """
        Build the validated equity option.

        Runs only the Rust ``EquityOptionBuilder::build`` validation (structural
        invariants); pricing-time checks run in :meth:`EquityOption.price`.

        Returns
        -------
        EquityOption
            The validated instrument.

        Raises
        ------
        ValueError
            If the builder was already consumed, a required field is missing
            (the message names the builder and the field, e.g.
            ``EquityOptionBuilder: missing required field 'id'``), or the instrument
            fails validation (non-positive strike, zero notional, unsorted dividends, inconsistent exercise state).
        """
        ...

class RepLine:
    """
    Aggregated representative line for pool modeling.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.dates import DayCount
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.valuations.instruments import RepLine
    >>> line = RepLine(
    ...     "LINE-1",
    ...     Money(80_000_000.0, Currency("USD")),
    ...     0.07,
    ...     datetime.date(2031, 1, 15),
    ...     12,
    ...     DayCount.ACT_360,
    ...     cpr=0.10,
    ...     cdr=0.02,
    ...     recovery_rate=0.45,
    ... )
    >>> "LINE-1" in repr(line)
    True
    """

    def __init__(
        self,
        id: str,
        balance: Money,
        rate: float | Rate,
        maturity: datetime.date | datetime.datetime | pd.Timestamp | str,
        seasoning_months: int,
        day_count: DayCount,
        *,
        spread_bp: float | Bps | None = None,
        index_id: str | None = None,
        cpr: float | None = None,
        cdr: float | None = None,
        recovery_rate: float | None = None,
    ) -> None:
        """
        Aggregated representative line for pool modeling.

        Parameters
        ----------
        id : str
            Unique identifier for the rep line.
        balance : Money
            Aggregated balance.
        rate : float | Rate
            Weighted average coupon as an annual decimal rate (e.g. ``0.07``
            = 7%).
        maturity : datetime.date | datetime.datetime | pandas.Timestamp | str
            Weighted average maturity date (date-like or ISO-8601 string).
        seasoning_months : int
            Weighted average seasoning in months.
        day_count : DayCount
            Day count convention.
        spread_bp : float | Bps, optional
            Weighted average spread over the reference index, in basis
            points (e.g. ``150.0`` = 150bp), for floating-rate lines.
        index_id : str, optional
            Reference index identifier, if floating.
        cpr : float, optional
            Constant prepayment rate override, as an annual decimal (e.g.
            ``0.10`` = 10% CPR).
        cdr : float, optional
            Constant default rate override, as an annual decimal (e.g.
            ``0.02`` = 2% CDR).
        recovery_rate : float, optional
            Recovery rate override, as a decimal fraction (e.g. ``0.45`` =
            45%).

        Returns
        -------
        RepLine
            The rep line.

        Raises
        ------
        TypeError
            If ``maturity`` is neither date-like nor a string, or ``rate`` /
            ``spread_bp`` are not numbers / ``Rate`` / ``Bps``.
        ValueError
            If a string ``maturity`` is not valid ISO-8601.

        Examples
        --------
        >>> import datetime
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.dates import DayCount
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.instruments import RepLine
        >>> line = RepLine(
        ...     "LINE-1",
        ...     Money(80_000_000.0, Currency("USD")),
        ...     0.07,
        ...     datetime.date(2031, 1, 15),
        ...     12,
        ...     DayCount.ACT_360,
        ...     cpr=0.10,
        ...     cdr=0.02,
        ...     recovery_rate=0.45,
        ... )
        >>> "LINE-1" in repr(line)
        True
        """
        ...

    @staticmethod
    def from_json(json: str) -> RepLine:
        """
        Deserialize from the JSON produced by :meth:`to_json`.

        Parameters
        ----------
        json : str
            Strict JSON object with exactly the fields ``to_json`` writes.

        Returns
        -------
        RepLine
            The reconstructed value.

        Raises
        ------
        ValueError
            If the JSON is malformed or has the wrong shape.

        Examples
        --------
        >>> import datetime
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.dates import DayCount
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.instruments import RepLine
        >>> restored = RepLine.from_json(
        ...     RepLine(
        ...         "LINE-1", Money(1.0, Currency("USD")), 0.07, datetime.date(2031, 1, 15), 12, DayCount.ACT_360
        ...     ).to_json()
        ... )
        >>> restored.to_json() == RepLine(
        ...     "LINE-1", Money(1.0, Currency("USD")), 0.07, datetime.date(2031, 1, 15), 12, DayCount.ACT_360
        ... ).to_json()
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON wire form (also used by ``pickle``).

        Returns
        -------
        str
            Strict JSON accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If canonical serialization fails.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return every field as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Same content as ``json.loads(self.to_json())``.

        Raises
        ------
        ValueError
            If canonical serialization fails.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` via the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(from_json, (payload,))``.

        Raises
        ------
        ValueError
            If the value cannot be reconstructed from its JSON form.
        """
        ...

    @property
    def id(self) -> str:
        """
        Rep line identifier.

        Returns
        -------
        str
            Unique identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def balance(self) -> Money:
        """
        Aggregated balance.

        Returns
        -------
        Money
            Currency-tagged balance.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def rate(self) -> float:
        """
        Weighted average coupon as an annual decimal rate.

        Returns
        -------
        float
            ``0.07`` for 7%.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def spread_bp(self) -> float | None:
        """
        Weighted average spread in basis points, or ``None`` for fixed lines.

        Returns
        -------
        float | None
            Spread in bp, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def index_id(self) -> str | None:
        """
        Reference index identifier, or ``None``.

        Returns
        -------
        str | None
            Index id, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def maturity(self) -> datetime.date:
        """
        Weighted average maturity date.

        Returns
        -------
        datetime.date
            The maturity date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def seasoning_months(self) -> int:
        """
        Weighted average seasoning in months.

        Returns
        -------
        int
            Months since origination.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def day_count(self) -> str:
        """
        Day count convention (serde string).

        Returns
        -------
        str
            e.g. ``"act_360"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def cpr(self) -> float | None:
        """
        Constant prepayment rate override (annual decimal), or ``None``.

        Returns
        -------
        float | None
            CPR, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def cdr(self) -> float | None:
        """
        Constant default rate override (annual decimal), or ``None``.

        Returns
        -------
        float | None
            CDR, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def recovery_rate(self) -> float | None:
        """
        Recovery rate override (decimal fraction), or ``None``.

        Returns
        -------
        float | None
            Recovery, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class AssetPool:
    """
    Structured-credit collateral pool.

    Examples
    --------
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.valuations.instruments import AssetPool
    >>> pool = AssetPool("POOL-1", "abs", Currency("USD"))
    >>> "POOL-1" in repr(pool)
    True
    """

    def __init__(
        self,
        id: str,
        deal_type: Literal["clo", "cbo", "abs", "rmbs", "cmbs", "auto", "card"],
        base_currency: Currency | str,
    ) -> None:
        """
        Structured-credit collateral pool.

        Parameters
        ----------
        id : str
            Pool identifier.
        deal_type : {"clo", "cbo", "abs", "rmbs", "cmbs", "auto", "card"}
            Deal classification for pool-level assumptions.
        base_currency : Currency | str
            Base currency (``Currency`` or ISO-4217 code) for every asset and
            pool-level account.

        Returns
        -------
        AssetPool
            A new, empty asset pool. Use :meth:`with_rep_lines` and/or
            :meth:`assets` to attach collateral.

        Raises
        ------
        ValueError
            If ``deal_type`` is not a recognized deal type.

        Examples
        --------
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.valuations.instruments import AssetPool
        >>> pool = AssetPool("POOL-1", "abs", Currency("USD"))
        >>> "POOL-1" in repr(pool)
        True
        """
        ...

    def with_rep_lines(self, rep_lines: list[RepLine]) -> AssetPool:
        """
        Attach representative pool lines, returning a new pool.

        Parameters
        ----------
        rep_lines : list[RepLine]
            Aggregated representative lines the pricing engine will use
            instead of individual assets.

        Returns
        -------
        AssetPool
            A new pool with ``rep_lines`` set (the original is unchanged).

        Raises
        ------
        TypeError
            If an element of ``rep_lines`` is not a ``RepLine``.

        Examples
        --------
        >>> import datetime
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.dates import DayCount
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.instruments import AssetPool, RepLine
        >>> pool = AssetPool("POOL-1", "abs", Currency("USD")).with_rep_lines([
        ...     RepLine(
        ...         "LINE-1",
        ...         Money(80_000_000.0, Currency("USD")),
        ...         0.07,
        ...         datetime.date(2031, 1, 15),
        ...         12,
        ...         DayCount.ACT_360,
        ...     )
        ... ])
        >>> "POOL-1" in repr(pool)
        True
        """
        ...

    def assets(self, value: list[dict[str, Any]] | str) -> AssetPool:
        """
        Attach loan-level assets, returning a new pool.

        Loan-level ``PoolAsset`` records carry ~30 fields and stay in their
        serde dict shape; use :meth:`with_rep_lines` for the typed,
        aggregated path.

        Parameters
        ----------
        value : list[dict[str, Any]] | str
            ``PoolAsset`` objects as a list of dicts or a JSON array string.

        Returns
        -------
        AssetPool
            A new pool with ``assets`` set (the original is unchanged).

        Raises
        ------
        ValueError
            If ``value`` does not match the ``PoolAsset`` list shape.
        """
        ...

    @staticmethod
    def from_json(json: str) -> AssetPool:
        """
        Deserialize from the JSON produced by :meth:`to_json`.

        Parameters
        ----------
        json : str
            Strict JSON object with exactly the fields ``to_json`` writes.

        Returns
        -------
        AssetPool
            The reconstructed value.

        Raises
        ------
        ValueError
            If the JSON is malformed or has the wrong shape.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import AssetPool
        >>> restored = AssetPool.from_json(AssetPool("POOL-1", "abs", "USD").to_json())
        >>> restored.to_json() == AssetPool("POOL-1", "abs", "USD").to_json()
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON wire form (also used by ``pickle``).

        Returns
        -------
        str
            Strict JSON accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If canonical serialization fails.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return every field as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Same content as ``json.loads(self.to_json())``.

        Raises
        ------
        ValueError
            If canonical serialization fails.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` via the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(from_json, (payload,))``.

        Raises
        ------
        ValueError
            If the value cannot be reconstructed from its JSON form.
        """
        ...

    @property
    def id(self) -> str:
        """
        Pool identifier.

        Returns
        -------
        str
            Unique identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def deal_type(self) -> str:
        """
        Deal classification (serde string).

        Returns
        -------
        str
            ``"abs"``, ``"clo"``, ...

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def base_currency(self) -> str:
        """
        Base ISO-4217 currency code.

        Returns
        -------
        str
            Three-letter code.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def asset_records(self) -> list[dict[str, Any]]:
        """
        Loan-level assets in their ``PoolAsset`` serde shape.

        Returns
        -------
        list[dict[str, Any]]
            One dict per asset (empty when rep lines are used).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def rep_lines(self) -> list[RepLine] | None:
        """
        Representative lines, or ``None`` when the pool is modelled loan-level.

        Returns
        -------
        list[RepLine] | None
            Rep lines, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def cumulative_defaults(self) -> Money:
        """
        Cumulative defaults to date.

        Returns
        -------
        Money
            Currency-tagged amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def cumulative_recoveries(self) -> Money:
        """
        Cumulative recoveries to date.

        Returns
        -------
        Money
            Currency-tagged amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def cumulative_prepayments(self) -> Money:
        """
        Cumulative prepayments to date.

        Returns
        -------
        Money
            Currency-tagged amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def cumulative_scheduled_amortization(self) -> Money:
        """
        Cumulative scheduled amortization to date.

        Returns
        -------
        Money
            Currency-tagged amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def collection_account(self) -> Money:
        """
        Collection account balance.

        Returns
        -------
        Money
            Currency-tagged amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def reserve_account(self) -> Money:
        """
        Reserve account balance.

        Returns
        -------
        Money
            Currency-tagged amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def excess_spread_account(self) -> Money:
        """
        Excess-spread account balance.

        Returns
        -------
        Money
            Currency-tagged amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class Tranche:
    """
    Structured-credit tranche with attachment/detachment points.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.valuations.instruments import Tranche
    >>> tranche = (
    ...     Tranche
    ...     .builder()
    ...     .id("A")
    ...     .attachment_point(0.0)
    ...     .detachment_point(100.0)
    ...     .seniority("senior")
    ...     .original_balance(Money(100.0, Currency("USD")))
    ...     .coupon_fixed(0.05)
    ...     .maturity(datetime.date(2029, 1, 1))
    ...     .build()
    ... )
    >>> tranche.seniority, tranche.attachment_point, tranche.detachment_point
    ('senior', 0.0, 100.0)

    """

    @staticmethod
    def builder() -> TrancheBuilder:
        """
        Create a fluent builder (mirrors Rust ``Tranche::builder()``).

        Returns
        -------
        TrancheBuilder
            A builder with fluent, consuming setter methods.

        Notes
        -----
        This factory does not raise; it returns a new instance with the documented defaults.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Tranche
        >>> builder = Tranche.builder()
        >>> builder.id("EXAMPLE") is builder
        True
        """
        ...

    @staticmethod
    def from_json(json: str) -> Tranche:
        """
        Deserialize from the JSON produced by :meth:`to_json`.

        Parameters
        ----------
        json : str
            Strict JSON object with exactly the fields ``to_json`` writes.

        Returns
        -------
        Tranche
            The reconstructed value.

        Raises
        ------
        ValueError
            If the JSON is malformed or has the wrong shape.

        Examples
        --------
        >>> import datetime
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.instruments import Tranche
        >>> tranche = (
        ...     Tranche
        ...     .builder()
        ...     .id("A")
        ...     .attachment_point(0.0)
        ...     .detachment_point(100.0)
        ...     .seniority("senior")
        ...     .original_balance(Money(100.0, Currency("USD")))
        ...     .coupon_fixed(0.05)
        ...     .maturity(datetime.date(2029, 1, 1))
        ...     .build()
        ... )
        >>> restored = Tranche.from_json(tranche.to_json())
        >>> restored.to_json() == tranche.to_json()
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON wire form (also used by ``pickle``).

        Returns
        -------
        str
            Strict JSON accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If canonical serialization fails.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return every field as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Same content as ``json.loads(self.to_json())``.

        Raises
        ------
        ValueError
            If canonical serialization fails.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` via the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(from_json, (payload,))``.

        Raises
        ------
        ValueError
            If the value cannot be reconstructed from its JSON form.
        """
        ...

    @property
    def id(self) -> str:
        """
        Tranche identifier.

        Returns
        -------
        str
            Unique identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def attachment_point(self) -> float:
        """
        Attachment point in percent (0-100 scale).

        Returns
        -------
        float
            e.g. ``10.0``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def detachment_point(self) -> float:
        """
        Detachment point in percent (0-100 scale).

        Returns
        -------
        float
            e.g. ``100.0``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def seniority(self) -> str:
        """
        Seniority (serde string, e.g. ``"Senior"``).

        Returns
        -------
        str
            Wire spelling of the seniority.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def behavior_type(self) -> str:
        """
        Tranche behavior type (serde string).

        Returns
        -------
        str
            Wire spelling of the behavior type.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def rating(self) -> str | None:
        """
        Credit rating (serde string) or ``None``.

        Returns
        -------
        str | None
            Rating, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def original_balance(self) -> Money:
        """
        Original (issuance) balance.

        Returns
        -------
        Money
            Currency-tagged amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def current_balance(self) -> Money:
        """
        Current outstanding balance.

        Returns
        -------
        Money
            Currency-tagged amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def target_balance(self) -> Money | None:
        """
        Target balance for revolving structures, or ``None``.

        Returns
        -------
        Money | None
            Amount, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def coupon(self) -> dict[str, Any]:
        """
        Coupon definition (``TrancheCoupon`` serde shape).

        Returns
        -------
        dict[str, Any]
            Tagged coupon dict.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def frequency(self) -> str:
        """
        Coupon payment frequency of the tranche premium leg.

        Returns
        -------
        str
            Tenor string giving the period between payments, such as
            ``"3M"`` for quarterly or ``"6M"`` for semi-annual. Payment
            dates are rolled from this frequency using the deal's
            business-day convention.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def day_count(self) -> str:
        """
        Accrual day count (serde string).

        Returns
        -------
        str
            e.g. ``"act_360"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def deferred_interest(self) -> Money:
        """
        Accumulated deferred (PIK) interest.

        Returns
        -------
        Money
            Currency-tagged amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def pik_enabled(self) -> bool:
        """
        Whether interest may be deferred (PIK).

        Returns
        -------
        bool
            ``True`` when PIK is enabled.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def is_revolving(self) -> bool:
        """
        Whether the tranche balance revolves.

        Returns
        -------
        bool
            ``True`` for revolving tranches.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def can_reinvest(self) -> bool:
        """
        Whether principal collections may be reinvested.

        Returns
        -------
        bool
            ``True`` when reinvestment is allowed.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def maturity(self) -> datetime.date:
        """
        Legal final maturity date.

        Returns
        -------
        datetime.date
            The maturity date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def expected_maturity(self) -> datetime.date | None:
        """
        Expected maturity date, or ``None``.

        Returns
        -------
        datetime.date | None
            Date, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def payment_priority(self) -> int:
        """
        Payment priority rank (1 = most senior).

        Returns
        -------
        int
            Priority rank.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def attributes(self) -> Attributes:
        """
        User attributes (tags and metadata).

        Returns
        -------
        Attributes
            Attribute bag.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class TrancheBuilder:
    """
    Fluent builder returned by :meth:`Tranche.builder`.

    ``attachment_point`` and ``detachment_point`` are tracked separately from
    the wrapped Rust builder (which only exposes a combined
    ``attachment_detachment(a, d)`` setter) and applied together on
    :meth:`build`, so either call order works.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import Tranche
    >>> isinstance(Tranche.builder(), Tranche.builder().__class__)
    True
    """

    def id(self, value: str) -> TrancheBuilder:
        """
        Set the tranche identifier.

        Parameters
        ----------
        value : str
            Unique identifier for the tranche.

        Returns
        -------
        TrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If this builder was already consumed by a prior call to
            :meth:`TrancheBuilder.build`.
        """
        ...

    def attachment_point(self, value: float) -> TrancheBuilder:
        """
        Set the attachment point.

        Parameters
        ----------
        value : float
            Attachment point quoted in percent on a 0-100 scale (e.g. ``0.0``
            for equity, ``10.0`` for a tranche attaching at 10%).

        Returns
        -------
        TrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If this builder was already consumed by a prior call to
            :meth:`TrancheBuilder.build`.
        """
        ...

    def detachment_point(self, value: float) -> TrancheBuilder:
        """
        Set the detachment point.

        Parameters
        ----------
        value : float
            Detachment point quoted in percent on a 0-100 scale (e.g.
            ``100.0`` for the most senior tranche).

        Returns
        -------
        TrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If this builder was already consumed by a prior call to
            :meth:`TrancheBuilder.build`.
        """
        ...

    def seniority(self, value: Literal["senior", "mezzanine", "subordinated", "equity"]) -> TrancheBuilder:
        """
        Set the tranche seniority.

        Parameters
        ----------
        value : {"senior", "mezzanine", "subordinated", "equity"}
            Structural seniority of the tranche.

        Returns
        -------
        TrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` is not a recognized seniority.
        """
        ...

    def original_balance(self, value: Money) -> TrancheBuilder:
        """
        Set the original tranche balance.

        Maps to the Rust ``TrancheBuilder::balance`` setter; named
        ``original_balance`` here to match the ``Tranche.original_balance``
        field it populates.

        Parameters
        ----------
        value : Money
            Original tranche balance. Must be positive.

        Returns
        -------
        TrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If this builder was already consumed by a prior call to
            :meth:`TrancheBuilder.build`.
        """
        ...

    def coupon_fixed(self, rate: float | Rate) -> TrancheBuilder:
        """
        Set a fixed-rate coupon.

        Parameters
        ----------
        rate : float | Rate
            Fixed interest rate as an annual decimal (e.g. ``0.05`` = 5%).

        Returns
        -------
        TrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If this builder was already consumed by a prior call to
            :meth:`TrancheBuilder.build`.
        """
        ...

    def coupon_floating(self, value: dict[str, Any] | str) -> TrancheBuilder:
        """
        Set a floating-rate coupon from a JSON ``TrancheCoupon::Floating`` payload.

        The floating-rate spec (``FloatingRateSpec``: index, spread, gearing,
        floors/caps, reset conventions) stays JSON per the nested-spec rule —
        the typed cashflows plan owns that shape.

        Parameters
        ----------
        value : dict[str, Any] | str
            JSON-encoded, externally-tagged ``TrancheCoupon`` value, e.g.
            ``{"floating": {...FloatingRateSpec fields...}}``.

        Returns
        -------
        TrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` is not valid JSON for the ``TrancheCoupon`` shape.
        """
        ...

    def maturity(self, value: datetime.date | datetime.datetime | pd.Timestamp | str) -> TrancheBuilder:
        """
        Set the legal final maturity date.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pandas.Timestamp | str
            Legal final maturity date.

        Returns
        -------
        TrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If this builder was already consumed by a prior call to
            :meth:`TrancheBuilder.build`.
        """
        ...

    def frequency(self, value: Tenor) -> TrancheBuilder:
        """
        Set the payment frequency.

        Parameters
        ----------
        value : Tenor
            Payment frequency. Defaults to quarterly when never set.

        Returns
        -------
        TrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If this builder was already consumed by a prior call to
            :meth:`TrancheBuilder.build`.
        """
        ...

    def day_count(self, value: DayCount) -> TrancheBuilder:
        """
        Set the day count convention for interest accrual.

        Parameters
        ----------
        value : DayCount
            Day count convention. Defaults to Act/360 when never set.

        Returns
        -------
        TrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If this builder was already consumed by a prior call to
            :meth:`TrancheBuilder.build`.
        """
        ...

    def __repr__(self) -> str:
        """
        Return ``repr(self)`` showing the tracked attachment/detachment points.

        Returns
        -------
        str
            ``TrancheBuilder(attachment_point=..., detachment_point=..., consumed=...)``.
        """
        ...

    def build(self) -> Tranche:
        """
        Build the validated tranche.

        Returns
        -------
        Tranche
            The validated tranche.

        Raises
        ------
        ValueError
            If a required field is missing, or attachment/detachment points
            are invalid (negative, out of the ``[0, 100]`` range, or
            detachment not strictly above attachment).
        """
        ...

class TrancheStructure:
    """
    Capital structure formed from a list of tranches.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.valuations.instruments import Tranche
    >>> tranche = (
    ...     Tranche
    ...     .builder()
    ...     .id("A")
    ...     .attachment_point(0.0)
    ...     .detachment_point(100.0)
    ...     .seniority("senior")
    ...     .original_balance(Money(100.0, Currency("USD")))
    ...     .coupon_fixed(0.05)
    ...     .maturity(datetime.date(2029, 1, 1))
    ...     .build()
    ... )
    >>> from finstack_quant.valuations.instruments import TrancheStructure
    >>> "tranches=1" in repr(TrancheStructure([tranche]))
    True

    """

    def __init__(self, tranches: list[Tranche]) -> None:
        """
        Capital structure formed from a list of tranches.

        Validates that attachment/detachment points tile ``[0, 100]`` without
        gaps or overlaps, that every tranche shares one currency, and assigns
        each tranche a distinct, strictly-increasing ``payment_priority``
        ranked by seniority.

        Parameters
        ----------
        tranches : list[Tranche]
            Tranches forming the capital structure.

        Returns
        -------
        TrancheStructure
            The validated tranche structure.

        Raises
        ------
        ValueError
            If ``tranches`` is empty, has non-finite attachment/detachment
            points, leaves a gap/overlap, doesn't tile to 100%, or mixes
            currencies.

        Examples
        --------
        >>> import datetime
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.instruments import Tranche, TrancheStructure
        >>> senior = (
        ...     Tranche
        ...     .builder()
        ...     .id("A")
        ...     .attachment_point(10.0)
        ...     .detachment_point(100.0)
        ...     .seniority("senior")
        ...     .original_balance(Money(72_000_000.0, Currency("USD")))
        ...     .coupon_fixed(0.05)
        ...     .maturity(datetime.date(2031, 1, 15))
        ...     .build()
        ... )
        >>> equity = (
        ...     Tranche
        ...     .builder()
        ...     .id("E")
        ...     .attachment_point(0.0)
        ...     .detachment_point(10.0)
        ...     .seniority("equity")
        ...     .original_balance(Money(8_000_000.0, Currency("USD")))
        ...     .coupon_fixed(0.0)
        ...     .maturity(datetime.date(2031, 1, 15))
        ...     .build()
        ... )
        >>> structure = TrancheStructure([senior, equity])
        >>> "tranches=2" in repr(structure)
        True
        """
        ...

    @staticmethod
    def from_json(json: str) -> TrancheStructure:
        """
        Deserialize from the JSON produced by :meth:`to_json`.

        Parameters
        ----------
        json : str
            Strict JSON object with exactly the fields ``to_json`` writes.

        Returns
        -------
        TrancheStructure
            The reconstructed value.

        Raises
        ------
        ValueError
            If the JSON is malformed or has the wrong shape.

        Examples
        --------
        >>> import datetime
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.instruments import Tranche
        >>> tranche = (
        ...     Tranche
        ...     .builder()
        ...     .id("A")
        ...     .attachment_point(0.0)
        ...     .detachment_point(100.0)
        ...     .seniority("senior")
        ...     .original_balance(Money(100.0, Currency("USD")))
        ...     .coupon_fixed(0.05)
        ...     .maturity(datetime.date(2029, 1, 1))
        ...     .build()
        ... )
        >>> from finstack_quant.valuations.instruments import TrancheStructure
        >>> structure = TrancheStructure([tranche])
        >>> restored = TrancheStructure.from_json(structure.to_json())
        >>> restored.to_json() == structure.to_json()
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON wire form (also used by ``pickle``).

        Returns
        -------
        str
            Strict JSON accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If canonical serialization fails.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return every field as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Same content as ``json.loads(self.to_json())``.

        Raises
        ------
        ValueError
            If canonical serialization fails.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` via the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(from_json, (payload,))``.

        Raises
        ------
        ValueError
            If the value cannot be reconstructed from its JSON form.
        """
        ...

    @property
    def tranches(self) -> list[Tranche]:
        """
        Tranches in payment-priority order.

        Returns
        -------
        list[Tranche]
            Independent copies of the tranches.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def total_size(self) -> Money:
        """
        Total original size of the capital structure.

        Returns
        -------
        Money
            Currency-tagged amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class StructuredCredit:
    """
    Structured-credit deal (ABS/CLO/CMBS/RMBS) with pool, tranches, and waterfall.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import StructuredCredit
    >>> builder = StructuredCredit.builder()
    >>> builder.id("EXAMPLE") is builder
    True

    """

    @staticmethod
    def builder() -> StructuredCreditBuilder:
        """
        Create a fluent builder (mirrors Rust ``StructuredCredit::builder()``).

        The builder pre-seeds ``market_conditions``, ``credit_factors``,
        ``deal_metadata``, ``behavior_overrides``, ``default_assumptions``,
        and ``hedge_swaps`` with their Rust ``Default`` values (the Rust
        builder fields have no default), which the corresponding ``*_json``
        setters can override. Prefer :meth:`new_abs` / :meth:`new_clo` /
        :meth:`new_cmbs` / :meth:`new_rmbs` for registry-calibrated deal-type
        defaults; use this builder for full manual control.

        Returns
        -------
        StructuredCreditBuilder
            A builder with fluent, consuming setter methods.

        Notes
        -----
        This factory does not raise; it returns a new instance with the documented defaults.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import StructuredCredit
        >>> builder = StructuredCredit.builder()
        >>> builder.id("EXAMPLE") is builder
        True
        """
        ...

    @staticmethod
    def new_abs(
        id: str,
        pool: AssetPool,
        tranches: TrancheStructure,
        closing_date: datetime.date,
        maturity: datetime.date,
        discount_curve_id: str,
    ) -> StructuredCredit:
        """
        Create a new ABS deal with registry-calibrated defaults.

        Parameters
        ----------
        id : str
            Unique instrument identifier.
        pool : AssetPool
            Asset pool definition.
        tranches : TrancheStructure
            Tranche capital structure.
        closing_date : datetime.date
            Deal closing date (issuance).
        maturity : datetime.date | datetime.datetime | pandas.Timestamp | str
            Legal final maturity date.
        discount_curve_id : str
            Discount curve identifier for valuation.

        Returns
        -------
        StructuredCredit
            The validated ABS deal.

        Raises
        ------
        ValueError
            If the deal fails pricing validation.

        Examples
        --------
        >>> import datetime
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.dates import DayCount
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.instruments import (
        ...     AssetPool,
        ...     RepLine,
        ...     StructuredCredit,
        ...     Tranche,
        ...     TrancheStructure,
        ... )
        >>> pool = AssetPool("POOL-1", "abs", Currency("USD")).with_rep_lines([
        ...     RepLine(
        ...         "LINE-1",
        ...         Money(80_000_000.0, Currency("USD")),
        ...         0.07,
        ...         datetime.date(2031, 1, 15),
        ...         12,
        ...         DayCount.ACT_360,
        ...     )
        ... ])
        >>> senior = (
        ...     Tranche
        ...     .builder()
        ...     .id("A")
        ...     .attachment_point(10.0)
        ...     .detachment_point(100.0)
        ...     .seniority("senior")
        ...     .original_balance(Money(72_000_000.0, Currency("USD")))
        ...     .coupon_fixed(0.05)
        ...     .maturity(datetime.date(2031, 1, 15))
        ...     .build()
        ... )
        >>> equity = (
        ...     Tranche
        ...     .builder()
        ...     .id("E")
        ...     .attachment_point(0.0)
        ...     .detachment_point(10.0)
        ...     .seniority("equity")
        ...     .original_balance(Money(8_000_000.0, Currency("USD")))
        ...     .coupon_fixed(0.0)
        ...     .maturity(datetime.date(2031, 1, 15))
        ...     .build()
        ... )
        >>> deal = StructuredCredit.new_abs(
        ...     "ABS-1",
        ...     pool,
        ...     TrancheStructure([senior, equity]),
        ...     datetime.date(2024, 1, 15),
        ...     datetime.date(2031, 1, 15),
        ...     "USD-SOFR-DISC",
        ... )
        >>> "ABS-1" in repr(deal)
        True
        """
        ...

    @staticmethod
    def new_clo(
        id: str,
        pool: AssetPool,
        tranches: TrancheStructure,
        closing_date: datetime.date,
        maturity: datetime.date,
        discount_curve_id: str,
    ) -> StructuredCredit:
        """
        Create a new CLO deal with registry-calibrated defaults.

        Same signature as :meth:`new_abs`; only the deal-type calibration
        (prepayment/default/recovery specs, frequency, fees) differs.

        Parameters
        ----------
        id : str
            Unique instrument identifier.
        pool : AssetPool
            Asset pool definition.
        tranches : TrancheStructure
            Tranche capital structure.
        closing_date : datetime.date
            Deal closing date (issuance).
        maturity : datetime.date | datetime.datetime | pandas.Timestamp | str
            Legal final maturity date.
        discount_curve_id : str
            Discount curve identifier for valuation.

        Returns
        -------
        StructuredCredit
            The validated CLO deal.

        Raises
        ------
        ValueError
            If the deal fails pricing validation.

        Examples
        --------
        >>> import datetime
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.instruments import Tranche
        >>> tranche = (
        ...     Tranche
        ...     .builder()
        ...     .id("A")
        ...     .attachment_point(0.0)
        ...     .detachment_point(100.0)
        ...     .seniority("senior")
        ...     .original_balance(Money(100.0, Currency("USD")))
        ...     .coupon_fixed(0.05)
        ...     .maturity(datetime.date(2029, 1, 1))
        ...     .build()
        ... )
        >>> from finstack_quant.valuations.instruments import AssetPool, StructuredCredit, TrancheStructure
        >>> pool = AssetPool("P", "clo", Currency("USD"))
        >>> deal = StructuredCredit.new_clo(
        ...     "D", pool, TrancheStructure([tranche]), datetime.date(2024, 1, 1), datetime.date(2029, 1, 1), "USD-OIS"
        ... )
        >>> (deal.id, "Clo" in repr(deal))
        ('D', True)

        """
        ...

    @staticmethod
    def new_cmbs(
        id: str,
        pool: AssetPool,
        tranches: TrancheStructure,
        closing_date: datetime.date,
        maturity: datetime.date,
        discount_curve_id: str,
    ) -> StructuredCredit:
        """
        Create a new CMBS deal with registry-calibrated defaults.

        Same signature as :meth:`new_abs`; only the deal-type calibration
        (prepayment/default/recovery specs, frequency, fees) differs.

        Parameters
        ----------
        id : str
            Unique instrument identifier.
        pool : AssetPool
            Asset pool definition.
        tranches : TrancheStructure
            Tranche capital structure.
        closing_date : datetime.date
            Deal closing date (issuance).
        maturity : datetime.date | datetime.datetime | pandas.Timestamp | str
            Legal final maturity date.
        discount_curve_id : str
            Discount curve identifier for valuation.

        Returns
        -------
        StructuredCredit
            The validated CMBS deal.

        Raises
        ------
        ValueError
            If the deal fails pricing validation.

        Examples
        --------
        >>> import datetime
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.instruments import Tranche
        >>> tranche = (
        ...     Tranche
        ...     .builder()
        ...     .id("A")
        ...     .attachment_point(0.0)
        ...     .detachment_point(100.0)
        ...     .seniority("senior")
        ...     .original_balance(Money(100.0, Currency("USD")))
        ...     .coupon_fixed(0.05)
        ...     .maturity(datetime.date(2029, 1, 1))
        ...     .build()
        ... )
        >>> from finstack_quant.valuations.instruments import AssetPool, StructuredCredit, TrancheStructure
        >>> pool = AssetPool("P", "cmbs", Currency("USD"))
        >>> deal = StructuredCredit.new_cmbs(
        ...     "D", pool, TrancheStructure([tranche]), datetime.date(2024, 1, 1), datetime.date(2029, 1, 1), "USD-OIS"
        ... )
        >>> (deal.id, "Cmbs" in repr(deal))
        ('D', True)

        """
        ...

    @staticmethod
    def new_rmbs(
        id: str,
        pool: AssetPool,
        tranches: TrancheStructure,
        closing_date: datetime.date,
        maturity: datetime.date,
        discount_curve_id: str,
    ) -> StructuredCredit:
        """
        Create a new RMBS deal with registry-calibrated defaults.

        Same signature as :meth:`new_abs`; only the deal-type calibration
        (prepayment/default/recovery specs, frequency, fees) differs.

        Parameters
        ----------
        id : str
            Unique instrument identifier.
        pool : AssetPool
            Asset pool definition.
        tranches : TrancheStructure
            Tranche capital structure.
        closing_date : datetime.date
            Deal closing date (issuance).
        maturity : datetime.date | datetime.datetime | pandas.Timestamp | str
            Legal final maturity date.
        discount_curve_id : str
            Discount curve identifier for valuation.

        Returns
        -------
        StructuredCredit
            The validated RMBS deal.

        Raises
        ------
        ValueError
            If the deal fails pricing validation.

        Examples
        --------
        >>> import datetime
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.instruments import Tranche
        >>> tranche = (
        ...     Tranche
        ...     .builder()
        ...     .id("A")
        ...     .attachment_point(0.0)
        ...     .detachment_point(100.0)
        ...     .seniority("senior")
        ...     .original_balance(Money(100.0, Currency("USD")))
        ...     .coupon_fixed(0.05)
        ...     .maturity(datetime.date(2029, 1, 1))
        ...     .build()
        ... )
        >>> from finstack_quant.valuations.instruments import AssetPool, StructuredCredit, TrancheStructure
        >>> pool = AssetPool("P", "rmbs", Currency("USD"))
        >>> deal = StructuredCredit.new_rmbs(
        ...     "D", pool, TrancheStructure([tranche]), datetime.date(2024, 1, 1), datetime.date(2029, 1, 1), "USD-OIS"
        ... )
        >>> (deal.id, "Rmbs" in repr(deal))
        ('D', True)

        """
        ...

    @classmethod
    def from_json(cls, json: str) -> StructuredCredit:
        """
        Deserialize a validated deal from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"structured_credit"`` payload. The UTF-8 input must not exceed
            16 MiB. Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        StructuredCredit
            The validated deal.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails structured-credit
            validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import StructuredCredit
        >>> try:
        ...     StructuredCredit.from_json("{}")
        ... except ValueError as exc:
        ...     print("schema" in str(exc))
        True

        """
        ...

    def to_json(self) -> str:
        """
        Serialize to a canonical ``finstack_quant.instrument/1`` envelope.

        Returns
        -------
        str
            Canonical instrument envelope accepted by :func:`price_instrument`
            and :meth:`StructuredCredit.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def id(self) -> str:
        """
        Stable instrument identifier used in market lookup and results.

        Returns
        -------
        str
            The unique instrument identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def deal_type(self) -> str:
        """
        Deal classification (serde string).

        Returns
        -------
        str
            ``"abs"``, ``"clo"``, ``"cmbs"``, ``"rmbs"`` ...

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def pool(self) -> AssetPool:
        """
        Collateral pool.

        Returns
        -------
        AssetPool
            Independent copy of the pool.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def tranches(self) -> TrancheStructure:
        """
        Capital structure.

        Returns
        -------
        TrancheStructure
            Independent copy of the tranche structure.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def closing_date(self) -> datetime.date:
        """
        Deal closing (issuance) date.

        Returns
        -------
        datetime.date
            The closing date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def first_payment_date(self) -> datetime.date:
        """
        First tranche payment date.

        Returns
        -------
        datetime.date
            The first payment date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def reinvestment_end_date(self) -> datetime.date | None:
        """
        End of the reinvestment period, or ``None``.

        Returns
        -------
        datetime.date | None
            Date, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def maturity(self) -> datetime.date:
        """
        Legal final maturity date.

        Returns
        -------
        datetime.date
            The maturity date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def discount_curve_id(self) -> str:
        """
        Discount curve identifier.

        Returns
        -------
        str
            Curve id used for discounting.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return the full deal as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Same content as the ``instrument`` payload of ``to_json()``.

        Raises
        ------
        ValueError
            If canonical serialization fails.
        """
        ...

class StructuredCreditBuilder:
    """
    Fluent builder returned by :meth:`StructuredCredit.builder`.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import StructuredCredit
    >>> isinstance(StructuredCredit.builder(), StructuredCredit.builder().__class__)
    True
    """

    def id(self, value: str) -> StructuredCreditBuilder:
        """
        Set the instrument identifier.

        Parameters
        ----------
        value : str
            Unique identifier for the deal.

        Returns
        -------
        StructuredCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If this builder was already consumed by a prior call to
            :meth:`StructuredCreditBuilder.build`.
        """
        ...

    def deal_type(self, value: Literal["clo", "cbo", "abs", "rmbs", "cmbs", "auto", "card"]) -> StructuredCreditBuilder:
        """
        Set the deal-type classification.

        Parameters
        ----------
        value : {"clo", "cbo", "abs", "rmbs", "cmbs", "auto", "card"}
            Deal classification.

        Returns
        -------
        StructuredCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` is not a recognized deal type.
        """
        ...

    def pool(self, value: AssetPool) -> StructuredCreditBuilder:
        """
        Set the structured-credit asset pool backing the deal.

        Parameters
        ----------
        value : AssetPool
            Asset pool definition.

        Returns
        -------
        StructuredCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If this builder was already consumed by a prior call to
            :meth:`StructuredCreditBuilder.build`.
        """
        ...

    def tranches(self, value: TrancheStructure) -> StructuredCreditBuilder:
        """
        Set the tranche capital structure.

        Parameters
        ----------
        value : TrancheStructure
            Tranche capital structure.

        Returns
        -------
        StructuredCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If this builder was already consumed by a prior call to
            :meth:`StructuredCreditBuilder.build`.
        """
        ...

    def closing_date(self, value: datetime.date | datetime.datetime | pd.Timestamp | str) -> StructuredCreditBuilder:
        """
        Set the deal closing (issuance) date.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pandas.Timestamp | str
            Deal closing date.

        Returns
        -------
        StructuredCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If this builder was already consumed by a prior call to
            :meth:`StructuredCreditBuilder.build`.
        """
        ...

    def first_payment_date(
        self, value: datetime.date | datetime.datetime | pd.Timestamp | str
    ) -> StructuredCreditBuilder:
        """
        Set the first payment date to tranches.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pandas.Timestamp | str
            First payment date.

        Returns
        -------
        StructuredCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If this builder was already consumed by a prior call to
            :meth:`StructuredCreditBuilder.build`.
        """
        ...

    def reinvestment_end_date(
        self, value: datetime.date | datetime.datetime | pd.Timestamp | str
    ) -> StructuredCreditBuilder:
        """
        Set the end of the reinvestment period.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pandas.Timestamp | str
            End date of the reinvestment period. Optional; when never set,
            the deal has no reinvestment period.

        Returns
        -------
        StructuredCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If this builder was already consumed by a prior call to
            :meth:`StructuredCreditBuilder.build`.
        """
        ...

    def maturity(self, value: datetime.date | datetime.datetime | pd.Timestamp | str) -> StructuredCreditBuilder:
        """
        Set the legal final maturity date.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pandas.Timestamp | str
            Legal final maturity date.

        Returns
        -------
        StructuredCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If this builder was already consumed by a prior call to
            :meth:`StructuredCreditBuilder.build`.
        """
        ...

    def frequency(self, value: Tenor) -> StructuredCreditBuilder:
        """
        Set the payment frequency for the structure.

        Parameters
        ----------
        value : Tenor
            Payment frequency.

        Returns
        -------
        StructuredCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If this builder was already consumed by a prior call to
            :meth:`StructuredCreditBuilder.build`.
        """
        ...

    def payment_calendar_id(self, value: str) -> StructuredCreditBuilder:
        """
        Set the payment calendar identifier for schedule adjustments.

        Parameters
        ----------
        value : str
            Holiday calendar identifier (e.g. ``"nyse"``). Required for
            accurate schedule generation.

        Returns
        -------
        StructuredCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If this builder was already consumed by a prior call to
            :meth:`StructuredCreditBuilder.build`.
        """
        ...

    def payment_business_day_convention(self, value: str) -> StructuredCreditBuilder:
        """
        Set the business day convention for tranche payments.

        Parameters
        ----------
        value : str
            Business day convention (e.g. ``"following"``,
            ``"modified_following"``). Defaults to ``"following"`` when
            never set.

        Returns
        -------
        StructuredCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` is not a recognized business day convention.
        """
        ...

    def discount_curve_id(self, value: str) -> StructuredCreditBuilder:
        """
        Set the discount curve identifier for valuation.

        Parameters
        ----------
        value : str
            Discount curve identifier.

        Returns
        -------
        StructuredCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If this builder was already consumed by a prior call to
            :meth:`StructuredCreditBuilder.build`.
        """
        ...

    def market_conditions(self, value: dict[str, Any] | str) -> StructuredCreditBuilder:
        """
        Set market conditions from a JSON object.

        Parameters
        ----------
        value : dict[str, Any] | str
            JSON-encoded ``MarketConditions`` object (refinancing rate, home
            price appreciation, unemployment, seasonal factor, custom
            factors). :meth:`StructuredCredit.builder` pre-seeds the registry
            default, which this overrides.

        Returns
        -------
        StructuredCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` does not match the ``MarketConditions`` shape.
        """
        ...

    def credit_factors(self, value: dict[str, Any] | str) -> StructuredCreditBuilder:
        """
        Set credit factors from a JSON object.

        Parameters
        ----------
        value : dict[str, Any] | str
            JSON-encoded ``CreditFactors`` object (credit score, DTI, LTV,
            delinquency, unemployment, CMBS NOI/debt-service, custom
            factors). :meth:`StructuredCredit.builder` pre-seeds
            ``CreditFactors``'s default, which this overrides.

        Returns
        -------
        StructuredCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` does not match the ``CreditFactors`` shape.
        """
        ...

    def waterfall_rules(self, value: dict[str, Any] | str) -> StructuredCreditBuilder:
        """
        Set declarative waterfall rules from a JSON object.

        Parameters
        ----------
        value : dict[str, Any] | str
            JSON-encoded ``WaterfallRules`` object (available-funds caps,
            step-down, shifting interest, controlled accumulation), layered
            onto the base waterfall.

        Returns
        -------
        StructuredCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` does not match the ``WaterfallRules`` shape.
        """
        ...

    def fees(self, value: dict[str, Any] | str) -> StructuredCreditBuilder:
        """
        Set senior transaction fees from a JSON object.

        Parameters
        ----------
        value : dict[str, Any] | str
            JSON-encoded ``DealFees`` object (trustee, senior management,
            servicing, and optional master/special servicer fees), paid
            ahead of every note. Skipped (``None``) by default.

        Returns
        -------
        StructuredCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` does not match the ``DealFees`` shape.
        """
        ...

    def __repr__(self) -> str:
        """
        Return ``repr(self)``.

        Returns
        -------
        str
            ``StructuredCreditBuilder(consumed=...)``.
        """
        ...

    def build(self) -> StructuredCredit:
        """
        Build the validated structured-credit deal.

        Returns
        -------
        StructuredCredit
            The validated deal.

        Raises
        ------
        ValueError
            If a required field is missing or Rust validation fails.
        """
        ...

def bond_from_cashflows_json(
    instrument_id: str,
    schedule_json: str,
    discount_curve_id: str,
    quoted_clean: float | None = None,
) -> str:
    """
    Construct tagged bond instrument JSON from a cashflow schedule.

    Parameters
    ----------
    instrument_id : str
        Identifier for the bond instrument.
    schedule_json : str
        JSON-encoded ``CashFlowSchedule``.
    discount_curve_id : str
        Discount curve ID required for pricing.
    quoted_clean : float, optional
        Clean quoted price as a percent of par.

    Returns
    -------
    str
        Canonical ``finstack_quant.instrument/1`` envelope containing the bond.

    Raises
    ------
    ValueError
        If the schedule is invalid or bond construction fails.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import bond_from_cashflows_json
    >>> try:
    ...     bond_from_cashflows_json("B", "{}", "USD-OIS")
    ... except ValueError as exc:
    ...     print("flows" in str(exc))
    True

    """
    ...

def validate_instrument_json(json: str) -> str:
    """
    Validate a canonical instrument envelope and return canonical JSON.

    Parameters
    ----------
    json : str
        A ``finstack_quant.instrument/1`` envelope. Bare instrument payloads
        are rejected.

    Returns
    -------
    str
        Canonical pretty-printed instrument JSON after Rust serde validation.

    Raises
    ------
    ValueError
        If the JSON is malformed, has an unknown instrument tag, or
        fails instrument-specific validation.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.valuations.instruments import TermLoan, validate_instrument_json
    >>> validated = json.loads(validate_instrument_json(TermLoan.example().to_json()))
    >>> validated["instrument"]["type"]
    'term_loan'

    """
    ...

def validate_typed_instrument_json(type_tag: str, json: str) -> str:
    """
    Validate a payload as one exact instrument type and return the envelope.

    Parameters
    ----------
    type_tag : str
        Canonical instrument discriminator, such as ``"term_loan"`` or
        ``"fx_forward"``.
    json : str
        A ``finstack_quant.instrument/1`` envelope whose instrument type must
        match *type_tag*.

    Returns
    -------
    str
        Canonical instrument envelope for the validated instrument.

    Raises
    ------
    ValueError
        If ``json`` is malformed, carries a different instrument type, or
        fails instrument validation.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.valuations.instruments import TermLoan, validate_typed_instrument_json
    >>> envelope = TermLoan.example().to_json()
    >>> json.loads(validate_typed_instrument_json("term_loan", envelope))["instrument"]["type"]
    'term_loan'

    """
    ...

def pretty_instrument_json(json: str) -> str:
    """
    Re-render a canonical instrument envelope as pretty-printed JSON.

    Parameters
    ----------
    json : str
        A canonical ``finstack_quant.instrument/1`` envelope.

    Returns
    -------
    str
        The same envelope, pretty-printed.

    Raises
    ------
    ValueError
        If ``json`` is malformed or cannot be rendered.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import TermLoan, pretty_instrument_json
    >>> "term_loan" in pretty_instrument_json(TermLoan.example().to_json())
    True

    """
    ...

def price_instrument(
    instrument: str
    | Bond
    | TermLoan
    | InterestRateSwap
    | Swaption
    | CapFloor
    | CreditDefaultSwap
    | CDSIndex
    | FxForward
    | FxOption
    | CDSTranche
    | ConvertibleBond
    | EquityOption
    | StructuredCredit
    | CompositeInstrument,
    market: MarketContext | str,
    as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
    model: str = "default",
    metrics: list[str] | None = None,
    pricing_options: MetricPricingOverrides | dict[str, Any] | str | None = None,
    market_history: MarketHistory | dict[str, Any] | str | None = None,
) -> ValuationResult:
    """
    Price one instrument and compute explicit risk metric requests.

    Parameters
    ----------
    instrument : str or Bond or TermLoan or InterestRateSwap or Swaption or CapFloor or CreditDefaultSwap or CDSIndex or FxForward or FxOption or CDSTranche or ConvertibleBond or EquityOption or StructuredCredit or CompositeInstrument
        Typed instrument instance (:class:`Bond`, :class:`TermLoan`,
        :class:`InterestRateSwap`, :class:`Swaption`, :class:`CapFloor`,
        :class:`CreditDefaultSwap`, :class:`CDSIndex`, :class:`FxForward`,
        :class:`FxOption`, :class:`CDSTranche`, :class:`ConvertibleBond`,
        :class:`EquityOption`, :class:`StructuredCredit`,
        :class:`~finstack_quant.valuations.composite.CompositeInstrument`) or a
        canonical ``finstack_quant.instrument/1`` JSON envelope.
    market : MarketContext or str
        Typed ``MarketContext`` or serialized market-context JSON.
    as_of : datetime.date | datetime.datetime | pd.Timestamp | str
        Valuation date, either a date-like object or an ISO 8601 string.
    model : str, default "default"
        Model key: ``"default"`` (the instrument's registered default),
        ``"discounting"``, ``"black76"``, ``"hazard_rate"``,
        ``"hull_white_1f"``, ``"tree"``, ``"rates_credit"``, ``"normal"``,
        ... — see :func:`list_models_grouped`. For bonds, ``"discounting"`` is
        non-callable rates-only PV, ``"hazard_rate"`` is non-callable
        fractional recovery of par,
        ``"tree"`` values rates-only exercise rights, and ``"rates_credit"``
        values joint rates-credit bonds including call, put, and return floors.
    metrics : list[str] or None, default None
        Metric IDs to compute, such as ``"ytm"``, ``"dv01"``,
        ``"duration_mod"``, ``"z_spread"``, ``"pv01"``, ``"bucketed_dv01"``,
        ``"hvar"`` or ``"expected_shortfall"`` when supported by the
        instrument (see :func:`list_standard_metrics`). ``None`` or ``[]``
        means valuation only.
    pricing_options : MetricPricingOverrides or dict or str, optional
        Metric-time overrides merged into the instrument's own
        ``pricing_overrides`` before pricing: ``theta_period`` (``"1D"``,
        ``"1W"``, ``"1M"``), ``breakeven_config``
        (``{"target": "z_spread", "mode": "linear"}``), ``bump_config``,
        ``bond_risk_basis``, ``var_config``, ``quoted_price_pct``. A dict or
        JSON string is accepted in place of the typed object.
    market_history : MarketHistory or dict or str, optional
        Historical scenarios required by the ``"hvar"`` and
        ``"expected_shortfall"`` metrics; a dict or JSON string is accepted in
        place of the typed object.

    Returns
    -------
    ValuationResult
        Typed valuation envelope including the requested metric values. A
        stochastic ``"rates_credit"`` bond result also carries Monte Carlo
        convergence and reproducibility diagnostics in ``details``.

    Raises
    ------
    KeyError
        If a curve, surface, fixing series or scalar the instrument depends
        on is missing from ``market``.
    ValueError
        If the instrument, market, date or option payloads are malformed, a
        metric is unknown or not applicable to the instrument, or the
        instrument fails validation for the requested model (for example a
        seasoned floating leg without a ``FIXING:<index>`` series).
    RuntimeError
        If the model or a metric solver fails numerically (calibration or
        convergence failure).
    TypeError
        If ``instrument`` is neither a typed instrument nor a string, or
        ``market`` is neither a ``MarketContext`` nor a string.

    Notes
    -----
    When stochastic rate or credit factors are configured for a
    ``"rates_credit"`` bond, ``result.details`` is tagged
    ``{"type": "monte_carlo", "data": ...}``. Its data contains the
    sampling-only standard error, configured independent exercise-policy paths
    (``training_paths``), configured independent make-whole-reference paths
    (``make_whole_training_paths``), the corresponding simulated-path counts
    including antithetic partners, independent estimator paths and their total
    simulated count, random seed, simulation time grid, and variance-reduction
    flags. The standard error measures pricing-path sampling
    uncertainty under the frozen fitted exercise policy and excludes regression
    approximation, time-grid discretization, and model error.

    The wire payload is still one call away: ``result.to_json()`` returns the
    JSON that :meth:`ValuationResult.from_json` accepts, for pipelines that
    serialize results.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.market_data import DiscountCurve, MarketContext
    >>> from finstack_quant.valuations.instruments import TermLoan
    >>> loan = TermLoan.example()
    >>> market = MarketContext().insert(DiscountCurve.flat("USD-OIS", datetime.date(2024, 1, 1), 0.04))
    >>> from finstack_quant.valuations.instruments import price_instrument
    >>> result = price_instrument(loan, market, "2024-01-01", metrics=["all_in_rate"])
    >>> (result.metric_keys(), round(result.get_metric("all_in_rate"), 4))
    (['all_in_rate'], 0.06)

    """
    ...

def instrument_cashflows_json(
    instrument: str
    | Bond
    | TermLoan
    | InterestRateSwap
    | Swaption
    | CapFloor
    | CreditDefaultSwap
    | CDSIndex
    | FxForward
    | FxOption
    | CDSTranche
    | ConvertibleBond
    | EquityOption
    | StructuredCredit
    | CompositeInstrument,
    market: MarketContext | str,
    as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
    model: str,
) -> str:
    """
    Per-flow cashflow envelope for a discountable instrument.

    Hazard-rate export rejects bonds with call, put, or return-floor rights
    because static rows cannot represent their exercise-contingent value.

    Parameters
    ----------
    instrument : str or Bond or TermLoan or InterestRateSwap or Swaption or CapFloor or CreditDefaultSwap or CDSIndex or FxForward or FxOption or CDSTranche or ConvertibleBond or EquityOption or StructuredCredit or CompositeInstrument
        Typed instrument instance or a canonical
        ``finstack_quant.instrument/1`` JSON envelope.
    market : MarketContext or str
        Typed ``MarketContext`` or serialized market-context JSON.
    as_of : datetime.date | datetime.datetime | pd.Timestamp | str
        Valuation date, either a date-like object or an ISO 8601 string.
    model : str
        Must be ``"discounting"`` or ``"hazard_rate"``. ``"default"`` is not
        accepted on cashflow export.

    Returns
    -------
    str
        JSON-serialized ``InstrumentCashflowEnvelope``.

    Raises
    ------
    KeyError
        If a curve or fixing series the instrument depends on is missing
        from ``market``.
    ValueError
        If ``model`` is unsupported, the instrument/model pair is not
        registered for cashflow export, a bond with embedded exercise rights
        is requested under a static cashflow model, or a payload is malformed.
    RuntimeError
        If the pricer fails numerically.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.dates import StubKind
    >>> from finstack_quant.core.market_data import DiscountCurve, MarketContext
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.core.types import Rate
    >>> from finstack_quant.valuations.instruments import Bond
    >>> as_of = datetime.date(2024, 1, 1)
    >>> bond = Bond.fixed(
    ...     "B", Money(1000.0, Currency("USD")), Rate(0.05), as_of, datetime.date(2026, 1, 1), StubKind.NONE, "USD-OIS"
    ... )
    >>> market = MarketContext().insert(DiscountCurve.flat("USD-OIS", as_of, 0.04))
    >>> import json
    >>> from finstack_quant.valuations.instruments import instrument_cashflows_json
    >>> payload = json.loads(instrument_cashflows_json(bond, market, "2024-01-01", "discounting"))
    >>> (payload["instrument_id"], len(payload["flows"]))
    ('B', 6)

    """
    ...

def list_models() -> list[str]:
    """
    Return every pricing model key registered in the standard pricer registry.

    The list is registry-derived rather than enum-derived, so it reflects real
    dispatch coverage: a model with no registered pricer is omitted. The names
    are the canonical keys accepted by the ``model`` argument of
    :func:`price_instrument`.

    Returns
    -------
    list[str]
        Canonical model keys such as ``"discounting"`` or ``"rates_credit"``,
        deduplicated and sorted.

    Notes
    -----
    This method does not raise; it returns the stored or derived value.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import list_models
    >>> models = list_models()
    >>> all(model in models for model in ("discounting", "hazard_rate", "tree", "rates_credit"))
    True
    """
    ...

def list_models_grouped() -> dict[str, list[str]]:
    """
    Return the standard registry's pricing models grouped by instrument type.

    Only instrument types with at least one registered pricer appear as keys,
    and each entry lists only the models that can price that instrument.

    Returns
    -------
    dict[str, list[str]]
        Mapping from canonical instrument-type name to its sorted model keys.

    Notes
    -----
    This method does not raise; it returns the stored or derived value.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import list_models_grouped
    >>> grouped = list_models_grouped()
    >>> set(("discounting", "hazard_rate", "tree", "rates_credit")).issubset(grouped["bond"])
    True
    """
    ...

def list_standard_metrics() -> list[str]:
    """
    Return all standard metric IDs registered by the Rust valuation engine.

    Returns
    -------
    list[str]
        Sorted list of fully qualified metric keys.

    Notes
    -----
    This method does not raise; it returns the stored or derived value.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import list_standard_metrics
    >>> metrics = list_standard_metrics()
    >>> (len(metrics), "dirty_price" in metrics, "dv01" in metrics)
    (220, True, True)
    """
    ...

def list_standard_metrics_grouped() -> dict[str, list[str]]:
    """
    Return standard metric IDs grouped by human-readable category.

    Returns
    -------
    dict[str, list[str]]
        Mapping from group label to sorted metric ID lists.

    Notes
    -----
    This method does not raise; it returns the stored or derived value.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import list_standard_metrics_grouped
    >>> grouped = list_standard_metrics_grouped()
    >>> ("Credit" in grouped, "Rates" in grouped)
    (True, True)
    """
    ...

class OasResult:
    """
    Result of an option-adjusted-spread calculation for a structured-credit
    tranche (``structured_credit_tranche_oas``'s return value).

    Examples
    --------
    >>> import json
    >>> from finstack_quant.valuations.instruments import OasResult
    >>> payload = json.dumps({
    ...     "oas": 0.0125,
    ...     "model_price": 99.5,
    ...     "market_price": 98.75,
    ...     "num_paths": 256,
    ...     "price_std_error": 0.05,
    ... })
    >>> result = OasResult.from_json(payload)
    >>> result.oas
    0.0125
    """

    @staticmethod
    def from_json(json: str) -> OasResult:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``OasResult``.

        Returns
        -------
        OasResult
            The decoded result.

        Raises
        ------
        ValueError
            If ``json`` is not valid JSON for the ``OasResult`` shape.

        Examples
        --------
        >>> import json
        >>> from finstack_quant.valuations.instruments import OasResult
        >>> result = OasResult.from_json(
        ...     json.dumps({
        ...         "oas": 0.0125,
        ...         "model_price": 99.5,
        ...         "market_price": 98.75,
        ...         "num_paths": 256,
        ...         "price_std_error": 0.05,
        ...     })
        ... )
        >>> (result.oas, result.model_price, result.num_paths)
        (0.0125, 99.5, 256)

        """
        ...

    def to_json(self) -> str:
        """
        Serialize back to the same JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            JSON-encoded ``OasResult``.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def oas(self) -> float:
        """
        Option-adjusted spread, as an annual decimal (``0.01`` = 100 bp).

        Returns
        -------
        float
            The option-adjusted spread.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def model_price(self) -> float:
        """
        Model price at the solved OAS, as a percentage of original balance.

        Returns
        -------
        float
            The model price.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def market_price(self) -> float:
        """
        Target market price, as a percentage of original balance.

        Returns
        -------
        float
            The target market price.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def num_paths(self) -> int:
        """
        Number of Monte-Carlo scenarios used.

        Returns
        -------
        int
            The scenario count.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def price_std_error(self) -> float:
        """
        Monte-Carlo standard error of the mean price, as a percentage of
        original balance.

        Returns
        -------
        float
            The standard error.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export as a single-row pandas DataFrame.

        Columns: ``oas`` (annual decimal), ``model_price`` and
        ``market_price`` (percentage of original balance), ``num_paths``,
        ``price_std_error`` (percentage of original balance).

        Returns
        -------
        pd.DataFrame
            Single-row DataFrame of the OAS solve, so a book of tranches
            stacks with ``pd.concat``.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

class TrancheMetrics:
    """
    Summary risk/pricing metrics for a structured-credit tranche
    (``structured_credit_tranche_metrics``'s return value).

    Examples
    --------
    >>> import json
    >>> from finstack_quant.valuations.instruments import TrancheMetrics
    >>> payload = json.dumps({
    ...     "tranche_id": "A",
    ...     "currency": "USD",
    ...     "pv": 1000.0,
    ...     "price_pct": 100.0,
    ...     "wal": 3.0,
    ...     "z_spread_bp": 0.0,
    ...     "cs01": -1.0,
    ...     "spread_duration": 3.0,
    ...     "modified_duration": 3.0,
    ...     "convexity": 12.0,
    ...     "target_price_pct": 100.0,
    ... })
    >>> metrics = TrancheMetrics.from_json(payload)
    >>> metrics.tranche_id
    'A'
    """

    @staticmethod
    def from_json(json: str) -> TrancheMetrics:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``TrancheMetrics``.

        Returns
        -------
        TrancheMetrics
            The decoded metrics bundle.

        Raises
        ------
        ValueError
            If ``json`` is not valid JSON for the ``TrancheMetrics`` shape.

        Examples
        --------
        >>> import json
        >>> from finstack_quant.valuations.instruments import TrancheMetrics
        >>> payload = {
        ...     "tranche_id": "A",
        ...     "currency": "USD",
        ...     "pv": 1000.0,
        ...     "price_pct": 100.0,
        ...     "wal": 3.0,
        ...     "z_spread_bp": 0.0,
        ...     "cs01": -1.0,
        ...     "spread_duration": 3.0,
        ...     "modified_duration": 3.0,
        ...     "convexity": 12.0,
        ...     "target_price_pct": 100.0,
        ... }
        >>> metrics = TrancheMetrics.from_json(json.dumps(payload))
        >>> (metrics.tranche_id, metrics.currency, metrics.pv)
        ('A', 'USD', 1000.0)

        """
        ...

    def to_json(self) -> str:
        """
        Serialize back to the same JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            JSON-encoded ``TrancheMetrics``.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def tranche_id(self) -> str:
        """
        Identifier of the tranche.

        Returns
        -------
        str
            The tranche identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def currency(self) -> str:
        """
        ISO-4217 code of the currency ``pv`` and ``cs01`` are denominated in.
        Empty when decoded from a legacy payload that predates this field.

        Returns
        -------
        str
            The ISO-4217 currency code, or an empty string for legacy payloads.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def pv(self) -> float:
        """
        Present value of the tranche, in ``currency`` units.

        Returns
        -------
        float
            The present value.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def price_pct(self) -> float:
        """
        Model price, as a percentage of original balance.

        Returns
        -------
        float
            The model price.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def wal(self) -> float:
        """
        Weighted-average life, in years.

        Returns
        -------
        float
            The weighted-average life.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def z_spread_bp(self) -> float:
        """
        Z-spread to ``target_price_pct``, in basis points.

        Returns
        -------
        float
            The z-spread in basis points.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def cs01(self) -> float:
        """
        Credit-spread DV01 -- currency change for a +1 bp z-spread shock, in
        ``currency`` units. Negative for a long tranche.

        Returns
        -------
        float
            The credit-spread DV01.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def spread_duration(self) -> float:
        """
        Spread duration, in years (``-cs01 / (pv * 1bp)``).

        Returns
        -------
        float
            The spread duration.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def modified_duration(self) -> float:
        """
        Modified (rate) duration of the projected cashflows, in years.

        Returns
        -------
        float
            The modified duration.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def convexity(self) -> float:
        """
        Modified convexity of the projected cashflows, in years squared.

        Returns
        -------
        float
            The modified convexity.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def target_price_pct(self) -> float:
        """
        Price the z-spread/CS01 were solved against, as a percentage of
        original balance.

        Returns
        -------
        float
            The target price.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export as a single-row pandas DataFrame.

        Columns: ``tranche_id``, ``currency``, ``pv``, ``price_pct``, ``wal``,
        ``z_spread_bp``, ``cs01``, ``spread_duration``, ``modified_duration``,
        ``convexity``, ``target_price_pct`` -- the same fields and units as
        the properties of the same name.

        Returns
        -------
        pd.DataFrame
            Single-row DataFrame, so a capital structure stacks with
            ``pd.concat``. ``pv`` and ``cs01`` are in ``currency`` units and
            are only additive across tranches sharing one currency.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

class ScenarioTable:
    """
    Scenario/yield table for a single structured-credit tranche
    (``structured_credit_tranche_scenario_table``'s return value).

    Examples
    --------
    >>> import json
    >>> from finstack_quant.valuations.instruments import ScenarioTable
    >>> payload = json.dumps({
    ...     "tranche_id": "A",
    ...     "cells": [{"cpr": 0.06, "cdr": 0.02, "severity": 0.6, "price": 98.2, "wal": 4.1, "writedown": 0.0}],
    ... })
    >>> table = ScenarioTable.from_json(payload)
    >>> table.tranche_id
    'A'
    """

    @staticmethod
    def from_json(json: str) -> ScenarioTable:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``ScenarioTable``.

        Returns
        -------
        ScenarioTable
            The decoded scenario table.

        Raises
        ------
        ValueError
            If ``json`` is not valid JSON for the ``ScenarioTable`` shape.

        Examples
        --------
        >>> import json
        >>> from finstack_quant.valuations.instruments import ScenarioTable
        >>> payload = {
        ...     "tranche_id": "A",
        ...     "cells": [{"cpr": 0.06, "cdr": 0.02, "severity": 0.6, "price": 98.2, "wal": 4.1, "writedown": 0.0}],
        ... }
        >>> table = ScenarioTable.from_json(json.dumps(payload))
        >>> (table.tranche_id, table.cells()[0]["price"])
        ('A', 98.2)

        """
        ...

    def to_json(self) -> str:
        """
        Serialize back to the same JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            JSON-encoded ``ScenarioTable``.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def tranche_id(self) -> str:
        """
        Identifier of the tranche evaluated.

        Returns
        -------
        str
            The tranche identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def cells(self) -> list[dict[str, float]]:
        """
        Evaluated cells, in CPR-major, then CDR, then severity order.

        Each cell is a dict with keys ``cpr`` (annual decimal), ``cdr``
        (annual decimal), ``severity`` (decimal), ``price`` (percentage of
        original balance), ``wal`` (years), and ``writedown`` (currency
        units).

        Returns
        -------
        list[dict[str, float]]
            One dict per evaluated scenario cell.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the evaluated cells as a pandas DataFrame.

        Columns: ``tranche_id``, ``cpr``, ``cdr``, ``severity``, ``price``
        (percentage of original balance), ``wal`` (years), ``writedown``
        (currency units). One row per cell, in CPR-major then CDR then
        severity order -- the same cells and order as ``cells``.

        Returns
        -------
        pd.DataFrame
            One row per scenario cell. A grid that evaluated no cells yields a
            zero-row frame that still carries the columns above.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

def structured_credit_tranche_discount_margin(
    instrument: StructuredCredit | str,
    tranche_id: str,
    market: MarketContext | str,
    as_of: datetime.date | str,
    target_pv: float,
) -> float:
    """Solve a z-spread-equivalent discount margin for a floating-rate tranche.

    Contractual cashflows are projected without changing coupon projection,
    then a constant additive spread is applied to the discount curve. The
    result is zero at model PV, negative for a richer (higher) target PV, and
    positive for a cheaper (lower) target PV; it is not the contractual quoted
    margin.

    Parameters
    ----------
    instrument : StructuredCredit or str
        Tagged JSON for a ``StructuredCredit`` deal, or a typed
        ``StructuredCredit`` instance.
    tranche_id : str
        Identifier of the floating-rate tranche whose contractual cashflows
        are spread-discounted.
    market : MarketContext or str
        Typed ``MarketContext`` or serialized market-context JSON supplying
        the discount curve and any forward curves or historical fixings
        required for cashflow projection.
    as_of : datetime.date | str
        Valuation date used for projection and discounting, either a date-like
        object or an ISO 8601 string.
    target_pv : float
        Target present value in the tranche's currency. Values above model PV
        produce a negative result; values below model PV produce a positive
        result.

    Returns
    -------
    float
        Z-spread-equivalent discount margin in decimal (``0.015`` = 150 bp).

    Raises
    ------
    KeyError
        If ``tranche_id`` is not part of the deal.
    ValueError
        If the JSON or date is malformed, the deal fails validation, the
        tranche is missing or fixed-rate, ``target_pv`` is not finite, required
        market data is unavailable, or the spread solve fails or exceeds
        ±5000 bp.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import structured_credit_tranche_discount_margin
    >>> try:
    ...     structured_credit_tranche_discount_margin("{}", "A", "{}", "2026-01-01", 100.0)
    ... except ValueError as exc:
    ...     print("schema" in str(exc))
    True

    """
    ...

def structured_credit_tranche_breakeven_cdr(
    instrument: StructuredCredit | str,
    tranche_id: str,
    market: MarketContext | str,
    as_of: datetime.date | str,
) -> float:
    """Solve the constant default rate at which a tranche first takes a writedown.

    Parameters
    ----------
    instrument : StructuredCredit or str
        Tagged JSON for a ``StructuredCredit`` deal, or a typed
        ``StructuredCredit`` instance.
    tranche_id : str
        Identifier of the tranche within the deal.
    market : MarketContext or str
        Typed ``MarketContext`` or serialized market-context JSON supplying
        curves and fixings.
    as_of : datetime.date | str
        Valuation date, either a date-like object or an ISO 8601 string.

    Returns
    -------
    float
        Break-even annual CDR in decimal.

    Raises
    ------
    KeyError
        If ``tranche_id`` is not part of the deal.
    ValueError
        If the instrument JSON is malformed, the deal fails validation, the
        tranche id is not part of the deal, or required market data is missing.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import structured_credit_tranche_breakeven_cdr
    >>> try:
    ...     structured_credit_tranche_breakeven_cdr("{}", "A", "{}", "2026-01-01")
    ... except ValueError as exc:
    ...     print("schema" in str(exc))
    True

    """
    ...

def structured_credit_tranche_oas(
    instrument: StructuredCredit | str,
    tranche_id: str,
    market_price_pct: float,
    market: MarketContext | str,
    as_of: datetime.date | str,
    config: dict[str, Any] | str | None = None,
) -> OasResult:
    """Compute option-adjusted spread for a tranche. Returns an ``OasResult``.

    Parameters
    ----------
    instrument : StructuredCredit or str
        Tagged JSON for a ``StructuredCredit`` deal, or a typed
        ``StructuredCredit`` instance.
    tranche_id : str
        Identifier of the tranche within the deal.
    market_price_pct : float
        Market price as a percentage of original balance (100.0 = par).
    market : MarketContext or str
        Typed ``MarketContext`` or serialized market-context JSON supplying
        curves and fixings.
    as_of : datetime.date | str
        Valuation date, either a date-like object or an ISO 8601 string.
    config : dict or str or None, optional
        Serialized ``OasConfig``. All fields are required when supplied.

    Returns
    -------
    OasResult
        Typed OAS result. Call :meth:`OasResult.to_json` on it for the wire
        payload.

    Raises
    ------
    KeyError
        If ``tranche_id`` is not part of the deal.
    ValueError
        If the instrument JSON is malformed, the deal fails validation, the
        tranche id is not part of the deal, or required market data is missing.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import structured_credit_tranche_oas
    >>> try:
    ...     structured_credit_tranche_oas("{}", "A", 100.0, "{}", "2026-01-01")
    ... except ValueError as exc:
    ...     print("schema" in str(exc))
    True

    """
    ...

def structured_credit_tranche_metrics(
    instrument: StructuredCredit | str,
    tranche_id: str,
    market: MarketContext | str,
    as_of: datetime.date | str,
    market_price_pct: float | None = None,
) -> TrancheMetrics:
    """Summary risk/pricing metrics for a tranche. Returns a ``TrancheMetrics``.

    Parameters
    ----------
    instrument : StructuredCredit or str
        Tagged JSON for a ``StructuredCredit`` deal, or a typed
        ``StructuredCredit`` instance.
    tranche_id : str
        Identifier of the tranche within the deal.
    market : MarketContext or str
        Typed ``MarketContext`` or serialized market-context JSON supplying
        curves and fixings.
    as_of : datetime.date | str
        Valuation date, either a date-like object or an ISO 8601 string.
    market_price_pct : float or None, optional
        Market price as a percentage of original balance; the model price is
        used when omitted.

    Returns
    -------
    TrancheMetrics
        Typed metrics bundle. Call :meth:`TrancheMetrics.to_json` on it for
        the wire payload.

    Raises
    ------
    KeyError
        If ``tranche_id`` is not part of the deal.
    ValueError
        If the instrument JSON is malformed, the deal fails validation, the
        tranche id is not part of the deal, or required market data is missing.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import structured_credit_tranche_metrics
    >>> try:
    ...     structured_credit_tranche_metrics("{}", "A", "{}", "2026-01-01")
    ... except ValueError as exc:
    ...     print("schema" in str(exc))
    True

    """
    ...

def structured_credit_tranche_scenario_table(
    instrument: StructuredCredit | str,
    tranche_id: str,
    market: MarketContext | str,
    as_of: datetime.date | str,
    grid: dict[str, Any] | str,
) -> ScenarioTable:
    """Price a tranche across a CPR x CDR x severity grid. Returns a ``ScenarioTable``.

    Parameters
    ----------
    instrument : StructuredCredit or str
        Tagged JSON for a ``StructuredCredit`` deal, or a typed
        ``StructuredCredit`` instance.
    tranche_id : str
        Identifier of the tranche within the deal.
    market : MarketContext or str
        Typed ``MarketContext`` or serialized market-context JSON supplying
        curves and fixings.
    as_of : datetime.date | str
        Valuation date, either a date-like object or an ISO 8601 string.
    grid : dict or str
        Serialized ``ScenarioGrid``. Capped at 10,000 cells because each cell
        reprices the entire deal.

    Returns
    -------
    ScenarioTable
        Typed scenario table. Call :meth:`ScenarioTable.to_json` on it for the
        wire payload.

    Raises
    ------
    KeyError
        If ``tranche_id`` is not part of the deal.
    ValueError
        If the instrument JSON is malformed, the deal fails validation, the
        tranche id is not part of the deal, or required market data is missing.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import structured_credit_tranche_scenario_table
    >>> try:
    ...     structured_credit_tranche_scenario_table("{}", "A", "{}", "2026-01-01", "{}")
    ... except ValueError as exc:
    ...     print("schema" in str(exc))
    True

    """
    ...

class CDSIndexParams:
    """
    Preset descriptor for a standardized CDS index (typed wrapper for the Rust
    ``CDSIndexParams``): index identity (name, series, version), fixed running
    coupon and regional convention. Trade state lives on the :class:`CDSIndex`
    built with :meth:`CDSIndex.from_preset`. Instances compare by value.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import CDSIndexParams
    >>> preset = CDSIndexParams.cdx_na_ig(42, 1, 100.0)
    >>> (preset.index_name, preset.convention, preset.num_constituents)
    ('CDX.NA.IG', 'isda_na', 125)
    """

    def __init__(
        self,
        index_name: str,
        series: int,
        version: int,
        fixed_coupon_bp: float | Bps,
        convention: Literal["isda_na", "isda_eu", "isda_as", "custom"] = "isda_na",
        num_constituents: int | None = None,
    ) -> None:
        """
        Describe a standardized CDS index.

        Parameters
        ----------
        index_name : str
            Index name, e.g. ``"CDX.NA.IG"`` or ``"iTraxx Europe"``.
        series : int
            Series number (e.g. ``42``).
        version : int
            Version within the series (e.g. ``1``).
        fixed_coupon_bp : float | Bps
            Fixed running coupon in basis points (``100.0`` = 1%).
        convention : {"isda_na", "isda_eu", "isda_as", "custom"}, default "isda_na"
            Regional ISDA convention (``"isda_na"`` is the SNAC standard).
        num_constituents : int | None
            Number of names in the pool, used by portfolio analytics when the
            constituent list is empty.

        Raises
        ------
        ValueError
            If ``convention`` is not an accepted string.
        TypeError
            If ``fixed_coupon_bp`` is neither a number nor ``Bps``.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CDSIndexParams
        >>> CDSIndexParams("CDX.NA.HY", 42, 1, 500.0).fixed_coupon_bp
        500.0
        """
        ...
    @staticmethod
    def cdx_na_ig(series: int, version: int, fixed_coupon_bp: float | Bps) -> CDSIndexParams:
        """
        CDX North American Investment Grade preset (125 names, ``isda_na``).

        Parameters
        ----------
        series : int
            Series number.
        version : int
            Version within the series.
        fixed_coupon_bp : float | Bps
            Fixed running coupon in basis points (``100.0`` for CDX.NA.IG).

        Returns
        -------
        CDSIndexParams
            The preset.

        Raises
        ------
        TypeError
            If ``fixed_coupon_bp`` is neither a number nor ``Bps``.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CDSIndexParams
        >>> CDSIndexParams.cdx_na_ig(42, 1, 100.0).num_constituents
        125
        """
        ...
    @staticmethod
    def cdx_na_hy(series: int, version: int, fixed_coupon_bp: float | Bps) -> CDSIndexParams:
        """
        CDX North American High Yield preset (100 names, ``isda_na``).

        Parameters
        ----------
        series : int
            Series number.
        version : int
            Version within the series.
        fixed_coupon_bp : float | Bps
            Fixed running coupon in basis points (``500.0`` for CDX.NA.HY).

        Returns
        -------
        CDSIndexParams
            The preset.

        Raises
        ------
        TypeError
            If ``fixed_coupon_bp`` is neither a number nor ``Bps``.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CDSIndexParams
        >>> CDSIndexParams.cdx_na_hy(42, 1, 500.0).num_constituents
        100
        """
        ...
    @staticmethod
    def itraxx_europe(series: int, version: int, fixed_coupon_bp: float | Bps) -> CDSIndexParams:
        """
        iTraxx Europe Main preset (125 names, ``isda_eu``).

        Parameters
        ----------
        series : int
            Series number.
        version : int
            Version within the series.
        fixed_coupon_bp : float | Bps
            Fixed running coupon in basis points (``100.0`` for iTraxx Europe).

        Returns
        -------
        CDSIndexParams
            The preset.

        Raises
        ------
        TypeError
            If ``fixed_coupon_bp`` is neither a number nor ``Bps``.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CDSIndexParams
        >>> CDSIndexParams.itraxx_europe(41, 1, 100.0).convention
        'isda_eu'
        """
        ...
    @property
    def index_name(self) -> str:
        """
        Ticker of the credit index family this contract references.

        Returns
        -------
        str
            Index family ticker as supplied at construction, for example
            ``"CDX.NA.IG"`` or ``"iTraxx Europe"``. The value is stored
            verbatim and is not normalised or validated against a registry.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def series(self) -> int:
        """
        Roll series of the credit index, incremented each semi-annual roll.

        Returns
        -------
        int
            Series number as an unsigned integer (for example ``41`` for
            CDX.NA.IG series 41). Higher numbers denote more recent
            on-the-run rolls.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def version(self) -> int:
        """
        Version within the series.

        Returns
        -------
        int
            The version number.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def fixed_coupon_bp(self) -> float:
        """
        Fixed running coupon.

        Returns
        -------
        float
            Coupon in basis points.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def convention(self) -> str:
        """
        Regional ISDA convention (serde name).

        Returns
        -------
        str
            ``"isda_na"``, ``"isda_eu"``, ``"isda_as"`` or ``"custom"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def num_constituents(self) -> int | None:
        """
        Number of names in the pool.

        Returns
        -------
        int | None
            The count, or ``None`` when unknown.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    def __eq__(self, other: object) -> bool:
        """
        Value equality (mirrors Rust ``PartialEq``).

        Parameters
        ----------
        other : object
            Value to compare with.

        Returns
        -------
        bool
            ``True`` when every field matches.

        Notes
        -----
        This method does not raise; unrelated types compare unequal.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style rendering of the key economics, e.g. ``CDSIndexParams(index_name='CDX.NA.IG', series=42, version=1, fixed_coupon_bp=100.0, convention='isda_na', num_constituents=125)``.

        Returns
        -------
        str
            ``CDSIndexParams(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise; it renders stored values.
        """
        ...

class CDSIndexConstituent:
    """
    One reference entity in a CDS index (typed wrapper for the Rust
    ``CDSIndexConstituent``): issuer credit parameters, index weight and
    default flag. Accepted by :meth:`CDSIndexBuilder.constituents` alongside
    dicts / JSON of the same shape; picklable.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import CDSIndexConstituent
    >>> row = CDSIndexConstituent("ACME-CORP", 0.4, "ACME-HZD", 1 / 125)
    >>> (row.reference_entity, row.defaulted)
    ('ACME-CORP', False)
    """

    def __init__(
        self,
        reference_entity: str,
        recovery_rate: float,
        credit_curve_id: str,
        weight: float,
        defaulted: bool = False,
    ) -> None:
        """
        Describe one index constituent.

        Parameters
        ----------
        reference_entity : str
            Issuer / reference-entity name.
        recovery_rate : float
            Assumed recovery as a fraction (``0.4`` = 40%).
        credit_curve_id : str
            Hazard curve identifier for the issuer.
        weight : float
            Weight of the issuer in the index notional (``1/125`` for CDX IG).
        defaulted : bool, default False
            Whether the name has defaulted; defaulted names drop out of the
            premium leg (their settlement is reflected in ``index_factor``).

        Notes
        -----
        This constructor does not raise; validation happens when the index is priced.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CDSIndexConstituent
        >>> CDSIndexConstituent("ACME-CORP", 0.4, "ACME-HZD", 0.008).weight
        0.008
        """
        ...
    @classmethod
    def from_json(cls, json: str) -> CDSIndexConstituent:
        """
        Deserialize from the canonical JSON shape.

        Parameters
        ----------
        json : str
            JSON object with ``credit`` (``reference_entity``, ``recovery_rate``,
            ``credit_curve_id``), ``weight`` and optional ``defaulted``.

        Returns
        -------
        CDSIndexConstituent
            The parsed constituent.

        Raises
        ------
        ValueError
            If ``json`` is malformed or has unknown fields.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CDSIndexConstituent
        >>> row = CDSIndexConstituent("ACME-CORP", 0.4, "ACME-HZD", 0.008)
        >>> CDSIndexConstituent.from_json(row.to_json()).credit_curve_id
        'ACME-HZD'
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to the canonical JSON shape.

        Returns
        -------
        str
            JSON accepted by :meth:`from_json` and :meth:`CDSIndexBuilder.constituents`.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...
    @property
    def reference_entity(self) -> str:
        """
        Issuer / reference-entity name.

        Returns
        -------
        str
            The issuer name.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def recovery_rate(self) -> float:
        """
        Assumed recovery as a fraction.

        Returns
        -------
        float
            Recovery in ``[0, 1]``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def credit_curve_id(self) -> str:
        """
        Hazard curve identifier for the issuer.

        Returns
        -------
        str
            The curve id.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def weight(self) -> float:
        """
        Weight of the issuer in the index notional.

        Returns
        -------
        float
            The weight.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def defaulted(self) -> bool:
        """
        Whether the name has defaulted.

        Returns
        -------
        bool
            ``True`` for defaulted names.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style rendering of the key economics, e.g. ``CDSIndexConstituent(reference_entity='ACME-CORP', recovery_rate=0.4, credit_curve_id='ACME-HZD', weight=0.008, defaulted=False)``.

        Returns
        -------
        str
            ``CDSIndexConstituent(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise; it renders stored values.
        """
        ...

class CDSTrancheParams:
    """
    Economic terms of an index tranche (typed wrapper for the Rust
    ``CDSTrancheParams``). Attachment and detachment are quoted in percent
    points (``3.0`` = 3%), the running coupon in basis points. Pass to
    :meth:`CDSTranche.standard` for a tranche on the standard quarterly
    ACT/360 schedule.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.valuations.instruments import CDSTrancheParams
    >>> params = CDSTrancheParams.mezzanine_tranche(
    ...     "CDX.NA.IG", 42, Money(10_000_000.0, Currency("USD")), datetime.date(2029, 12, 20), 100.0
    ... )
    >>> (params.attach_pct, params.detach_pct)
    (3.0, 7.0)
    """

    def __init__(
        self,
        index_name: str,
        series: int,
        attach_pct: float,
        detach_pct: float,
        notional: Money,
        maturity: datetime.date | datetime.datetime | pd.Timestamp | str,
        running_coupon_bp: float | Bps,
        accumulated_loss: float = 0.0,
    ) -> None:
        """
        Describe a tranche on a credit index.

        Parameters
        ----------
        index_name : str
            Underlying index name, e.g. ``"CDX.NA.IG"``.
        series : int
            Index series number.
        attach_pct : float
            Attachment point in percent (``3.0`` = 3%).
        detach_pct : float
            Detachment point in percent (``7.0`` = 7%); must exceed ``attach_pct``.
        notional : Money
            Tranche notional.
        maturity : datetime.date | datetime.datetime | pd.Timestamp | str
            Scheduled maturity (an IMM date for standard tranches).
        running_coupon_bp : float | Bps
            Running coupon in basis points (``100.0`` = 1%).
        accumulated_loss : float, default 0.0
            Realized portfolio loss so far as a fraction of the original
            portfolio notional, in ``[0, 1]``.

        Raises
        ------
        ValueError
            If ``accumulated_loss`` is outside ``[0, 1]`` or a date cannot be interpreted.
        TypeError
            If ``running_coupon_bp`` is neither a number nor ``Bps``.

        Examples
        --------
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.instruments import CDSTrancheParams
        >>> CDSTrancheParams(
        ...     "CDX.NA.IG", 42, 7.0, 15.0, Money(5_000_000.0, Currency("USD")), "2029-12-20", 100.0
        ... ).running_coupon_bp
        100.0
        """
        ...
    @staticmethod
    def equity_tranche(
        index_name: str,
        series: int,
        notional: Money,
        maturity: datetime.date | datetime.datetime | pd.Timestamp | str,
        running_coupon_bp: float | Bps,
    ) -> CDSTrancheParams:
        """
        Standard equity tranche (0%–3%).

        Parameters
        ----------
        index_name : str
            Underlying index name.
        series : int
            Index series number.
        notional : Money
            Tranche notional.
        maturity : datetime.date | datetime.datetime | pd.Timestamp | str
            Scheduled maturity.
        running_coupon_bp : float | Bps
            Running coupon in basis points.

        Returns
        -------
        CDSTrancheParams
            Tranche terms with ``attach_pct=0.0`` and ``detach_pct=3.0``.

        Raises
        ------
        ValueError
            If ``maturity`` cannot be interpreted.
        TypeError
            If ``running_coupon_bp`` is neither a number nor ``Bps``.

        Examples
        --------
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.instruments import CDSTrancheParams
        >>> CDSTrancheParams.equity_tranche(
        ...     "CDX.NA.IG", 42, Money(1e7, Currency("USD")), "2029-12-20", 500.0
        ... ).detach_pct
        3.0
        """
        ...
    @staticmethod
    def mezzanine_tranche(
        index_name: str,
        series: int,
        notional: Money,
        maturity: datetime.date | datetime.datetime | pd.Timestamp | str,
        running_coupon_bp: float | Bps,
    ) -> CDSTrancheParams:
        """
        Standard mezzanine tranche (3%–7%).

        Parameters
        ----------
        index_name : str
            Underlying index name.
        series : int
            Index series number.
        notional : Money
            Tranche notional.
        maturity : datetime.date | datetime.datetime | pd.Timestamp | str
            Scheduled maturity.
        running_coupon_bp : float | Bps
            Running coupon in basis points.

        Returns
        -------
        CDSTrancheParams
            Tranche terms with ``attach_pct=3.0`` and ``detach_pct=7.0``.

        Raises
        ------
        ValueError
            If ``maturity`` cannot be interpreted.
        TypeError
            If ``running_coupon_bp`` is neither a number nor ``Bps``.

        Examples
        --------
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.instruments import CDSTrancheParams
        >>> CDSTrancheParams.mezzanine_tranche(
        ...     "CDX.NA.IG", 42, Money(1e7, Currency("USD")), "2029-12-20", 100.0
        ... ).attach_pct
        3.0
        """
        ...
    @property
    def index_name(self) -> str:
        """
        Underlying index name.

        Returns
        -------
        str
            The index name.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def series(self) -> int:
        """
        Index series number.

        Returns
        -------
        int
            The series number.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def attach_pct(self) -> float:
        """
        Attachment point in percent.

        Returns
        -------
        float
            Attachment (``3.0`` = 3%).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def detach_pct(self) -> float:
        """
        Detachment point in percent.

        Returns
        -------
        float
            Detachment (``7.0`` = 7%).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def notional(self) -> Money:
        """
        Tranche notional.

        Returns
        -------
        Money
            Currency-tagged notional.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def maturity(self) -> datetime.date:
        """
        Scheduled maturity.

        Returns
        -------
        datetime.date
            The maturity date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def running_coupon_bp(self) -> float:
        """
        Fixed running spread paid on the tranche premium leg.

        Returns
        -------
        float
            Coupon quoted in basis points per annum on the outstanding
            tranche notional (for example ``100.0`` for a 100 bp coupon),
            not as a decimal rate. Accrues on the premium-leg day count.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def accumulated_loss(self) -> float:
        """
        Realized portfolio loss so far.

        Returns
        -------
        float
            Fraction of the original portfolio notional in ``[0, 1]``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style rendering of the key economics, e.g. ``CDSTrancheParams(index_name='CDX.NA.IG', series=42, attach_pct=3.0, detach_pct=7.0, ...)``.

        Returns
        -------
        str
            ``CDSTrancheParams(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise; it renders stored values.
        """
        ...

class ConversionSpec:
    """
    Conversion terms of a convertible bond (typed wrapper for the Rust
    ``ConversionSpec``). At least one of ``ratio`` (shares per bond) and
    ``price`` (conversion price per share) must be given; when both are,
    they must agree with ``notional / price``. Accepted by
    :meth:`ConvertibleBondBuilder.conversion` alongside dicts / JSON of the
    same shape; picklable.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import ConversionSpec
    >>> spec = ConversionSpec(ratio=25.0)
    >>> (spec.ratio, spec.policy, spec.anti_dilution)
    (25.0, 'voluntary', 'none')
    """

    def __init__(
        self,
        ratio: float | None = None,
        price: float | None = None,
        policy: str | dict[str, object] | None = None,
        anti_dilution: Literal["none", "full_ratchet", "weighted_average"] = "none",
        dividend_adjustment: Literal["none", "adjust_price", "adjust_ratio"] = "none",
        dilution_events: list[dict[str, object]] | None = None,
    ) -> None:
        """
        Describe the conversion terms.

        Parameters
        ----------
        ratio : float | None
            Conversion ratio (shares per bond); derived from ``price`` when ``None``.
        price : float | None
            Conversion price per share; derived from ``ratio`` when ``None``.
        policy : str | dict[str, object] | None
            Conversion policy: ``"voluntary"`` (the default when ``None``), or a
            tagged dict such as ``{"mandatory_on": "2027-03-15"}``,
            ``{"window": {"start": "2025-01-15", "end": "2028-01-15"}}``,
            ``{"upon_event": "qualified_ipo"}`` or ``{"mandatory_variable":
            {"conversion_date": ..., "upper_conversion_price": ...,
            "lower_conversion_price": ...}}`` (dates as ISO strings).
        anti_dilution : {"none", "full_ratchet", "weighted_average"}, default "none"
            Anti-dilution protection.
        dividend_adjustment : {"none", "adjust_price", "adjust_ratio"}, default "none"
            Dividend protection.
        dilution_events : list[dict[str, object]] | None
            Dilution events (``date``, ``new_issue_price``, ``new_shares_issued``,
            ``shares_outstanding_before``); default empty.

        Raises
        ------
        ValueError
            If ``policy`` / ``anti_dilution`` / ``dividend_adjustment`` are not
            recognized, or a dilution event does not match the schema.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import ConversionSpec
        >>> ConversionSpec(price=50.0, dividend_adjustment="adjust_ratio").dividend_adjustment
        'adjust_ratio'
        """
        ...
    @classmethod
    def from_json(cls, json: str) -> ConversionSpec:
        """
        Deserialize from the canonical JSON shape.

        Parameters
        ----------
        json : str
            JSON object with ``ratio``, ``price``, ``policy``, ``anti_dilution``,
            ``dividend_adjustment`` and optional ``dilution_events``.

        Returns
        -------
        ConversionSpec
            The parsed terms.

        Raises
        ------
        ValueError
            If ``json`` is malformed or has unknown fields.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import ConversionSpec
        >>> ConversionSpec.from_json(ConversionSpec(ratio=20.0).to_json()).ratio
        20.0
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to the canonical JSON shape.

        Returns
        -------
        str
            JSON accepted by :meth:`from_json` and :meth:`ConvertibleBondBuilder.conversion`.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...
    @property
    def ratio(self) -> float | None:
        """
        Conversion ratio (shares per bond).

        Returns
        -------
        float | None
            The explicit ratio, or ``None`` when derived from ``price``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def price(self) -> float | None:
        """
        Conversion price per share.

        Returns
        -------
        float | None
            The explicit price, or ``None`` when derived from ``ratio``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def policy(self) -> str | dict[str, object]:
        """
        Conversion policy in serde form.

        Returns
        -------
        str | dict[str, object]
            ``"voluntary"`` or the tagged dict form.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def anti_dilution(self) -> str:
        """
        Anti-dilution policy (serde name).

        Returns
        -------
        str
            ``"none"``, ``"full_ratchet"`` or ``"weighted_average"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def dividend_adjustment(self) -> str:
        """
        Dividend adjustment policy (serde name).

        Returns
        -------
        str
            ``"none"``, ``"adjust_price"`` or ``"adjust_ratio"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def dilution_events(self) -> list[dict[str, object]]:
        """
        Dilution events.

        Returns
        -------
        list[dict[str, object]]
            Serde dicts, in order.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style rendering of the key economics, e.g. ``ConversionSpec(ratio=25.0, price=None, policy='voluntary', anti_dilution='none', dividend_adjustment='none', dilution_events=<0>)``.

        Returns
        -------
        str
            ``ConversionSpec(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise; it renders stored values.
        """
        ...

class CallPutSchedule:
    """
    Issuer call and holder put windows (typed wrapper for the Rust
    ``CallPutSchedule``). Each window is a dict ``{"start_date", "end_date",
    "price_pct_of_par", "make_whole"?}`` with dates as ISO strings and prices
    in percent of par (``101.0`` = 101%). Accepted by
    :meth:`ConvertibleBondBuilder.call_put` alongside dicts / JSON of the
    same shape; picklable.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import CallPutSchedule
    >>> sched = CallPutSchedule(
    ...     calls=[{"start_date": "2026-03-15", "end_date": "2027-03-15", "price_pct_of_par": 101.0}]
    ... )
    >>> (len(sched.calls), len(sched.puts))
    (1, 0)
    """

    def __init__(
        self,
        calls: list[dict[str, object]] | str | None = None,
        puts: list[dict[str, object]] | str | None = None,
    ) -> None:
        """
        Describe the call and put windows.

        Parameters
        ----------
        calls : list[dict[str, object]] | str | None
            Issuer call windows (``start_date``, ``end_date``,
            ``price_pct_of_par``, optional ``make_whole``); default none.
        puts : list[dict[str, object]] | str | None
            Holder put windows of the same shape; default none.

        Raises
        ------
        ValueError
            If a window does not match the schema.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CallPutSchedule
        >>> CallPutSchedule().calls
        []
        """
        ...
    @classmethod
    def from_json(cls, json: str) -> CallPutSchedule:
        """
        Deserialize from the canonical JSON shape (``{"calls": [...], "puts": [...]}``).

        Parameters
        ----------
        json : str
            JSON object with ``calls`` and ``puts`` arrays.

        Returns
        -------
        CallPutSchedule
            The parsed schedule.

        Raises
        ------
        ValueError
            If ``json`` is malformed or has unknown fields.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CallPutSchedule
        >>> CallPutSchedule.from_json('{"calls": [], "puts": []}').puts
        []
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to the canonical JSON shape.

        Returns
        -------
        str
            JSON accepted by :meth:`from_json` and :meth:`ConvertibleBondBuilder.call_put`.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...
    @property
    def calls(self) -> list[dict[str, object]]:
        """
        Issuer call windows.

        Returns
        -------
        list[dict[str, object]]
            Serde dicts, in order.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def puts(self) -> list[dict[str, object]]:
        """
        Holder put windows.

        Returns
        -------
        list[dict[str, object]]
            Serde dicts, in order.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style rendering of the key economics, e.g. ``CallPutSchedule(calls=<1>, puts=<0>)``.

        Returns
        -------
        str
            ``CallPutSchedule(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise; it renders stored values.
        """
        ...

class MetricPricingOverrides:
    """
    Metric-time pricing overrides merged into an instrument before pricing.

    Typed twin of the ``pricing_options`` JSON accepted by
    :func:`price_instrument`. Every field mirrors the Rust
    ``MetricPricingOverrides`` struct; omitted fields keep the instrument's
    own overrides. Instances are immutable and compare by value.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import MetricPricingOverrides
    >>> opts = MetricPricingOverrides(theta_period="1W", bond_risk_basis="callable_oas")
    >>> (opts.theta_period, opts.bond_risk_basis, opts.quoted_price_pct)
    ('1W', 'callable_oas', None)
    >>> MetricPricingOverrides.from_json(opts.to_json()) == opts
    True
    """

    def __init__(
        self,
        *,
        bump_config: dict[str, Any] | None = None,
        mc_seed_scenario: str | None = None,
        theta_period: str | None = None,
        breakeven_config: dict[str, Any] | None = None,
        bond_risk_basis: Literal["bullet_discountable", "callable_oas"] | None = None,
        var_config: dict[str, Any] | None = None,
        quoted_price_pct: float | None = None,
    ) -> None:
        """
        Build metric-time overrides from keyword fields.

        Parameters
        ----------
        bump_config : dict[str, Any], optional
            Finite-difference bump sizes: ``spot_bump_pct`` (``0.01`` = 1%),
            ``vol_bump_pct`` (absolute vol, ``0.01`` = 1 vol point),
            ``rate_bump_bp``, ``credit_spread_bump_bp`` (basis points),
            ``ytm_bump_decimal``, ``rho_bump_decimal`` (decimal) and
            ``adaptive_bumps`` (bool). ``None`` keeps the defaults.
        mc_seed_scenario : str, optional
            Scenario name used to derive deterministic Monte Carlo seeds for
            finite-difference Greeks (for example ``"delta_up"``).
        theta_period : str, optional
            Theta / carry horizon as ``<digits><D|W|M|Y>`` (``"1D"``, ``"1W"``,
            ``"1M"``, ``"3M"``); the default horizon is one day.
        breakeven_config : dict[str, Any], optional
            Breakeven solve configuration such as
            ``{"target": "z_spread", "mode": "linear"}``.
        bond_risk_basis : {"bullet_discountable", "callable_oas"}, optional
            Basis for bond duration/convexity/DV01: Bloomberg-style workout
            risk (default) or callable OAS repricing.
        var_config : dict[str, Any], optional
            Historical VaR / expected-shortfall configuration override
            (confidence level, horizon, decay).
        quoted_price_pct : float, optional
            Externally quoted price as a percentage of original balance
            (``100.0`` = par), required by structured-credit spread metrics.

        Raises
        ------
        ValueError
            If a sub-document is malformed, ``bond_risk_basis`` is not one of
            the accepted names, or ``theta_period`` is not
            ``<digits><D|W|M|Y>``.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import MetricPricingOverrides
        >>> MetricPricingOverrides(theta_period="1M").theta_period
        '1M'
        """
        ...

    @property
    def bump_config(self) -> dict[str, Any]:
        """
        Finite-difference bump configuration.

        Returns
        -------
        dict[str, Any]
            Bump-size document; an empty dict when every size is defaulted.

        Raises
        ------
        ValueError
            If the configuration cannot be serialized to a Python object.
        """
        ...

    @property
    def mc_seed_scenario(self) -> str | None:
        """
        Monte Carlo seed scenario name.

        Returns
        -------
        str or None
            Scenario name, or ``None`` when the pricer derives its own seed.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def theta_period(self) -> str | None:
        """
        Theta / carry horizon.

        Returns
        -------
        str or None
            Horizon such as ``"1D"`` or ``"1W"``, or ``None`` for the default.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def breakeven_config(self) -> dict[str, Any] | None:
        """
        Breakeven solve configuration.

        Returns
        -------
        dict[str, Any] or None
            Configuration document, or ``None`` when breakeven is not requested.

        Raises
        ------
        ValueError
            If the configuration cannot be serialized to a Python object.
        """
        ...

    @property
    def bond_risk_basis(self) -> Literal["bullet_discountable", "callable_oas"] | None:
        """
        Basis for bond duration, convexity and DV01-style metrics.

        Returns
        -------
        {"bullet_discountable", "callable_oas"} or None
            Serde name of the basis, or ``None`` for the default
            (``"bullet_discountable"``).

        Raises
        ------
        ValueError
            If the basis cannot be rendered as its serde name.
        """
        ...

    @property
    def var_config(self) -> dict[str, Any] | None:
        """
        Historical VaR configuration override.

        Returns
        -------
        dict[str, Any] or None
            Configuration document, or ``None`` when defaults apply.

        Raises
        ------
        ValueError
            If the configuration cannot be serialized to a Python object.
        """
        ...

    @property
    def quoted_price_pct(self) -> float | None:
        """
        Externally quoted price as a percentage of original balance.

        Returns
        -------
        float or None
            Quoted price (``100.0`` = par), or ``None`` when not supplied.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @staticmethod
    def from_json(json: str) -> MetricPricingOverrides:
        """
        Deserialize overrides from canonical JSON.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`; unknown fields are
            rejected.

        Returns
        -------
        MetricPricingOverrides
            Parsed overrides.

        Raises
        ------
        ValueError
            If ``json`` is malformed or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import MetricPricingOverrides
        >>> MetricPricingOverrides.from_json('{"theta_period": "1W"}').theta_period
        '1W'
        """
        ...

    def to_json(self) -> str:
        """
        Serialize these overrides to compact JSON.

        Returns
        -------
        str
            JSON document accepted by :meth:`from_json` and by the
            ``pricing_options`` argument of :func:`price_instrument`.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __eq__(self, other: object) -> bool:
        """
        Value equality on every field.

        Parameters
        ----------
        other : object
            Any object; non-``MetricPricingOverrides`` values compare unequal.

        Returns
        -------
        bool
            Whether all fields match.

        Notes
        -----
        This method does not raise.
        """
        ...

    def __repr__(self) -> str:
        """Return a Python-style constructor form of this value.

        Returns
        -------
        str
            ``MetricPricingOverrides(bump_config=None, ..., theta_period='1W', ...)`` text.
        """
        ...

class MarketHistory:
    """
    Historical market shifts for historical VaR / expected shortfall.

    Typed twin of the ``market_history`` JSON accepted by
    :func:`price_instrument`. Each scenario is one historical date carrying a
    list of risk-factor shifts relative to the base market; the ``"hvar"``
    and ``"expected_shortfall"`` metrics revalue the instrument under every
    scenario.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import MarketHistory
    >>> history = MarketHistory(
    ...     "2024-01-01",
    ...     2,
    ...     [
    ...         {
    ...             "date": "2023-12-29",
    ...             "shifts": [
    ...                 {
    ...                     "factor": {"type": "discount_rate", "curve_id": "USD-OIS", "tenor_years": 5.0},
    ...                     "shift": 0.0010,
    ...                 }
    ...             ],
    ...         },
    ...         {
    ...             "date": "2023-12-28",
    ...             "shifts": [
    ...                 {
    ...                     "factor": {"type": "discount_rate", "curve_id": "USD-OIS", "tenor_years": 5.0},
    ...                     "shift": -0.0005,
    ...                 }
    ...             ],
    ...         },
    ...     ],
    ... )
    >>> (len(history), history.window_days, history.to_dataframe()["shift"].tolist())
    (2, 2, [0.001, -0.0005])
    """

    def __init__(
        self,
        base_date: datetime.date | datetime.datetime | pd.Timestamp | str,
        window_days: int,
        scenarios: list[dict[str, Any]],
    ) -> None:
        """
        Build a market history from scenario documents.

        Parameters
        ----------
        base_date : datetime.date | datetime.datetime | pd.Timestamp | str
            Reference date of the base market the shifts are relative to.
        window_days : int
            Length of the historical lookback window in calendar days.
        scenarios : list[dict[str, Any]]
            Chronological scenarios, each ``{"date": "YYYY-MM-DD", "shifts":
            [{"factor": {...}, "shift": float}, ...]}``. ``factor`` is a
            tagged risk factor: ``{"type": "discount_rate" | "forward_rate" |
            "credit_spread", "curve_id": str, "tenor_years": float}``,
            ``{"type": "equity_spot", "ticker": str}``, ``{"type": "fx_spot",
            "base": "EUR", "quote": "USD"}`` or ``{"type": "implied_vol",
            "vol_surface_id": str, "expiry_years": float, "strike": float}``.
            Rate and spread shifts are decimal (``0.0015`` = 15bp); spot
            shifts are relative (``-0.025`` = -2.5%); vol shifts are absolute
            vol points.

        Raises
        ------
        ValueError
            If a scenario document is malformed or carries unknown fields.
        TypeError
            If ``base_date`` is not a date-like value.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import MarketHistory
        >>> len(MarketHistory("2024-01-01", 0, []))
        0
        """
        ...

    @staticmethod
    def from_dict(data: dict[str, Any]) -> MarketHistory:
        """
        Build from a plain ``dict`` with keys ``base_date``, ``window_days`` and ``scenarios``.

        Parameters
        ----------
        data : dict[str, Any]
            Same document shape as :meth:`to_json` emits, as a Python dict.

        Returns
        -------
        MarketHistory
            Parsed history.

        Raises
        ------
        ValueError
            If the document is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import MarketHistory
        >>> MarketHistory.from_dict({"base_date": "2024-01-01", "window_days": 0, "scenarios": []}).window_days
        0
        """
        ...

    @property
    def base_date(self) -> datetime.date:
        """
        Reference date of the base market.

        Returns
        -------
        datetime.date
            Base date the shifts are relative to.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def window_days(self) -> int:
        """
        Historical window length.

        Returns
        -------
        int
            Lookback window in calendar days.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def scenarios(self) -> list[dict[str, Any]]:
        """
        Scenario documents in chronological order.

        Returns
        -------
        list[dict[str, Any]]
            ``{"date": ..., "shifts": [...]}`` documents.

        Raises
        ------
        ValueError
            If the scenarios cannot be serialized to Python objects.
        """
        ...

    def __len__(self) -> int:
        """
        Number of scenarios.

        Returns
        -------
        int
            Scenario count.

        Notes
        -----
        This method does not raise.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        One row per risk-factor shift as a tidy ``DataFrame``.

        Returns
        -------
        pd.DataFrame
            Columns ``date`` (ISO 8601 string), ``type`` (risk-factor tag),
            ``curve_id``, ``tenor_years``, ``ticker``, ``base``, ``quote``,
            ``vol_surface_id``, ``expiry_years``, ``strike`` (``NaN``/``None``
            where the factor type has no such coordinate) and ``shift``.

        Raises
        ------
        ValueError
            If the rows cannot be serialized into a pandas object.
        """
        ...

    @staticmethod
    def from_json(json: str) -> MarketHistory:
        """
        Deserialize a market history from canonical JSON.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`.

        Returns
        -------
        MarketHistory
            Parsed history.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import MarketHistory
        >>> MarketHistory.from_json('{"base_date": "2024-01-01", "window_days": 0, "scenarios": []}').window_days
        0
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this history to compact JSON.

        Returns
        -------
        str
            JSON document accepted by :meth:`from_json` and by the
            ``market_history`` argument of :func:`price_instrument`.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __repr__(self) -> str:
        """Return a Python-style constructor form of this value.

        Returns
        -------
        str
            ``MarketHistory(base_date=2024-01-01, window_days=2, scenarios=<2 items>)`` text.
        """
        ...
