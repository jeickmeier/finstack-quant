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

import builtins
import datetime
from typing import Any, Literal

import pandas as pd

from finstack_quant.cashflows.builder import (
    CashFlowSchedule,
    DefaultModelSpec,
    PrepaymentModelSpec,
    RecoveryModelSpec,
)
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
    "CallAssumption",
    "CallPutSchedule",
    "CapFloor",
    "CapFloorBuilder",
    "ConversionSpec",
    "ConvertibleBond",
    "ConvertibleBondBuilder",
    "CoverageRules",
    "CreditDefaultSwap",
    "CreditDefaultSwapBuilder",
    "EquityMetrics",
    "EquityOption",
    "EquityOptionBuilder",
    "FixedLegSpec",
    "FloatLegSpec",
    "FxForward",
    "FxForwardBuilder",
    "FxOption",
    "FxOptionBuilder",
    "HedgeSwap",
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
    "PoolAsset",
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
    "TrancheCashflows",
    "TrancheMetrics",
    "TrancheStructure",
    "Waterfall",
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
    instrument. Required fields: ``id``, ``notional``, ``issue_date``, ``maturity``,
    ``cashflow_spec``, ``discount_curve_id``. Nested specs accept a ``dict`` or JSON ``str``
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
        Set the required contractual issue date.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pd.Timestamp | str
            Required contractual issue date (ISO 8601 strings accepted). Omission raises ValueError on build().

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
        Set the required contractual issue / funding date.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pd.Timestamp | str
            Required contractual issue / funding date (ISO 8601 strings accepted). Omission raises ValueError on build().

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
    ...     standard_imm_dates=True,
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
        standard_imm_dates: bool,
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
        standard_imm_dates : bool
            True for prescribed quarterly CDS 20th dates. False generates a
            bespoke schedule from ``frequency`` and ``stub``. Standard dates
            require quarterly frequency and a short-front stub at pricing.
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
        ...     standard_imm_dates=True,
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
        ...     standard_imm_dates=True,
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
    def standard_imm_dates(self) -> bool:
        """Whether the leg uses prescribed quarterly CDS 20th dates.

        Returns
        -------
        bool
            True for the standard roll grid; false for bespoke frequency/stub dates.

        Notes
        -----
        This accessor does not raise; it returns the stored flag.
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
        notional: Money | builtins.float,
        side: Literal["pay", "receive"],
        fixed_rate: builtins.float | Rate,
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
    ) -> builtins.float:
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
    def notional(self, value: Money | builtins.float, currency: str | None = None) -> InterestRateSwapBuilder:
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
            Volatility type convention (serde string). Must match the configured surface; ``"auto"`` (the default when unset) follows source convention and displacement metadata. Normal quotes use decimal rate units; Black quotes use dimensionless annual volatility. Incompatible model/source conventions raise ``ValueError``.

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
    same pricer as :func:`price_instrument`. The ``"cs01"`` and
    ``"bucketed_cs01"`` metrics rebootstrap a quote-backed hazard curve from
    its stored calibration recipe.

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
        Compute one scalar metric for this instrument (e.g. ``"cs01"`` or ``"par_spread"``).

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
    ...     standard_imm_dates=True,
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
        Compute one scalar metric for this instrument (e.g. ``"cs01"``).

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
        1bp parallel spread bump. Hazard curves built by hand without a lossless
        calibration recipe raise.

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
    ...     standard_imm_dates=True,
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
        Conversion value divided by notional (mirrors Rust
        ``ConvertibleBond::parity``). Ordinary conversion uses the effective
        ratio times spot; mandatory-variable conversion uses the contractual
        lower-price, variable-share and upper-price regimes.

        Parameters
        ----------
        market : MarketContext | str
            Market carrying the underlying equity price.

        Returns
        -------
        float
            Dimensionless conversion-value-to-notional ratio; 1.0 means par.

        Raises
        ------
        KeyError
            If the underlying price is missing from ``market``.
        ValueError
            If conversion terms are invalid or the equity price is negative
            or non-finite.
        RuntimeError
            If the bond has no ``underlying_equity_id``.
        """
        ...
    def conversion_premium(self, market: MarketContext | str, bond_price: float) -> float:
        """
        Conversion premium over parity (mirrors Rust
        ``ConvertibleBond::conversion_premium``):
        ``bond_price / conversion_value - 1``. Conversion value follows the
        policy, including mandatory-variable share delivery.

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
            If conversion value is nonpositive or ``bond_price`` is negative
            or non-finite.
        RuntimeError
            If the bond has no ``underlying_equity_id``.
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
            Market carrying the curves, equity price and volatility. An active
            instrument volatility override takes precedence; otherwise surface
            volatility is sampled at the contractual conversion strike. Floating
            coupons require their forward curve and realized historical fixings.
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
        Set the model day count for volatility, dividend carry and exercise times.

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
        Recover decimal volatility with the configured exercise engine,
        dividend schedule and model day count. Trial volatility replaces the
        active market surface or override. American and Bermudan exercise use
        the configured lattice and remaining exercise dates.

        Parameters
        ----------
        market : MarketContext | str
            Market carrying the discount curve, spot and (optional) dividend yield.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date strictly before expiry and observed exercise.
        market_price : float
            Finite non-negative total trade PV in the notional currency.

        Returns
        -------
        float
            Annualized lognormal volatility as a decimal (``0.20`` = 20%).

        Raises
        ------
        ValueError
            If PV or notional is invalid, the option has expired or exercised,
            or the target is indistinguishable from the deterministic price.
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
        Model day count for volatility, carry and exercise times (serde name).
        Discount factors use the discount curve's own date convention.

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
        Set the model day count for volatility, dividend carry and exercise times.

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
    ...     asset_type={"type": "first_lien_loan", "industry": None},
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
        asset_type: dict[str, Any],
        spread_bp: float | Bps | None = None,
        index_id: str | None = None,
        index_floor: float | None = None,
        cpr: float | None = None,
        cdr: float | None = None,
        recovery_rate: float | None = None,
        contractual_payment: Money | None = None,
        amortization_term_months: int | None = None,
        io_months: int | None = None,
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
        asset_type : dict[str, Any]
            Canonical AssetType JSON object; specifies bullet or level-pay amortization.
        spread_bp : float | Bps, optional
            Weighted average spread over the reference index, in basis
            points (e.g. ``150.0`` = 150bp), for floating-rate lines.
        index_id : str, optional
            Reference index identifier, if floating.
        index_floor : float, optional
            Floor on the floating index as an annual decimal, applied before
            ``spread_bp``; ignored on fixed-rate lines.
        cpr : float, optional
            Constant prepayment rate override, as an annual decimal (e.g.
            ``0.10`` = 10% CPR).
        cdr : float, optional
            Constant default rate override, as an annual decimal (e.g.
            ``0.02`` = 2% CDR).
        recovery_rate : float, optional
            Recovery rate override, as a decimal fraction (e.g. ``0.45`` =
            45%).
        contractual_payment : Money, optional
            Contractual periodic payment for level-pay lines; inferred from the
            balance and remaining schedule when ``None``.
        amortization_term_months : int, optional
            Schedule length in months from origination; with
            ``seasoning_months`` the line amortizes over ``term − seasoning``.
        io_months : int, optional
            Interest-only window in months from origination.

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
        ...     asset_type={"type": "first_lien_loan", "industry": None},
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
        ...         "LINE-1",
        ...         Money(1.0, Currency("USD")),
        ...         0.07,
        ...         datetime.date(2031, 1, 15),
        ...         12,
        ...         DayCount.ACT_360,
        ...         asset_type={"type": "first_lien_loan", "industry": None},
        ...     ).to_json()
        ... )
        >>> restored.to_json() == RepLine(
        ...     "LINE-1",
        ...     Money(1.0, Currency("USD")),
        ...     0.07,
        ...     datetime.date(2031, 1, 15),
        ...     12,
        ...     DayCount.ACT_360,
        ...     asset_type={"type": "first_lien_loan", "industry": None},
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
    def asset_type(self) -> dict[str, Any]:
        """Return the canonical asset classification and amortization behavior.

        Returns
        -------
        dict[str, Any]
            Rust AssetType wire object, such as ``{"type": "first_lien_loan", "industry": None}``.

        Raises
        ------
        RuntimeError
            If conversion to a Python dictionary fails.
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
    def index_floor(self) -> float | None:
        """
        Floor on the floating index (annual decimal) applied before the spread.

        Returns
        -------
        float | None
            ``None`` when the row is unfloored or fixed-rate.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def contractual_payment(self) -> Money | None:
        """
        Contractual periodic payment for level-pay lines.

        Returns
        -------
        Money | None
            ``None`` when the payment is inferred.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def amortization_term_months(self) -> int | None:
        """
        Amortization schedule length in months from origination.

        Returns
        -------
        int | None
            ``None`` when unset.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def io_months(self) -> int | None:
        """
        Interest-only window in months from origination.

        Returns
        -------
        int | None
            ``None`` when unset.

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
            A new, empty asset pool. Use either :meth:`with_rep_lines` or
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
        ...         asset_type={"type": "first_lien_loan", "industry": None},
        ...     )
        ... ])
        >>> "POOL-1" in repr(pool)
        True
        """
        ...

    def with_assets(self, value: list[PoolAsset | dict[str, Any]] | str) -> AssetPool:
        """
        Attach loan-level assets, returning a new pool.

        Parameters
        ----------
        value : list[PoolAsset | dict[str, Any]] | str
            Typed :class:`PoolAsset` rows, their serde dicts, or a JSON array
            string (mixing typed rows and dicts is allowed).

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
    def with_instruments(
        self,
        bonds: list[Bond] | None = None,
        term_loans: list[TermLoan] | None = None,
        revolvers: list[RevolvingCredit] | None = None,
        call_exercise: str | dict[str, Any] | None = None,
        put_exercise: str | dict[str, Any] | None = None,
        overrides: list[dict[str, Any]] | None = None,
    ) -> AssetPool:
        """
        Attach real instruments as the collateral, returning a new pool.

        The pool then holds ``Bond``, ``TermLoan`` and ``RevolvingCredit``
        instruments instead of asset rows or representative lines; each
        instrument's own cashflow schedule drives the deal, defaults come from
        the instrument's credit curve when present, and collateral draws are
        funded from the reserve account (see :meth:`with_reserve`).

        Parameters
        ----------
        bonds : list[Bond], optional
            Bonds in any form (fixed, floating, step-up, amortizing, callable).
        term_loans : list[TermLoan], optional
            Term loans, including delayed-draw facilities.
        revolvers : list[RevolvingCredit], optional
            Revolving facilities; stochastic ones simulate draws on the paths.
        call_exercise : str | dict[str, Any], optional
            Default issuer-call policy: ``"contractual"`` (never, the default),
            ``"first_call"``, ``"worst"`` (yield-to-worst, needs a quoted clean
            price) or ``{"policy": "refinancing_incentive", "threshold_bp": 50.0}``.
        put_exercise : str | dict[str, Any], optional
            Default holder-put policy: ``"never"`` (default), ``"first_put"``, or the
            dict ``{"policy": "reinvestment_incentive", "threshold_bp": 50.0}`` (put
            when the reinvestment rate exceeds the coupon by more than the threshold).
        overrides : list[dict[str, Any]], optional
            Per-instrument overrides ``{"id": ..., "call": {...}, "put": {...}}``.

        Returns
        -------
        AssetPool
            A new pool with ``instruments`` set (the original is unchanged).

        Raises
        ------
        ValueError
            If a policy or override does not match its serde shape.
        TypeError
            If a list element is not the expected typed instrument.

        Examples
        --------
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.valuations.instruments import AssetPool, Bond, RevolvingCredit
        >>> pool = AssetPool("POOL-1", "clo", Currency("USD")).with_instruments(
        ...     bonds=[Bond.example()],
        ...     revolvers=[RevolvingCredit.example()],
        ...     call_exercise="first_call",
        ... )
        >>> sorted(pool.instruments)
        ['bonds', 'call_exercise', 'overrides', 'put_exercise', 'revolvers', 'term_loans']
        """
        ...
    def with_reserve(
        self,
        reserve_account: Money | float,
        reserve_account_rate: float = 0.0,
        reserve_target: Money | float | None = None,
        reserve_interest_destination: str | dict[str, Any] | None = None,
        currency: str | None = None,
    ) -> AssetPool:
        """
        Configure the reserve account, returning a new pool.

        The reserve funds collateral draws (revolver utilization increases,
        delayed draws and loan-equivalent draws at default), is replenished by
        revolver repayments up to ``reserve_target``, and earns
        ``reserve_account_rate`` routed per ``reserve_interest_destination``.

        Parameters
        ----------
        reserve_account : Money | float
            Opening reserve balance; a bare number needs ``currency``.
        reserve_account_rate : float, default 0.0
            Annual interest rate earned by the reserve, as a decimal (simple
            ACT/360 on the opening balance each period).
        reserve_target : Money | float, optional
            Balance revolver repayments replenish toward; ``None`` disables
            replenishment.
        reserve_interest_destination : str | dict[str, Any], optional
            ``"waterfall"`` (default, interest proceeds), ``"retain"``
            (capitalized into the reserve) or
            ``{"kind": "tranche", "tranche_id": "EQ"}`` (paid directly to that
            tranche).
        currency : str, optional
            ISO-4217 code applied when a bare number is passed.

        Returns
        -------
        AssetPool
            A new pool with the reserve configured (the original is unchanged).

        Raises
        ------
        ValueError
            If an amount is not finite, a bare number has no currency, or the
            destination does not match its serde shape.
        TypeError
            If an amount is neither ``Money`` nor a number.

        Examples
        --------
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.instruments import AssetPool
        >>> pool = AssetPool("POOL-1", "clo", Currency("USD")).with_reserve(
        ...     Money(5_000_000.0, Currency("USD")),
        ...     reserve_account_rate=0.03,
        ...     reserve_interest_destination={"kind": "tranche", "tranche_id": "EQ"},
        ... )
        >>> (pool.reserve_account_rate, pool.reserve_interest_destination["kind"])
        (0.03, 'tranche')
        """
        ...

    def with_reinvestment_period(self, value: dict[str, Any] | str) -> AssetPool:
        """
        Configure the deal-level reinvestment period, returning a new pool.

        Principal proceeds collected while the period is active are recycled
        into collateral instead of repaying the notes; every note is held flat
        except those listed in ``amortizing_tranches``, which are paid down
        first. Instrument-collateral pools cannot reinvest.

        Parameters
        ----------
        value : dict[str, Any] | str
            ``ReinvestmentPeriod`` in its serde shape, or that JSON as a
            string: ISO ``end_date`` (inclusive), ``is_active``, ``criteria``
            (``max_price`` percent of par, ``min_yield`` annual decimal current
            yield below which a surviving asset is skipped), optional
            ``amortizing_tranches`` (note ids paid down inside the window) and
            optional ``assumptions`` (``spread_bp``, ``price_pct``,
            ``maturity_months``, ``index_id``, ``coupon_floor``) describing the
            replacement collateral; omitted assumptions clone the surviving
            pool pro rata.

        Returns
        -------
        AssetPool
            A new pool with the reinvestment period set (the original is
            unchanged).

        Raises
        ------
        ValueError
            If ``value`` does not match the ``ReinvestmentPeriod`` serde shape.
            Tranche ids, dates and assumption ranges are validated when the
            deal is built or priced.

        Examples
        --------
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.valuations.instruments import AssetPool
        >>> pool = AssetPool("POOL-1", "clo", Currency("USD")).with_reinvestment_period({
        ...     "end_date": "2028-01-01",
        ...     "is_active": True,
        ...     "criteria": {
        ...         "max_price": 100.0,
        ...         "min_yield": 0.0,
        ...     },
        ...     "amortizing_tranches": ["A"],
        ... })
        >>> pool.reinvestment_period["amortizing_tranches"]
        ['A']
        """
        ...

    def with_accounts(
        self,
        *,
        cumulative_defaults: Money | None = None,
        cumulative_recoveries: Money | None = None,
        cumulative_prepayments: Money | None = None,
        cumulative_scheduled_amortization: Money | None = None,
        collection_account: Money | None = None,
        excess_spread_account: Money | None = None,
        original_balance: Money | None = None,
    ) -> AssetPool:
        """
        Set the pool's historical tallies and cash accounts, returning a new
        pool (seasoned-deal inputs).

        Parameters
        ----------
        cumulative_defaults : Money, optional
            Defaulted par to date; unchanged when omitted.
        cumulative_recoveries : Money, optional
            Recoveries received to date; unchanged when omitted.
        cumulative_prepayments : Money, optional
            Prepayments received to date; unchanged when omitted.
        cumulative_scheduled_amortization : Money, optional
            Scheduled principal received to date; unchanged when omitted.
        collection_account : Money, optional
            Undistributed collections held at closing; unchanged when omitted.
        excess_spread_account : Money, optional
            Trapped excess spread held at closing; unchanged when omitted.
        original_balance : Money, optional
            Original (cut-off) pool balance the cumulative-loss triggers,
            clean-up call factor and loss curves are stated against; when
            omitted the engine reconstructs it as the current balance plus
            the cumulative defaults, prepayments and scheduled amortization.
            Must be at least the current balance (``ValueError`` at pricing).

        Returns
        -------
        AssetPool
            A new pool with the supplied balances (the original is unchanged).

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.instruments import AssetPool
        >>> pool = AssetPool("POOL-1", "clo", Currency("USD")).with_accounts(
        ...     cumulative_defaults=Money(8_000_000.0, Currency("USD"))
        ... )
        >>> pool.cumulative_defaults.amount
        8000000.0
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
            Defaulted assets specify economic ``default_date`` and outstanding,
            unreceived ``recovery_amount`` in asset currency. Their par is excluded
            from performing collateral. ``reinvestment_period`` owns the active
            flag, inclusive end date, maximum percent-of-par purchase price and
            minimum annual decimal current yield.

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
    def assets(self) -> list[PoolAsset]:
        """
        Loan-level assets as typed :class:`PoolAsset` rows.

        Returns
        -------
        list[PoolAsset]
            One row per asset (empty when rep lines or instruments are used).

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
    def original_balance(self) -> Money | None:
        """
        Original (cut-off) pool balance when supplied.

        Returns
        -------
        Money or None
            The explicit original balance, or ``None`` when the engine
            reconstructs it from the current balance and cumulative tallies.

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

    @property
    def reserve_account_rate(self) -> float:
        """
        Annual interest rate earned by the reserve account, as a decimal.

        Returns
        -------
        float
            The reserve rate (``0.0`` when the reserve earns nothing).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def reserve_target(self) -> Money | None:
        """
        Reserve balance revolver repayments replenish toward, or ``None``.

        Returns
        -------
        Money | None
            The target balance, or ``None`` when replenishment is disabled.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def reserve_interest_destination(self) -> dict[str, Any]:
        """
        Destination of the reserve interest as its serde ``dict``.

        Returns
        -------
        dict[str, Any]
            ``{"kind": "waterfall"}``, ``{"kind": "retain"}`` or
            ``{"kind": "tranche", "tranche_id": ...}``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def instruments(self) -> dict[str, Any] | None:
        """
        Instrument collateral as its serde ``dict``, or ``None``.

        Returns
        -------
        dict[str, Any] | None
            ``bonds``, ``term_loans``, ``revolvers``, ``call_exercise``,
            ``put_exercise`` and ``overrides``; ``None`` when the pool is
            modelled with asset rows or representative lines.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def reinvestment_period(self) -> dict[str, Any] | None:
        """
        Reinvestment period as its serde ``dict``, or ``None``.

        Returns
        -------
        dict[str, Any] | None
            ``end_date`` (ISO), ``is_active``, ``criteria``,
            ``amortizing_tranches`` and ``assumptions`` as set with
            :meth:`with_reinvestment_period`; ``None`` when the deal does not
            reinvest.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
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
    def attachment_point(self) -> float | None:
        """
        Attachment point in percent (0-100 scale).

        ``None`` for a note built without points until a ``TrancheStructure``
        derives it from the balance shares.

        Returns
        -------
        float | None
            e.g. ``10.0``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def detachment_point(self) -> float | None:
        """
        Detachment point in percent (0-100 scale).

        ``None`` for a note built without points until a ``TrancheStructure``
        derives it from the balance shares.

        Returns
        -------
        float | None
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
    def non_deferrable(self) -> bool | None:
        """
        Explicit non-deferrable flag on the coupon.

        Returns
        -------
        bool | None
            ``None`` for the seniority convention (senior notes are
            non-deferrable, every other class defers).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def is_non_deferrable(self) -> bool:
        """
        Whether the coupon is a non-deferrable claim the template pays from
        principal proceeds when interest falls short.

        Returns
        -------
        bool
            The explicit flag when set, else ``True`` for senior notes only.

        Notes
        -----
        This accessor does not raise; it returns the resolved convention.
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
    :meth:`build`, so either call order works. Both may be omitted: a
    ``TrancheStructure`` then derives them from the balance shares in
    payment-priority order (the first-loss class attaches at 0).

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

    def current_balance(self, value: Money) -> TrancheBuilder:
        """
        Set the current (factored) balance.

        Parameters
        ----------
        value : Money
            Outstanding principal today, at most the original balance. Defaults
            to the original balance when never set.
        Returns
        -------
        TrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` is invalid or this builder was already consumed by a
            prior call to :meth:`TrancheBuilder.build`.
        """
        ...

    def deferred_interest(self, value: Money) -> TrancheBuilder:
        """
        Set interest already deferred (unpaid, still owed) at closing.

        Parameters
        ----------
        value : Money
            Deferred interest carried into the projection; zero when never set.
        Returns
        -------
        TrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` is invalid or this builder was already consumed by a
            prior call to :meth:`TrancheBuilder.build`.
        """
        ...

    def pik_enabled(self, value: bool) -> TrancheBuilder:
        """
        Enable payment-in-kind accretion of interest shortfalls.

        Parameters
        ----------
        value : bool
            ``True`` capitalizes unpaid interest into the balance; ``False`` (the
            default) defers it as a claim.
        Returns
        -------
        TrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` is invalid or this builder was already consumed by a
            prior call to :meth:`TrancheBuilder.build`.
        """
        ...

    def non_deferrable(self, value: bool) -> TrancheBuilder:
        """
        Mark the coupon non-deferrable or deferrable, overriding the seniority
        convention.

        Parameters
        ----------
        value : bool
            ``True`` for a coupon the template pays from principal proceeds when
            interest falls short (the senior default); ``False`` for one that
            defers (the default for every other class).

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

    def rating(self, value: str) -> TrancheBuilder:
        """
        Set the credit rating.

        Parameters
        ----------
        value : str
            Rating string (``"AAA"``, ``"BBB"``, ``"NR"`` ...).
        Returns
        -------
        TrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` is invalid or this builder was already consumed by a
            prior call to :meth:`TrancheBuilder.build`.
        """
        ...

    def oc_trigger(self, value: dict[str, Any] | str) -> TrancheBuilder:
        """
        Attach a per-tranche overcollateralization trigger.

        Parameters
        ----------
        value : dict[str, Any] | str
            ``CoverageTrigger`` serde object: ``trigger_level`` (ratio), optional
            ``cure_level``, ``consequence`` and breach memory fields.
        Returns
        -------
        TrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` is invalid or this builder was already consumed by a
            prior call to :meth:`TrancheBuilder.build`.
        """
        ...

    def ic_trigger(self, value: dict[str, Any] | str) -> TrancheBuilder:
        """
        Attach a per-tranche interest-coverage trigger.

        Parameters
        ----------
        value : dict[str, Any] | str
            ``CoverageTrigger`` serde object (see :meth:`oc_trigger`).
        Returns
        -------
        TrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` is invalid or this builder was already consumed by a
            prior call to :meth:`TrancheBuilder.build`.
        """
        ...

    def attributes(self, value: Attributes | dict[str, str]) -> TrancheBuilder:
        """
        Set free-form attributes (tags and metadata).

        Parameters
        ----------
        value : Attributes | dict[str, str]
            Attribute bag; a dict populates ``meta`` (an optional ``"tags"`` list
            populates ``tags``).
        Returns
        -------
        TrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` is invalid or this builder was already consumed by a
            prior call to :meth:`TrancheBuilder.build`.
        """
        ...

    @property
    def oc_trigger(self) -> dict[str, Any] | None:
        """
        Per-tranche overcollateralization trigger as its ``CoverageTrigger`` serde ``dict``.

        Returns
        -------
        dict[str, Any] | None
            ``None`` without a trigger.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def ic_trigger(self) -> dict[str, Any] | None:
        """
        Per-tranche interest-coverage trigger as its ``CoverageTrigger`` serde ``dict``.

        Returns
        -------
        dict[str, Any] | None
            ``None`` without a trigger.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
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
    def from_balances(tranches: list[Tranche]) -> TrancheStructure:
        """
        Build a structure whose attachment and detachment points come from the
        balance shares alone (mirrors Rust ``TrancheStructure::from_balances``).

        Any points declared on the tranches are discarded: the first-loss class
        attaches at 0, each senior class stacks on top in payment-priority order
        and the most senior class detaches at 100.

        Parameters
        ----------
        tranches : list[Tranche]
            Notes of the capital structure; ``seniority`` and
            ``original_balance`` decide the boundaries.

        Returns
        -------
        TrancheStructure
            The validated structure with derived points.

        Raises
        ------
        ValueError
            If the list is empty or the notes mix currencies.

        Examples
        --------
        >>> import datetime
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.instruments import Tranche, TrancheStructure
        >>> def note(id_, seniority, balance):
        ...     return (
        ...         Tranche
        ...         .builder()
        ...         .id(id_)
        ...         .seniority(seniority)
        ...         .original_balance(Money(balance, Currency("USD")))
        ...         .coupon_fixed(0.05)
        ...         .maturity(datetime.date(2031, 1, 15))
        ...         .build()
        ...     )
        >>> structure = TrancheStructure.from_balances([
        ...     note("A", "senior", 60.0),
        ...     note("B", "mezzanine", 30.0),
        ...     note("E", "equity", 10.0),
        ... ])
        >>> [(t.id, t.attachment_point, t.detachment_point) for t in structure.tranches]
        [('A', 40.0, 100.0), ('B', 10.0, 40.0), ('E', 0.0, 10.0)]
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

class PoolAsset:
    """
    One loan-level collateral row of an :class:`AssetPool`: the contractual
    terms (balance, coupon or index + spread, maturity, day count,
    amortization type), the credit state (rating, default, recovery, purchase
    price) and the behavioural overrides (SMM / MDR / recovery overrides,
    delinquency buckets, commercial-mortgage balloon, prepayment penalty,
    special servicing, NOI).

    Percent fields (``market_price_pct``) are percent values; ``rate``,
    ``smm_override``, ``mdr_override`` and ``recovery_rate`` are decimals;
    ``spread_bp`` is in basis points.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.valuations.instruments import PoolAsset
    >>> loan = PoolAsset(
    ...     "LOAN-1",
    ...     {"type": "first_lien_loan", "industry": "Software"},
    ...     Money(10_000_000.0, Currency("USD")),
    ...     0.08,
    ...     datetime.date(2031, 1, 15),
    ...     credit_quality="B",
    ...     balloon={"extension_prob": 0.3, "extension_months": 24},
    ... )
    >>> loan.credit_quality, loan.balloon["extension_months"]
    ('B', 24)
    """

    def __init__(
        self,
        id: str,
        asset_type: dict[str, Any] | str,
        balance: Money,
        rate: float,
        maturity: datetime.date,
        *,
        day_count: DayCount | None = None,
        spread_bp: float | None = None,
        index_id: str | None = None,
        index_floor: float | None = None,
        credit_quality: str | None = None,
        industry: str | None = None,
        obligor_id: str | None = None,
        is_defaulted: bool = False,
        recovery_amount: Money | None = None,
        default_date: datetime.date | None = None,
        purchase_price: Money | None = None,
        acquisition_date: datetime.date | None = None,
        origination_date: datetime.date | None = None,
        smm_override: float | None = None,
        mdr_override: float | None = None,
        recovery_rate: float | None = None,
        commitment: Money | None = None,
        contractual_payment: Money | None = None,
        amortization_term_months: int | None = None,
        io_months: int | None = None,
        market_price_pct: float | None = None,
        delinquency_buckets: list[Money] | None = None,
        balloon: dict[str, Any] | str | None = None,
        prepayment_penalty: dict[str, Any] | str | None = None,
        special_servicing: dict[str, Any] | str | None = None,
        noi: Money | None = None,
        liquidation: dict[str, Any] | str | None = None,
    ) -> None:
        """
        Construct a collateral row from its contractual terms.

        Parameters
        ----------
        id : str
            Stable asset identifier, unique within the pool.
        asset_type : dict[str, Any] | str
            ``AssetType`` serde object such as ``{"type": "first_lien_loan",
            "industry": None}`` or ``{"type": "high_yield_bond"}``; the type decides whether
            the row amortizes (level pay) or pays as a bullet.
        balance : Money
            Current principal balance in the pool currency.
        rate : float
            Annual coupon as a decimal (``0.08`` = 8%). For floating rows the
            engine adds ``spread_bp`` to the ``index_id`` projection.
        maturity : datetime.date
            Contractual maturity (balloon date for commercial mortgages).
        day_count : DayCount, optional
            Accrual convention; Act/360 when omitted.
        spread_bp : float, optional
            Floating spread over ``index_id`` in basis points.
        index_id : str, optional
            Forward-curve identifier of the floating index; ``None`` for a
            fixed-rate row.
        index_floor : float, optional
            Floor on the floating index as an annual decimal (``0.01`` = 1%),
            applied before ``spread_bp`` is added; ignored on fixed-rate rows.
        credit_quality : str, optional
            Credit rating (``"BB"``, ``"CCC"``, ``"NR"`` ...), used by the
            coverage-test haircuts and the CCC bucket.
        industry : str, optional
            Industry label for concentration reporting.
        obligor_id : str, optional
            Obligor identifier for exposure aggregation.
        is_defaulted : bool, optional
            ``True`` marks the row defaulted at closing; ``recovery_amount`` and
            ``default_date`` describe its state.
        recovery_amount : Money, optional
            Recovery still expected on a defaulted row.
        default_date : datetime.date, optional
            Date of default for a defaulted row.
        purchase_price : Money, optional
            Price paid for the row (discount-obligation test).
        acquisition_date : datetime.date, optional
            Date the row entered the pool (anchors non-performing-loan
            timelines).
        origination_date : datetime.date, optional
            Date the loan was originated; anchors the seasoning-dependent
            prepayment/default curves (PSA, SDA, ABS, vector) and the
            amortization schedule. Falls back to ``acquisition_date``, then
            to the deal closing date.
        smm_override : float, optional
            Row-level single-month mortality (decimal) overriding the deal
            prepayment model.
        mdr_override : float, optional
            Row-level monthly default rate (decimal) overriding the deal default
            model.
        recovery_rate : float, optional
            Row-level recovery rate (decimal) overriding the deal recovery model.
        commitment : Money, optional
            Total commitment for revolving rows (drawn balance is ``balance``).
        contractual_payment : Money, optional
            Monthly level payment of an amortizing row; derived from the terms
            when omitted.
        amortization_term_months : int, optional
            Schedule length in months from origination (``origination_date``,
            else ``acquisition_date``, else closing) for level-pay rows; the unamortized balance pays as
            the balloon at maturity. ``None`` amortizes fully by maturity.
        io_months : int, optional
            Interest-only window in months from origination: no scheduled
            principal while the loan is younger than this.
        market_price_pct : float, optional
            Market price in percent of par for market-value coverage rules.
        delinquency_buckets : list[Money], optional
            Seeded delinquent balances per bucket (30/60/90 ...); requires a
            ``credit_model.delinquency`` model on the deal.
        balloon : dict[str, Any] | str, optional
            ``BalloonSpec`` (``extension_prob`` decimal, ``extension_months``,
            optional ``extension_rate`` decimal, and ``loss_prob`` decimal,
            ``severity_pct`` percent, ``workout_months`` for the share that
            defaults at maturity) for balloon extension and workout.
        prepayment_penalty : dict[str, Any] | str, optional
            ``PrepaymentPenalty``: ``{"kind": "lockout", "through":
            "2025-12-31"}`` (no voluntary prepayment inside the window),
            ``{"kind": "fixed", "pct": 3.0, "through": "2026-01-01"}``,
            ``{"kind": "step_down", "schedule": [{"through": "2025-12-31",
            "pct": 5.0}, ...]}`` or ``{"kind": "yield_maintenance",
            "reinvestment_rate": 0.05, "discount_curve_id": "USD-OIS",
            "floor_pct": 1.0, "through": None}`` (the curve discounts the lost
            coupons and supplies the reinvestment rate when it is omitted).
        special_servicing : dict[str, Any] | str, optional
            ``SpecialServicingSpec`` (``appraisal_reduction_pct`` percent).
        noi : Money, optional
            Annual net operating income of the property for the CMBS DSCR.
        liquidation : dict[str, Any] | str, optional
            ``LiquidationSpec`` for a non-performing loan
            (``months_to_resolution``, ``proceeds_pct`` and ``carry_cost_pct``
            percents, ``reperformance_prob`` decimal, optional
            ``modified_rate`` decimal); leave ``is_defaulted`` false.

        Raises
        ------
        ValueError
            If a sub-spec does not match its serde shape, a date is invalid or
            ``credit_quality`` is not a known rating.
        """
        ...

    @staticmethod
    def fixed_rate_bond(
        id: str, balance: Money, rate: float, maturity: datetime.date, day_count: DayCount
    ) -> PoolAsset:
        """
        Fixed-rate bullet bond row (mirrors Rust ``PoolAsset::fixed_rate_bond``).

        Parameters
        ----------
        id : str
            Stable asset identifier.
        balance : Money
            Current principal balance.
        rate : float
            Annual fixed coupon as a decimal.
        maturity : datetime.date
            Bullet maturity.
        day_count : DayCount
            Accrual convention.

        Returns
        -------
        PoolAsset
            A performing, unrated bond row.

        Raises
        ------
        ValueError
            If ``maturity`` is not a valid date.

        Examples
        --------
        >>> import datetime
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.dates import DayCount
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.instruments import PoolAsset
        >>> bond = PoolAsset.fixed_rate_bond(
        ...     "B1", Money(1_000_000.0, Currency("USD")), 0.06, datetime.date(2030, 1, 1), DayCount.THIRTY_360
        ... )
        >>> bond.asset_type["type"], bond.rate
        ('high_yield_bond', 0.06)
        """
        ...

    @staticmethod
    def floating_rate_loan(
        id: str, balance: Money, index_id: str, spread_bp: float, maturity: datetime.date, day_count: DayCount
    ) -> PoolAsset:
        """
        Floating-rate first-lien loan row (mirrors Rust ``PoolAsset::floating_rate_loan``).

        Parameters
        ----------
        id : str
            Stable asset identifier.
        balance : Money
            Current principal balance.
        index_id : str
            Forward-curve identifier of the floating index (e.g. ``"USD-SOFR-3M"``).
        spread_bp : float
            Spread over the index in basis points.
        maturity : datetime.date
            Loan maturity.
        day_count : DayCount
            Accrual convention.

        Returns
        -------
        PoolAsset
            A performing first-lien loan row.

        Raises
        ------
        ValueError
            If ``maturity`` is not a valid date.

        Examples
        --------
        >>> import datetime
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.dates import DayCount
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.instruments import PoolAsset
        >>> loan = PoolAsset.floating_rate_loan(
        ...     "L1",
        ...     Money(1_000_000.0, Currency("USD")),
        ...     "USD-SOFR-3M",
        ...     350.0,
        ...     datetime.date(2030, 1, 1),
        ...     DayCount.ACT_360,
        ... )
        >>> loan.index_id, loan.spread_bp
        ('USD-SOFR-3M', 350.0)
        """
        ...

    @staticmethod
    def from_json(json: str) -> PoolAsset:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``PoolAsset`` (the shape ``to_json`` writes).

        Returns
        -------
        PoolAsset
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import PoolAsset
        >>> try:
        ...     PoolAsset.from_json("{}")
        ... except ValueError:
        ...     print("rejected")
        rejected
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            JSON-encoded ``PoolAsset``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return every field as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Serde form of the Rust value.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(PoolAsset.from_json, (json,))``.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __repr__(self) -> str:
        """
        Return ``repr(self)``.

        Returns
        -------
        str
            A one-line summary of the value.
        """
        ...

    @property
    def id(self) -> str:
        """
        Asset identifier.

        Returns
        -------
        str
            The stable identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def asset_type(self) -> dict[str, Any]:
        """
        ``AssetType`` serde object (``{"type": ..., ...}``).

        Returns
        -------
        dict[str, Any]
            The asset type and its variant fields.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def balance(self) -> Money:
        """
        Current principal balance.

        Returns
        -------
        Money
            The balance in the pool currency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def rate(self) -> float:
        """
        Annual coupon as a decimal.

        Returns
        -------
        float
            The coupon (spread-only for floating rows).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def spread_bp(self) -> float | None:
        """
        Floating spread in basis points.

        Returns
        -------
        float | None
            ``None`` for a fixed-rate row.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def index_id(self) -> str | None:
        """
        Floating index curve identifier.

        Returns
        -------
        str | None
            ``None`` for a fixed-rate row.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def index_floor(self) -> float | None:
        """
        Floor on the floating index (annual decimal) applied before the spread.

        Returns
        -------
        float | None
            ``None`` when the row is unfloored or fixed-rate.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def maturity(self) -> datetime.date:
        """
        Contractual maturity.

        Returns
        -------
        datetime.date
            The maturity date.

        Raises
        ------
        ValueError
            If the date cannot be converted.
        """
        ...

    @property
    def credit_quality(self) -> str | None:
        """
        Credit rating string.

        Returns
        -------
        str | None
            ``None`` when unrated.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def industry(self) -> str | None:
        """
        Industry label used for concentration reporting.

        Returns
        -------
        str | None
            ``None`` when unset.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def obligor_id(self) -> str | None:
        """
        Obligor identifier.

        Returns
        -------
        str | None
            ``None`` when unset.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def is_defaulted(self) -> bool:
        """
        Whether the row is defaulted.

        Returns
        -------
        bool
            ``True`` for a defaulted row.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def recovery_amount(self) -> Money | None:
        """
        Expected recovery on a defaulted row.

        Returns
        -------
        Money | None
            ``None`` when unset.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def default_date(self) -> datetime.date | None:
        """
        Date the row defaulted, for rows marked defaulted.

        Returns
        -------
        datetime.date | None
            ``None`` when the row is performing.

        Raises
        ------
        ValueError
            If the date cannot be converted.
        """
        ...

    @property
    def purchase_price(self) -> Money | None:
        """
        Price paid for the row, used by the discount-obligation test.

        Returns
        -------
        Money | None
            ``None`` when unset.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def acquisition_date(self) -> datetime.date | None:
        """
        Acquisition date.

        Returns
        -------
        datetime.date | None
            ``None`` when unset.

        Raises
        ------
        ValueError
            If the date cannot be converted.
        """
        ...

    @property
    def origination_date(self) -> datetime.date | None:
        """
        Origination date anchoring the row's seasoning.

        Returns
        -------
        datetime.date | None
            ``None`` when unset (the row then ages from ``acquisition_date``,
            else the closing date).

        Raises
        ------
        ValueError
            If the date cannot be converted.
        """
        ...

    @property
    def day_count(self) -> DayCount:
        """
        Accrual day count.

        Returns
        -------
        DayCount
            The convention.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def smm_override(self) -> float | None:
        """
        Row-level SMM override (decimal).

        Returns
        -------
        float | None
            ``None`` when the deal model applies.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def mdr_override(self) -> float | None:
        """
        Row-level MDR override (decimal).

        Returns
        -------
        float | None
            ``None`` when the deal model applies.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def recovery_rate(self) -> float | None:
        """
        Row-level recovery rate (decimal).

        Returns
        -------
        float | None
            ``None`` when the deal model applies.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def commitment(self) -> Money | None:
        """
        Total commitment of a revolving row.

        Returns
        -------
        Money | None
            ``None`` for term rows.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def contractual_payment(self) -> Money | None:
        """
        Monthly level payment.

        Returns
        -------
        Money | None
            ``None`` when derived from the terms.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def amortization_term_months(self) -> int | None:
        """
        Amortization schedule length in months from origination.

        Returns
        -------
        int | None
            ``None`` when unset.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def io_months(self) -> int | None:
        """
        Interest-only window in months from origination.

        Returns
        -------
        int | None
            ``None`` when unset.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def market_price_pct(self) -> float | None:
        """
        Market price in percent of par.

        Returns
        -------
        float | None
            ``None`` when unset.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def delinquency_buckets(self) -> list[Money] | None:
        """
        Seeded delinquent balances per bucket.

        Returns
        -------
        list[Money] | None
            ``None`` when the row is current.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def balloon(self) -> dict[str, Any] | None:
        """
        ``BalloonSpec`` serde dict.

        Returns
        -------
        dict[str, Any] | None
            ``None`` without balloon terms.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def prepayment_penalty(self) -> dict[str, Any] | None:
        """
        ``PrepaymentPenalty`` serde dict.

        Returns
        -------
        dict[str, Any] | None
            ``None`` without a penalty.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def special_servicing(self) -> dict[str, Any] | None:
        """
        ``SpecialServicingSpec`` serde dict.

        Returns
        -------
        dict[str, Any] | None
            ``None`` when not specially serviced.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def noi(self) -> Money | None:
        """
        Annual net operating income.

        Returns
        -------
        Money | None
            ``None`` when unset.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def liquidation(self) -> dict[str, Any] | None:
        """
        ``LiquidationSpec`` serde dict of a non-performing loan.

        Returns
        -------
        dict[str, Any] | None
            ``None`` for a performing loan.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

class CallAssumption:
    """
    Assumed optional redemption for price-to-call analytics.

    A deal-scope call liquidates the collateral on the first payment date at
    or after ``date`` and redeems every note at ``price_pct`` of its balance;
    a tranche-scope call (``tranche_id`` given) leaves the deal's cashflows
    unchanged and only truncates that class's ``*_to_call`` metrics.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.valuations.instruments import CallAssumption
    >>> call = CallAssumption(datetime.date(2027, 1, 15), 100.0)
    >>> call.scope, call.tranche_id
    ('deal', None)
    >>> CallAssumption(datetime.date(2027, 1, 15), 101.0, tranche_id="B").tranche_id
    'B'
    """

    def __init__(self, date: datetime.date, price_pct: float, tranche_id: str | None = None) -> None:
        """
        Construct a call assumption.

        Parameters
        ----------
        date : datetime.date
            Assumed call date; the redemption happens on the first payment date
            at or after it.
        price_pct : float
            Redemption price as a percent of the note balance (``100.0`` = par;
            a premium is paid as interest, a discount is a write-down).
        tranche_id : str, optional
            Restrict the call to one class (tranche-scope). ``None`` calls the
            whole deal.

        Raises
        ------
        ValueError
            If ``date`` is not a valid date.
        """
        ...

    @staticmethod
    def from_json(json: str) -> CallAssumption:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``CallAssumption`` (the shape ``to_json`` writes).

        Returns
        -------
        CallAssumption
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CallAssumption
        >>> try:
        ...     CallAssumption.from_json("{}")
        ... except ValueError:
        ...     print("rejected")
        rejected
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            JSON-encoded ``CallAssumption``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return every field as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Serde form of the Rust value.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(CallAssumption.from_json, (json,))``.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __repr__(self) -> str:
        """
        Return ``repr(self)``.

        Returns
        -------
        str
            A one-line summary of the value.
        """
        ...

    @property
    def date(self) -> datetime.date:
        """
        Assumed call date.

        Returns
        -------
        datetime.date
            The call date.

        Raises
        ------
        ValueError
            If the date cannot be converted.
        """
        ...

    @property
    def price_pct(self) -> float:
        """
        Redemption price as a percent of balance.

        Returns
        -------
        float
            The call price.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def scope(self) -> str:
        """
        Whether the call covers the whole deal or one class.

        Returns
        -------
        str
            ``"deal"`` or ``"tranche"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def tranche_id(self) -> str | None:
        """
        The called class for a tranche-scope call.

        Returns
        -------
        str | None
            ``None`` for a deal-scope call.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def scope_spec(self) -> Any:
        """
        ``CallScope`` serde value.

        Returns
        -------
        Any
            ``"deal"`` or ``{"tranche": id}``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

class BalloonSpec:
    """
    Balloon terms of a commercial mortgage at maturity.

    At the loan's maturity a share ``extension_prob`` of the performing
    balance is extended ``extension_months`` at ``extension_rate`` (the
    original coupon when ``None``), a share ``loss_prob`` defaults into a
    workout that recovers ``1 − severity_pct / 100`` after ``workout_months``,
    and the rest pays as the balloon.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import BalloonSpec
    >>> spec = BalloonSpec(0.3, 24, extension_rate=0.07, loss_prob=0.1, severity_pct=40.0, workout_months=18)
    >>> spec.extension_prob, spec.workout_months
    (0.3, 18)
    """

    def __init__(
        self,
        extension_prob: float,
        extension_months: int,
        *,
        extension_rate: float | None = None,
        loss_prob: float = 0.0,
        severity_pct: float = 0.0,
        workout_months: int = 0,
    ) -> None:
        """
        Construct balloon terms.

        Parameters
        ----------
        extension_prob : float
            Share of the performing balance extended at maturity, decimal in
            ``[0, 1]``.
        extension_months : int
            Length of the extension in months.
        extension_rate : float, optional
            Annual coupon (decimal) on the extended balance; ``None`` keeps the
            loan's coupon (spread and index for a floater).
        loss_prob : float, optional
            Share of the performing balance that defaults into a workout at
            maturity, decimal in ``[0, 1]``; ``0.0`` by default.
        severity_pct : float, optional
            Loss severity on the workout share in percent (``40.0`` = 40%);
            ``0.0`` by default.
        workout_months : int, optional
            Months after maturity until the workout recovery is received;
            ``0`` by default (the deal's recovery lag).

        Raises
        ------
        ValueError
            If a share is outside ``[0, 1]``, the severity is outside
            ``[0, 100]`` or the rate is not finite.
        """
        ...

    @staticmethod
    def from_json(json: str) -> BalloonSpec:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``BalloonSpec`` (the shape ``to_json`` writes).

        Returns
        -------
        BalloonSpec
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import BalloonSpec
        >>> try:
        ...     BalloonSpec.from_json("{}")
        ... except ValueError:
        ...     print("rejected")
        rejected
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            Canonical JSON.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return every field as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Serde form of the Rust value.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(BalloonSpec.from_json, (json,))``.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __repr__(self) -> str:
        """
        Return ``repr(self)``.

        Returns
        -------
        str
            A one-line summary of the value.
        """
        ...

    @property
    def extension_prob(self) -> float:
        """
        Share of the performing balance extended at maturity.

        Returns
        -------
        float
            Decimal in ``[0, 1]``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def extension_months(self) -> int:
        """
        Length of the balloon extension.

        Returns
        -------
        int
            Number of months the extended share is carried past maturity.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def extension_rate(self) -> float | None:
        """
        Coupon on the extended balance.

        Returns
        -------
        float | None
            Annual decimal, or ``None`` to keep the loan's coupon.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def loss_prob(self) -> float:
        """
        Share of the performing balance that defaults into a workout.

        Returns
        -------
        float
            Decimal in ``[0, 1]``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def severity_pct(self) -> float:
        """
        Loss severity on the workout share.

        Returns
        -------
        float
            Percent of the workout balance.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def workout_months(self) -> int:
        """
        Months after maturity until the workout recovery is received.

        Returns
        -------
        int
            Months; ``0`` uses the deal's recovery lag.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class PrepaymentPenalty:
    """
    Prepayment protection on a commercial mortgage.

    Built through one of the four constructors: :meth:`lockout` blocks
    voluntary prepayment, :meth:`fixed` charges a percent of the prepaid
    balance, :meth:`step_down` charges the step in force from a declining
    schedule and :meth:`yield_maintenance` charges the coupon lost over the
    remaining term. Premiums are collected by the trust as interest.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.valuations.instruments import PrepaymentPenalty
    >>> PrepaymentPenalty.fixed(3.0, through=datetime.date(2026, 1, 1)).kind
    'fixed'
    >>> PrepaymentPenalty.lockout(datetime.date(2025, 12, 31)).through
    datetime.date(2025, 12, 31)
    """

    @staticmethod
    def lockout(through: datetime.date | None = None) -> PrepaymentPenalty:
        """
        A lockout: no voluntary prepayment on or before ``through``.

        Parameters
        ----------
        through : datetime.date, optional
            Last date of the lockout; ``None`` locks the loan out to maturity.

        Returns
        -------
        PrepaymentPenalty
            The lockout.

        Raises
        ------
        ValueError
            If ``through`` is not a valid date.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import PrepaymentPenalty
        >>> PrepaymentPenalty.lockout().kind
        'lockout'
        """
        ...

    @staticmethod
    def fixed(pct: float, through: datetime.date | None = None) -> PrepaymentPenalty:
        """
        A fixed percent of the prepaid balance.

        Parameters
        ----------
        pct : float
            Premium in percent of the prepaid balance (``3.0`` = 3%).
        through : datetime.date, optional
            Last date the penalty applies; ``None`` applies it to maturity.

        Returns
        -------
        PrepaymentPenalty
            The fixed penalty.

        Raises
        ------
        ValueError
            If ``pct`` is negative or ``through`` is not a valid date.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import PrepaymentPenalty
        >>> PrepaymentPenalty.fixed(3.0).pct
        3.0
        """
        ...

    @staticmethod
    def step_down(schedule: list[tuple[datetime.date, float]]) -> PrepaymentPenalty:
        """
        A declining schedule of percents (5-4-3-2-1).

        Parameters
        ----------
        schedule : list[tuple[datetime.date, float]]
            ``(through, pct)`` steps in ascending date order; the first step
            whose date is on or after the prepayment applies, nothing after the
            last.

        Returns
        -------
        PrepaymentPenalty
            The step-down penalty.

        Raises
        ------
        ValueError
            If the schedule is empty, not ascending, or a percent is negative.

        Examples
        --------
        >>> import datetime
        >>> from finstack_quant.valuations.instruments import PrepaymentPenalty
        >>> steps = [(datetime.date(2025, 12, 31), 5.0), (datetime.date(2026, 12, 31), 3.0)]
        >>> len(PrepaymentPenalty.step_down(steps).schedule)
        2
        """
        ...

    @staticmethod
    def yield_maintenance(
        *,
        reinvestment_rate: float | None = None,
        discount_curve_id: str | None = None,
        floor_pct: float | None = None,
        through: datetime.date | None = None,
    ) -> PrepaymentPenalty:
        """
        Yield maintenance: the coupon lost over the remaining term.

        Parameters
        ----------
        reinvestment_rate : float, optional
            Annual reinvestment rate as a decimal; ``None`` uses the curve's
            zero rate to maturity (then ``discount_curve_id`` is required).
        discount_curve_id : str, optional
            Market-context discount curve the annual lost coupons are
            discounted on; ``None`` leaves the premium undiscounted.
        floor_pct : float, optional
            Minimum premium in percent of the prepaid balance (the common
            ``1.0``); ``None`` for no floor.
        through : datetime.date, optional
            Last date the penalty applies; ``None`` applies it to maturity.

        Returns
        -------
        PrepaymentPenalty
            The yield-maintenance penalty.

        Raises
        ------
        ValueError
            If neither a rate nor a curve is given, a rate or floor is
            negative, or ``through`` is not a valid date.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import PrepaymentPenalty
        >>> ym = PrepaymentPenalty.yield_maintenance(discount_curve_id="USD-OIS", floor_pct=1.0)
        >>> ym.kind, ym.floor_pct
        ('yield_maintenance', 1.0)
        """
        ...

    @staticmethod
    def from_json(json: str) -> PrepaymentPenalty:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``PrepaymentPenalty`` (the shape ``to_json`` writes).

        Returns
        -------
        PrepaymentPenalty
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import PrepaymentPenalty
        >>> try:
        ...     PrepaymentPenalty.from_json("{}")
        ... except ValueError:
        ...     print("rejected")
        rejected
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            Canonical JSON.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return every field as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Serde form of the Rust value.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(PrepaymentPenalty.from_json, (json,))``.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __repr__(self) -> str:
        """
        Return ``repr(self)``.

        Returns
        -------
        str
            A one-line summary of the value.
        """
        ...

    @property
    def kind(self) -> str:
        """
        Kind of protection.

        Returns
        -------
        str
            ``"lockout"``, ``"fixed"``, ``"step_down"`` or ``"yield_maintenance"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def through(self) -> datetime.date | None:
        """
        Last date the penalty or lockout applies.

        Returns
        -------
        datetime.date | None
            ``None`` to maturity, or for a step-down schedule.

        Raises
        ------
        ValueError
            If the date cannot be converted.
        """
        ...

    @property
    def pct(self) -> float | None:
        """
        Fixed premium percent.

        Returns
        -------
        float | None
            ``None`` for the other kinds.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def schedule(self) -> list[tuple[datetime.date, float]] | None:
        """
        Step-down schedule.

        Returns
        -------
        list[tuple[datetime.date, float]] | None
            ``(through, pct)`` pairs, or ``None`` for the other kinds.

        Raises
        ------
        ValueError
            If the date cannot be converted.
        """
        ...

    @property
    def reinvestment_rate(self) -> float | None:
        """
        Yield-maintenance reinvestment rate.

        Returns
        -------
        float | None
            Annual decimal, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def discount_curve_id(self) -> str | None:
        """
        Yield-maintenance discount curve identifier.

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
    def floor_pct(self) -> float | None:
        """
        Yield-maintenance floor.

        Returns
        -------
        float | None
            Percent of the prepaid balance, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class SpecialServicingSpec:
    """
    Special-servicing state of a commercial mortgage at closing.

    The appraisal reduction (ASER) cuts the interest advanced on the loan to
    ``1 − appraisal_reduction_pct / 100`` of its balance; the loan is
    specially serviced from closing, so the deal's special servicer fee,
    workout fee and liquidation fee apply to it.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import SpecialServicingSpec
    >>> SpecialServicingSpec(40.0).appraisal_reduction_pct
    40.0
    """

    def __init__(self, appraisal_reduction_pct: float = 0.0) -> None:
        """
        Construct the special-servicing state.

        Parameters
        ----------
        appraisal_reduction_pct : float, optional
            Appraisal reduction in percent of the loan balance (``40.0`` =
            40%); ``0.0`` by default (specially serviced, full advancing).

        Raises
        ------
        ValueError
            If the percent is outside ``[0, 100]``.
        """
        ...

    @staticmethod
    def from_json(json: str) -> SpecialServicingSpec:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``SpecialServicingSpec`` (the shape ``to_json`` writes).

        Returns
        -------
        SpecialServicingSpec
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import SpecialServicingSpec
        >>> try:
        ...     SpecialServicingSpec.from_json("{}")
        ... except ValueError:
        ...     print("rejected")
        rejected
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            Canonical JSON.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return every field as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Serde form of the Rust value.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(SpecialServicingSpec.from_json, (json,))``.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __repr__(self) -> str:
        """
        Return ``repr(self)``.

        Returns
        -------
        str
            A one-line summary of the value.
        """
        ...

    @property
    def appraisal_reduction_pct(self) -> float:
        """
        Appraisal reduction.

        Returns
        -------
        float
            Percent of the loan balance.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class LiquidationSpec:
    """
    Resolution timeline of a non-performing or re-performing loan.

    The loan pays nothing until ``months_to_resolution`` months from its
    origination (acquisition date, else closing); then the share
    ``1 − reperformance_prob`` liquidates at ``proceeds_pct − carry_cost_pct``
    percent of its balance and the rest re-performs at ``modified_rate``.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import LiquidationSpec
    >>> spec = LiquidationSpec(18, 65.0, carry_cost_pct=5.0, reperformance_prob=0.2, modified_rate=0.04)
    >>> spec.months_to_resolution, spec.net_proceeds_fraction
    (18, 0.6)
    """

    def __init__(
        self,
        months_to_resolution: int,
        proceeds_pct: float,
        *,
        carry_cost_pct: float = 0.0,
        reperformance_prob: float = 0.0,
        modified_rate: float | None = None,
    ) -> None:
        """
        Construct the resolution terms.

        Parameters
        ----------
        months_to_resolution : int
            Months from origination until the loan resolves.
        proceeds_pct : float
            Gross liquidation proceeds in percent of the balance.
        carry_cost_pct : float, optional
            Carry and disposition costs in percent of the balance, netted from
            the proceeds; ``0.0`` by default.
        reperformance_prob : float, optional
            Share of the balance that re-performs instead of liquidating,
            decimal in ``[0, 1]``; ``0.0`` by default.
        modified_rate : float, optional
            Annual coupon (decimal) of the re-performing share; ``None`` keeps
            the loan's coupon.

        Raises
        ------
        ValueError
            If a percent is outside ``[0, 100]``, the share is outside
            ``[0, 1]`` or the rate is not finite.
        """
        ...

    @staticmethod
    def from_json(json: str) -> LiquidationSpec:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``LiquidationSpec`` (the shape ``to_json`` writes).

        Returns
        -------
        LiquidationSpec
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import LiquidationSpec
        >>> try:
        ...     LiquidationSpec.from_json("{}")
        ... except ValueError:
        ...     print("rejected")
        rejected
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            Canonical JSON.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return every field as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Serde form of the Rust value.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(LiquidationSpec.from_json, (json,))``.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __repr__(self) -> str:
        """
        Return ``repr(self)``.

        Returns
        -------
        str
            A one-line summary of the value.
        """
        ...

    @property
    def months_to_resolution(self) -> int:
        """
        Months from origination until the loan resolves.

        Returns
        -------
        int
            Number of months after origination on which the liquidation and
            re-performing shares are booked.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def proceeds_pct(self) -> float:
        """
        Gross liquidation proceeds.

        Returns
        -------
        float
            Percent of the balance.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def carry_cost_pct(self) -> float:
        """
        Carry and disposition costs.

        Returns
        -------
        float
            Percent of the balance.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def reperformance_prob(self) -> float:
        """
        Share of the balance that re-performs.

        Returns
        -------
        float
            Decimal in ``[0, 1]``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def modified_rate(self) -> float | None:
        """
        Coupon of the re-performing share.

        Returns
        -------
        float | None
            Annual decimal, or ``None`` to keep the loan's coupon.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def net_proceeds_fraction(self) -> float:
        """
        Net liquidation proceeds as a fraction of the balance.

        Returns
        -------
        float
            ``(proceeds_pct − carry_cost_pct) / 100``, floored at zero.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class EligibilityRule:
    """
    Which collateral an advance rate applies to.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.valuations.instruments import EligibilityRule
    >>> rule = EligibilityRule(max_days_past_due=60, max_maturity=datetime.date(2030, 1, 1))
    >>> rule.exclude_defaulted, rule.max_days_past_due
    (True, 60)
    """

    def __init__(
        self,
        *,
        exclude_defaulted: bool = True,
        max_maturity: datetime.date | None = None,
        max_days_past_due: int | None = None,
        exclude_non_performing: bool = True,
    ) -> None:
        """
        Construct an eligibility rule.

        Parameters
        ----------
        exclude_defaulted : bool, optional
            Exclude defaulted rows; ``True`` by default.
        max_maturity : datetime.date, optional
            Exclude rows maturing after this date; ``None`` for no limit.
        max_days_past_due : int, optional
            Exclude balances more than this many days delinquent (delinquency
            buckets are 30 days each); ``None`` for no limit.
        exclude_non_performing : bool, optional
            Exclude unresolved non-performing loans; ``True`` by default.

        Raises
        ------
        ValueError
            If ``max_maturity`` is not a valid date.
        """
        ...

    @staticmethod
    def from_json(json: str) -> EligibilityRule:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``EligibilityRule`` (the shape ``to_json`` writes).

        Returns
        -------
        EligibilityRule
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import EligibilityRule
        >>> try:
        ...     EligibilityRule.from_json('{"unknown": 1}')
        ... except ValueError:
        ...     print("rejected")
        rejected
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            Canonical JSON.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return every field as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Serde form of the Rust value.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(EligibilityRule.from_json, (json,))``.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __repr__(self) -> str:
        """
        Return ``repr(self)``.

        Returns
        -------
        str
            A one-line summary of the value.
        """
        ...

    @property
    def exclude_defaulted(self) -> bool:
        """
        Whether defaulted rows are excluded.

        Returns
        -------
        bool
            ``True`` when excluded.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def max_maturity(self) -> datetime.date | None:
        """
        Latest eligible maturity.

        Returns
        -------
        datetime.date | None
            ``None`` for no limit.

        Raises
        ------
        ValueError
            If the date cannot be converted.
        """
        ...

    @property
    def max_days_past_due(self) -> int | None:
        """
        Maximum days past due of an eligible balance.

        Returns
        -------
        int | None
            ``None`` for no limit.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def exclude_non_performing(self) -> bool:
        """
        Whether unresolved non-performing loans are excluded.

        Returns
        -------
        bool
            ``True`` when excluded.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class AdvanceRate:
    """
    Advance rate on one asset class of the collateral.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import AdvanceRate, EligibilityRule
    >>> rate = AdvanceRate("commercial_mortgage", 0.8, eligibility=EligibilityRule(max_days_past_due=60))
    >>> rate.asset_class, rate.rate
    ('commercial_mortgage', 0.8)
    """

    def __init__(self, asset_class: str, rate: float, *, eligibility: EligibilityRule | None = None) -> None:
        """
        Construct an advance rate.

        Parameters
        ----------
        asset_class : str
            ``AssetType`` wire name the rate applies to (for example
            ``"commercial_mortgage"`` or ``"leveraged_loan"``).
        rate : float
            Advance rate as a decimal in ``[0, 1]`` (``0.8`` lends 80% of
            eligible balance).
        eligibility : EligibilityRule, optional
            Which rows of the class are eligible; the default rule excludes
            defaulted and non-performing rows.

        Raises
        ------
        ValueError
            If ``rate`` is outside ``[0, 1]`` or not finite.
        """
        ...

    @staticmethod
    def from_json(json: str) -> AdvanceRate:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``AdvanceRate`` (the shape ``to_json`` writes).

        Returns
        -------
        AdvanceRate
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import AdvanceRate
        >>> try:
        ...     AdvanceRate.from_json("{}")
        ... except ValueError:
        ...     print("rejected")
        rejected
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            Canonical JSON.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return every field as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Serde form of the Rust value.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(AdvanceRate.from_json, (json,))``.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __repr__(self) -> str:
        """
        Return ``repr(self)``.

        Returns
        -------
        str
            A one-line summary of the value.
        """
        ...

    @property
    def asset_class(self) -> str:
        """
        Asset class the rate applies to.

        Returns
        -------
        str
            ``AssetType`` wire name.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def rate(self) -> float:
        """
        Advance rate lent against the eligible balance of the class.

        Returns
        -------
        float
            Decimal in ``[0, 1]`` (``0.8`` lends 80% of eligible balance).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def eligibility(self) -> EligibilityRule:
        """
        Eligibility rule of the class.

        Returns
        -------
        EligibilityRule
            The rule.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class ConcentrationLimit:
    """
    Cap on the eligible collateral one obligor, industry or asset class may
    contribute; balance above the cap is excluded from the borrowing base.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import ConcentrationLimit
    >>> ConcentrationLimit("obligor", 5.0).scope
    'obligor'
    """

    def __init__(self, scope: str, max_pct: float) -> None:
        """
        Construct a concentration limit.

        Parameters
        ----------
        scope : str
            ``"obligor"`` (per ``obligor_id``), ``"industry"`` (per
            ``industry``) or ``"asset_class"`` (per asset type).
        max_pct : float
            Maximum share of the eligible collateral in percent (``20.0`` =
            20%).

        Raises
        ------
        ValueError
            If ``scope`` is not one of the three names or ``max_pct`` is
            outside ``[0, 100]``.
        """
        ...

    @staticmethod
    def from_json(json: str) -> ConcentrationLimit:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``ConcentrationLimit`` (the shape ``to_json`` writes).

        Returns
        -------
        ConcentrationLimit
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import ConcentrationLimit
        >>> try:
        ...     ConcentrationLimit.from_json("{}")
        ... except ValueError:
        ...     print("rejected")
        rejected
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            Canonical JSON.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return every field as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Serde form of the Rust value.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(ConcentrationLimit.from_json, (json,))``.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __repr__(self) -> str:
        """
        Return ``repr(self)``.

        Returns
        -------
        str
            A one-line summary of the value.
        """
        ...

    @property
    def scope(self) -> str:
        """
        Dimension the cap is measured on.

        Returns
        -------
        str
            ``"obligor"``, ``"industry"`` or ``"asset_class"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def max_pct(self) -> float:
        """
        Maximum share of the eligible collateral one bucket may contribute.

        Returns
        -------
        float
            Percent of the eligible collateral (``20.0`` = 20%).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class BorrowingBaseRules:
    """
    Advance rates and concentration limits that size a borrowing base.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import AdvanceRate, BorrowingBaseRules, ConcentrationLimit
    >>> rules = BorrowingBaseRules(
    ...     [AdvanceRate("commercial_mortgage", 0.8)],
    ...     concentration_limits=[ConcentrationLimit("obligor", 20.0)],
    ... )
    >>> len(rules.advance_rates), rules.concentration_limits[0].max_pct
    (1, 20.0)
    """

    def __init__(
        self,
        advance_rates: list[AdvanceRate],
        *,
        concentration_limits: list[ConcentrationLimit] | None = None,
    ) -> None:
        """
        Construct borrowing-base rules.

        Parameters
        ----------
        advance_rates : list[AdvanceRate]
            One advance rate per eligible asset class (at least one).
        concentration_limits : list[ConcentrationLimit], optional
            Caps on obligor, industry or asset-class shares; none by default.

        Raises
        ------
        ValueError
            If no advance rate is given, a class is repeated or a rate or
            limit is invalid.
        """
        ...

    @staticmethod
    def from_json(json: str) -> BorrowingBaseRules:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``BorrowingBaseRules`` (the shape ``to_json`` writes).

        Returns
        -------
        BorrowingBaseRules
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import BorrowingBaseRules
        >>> try:
        ...     BorrowingBaseRules.from_json("{}")
        ... except ValueError:
        ...     print("rejected")
        rejected
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            Canonical JSON.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return every field as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Serde form of the Rust value.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(BorrowingBaseRules.from_json, (json,))``.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __repr__(self) -> str:
        """
        Return ``repr(self)``.

        Returns
        -------
        str
            A one-line summary of the value.
        """
        ...

    @property
    def advance_rates(self) -> list[AdvanceRate]:
        """
        Advance rates by asset class.

        Returns
        -------
        list[AdvanceRate]
            One entry per class.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def concentration_limits(self) -> list[ConcentrationLimit]:
        """
        Concentration limits.

        Returns
        -------
        list[ConcentrationLimit]
            Possibly empty.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class CoverageRules:
    """
    Collateral valuation rules for the OC tests: rating haircuts, the value
    carried for defaulted collateral, the excess-CCC bucket and discount
    obligations (CLO indenture conventions). Percentages are percent values
    (``7.5`` = 7.5%); haircuts are decimal fractions.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import CoverageRules
    >>> rules = CoverageRules.clo_standard()
    >>> rules.ccc_bucket["threshold_pct"]
    7.5
    >>> CoverageRules(rating_haircuts={"NR": 0.5}).rating_haircuts
    {'NR': 0.5}
    """

    def __init__(
        self,
        rating_haircuts: dict[str, float] | str | None = None,
        defaulted_valuation: dict[str, Any] | str | None = None,
        ccc_bucket: dict[str, Any] | str | None = None,
        discount_obligation: dict[str, Any] | str | None = None,
    ) -> None:
        """
        Construct rules from their serde parts (every part optional).

        Parameters
        ----------
        rating_haircuts : dict[str, float] | str, optional
            Haircut per rating as a decimal fraction (``{"CCC": 0.7, "NR":
            0.5}``); performing collateral is carried at ``par × (1 − haircut)``.
            Empty when omitted.
        defaulted_valuation : dict[str, Any] | str, optional
            ``DefaultedValuation`` serde value: ``"recovery"`` (modeled recovery,
            the default) or ``{"market_value": {"pct": 60.0}}``.
        ccc_bucket : dict[str, Any] | str, optional
            ``CccBucketRule`` (``threshold_pct``, ``treatment``); ``None``
            disables the bucket.
        discount_obligation : dict[str, Any] | str, optional
            ``DiscountObligationRule`` (``price_threshold_pct``); ``None``
            disables it.

        Raises
        ------
        ValueError
            If a part does not match its serde shape or the rules fail
            validation.
        """
        ...

    @staticmethod
    def clo_standard() -> CoverageRules:
        """
        Standard CLO rules: performing collateral at par (no rating haircuts),
        defaulted collateral at recovery,
        a 7.5% CCC bucket at market value and discount obligations below 80.

        Returns
        -------
        CoverageRules
            The standard rules.

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CoverageRules
        >>> CoverageRules.clo_standard().discount_obligation["price_threshold_pct"]
        80.0
        """
        ...

    def validate(self) -> None:
        """
        Validate haircuts, percentages and thresholds.

        Raises
        ------
        ValueError
            If a haircut is outside ``[0, 1]`` or a percent is out of range.
        """
        ...

    @staticmethod
    def from_json(json: str) -> CoverageRules:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``CoverageRules`` (the shape ``to_json`` writes).

        Returns
        -------
        CoverageRules
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CoverageRules
        >>> CoverageRules.from_json("{}").ccc_bucket is None
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            JSON-encoded ``CoverageRules``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return every field as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Serde form of the Rust value.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(CoverageRules.from_json, (json,))``.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __repr__(self) -> str:
        """
        Return ``repr(self)``.

        Returns
        -------
        str
            A one-line summary of the value.
        """
        ...

    @property
    def rating_haircuts(self) -> dict[str, float]:
        """
        Haircut per rating as decimal fractions.

        Returns
        -------
        dict[str, float]
            Rating string to haircut.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def defaulted_valuation(self) -> Any:
        """
        ``DefaultedValuation`` serde value.

        Returns
        -------
        Any
            ``"recovery"`` or ``{"market_value": {"pct": ...}}``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def ccc_bucket(self) -> dict[str, Any] | None:
        """
        ``CccBucketRule`` serde dict.

        Returns
        -------
        dict[str, Any] | None
            ``None`` when the bucket is disabled.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def discount_obligation(self) -> dict[str, Any] | None:
        """
        ``DiscountObligationRule`` serde dict.

        Returns
        -------
        dict[str, Any] | None
            ``None`` when disabled.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

class HedgeSwap:
    """
    An interest-rate swap settled through the deal waterfall: net receipts
    join interest collections (and the IC test), net payments rank as a
    senior or junior fee.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import HedgeSwap, InterestRateSwap
    >>> swap = InterestRateSwap.example_standard()
    >>> hedge = HedgeSwap(swap, notional="pool_par", priority="senior_fee")
    >>> hedge.priority, hedge.notional
    ('senior_fee', 'pool_par')
    >>> HedgeSwap(swap, notional={"tranche_par": "A"}).notional
    {'tranche_par': 'A'}
    """

    def __init__(
        self,
        swap: InterestRateSwap,
        notional: str | dict[str, Any] | None = None,
        priority: Literal["senior_fee", "junior_fee"] | None = None,
    ) -> None:
        """
        Construct a hedge from a typed swap.

        Parameters
        ----------
        swap : InterestRateSwap
            The swap; its side, legs and market dependencies drive the projected
            net settlements.
        notional : str | dict[str, Any], optional
            ``SwapNotional`` serde value: ``"contractual"`` (the swap's own
            notional, the default), ``"pool_par"`` (balance-tracking on the
            pool) or ``{"tranche_par": "<tranche id>"}`` (balance-tracking on
            one class).
        priority : {"senior_fee", "junior_fee"}, optional
            ``"senior_fee"`` (default: net payments rank with the senior fees)
            or ``"junior_fee"`` (after every note coupon).

        Raises
        ------
        ValueError
            If ``notional`` or ``priority`` is not a recognized value.
        """
        ...

    @staticmethod
    def from_json(json: str) -> HedgeSwap:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``HedgeSwap`` (the shape ``to_json`` writes).

        Returns
        -------
        HedgeSwap
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import HedgeSwap
        >>> try:
        ...     HedgeSwap.from_json("{}")
        ... except ValueError:
        ...     print("rejected")
        rejected
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            JSON-encoded ``HedgeSwap``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return every field as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Serde form of the Rust value.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(HedgeSwap.from_json, (json,))``.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __repr__(self) -> str:
        """
        Return ``repr(self)``.

        Returns
        -------
        str
            A one-line summary of the value.
        """
        ...

    @property
    def swap(self) -> InterestRateSwap:
        """
        The hedged swap.

        Returns
        -------
        InterestRateSwap
            A typed copy of the swap.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def notional(self) -> Any:
        """
        ``SwapNotional`` serde value.

        Returns
        -------
        Any
            ``"contractual"``, ``"pool_par"`` or ``{"tranche_par": id}``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def priority(self) -> str:
        """
        Fee rank of the net payments.

        Returns
        -------
        str
            ``"senior_fee"`` or ``"junior_fee"``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

class Waterfall:
    """
    The deal's priority of payments: ordered tiers (fees, interest, coverage
    tests, principal, residual) with their recipients, allocation mode,
    funding source and the coverage rules the tests use.

    Obtain one from :meth:`StructuredCredit.create_waterfall` (the effective
    template or custom waterfall) or :meth:`Waterfall.from_json`; pass a
    ``Waterfall`` (or its dict) to :meth:`StructuredCreditBuilder.waterfall`
    to run a custom priority of payments.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.dates import DayCount
    >>> from finstack_quant.core.market_data import DiscountCurve, MarketContext
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.valuations.instruments import (
    ...     AssetPool,
    ...     PoolAsset,
    ...     StructuredCredit,
    ...     Tranche,
    ...     TrancheStructure,
    ... )
    >>> as_of, maturity = datetime.date(2024, 1, 15), datetime.date(2031, 1, 15)
    >>> pool = AssetPool("POOL-1", "clo", Currency("USD")).with_assets([
    ...     PoolAsset.fixed_rate_bond("LOAN-1", Money(80_000_000.0, Currency("USD")), 0.07, maturity, DayCount.ACT_360)
    ... ])
    >>> note = (
    ...     Tranche
    ...     .builder()
    ...     .id("A")
    ...     .attachment_point(0.0)
    ...     .detachment_point(100.0)
    ...     .seniority("senior")
    ...     .original_balance(Money(80_000_000.0, Currency("USD")))
    ...     .coupon_fixed(0.05)
    ...     .maturity(maturity)
    ...     .build()
    ... )
    >>> deal = StructuredCredit.new_clo(
    ...     "CLO-1", pool, TrancheStructure([note]), as_of, maturity, "USD-SOFR-DISC", payment_calendar_id="nyse"
    ... )
    >>> market = MarketContext().insert(DiscountCurve.flat("USD-SOFR-DISC", as_of, 0.03))
    >>> waterfall = deal.create_waterfall()
    >>> waterfall.base_currency, len(waterfall.tiers) > 0
    ('USD', True)
    """

    @staticmethod
    def from_json(json: str) -> Waterfall:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``Waterfall`` (the shape ``to_json`` writes).

        Returns
        -------
        Waterfall
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Waterfall
        >>> try:
        ...     Waterfall.from_json("{}")
        ... except ValueError:
        ...     print("rejected")
        rejected
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            JSON-encoded ``Waterfall``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return every field as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Serde form of the Rust value.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(Waterfall.from_json, (json,))``.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __repr__(self) -> str:
        """
        Return ``repr(self)``.

        Returns
        -------
        str
            A one-line summary of the value.
        """
        ...

    def coverage_tests(self) -> list[dict[str, Any]]:
        """
        Every coverage test placed in the waterfall as ``CoverageTestSpec``
        serde dicts, in tier order.

        Returns
        -------
        list[dict[str, Any]]
            One dict per test (``id``, ``tranche_id``, ``kind``,
            ``trigger_level``, ``action`` ...).

        Raises
        ------
        ValueError
            If the specs cannot be serialized.
        """
        ...

    @property
    def base_currency(self) -> str:
        """
        Base ISO-4217 currency code of the waterfall.

        Returns
        -------
        str
            The currency code.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def tiers(self) -> list[dict[str, Any]]:
        """
        Ordered tiers as ``WaterfallTier`` serde dicts.

        Returns
        -------
        list[dict[str, Any]]
            ``id``, ``priority``, ``payment_type``, ``allocation_mode``, ``recipients``, ``tests`` and ``funding`` per tier.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def coverage_rules(self) -> CoverageRules | None:
        """
        Collateral valuation rules attached to the coverage tests.

        Returns
        -------
        CoverageRules | None
            ``None`` when collateral is carried at par.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class TrancheCashflows:
    """
    Projected cashflows of one tranche from a deterministic simulation
    (:meth:`StructuredCredit.tranche_cashflows`'s return value): the total
    flows and their interest, principal, PIK, deferred-interest and
    write-down components, plus the accrual periods behind them.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.dates import DayCount
    >>> from finstack_quant.core.market_data import DiscountCurve, MarketContext
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.valuations.instruments import (
    ...     AssetPool,
    ...     PoolAsset,
    ...     StructuredCredit,
    ...     Tranche,
    ...     TrancheStructure,
    ... )
    >>> as_of, maturity = datetime.date(2024, 1, 15), datetime.date(2031, 1, 15)
    >>> pool = AssetPool("POOL-1", "clo", Currency("USD")).with_assets([
    ...     PoolAsset.fixed_rate_bond("LOAN-1", Money(80_000_000.0, Currency("USD")), 0.07, maturity, DayCount.ACT_360)
    ... ])
    >>> note = (
    ...     Tranche
    ...     .builder()
    ...     .id("A")
    ...     .attachment_point(0.0)
    ...     .detachment_point(100.0)
    ...     .seniority("senior")
    ...     .original_balance(Money(80_000_000.0, Currency("USD")))
    ...     .coupon_fixed(0.05)
    ...     .maturity(maturity)
    ...     .build()
    ... )
    >>> deal = StructuredCredit.new_clo(
    ...     "CLO-1", pool, TrancheStructure([note]), as_of, maturity, "USD-SOFR-DISC", payment_calendar_id="nyse"
    ... )
    >>> market = MarketContext().insert(DiscountCurve.flat("USD-SOFR-DISC", as_of, 0.03))
    >>> flows = deal.tranche_cashflows("A", market, as_of)
    >>> flows.tranche_id, list(flows.to_dataframe().columns)
    ('A', ['date', 'cashflow', 'interest', 'principal', 'pik', 'deferred', 'writedown'])
    """

    @staticmethod
    def from_json(json: str) -> TrancheCashflows:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``TrancheCashflows`` (the shape ``to_json`` writes).

        Returns
        -------
        TrancheCashflows
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import TrancheCashflows
        >>> try:
        ...     TrancheCashflows.from_json("{}")
        ... except ValueError:
        ...     print("rejected")
        rejected
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            JSON-encoded ``TrancheCashflows``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return every field as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Serde form of the Rust value.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(TrancheCashflows.from_json, (json,))``.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __repr__(self) -> str:
        """
        Return ``repr(self)``.

        Returns
        -------
        str
            A one-line summary of the value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        One row per payment date as a pandas ``DataFrame``.

        Columns: ``date`` (ISO 8601 string), ``cashflow`` (total paid),
        ``interest``, ``principal``, ``pik``, ``deferred`` and ``writedown``,
        all in currency units; a component absent on a date is ``0.0``.

        Returns
        -------
        pd.DataFrame
            The projected flows in date order.

        Raises
        ------
        ValueError
            If the rows cannot be serialized.
        """
        ...

    @property
    def tranche_id(self) -> str:
        """
        Tranche identifier.

        Returns
        -------
        str
            The class identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def cashflows(self) -> list[tuple[datetime.date, Money]]:
        """
        Total cash paid per payment date.

        Returns
        -------
        list[tuple[datetime.date, Money]]
            ``(date, amount)`` pairs.

        Raises
        ------
        ValueError
            If a date cannot be converted.
        """
        ...

    @property
    def interest_flows(self) -> list[tuple[datetime.date, Money]]:
        """
        Interest paid per payment date.

        Returns
        -------
        list[tuple[datetime.date, Money]]
            ``(date, amount)`` pairs.

        Raises
        ------
        ValueError
            If a date cannot be converted.
        """
        ...

    @property
    def principal_flows(self) -> list[tuple[datetime.date, Money]]:
        """
        Principal paid per payment date.

        Returns
        -------
        list[tuple[datetime.date, Money]]
            ``(date, amount)`` pairs.

        Raises
        ------
        ValueError
            If a date cannot be converted.
        """
        ...

    @property
    def pik_flows(self) -> list[tuple[datetime.date, Money]]:
        """
        Interest capitalized (PIK) per payment date.

        Returns
        -------
        list[tuple[datetime.date, Money]]
            ``(date, amount)`` pairs.

        Raises
        ------
        ValueError
            If a date cannot be converted.
        """
        ...

    @property
    def deferred_flows(self) -> list[tuple[datetime.date, Money]]:
        """
        Interest deferred per payment date.

        Returns
        -------
        list[tuple[datetime.date, Money]]
            ``(date, amount)`` pairs.

        Raises
        ------
        ValueError
            If a date cannot be converted.
        """
        ...

    @property
    def writedown_flows(self) -> list[tuple[datetime.date, Money]]:
        """
        Principal written down per payment date.

        Returns
        -------
        list[tuple[datetime.date, Money]]
            ``(date, amount)`` pairs.

        Raises
        ------
        ValueError
            If a date cannot be converted.
        """
        ...

    @property
    def accrual_periods(self) -> list[dict[str, Any]]:
        """
        Accrual periods as ``TrancheAccrualPeriod`` serde dicts.

        Returns
        -------
        list[dict[str, Any]]
            One dict per accrual period.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def detailed_flows(self) -> list[dict[str, Any]]:
        """
        Detailed classified flows as ``CashFlow`` serde dicts.

        Returns
        -------
        list[dict[str, Any]]
            One dict per flow.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def final_balance(self) -> Money:
        """
        Balance outstanding after the last projected payment.

        Returns
        -------
        Money
            The final balance.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def total_interest(self) -> Money:
        """
        Total interest paid over the projection.

        Returns
        -------
        Money
            The interest total.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def total_principal(self) -> Money:
        """
        Total principal paid over the projection.

        Returns
        -------
        Money
            The principal total.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def total_pik(self) -> Money:
        """
        Total interest capitalized over the projection.

        Returns
        -------
        Money
            The PIK total.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def total_deferred(self) -> Money:
        """
        Total interest deferred as a claim over the projection.

        Returns
        -------
        Money
            The deferred total.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def total_writedown(self) -> Money:
        """
        Total principal written down over the projection.

        Returns
        -------
        Money
            The write-down total.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class EquityMetrics:
    """
    Residual-class return analytics of one deterministic projection
    (:meth:`StructuredCredit.equity_metrics`'s return value): the XIRR of the
    invested amount against every projected equity distribution, the
    multiple on invested capital, the NAV and the cash-on-cash series.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import EquityMetrics
    >>> metrics = EquityMetrics.from_json(
    ...     '{"tranche_id": "E", "currency": "USD", "invested": 100.0, "irr": 0.12,'
    ...     ' "moic": 1.5, "nav_pct": 90.0, "cash_on_cash": [["2025-01-15", 0.1]]}'
    ... )
    >>> metrics.moic, list(metrics.to_dataframe().columns)
    (1.5, ['date', 'cash_on_cash'])
    """

    @staticmethod
    def from_json(json: str) -> EquityMetrics:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``EquityMetrics`` (the shape ``to_json`` writes).

        Returns
        -------
        EquityMetrics
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import EquityMetrics
        >>> try:
        ...     EquityMetrics.from_json("{}")
        ... except ValueError:
        ...     print("rejected")
        rejected
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            JSON-encoded ``EquityMetrics``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return every field as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Serde form of the Rust value.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(EquityMetrics.from_json, (json,))``.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __repr__(self) -> str:
        """
        Return ``repr(self)``.

        Returns
        -------
        str
            A one-line summary of the value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Cash-on-cash series as a pandas ``DataFrame``.

        Columns: ``date`` (ISO 8601 string) and ``cash_on_cash`` (decimal
        share of the invested amount distributed on that date).

        Returns
        -------
        pd.DataFrame
            One row per distribution date.

        Raises
        ------
        ValueError
            If the rows cannot be serialized.
        """
        ...

    @property
    def tranche_id(self) -> str:
        """
        Identifier of the residual class.

        Returns
        -------
        str
            The class identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def currency(self) -> str:
        """
        ISO-4217 code of the currency ``invested`` is denominated in.

        Returns
        -------
        str
            The currency code.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def invested(self) -> float:
        """
        Amount invested on the valuation date (balance × purchase price).

        Returns
        -------
        float
            The amount in currency units.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def irr(self) -> float | None:
        """
        Annualized XIRR of the equity flows as a decimal.

        Returns
        -------
        float | None
            ``None`` when no rate solves (for example no positive distribution).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def moic(self) -> float:
        """
        Multiple on invested capital: total distributions over ``invested``.

        Returns
        -------
        float
            The multiple.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def nav_pct(self) -> float:
        """
        Discounted value of the distributions as a percent of the invested balance.

        Returns
        -------
        float
            The NAV percent.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def cash_on_cash(self) -> list[tuple[datetime.date, float]]:
        """
        Per-distribution cash-on-cash yield (distribution over ``invested``).

        Returns
        -------
        list[tuple[datetime.date, float]]
            ``(date, yield)`` pairs.

        Raises
        ------
        ValueError
            If a date cannot be converted.
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
        ``deal_metadata``, ``behavior_overrides``,
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
        payment_calendar_id: str | None = None,
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
        payment_calendar_id : str, optional
            Holiday calendar for the payment schedule (e.g. ``"nyse"``);
            required before pricing, so pass it here or set it on the JSON.

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
        ...         asset_type={"type": "first_lien_loan", "industry": None},
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
        payment_calendar_id: str | None = None,
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
        payment_calendar_id : str, optional
            Holiday calendar for the payment schedule (e.g. ``"nyse"``);
            required before pricing, so pass it here or set it on the JSON.

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
        payment_calendar_id: str | None = None,
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
        payment_calendar_id : str, optional
            Holiday calendar for the payment schedule (e.g. ``"nyse"``);
            required before pricing, so pass it here or set it on the JSON.

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
        payment_calendar_id: str | None = None,
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
        payment_calendar_id : str, optional
            Holiday calendar for the payment schedule (e.g. ``"nyse"``);
            required before pricing, so pass it here or set it on the JSON.

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
            Pool and note balances represent current state after past losses.
            Reinvestment configuration belongs to ``pool.reinvestment_period``
            (notes are held flat inside the window unless listed in its
            ``amortizing_tranches``). Tranche coverage triggers carry
            breach/cure ratios and executable cash or reinvestment actions.

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
    def quote_settlement_date(self) -> datetime.date | None:
        """Buyer settlement date for note price and spread calculations.

        Returns
        -------
        datetime.date or None
            Settlement date; ``None`` uses the valuation date. Payments on or
            before settlement belong to the seller. Model PV stays at valuation.

        Notes
        -----
        This accessor does not raise; it returns the configured date.
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

    def with_standard_fees(self) -> StructuredCredit:
        """
        Return a copy carrying the deal-type standard fee schedule.

        Returns
        -------
        StructuredCredit
            A new deal with ``fees`` set to the CLO / CMBS / RMBS / ABS
            standard (this deal is unchanged).

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from tests.tests_typed_helpers import build_structured_credit
        >>> build_structured_credit().with_standard_fees().fees is not None
        True
        """
        ...

    def enable_stochastic_defaults(self) -> StructuredCredit:
        """
        Return a copy with the deal-type stochastic prepayment, default and
        correlation specifications enabled for :meth:`price_stochastic`.

        Returns
        -------
        StructuredCredit
            A new deal with the stochastic specs attached.

        Raises
        ------
        ValueError
            If the deal-type defaults cannot be built (for example an empty
            pool for the RMBS coupon-driven incentive).
        """
        ...

    def create_waterfall(self) -> Waterfall:
        """
        Effective priority of payments: the custom waterfall when one is set,
        otherwise the deal-type template with fees, coverage tests and hedges
        placed.

        Returns
        -------
        Waterfall
            The waterfall the deterministic engine executes.

        Raises
        ------
        ValueError
            If the template cannot be synthesized (invalid tranches, tests or
            fees).
        """
        ...

    def tranche_cashflows(self, tranche_id: str, market: MarketContext, as_of: datetime.date) -> TrancheCashflows:
        """
        Project one tranche's cashflows through the deterministic engine.

        Parameters
        ----------
        tranche_id : str
            Identifier of the class.
        market : MarketContext
            Curves and fixings for floating coupons and collateral.
        as_of : datetime.date
            Valuation date the projection starts from.

        Returns
        -------
        TrancheCashflows
            The class's projected flows and components.

        Raises
        ------
        KeyError
            If ``tranche_id`` is not a class of the deal.
        ValueError
            If the deal fails pricing validation, ``as_of`` is invalid or
            required market data is missing.
        """
        ...

    def equity_metrics(
        self, market: MarketContext, as_of: datetime.date, purchase_price_pct: float | None = None
    ) -> EquityMetrics:
        """
        Residual-class return analytics of the deterministic projection.

        Parameters
        ----------
        market : MarketContext
            Curves and fixings for the projection and the NAV discounting.
        as_of : datetime.date
            Valuation date; the invested amount is dated here.
        purchase_price_pct : float, optional
            Entry price as a percent of the equity balance; par when omitted.

        Returns
        -------
        EquityMetrics
            IRR, MOIC, NAV and cash-on-cash series of the residual class.

        Raises
        ------
        ValueError
            If the deal has no residual class, fails pricing validation, or
            ``as_of`` is invalid.
        """
        ...

    @property
    def frequency(self) -> Tenor:
        """
        Payment frequency.

        Returns
        -------
        Tenor
            The tranche payment tenor.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def payment_calendar_id(self) -> str | None:
        """
        Payment calendar identifier.

        Returns
        -------
        str | None
            ``None`` when no calendar is set.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def payment_business_day_convention(self) -> str | None:
        """
        Payment business-day convention string.

        Returns
        -------
        str | None
            ``None`` for the default convention.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def credit_model(self) -> dict[str, Any]:
        """
        Credit model as its ``CreditModelConfig`` serde ``dict``.

        Returns
        -------
        dict[str, Any]
            Prepayment, default, recovery, stochastic, delinquency and card specs.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def market_conditions(self) -> dict[str, Any]:
        """
        Market conditions as their serde ``dict``.

        Returns
        -------
        dict[str, Any]
            ``MarketConditions`` fields.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def credit_factors(self) -> dict[str, Any]:
        """
        Credit factors as their serde ``dict``.

        Returns
        -------
        dict[str, Any]
            ``CreditFactors`` fields (``annual_noi`` ...).

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def deal_metadata(self) -> dict[str, Any]:
        """
        Deal metadata as its serde ``dict``.

        Returns
        -------
        dict[str, Any]
            ``Metadata`` fields.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def behavior_overrides(self) -> dict[str, Any]:
        """
        Behavioural overrides as their serde ``dict``.

        Returns
        -------
        dict[str, Any]
            ``Overrides`` fields.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def hedge_swaps(self) -> list[HedgeSwap]:
        """
        Hedges settled through the waterfall.

        Returns
        -------
        list[HedgeSwap]
            One typed :class:`HedgeSwap` per hedge (empty when unhedged).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def fees(self) -> dict[str, Any] | None:
        """
        Senior transaction fees as their ``DealFees`` serde ``dict``.

        Returns
        -------
        dict[str, Any] | None
            ``None`` when no fee tier is attached.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def coverage_triggers(self) -> list[dict[str, Any]]:
        """
        Deal-level coverage tests as ``CoverageTestSpec`` serde dicts.

        Returns
        -------
        list[dict[str, Any]]
            One dict per test (empty when none run).

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def cleanup_call_pct(self) -> float | None:
        """
        Clean-up call pool-factor threshold (decimal).

        Returns
        -------
        float | None
            ``None`` when no clean-up call is set.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def call_assumption(self) -> CallAssumption | None:
        """
        Assumed optional redemption.

        Returns
        -------
        CallAssumption | None
            ``None`` without a call assumption.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def liquidation_price_pct(self) -> float | None:
        """
        Collateral liquidation price in percent of par.

        Returns
        -------
        float | None
            ``None`` for par.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def loss_allocation(self) -> str | None:
        """
        Explicit loss-allocation policy (``"write_down"`` / ``"par_preserving"``).

        Returns
        -------
        str | None
            ``None`` for the deal-type default.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def loss_recognition(self) -> str | None:
        """
        Explicit loss-recognition timing (``"at_default"`` / ``"at_liquidation"``).

        Returns
        -------
        str | None
            ``None`` for the deal-type default (at liquidation for RMBS/CMBS,
            at default otherwise).

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def tranche_draws(self) -> list[dict[str, Any]]:
        """
        Scheduled lender draws on notes as ``TrancheDraw`` serde dicts (``tranche_id``, ``date``, ``amount``).

        Returns
        -------
        list[dict[str, Any]]
            Empty for a fully funded structure.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def tranche_readvance(self) -> dict[str, Any] | None:
        """
        Per-period re-advance rule as its ``TrancheReadvance`` serde dict (``tranche_id``, ``commitment``).

        Returns
        -------
        dict[str, Any] | None
            ``None`` for no re-advances.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def principal_covers_senior_interest(self) -> bool | None:
        """
        Explicit principal-covers-senior-interest flag.

        Returns
        -------
        bool | None
            ``None`` for the deal-type default.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def coverage_rules(self) -> CoverageRules | None:
        """
        Collateral valuation rules for the coverage tests.

        Returns
        -------
        CoverageRules | None
            ``None`` when performing collateral is carried at par.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def waterfall_rules(self) -> dict[str, Any] | None:
        """
        Declarative waterfall rules as their serde ``dict``.

        Returns
        -------
        dict[str, Any] | None
            ``None`` when no rules are layered on the base waterfall.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def waterfall(self) -> Waterfall | None:
        """
        Custom priority of payments.

        Returns
        -------
        Waterfall | None
            ``None`` when the deal-type template applies.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def attributes(self) -> Attributes:
        """
        Free-form attributes (tags and metadata).

        Returns
        -------
        Attributes
            The attribute bag.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def prepayment_spec(self) -> PrepaymentModelSpec:
        """
        Deterministic prepayment model.

        Returns
        -------
        PrepaymentModelSpec
            A typed copy of the spec.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def default_spec(self) -> DefaultModelSpec:
        """
        Deterministic default model.

        Returns
        -------
        DefaultModelSpec
            A typed copy of the spec.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def recovery_spec(self) -> RecoveryModelSpec:
        """
        Recovery rate and lag model for defaulted collateral.

        Returns
        -------
        RecoveryModelSpec
            A typed copy of the spec.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def stochastic_prepay_spec(self) -> dict[str, Any] | None:
        """
        Stochastic prepayment specification as its serde ``dict``.

        Returns
        -------
        dict[str, Any] | None
            ``None`` until set or enabled.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def stochastic_default_spec(self) -> dict[str, Any] | None:
        """
        Stochastic default specification as its serde ``dict``.

        Returns
        -------
        dict[str, Any] | None
            ``None`` until set or enabled.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def stochastic_recovery_spec(self) -> dict[str, Any] | None:
        """
        Stochastic recovery specification as its ``RecoverySpec`` serde ``dict``.

        Returns
        -------
        dict[str, Any] | None
            ``{"type": "constant", "rate": ...}`` or ``{"type":
            "market_correlated", "mean_recovery": ..., "recovery_volatility":
            ..., "factor_correlation": ...}``; ``None`` for constant recoveries
            at the deterministic rate.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def correlation_structure(self) -> dict[str, Any] | None:
        """
        Default correlation structure as its serde ``dict``.

        Returns
        -------
        dict[str, Any] | None
            ``None`` until set or enabled.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def delinquency(self) -> dict[str, Any] | None:
        """
        Delinquency model as its ``DelinquencyModel`` serde ``dict``.

        Returns
        -------
        dict[str, Any] | None
            ``None`` when no roll-rate model is set.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def card(self) -> dict[str, Any] | None:
        """
        Card portfolio model as its ``CardPortfolioSpec`` serde ``dict``.

        Returns
        -------
        dict[str, Any] | None
            ``None`` for non-card deals.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
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
    def price_stochastic(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
        num_paths: int | None = None,
        antithetic: bool = True,
    ) -> StochasticPricingResult:
        """
        Price the deal with the scenario-waterfall Monte Carlo engine.

        Every path runs the full period loop and waterfall on simulated
        prepayment, default and recovery paths (and, for pools of real
        instruments, per-name defaults and simulated revolver draws).

        Parameters
        ----------
        market : MarketContext | str
            Market context with the deal's discount curve and every curve the
            collateral references.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date (ISO 8601 strings accepted).
        num_paths : int, optional
            Number of Monte Carlo paths; defaults to the deal's configured
            ``mc_paths`` override or 10,000.
        antithetic : bool, default True
            Use antithetic variates (pairs share random numbers).

        Returns
        -------
        StochasticPricingResult
            Deal and tranche present values, loss statistics, Monte Carlo
            error, draw diagnostics and the draw option cost.

        Raises
        ------
        ValueError
            If the deal fails validation or ``num_paths`` is zero.
        KeyError
            If a required curve is missing from ``market``.
        RuntimeError
            If the simulation fails.
        """
        ...
    def run_simulation_with_diagnostics(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
    ) -> SimulationDiagnostics:
        """
        Run the deterministic simulation and return the deal-level accounting.

        Parameters
        ----------
        market : MarketContext | str
            Market context with the deal's discount curve and every curve the
            collateral references.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date (ISO 8601 strings accepted).

        Returns
        -------
        SimulationDiagnostics
            Reserve balance and interest per period, draw funding by source
            and unfunded draws.

        Raises
        ------
        ValueError
            If the deal fails validation or (for instrument collateral) the
            contractual draw calendar cannot be funded.
        KeyError
            If a required curve is missing from ``market``.
        RuntimeError
            If the simulation fails.
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

    def quote_settlement_date(
        self, value: datetime.date | datetime.datetime | pd.Timestamp | str
    ) -> StructuredCreditBuilder:
        """Set buyer settlement for prices, yields and spreads.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pandas.Timestamp | str
            Settlement on or after valuation and closing. Payments on or before
            this date are excluded. Omission uses the valuation date.

        Returns
        -------
        StructuredCreditBuilder
            This builder for further configuration.

        Raises
        ------
        ValueError
            If the date is invalid or the builder has already been consumed.
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
            ``MarketConditions`` object with finite annual decimal ``refi_rate``
            for Richard-Roll refinancing incentives. Negative rates are accepted.
            This replaces the registry default; unknown macro-factor fields fail.

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
            ``CreditFactors`` object with optional ``annual_noi`` and
            ``annual_debt_service`` Money values for CMBS coverage metrics.
            Missing values remain absent; unknown macro-factor fields fail.

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
            ahead of every note. Optional ``workout_fee_pct`` (percent of the
            P&I collected on specially serviced loans) and
            ``liquidation_fee_pct`` (percent of liquidation proceeds) are
            taken inside the collateral flows. Skipped (``None``) by default.

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

    def credit_model(self, value: dict[str, Any] | str) -> StructuredCreditBuilder:
        """
        Replace the whole credit model (prepayment, default, recovery,
        stochastic and correlation specs, delinquency and card models).

        Parameters
        ----------
        value : dict[str, Any] | str
            ``CreditModelConfig`` serde object. Later per-field setters
            (:meth:`prepayment_spec` ...) modify this model.
        Returns
        -------
        StructuredCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` does not match the expected shape or this builder
            was already consumed by :meth:`StructuredCreditBuilder.build`.
        """
        ...

    def prepayment_spec(self, value: PrepaymentModelSpec | dict[str, Any] | str) -> StructuredCreditBuilder:
        """
        Set the deterministic prepayment model.

        Parameters
        ----------
        value : PrepaymentModelSpec | dict[str, Any] | str
            Typed spec (``PrepaymentModelSpec.constant_cpr`` / ``psa`` / ``abs`` /
            ``vector`` / ``cmbs_with_lockout``) or its serde form.
        Returns
        -------
        StructuredCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` does not match the expected shape or this builder
            was already consumed by :meth:`StructuredCreditBuilder.build`.
        """
        ...

    def default_spec(self, value: DefaultModelSpec | dict[str, Any] | str) -> StructuredCreditBuilder:
        """
        Set the deterministic default model.

        Parameters
        ----------
        value : DefaultModelSpec | dict[str, Any] | str
            Typed spec (``DefaultModelSpec.constant_cdr`` / ``sda`` / ``vector`` /
            ``cumulative_loss`` / ``timing``) or its serde form.
        Returns
        -------
        StructuredCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` does not match the expected shape or this builder
            was already consumed by :meth:`StructuredCreditBuilder.build`.
        """
        ...

    def recovery_spec(self, value: RecoveryModelSpec | dict[str, Any] | str) -> StructuredCreditBuilder:
        """
        Set the recovery model.

        Parameters
        ----------
        value : RecoveryModelSpec | dict[str, Any] | str
            Typed spec (rate, lag, optional severity vector) or its serde form.
        Returns
        -------
        StructuredCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` does not match the expected shape or this builder
            was already consumed by :meth:`StructuredCreditBuilder.build`.
        """
        ...

    def stochastic_prepay_spec(self, value: dict[str, Any] | str) -> StructuredCreditBuilder:
        """
        Set the stochastic prepayment specification used by
        :meth:`StructuredCredit.price_stochastic`.

        Parameters
        ----------
        value : dict[str, Any] | str
            ``StochasticPrepaySpec`` serde object (Richard-Roll or factor
            parameters).
        Returns
        -------
        StructuredCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` does not match the expected shape or this builder
            was already consumed by :meth:`StructuredCreditBuilder.build`.
        """
        ...

    def stochastic_default_spec(self, value: dict[str, Any] | str) -> StructuredCreditBuilder:
        """
        Set the stochastic default specification used by
        :meth:`StructuredCredit.price_stochastic`.

        Parameters
        ----------
        value : dict[str, Any] | str
            ``StochasticDefaultSpec`` serde object.
        Returns
        -------
        StructuredCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` does not match the expected shape or this builder
            was already consumed by :meth:`StructuredCreditBuilder.build`.
        """
        ...

    def stochastic_recovery_spec(self, value: dict[str, Any] | str) -> StructuredCreditBuilder:
        """
        Set the stochastic recovery specification used by
        :meth:`StructuredCredit.price_stochastic`.

        Parameters
        ----------
        value : dict[str, Any] | str
            ``RecoverySpec`` serde object: ``{"type": "constant", "rate": 0.4}``
            or ``{"type": "market_correlated", "mean_recovery": 0.4,
            "recovery_volatility": 0.25, "factor_correlation": 0.4}`` (recovery
            falls with the systematic factor, so heavy-default paths recover
            less).

        Returns
        -------
        StructuredCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` does not match the ``RecoverySpec`` shape or this
            builder was already consumed by :meth:`StructuredCreditBuilder.build`.
        """
        ...

    def correlation_structure(self, value: dict[str, Any] | str) -> StructuredCreditBuilder:
        """
        Set the default correlation structure used by
        :meth:`StructuredCredit.price_stochastic`.

        Parameters
        ----------
        value : dict[str, Any] | str
            ``CorrelationStructure`` serde object (factor loadings).
        Returns
        -------
        StructuredCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` does not match the expected shape or this builder
            was already consumed by :meth:`StructuredCreditBuilder.build`.
        """
        ...

    def delinquency(self, value: dict[str, Any] | str) -> StructuredCreditBuilder:
        """
        Set the delinquency roll-rate, advancing and modification model
        (ABS/RMBS asset and rep-line pools).

        Parameters
        ----------
        value : dict[str, Any] | str
            ``DelinquencyModel`` serde object: ``roll_rates`` (per bucket, the
            last rolls to charge-off), ``cure_rates``, ``advancing``
            (``{"policy": "none"}`` or ``{"policy": "principal_and_interest",
            "recoverability_cap_pct": ..., "reimburse_from_collections":
            false}``) and optional ``modification``.
        Returns
        -------
        StructuredCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` does not match the expected shape or this builder
            was already consumed by :meth:`StructuredCreditBuilder.build`.
        """
        ...

    def card(self, value: dict[str, Any] | str) -> StructuredCreditBuilder:
        """
        Set the card master-trust portfolio model.

        Parameters
        ----------
        value : dict[str, Any] | str
            ``CardPortfolioSpec`` serde object: ``monthly_payment_rate``,
            ``portfolio_yield`` and ``charge_off_rate`` (annual decimals),
            plus the optional ``seller_interest`` (``Money`` serde object in
            the pool currency) and ``fixed_allocation_pct`` (decimal in
            ``(0, 1]``) that fix the investor allocation of trust collections
            once the revolving period ends.
        Returns
        -------
        StructuredCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` does not match the expected shape or this builder
            was already consumed by :meth:`StructuredCreditBuilder.build`.
        """
        ...

    def behavior_overrides(self, value: dict[str, Any] | str) -> StructuredCreditBuilder:
        """
        Set behavioural assumption overrides.

        Parameters
        ----------
        value : dict[str, Any] | str
            ``Overrides`` serde object (``cpr_annual``, ``psa_speed_multiplier``,
            ``cdr_annual``, ``sda_speed_multiplier``, ``recovery_rate``,
            ``recovery_lag_months``, ``reinvestment_price`` ...).
        Returns
        -------
        StructuredCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` does not match the expected shape or this builder
            was already consumed by :meth:`StructuredCreditBuilder.build`.
        """
        ...

    def coverage_triggers(self, value: list[dict[str, Any]] | str) -> StructuredCreditBuilder:
        """
        Set the deal-level OC / IC coverage tests.

        Parameters
        ----------
        value : list[dict[str, Any]] | str
            ``CoverageTestSpec`` objects (``id``, ``tranche_id``, ``kind``
            (``"oc"`` / ``"ic"``), ``trigger_level`` ratio, ``action``, optional
            ``placement`` (``{"kind": "after_tranche", "tranche_id": ...}`` or
            ``{"kind": "after_junior_fees"}``) and ``divert_pct``).
        Returns
        -------
        StructuredCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` does not match the expected shape or this builder
            was already consumed by :meth:`StructuredCreditBuilder.build`.
        """
        ...

    def coverage_rules(self, value: CoverageRules | dict[str, Any] | str) -> StructuredCreditBuilder:
        """
        Set the collateral valuation rules for the coverage tests.

        Parameters
        ----------
        value : CoverageRules | dict[str, Any] | str
            Typed :class:`CoverageRules` or its serde form.
        Returns
        -------
        StructuredCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` does not match the expected shape or this builder
            was already consumed by :meth:`StructuredCreditBuilder.build`.
        """
        ...

    def call_assumption(self, value: CallAssumption | dict[str, Any] | str) -> StructuredCreditBuilder:
        """
        Set the assumed optional redemption for price-to-call analytics.

        Parameters
        ----------
        value : CallAssumption | dict[str, Any] | str
            Typed :class:`CallAssumption` or its serde form (``date``,
            ``price_pct``, ``scope``).
        Returns
        -------
        StructuredCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` does not match the expected shape or this builder
            was already consumed by :meth:`StructuredCreditBuilder.build`.
        """
        ...

    def waterfall(self, value: Waterfall | dict[str, Any] | str) -> StructuredCreditBuilder:
        """
        Set a custom priority of payments in place of the deal-type template.

        Parameters
        ----------
        value : Waterfall | dict[str, Any] | str
            Typed :class:`Waterfall` or its serde form (``tiers``,
            ``base_currency``, optional ``coverage_rules``).
        Returns
        -------
        StructuredCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` does not match the expected shape or this builder
            was already consumed by :meth:`StructuredCreditBuilder.build`.
        """
        ...

    def hedge_swaps(self, value: list[HedgeSwap | dict[str, Any]] | str) -> StructuredCreditBuilder:
        """
        Set the interest-rate hedges settled through the waterfall.

        Parameters
        ----------
        value : list[HedgeSwap | dict[str, Any]] | str
            Typed :class:`HedgeSwap` objects, their serde dicts, or a JSON
            array string.
        Returns
        -------
        StructuredCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` does not match the expected shape or this builder
            was already consumed by :meth:`StructuredCreditBuilder.build`.
        """
        ...

    def cleanup_call_pct(self, value: float) -> StructuredCreditBuilder:
        """
        Set the clean-up call pool-factor threshold.

        Parameters
        ----------
        value : float
            Pool factor (decimal in ``(0, 1)``, typically ``0.10``) below which
            the deal is redeemed when the liquidation proceeds cover the notes.
        Returns
        -------
        StructuredCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` does not match the expected shape or this builder
            was already consumed by :meth:`StructuredCreditBuilder.build`.
        """
        ...

    def liquidation_price_pct(self, value: float) -> StructuredCreditBuilder:
        """
        Set the collateral liquidation price used by deal calls and clean-up
        calls.

        Parameters
        ----------
        value : float
            Percent of par the collateral realizes (``100.0`` = par).
        Returns
        -------
        StructuredCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` does not match the expected shape or this builder
            was already consumed by :meth:`StructuredCreditBuilder.build`.
        """
        ...

    def loss_allocation(self, value: Literal["write_down", "par_preserving"]) -> StructuredCreditBuilder:
        """
        Set how collateral losses reach the note balances.

        Parameters
        ----------
        value : {"write_down", "par_preserving"}
            ``"write_down"`` allocates realized losses junior-first at default
            (RMBS/CMBS convention); ``"par_preserving"`` keeps note balances at
            par and realizes shortfalls at legal final (CLO/ABS convention).
            The deal type's default applies when never set.
        Returns
        -------
        StructuredCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` does not match the expected shape or this builder
            was already consumed by :meth:`StructuredCreditBuilder.build`.
        """
        ...

    def loss_recognition(self, value: Literal["at_default", "at_liquidation"]) -> StructuredCreditBuilder:
        """
        Set when collateral losses are booked.

        Parameters
        ----------
        value : {"at_default", "at_liquidation"}
            ``"at_default"`` books the expected net loss on the default date
            (CLO/ABS convention); ``"at_liquidation"`` books the realized loss
            when the claim settles after the recovery lag (RMBS/CMBS
            convention), which delays write-downs and every cumulative-loss
            trigger. The deal type's default applies when never set.
        Returns
        -------
        StructuredCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` does not match the expected shape or this builder
            was already consumed by :meth:`StructuredCreditBuilder.build`.
        """
        ...

    def tranche_draws(self, value: list[dict[str, Any]] | str) -> StructuredCreditBuilder:
        """
        Set the scheduled lender draws on notes after closing.

        Parameters
        ----------
        value : list[dict[str, Any]] | str
            ``TrancheDraw`` serde objects ``{"tranche_id": "A", "date":
            "2025-01-01", "amount": {"amount": "5000000", "currency": "USD"}}``,
            ascending by date; each is applied on the first payment date at or
            after its date, lifting the note's balance and adding the cash to
            principal proceeds.

        Returns
        -------
        StructuredCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` does not match the ``TrancheDraw`` shape or this
            builder was already consumed by :meth:`StructuredCreditBuilder.build`.
        """
        ...

    def tranche_readvance(self, value: dict[str, Any] | str) -> StructuredCreditBuilder:
        """
        Set the per-period re-advance of one note up to its commitment and the
        borrowing base while the deal revolves.

        Parameters
        ----------
        value : dict[str, Any] | str
            ``TrancheReadvance`` serde object ``{"tranche_id": "A",
            "commitment": {"amount": "70000000", "currency": "USD"}}``; requires
            ``coverage_rules.borrowing_base``.

        Returns
        -------
        StructuredCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` does not match the ``TrancheReadvance`` shape or this
            builder was already consumed by :meth:`StructuredCreditBuilder.build`.
        """
        ...

    def principal_covers_senior_interest(self, value: bool) -> StructuredCreditBuilder:
        """
        Set whether principal proceeds cover senior fees and senior interest
        shortfalls before any note is redeemed.

        Parameters
        ----------
        value : bool
            ``True`` for the CLO principal-waterfall convention, ``False`` for
            strictly separate accounts. The deal type's default applies when
            never set.
        Returns
        -------
        StructuredCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` does not match the expected shape or this builder
            was already consumed by :meth:`StructuredCreditBuilder.build`.
        """
        ...

    def deal_metadata(self, value: dict[str, Any] | str) -> StructuredCreditBuilder:
        """
        Set deal metadata (counterparties, identifiers).

        Parameters
        ----------
        value : dict[str, Any] | str
            ``Metadata`` serde object.
        Returns
        -------
        StructuredCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` does not match the expected shape or this builder
            was already consumed by :meth:`StructuredCreditBuilder.build`.
        """
        ...

    def attributes(self, value: Attributes | dict[str, str]) -> StructuredCreditBuilder:
        """
        Set free-form attributes (tags and metadata) on the deal.

        Parameters
        ----------
        value : Attributes | dict[str, str]
            Attribute bag; a dict populates ``meta`` (an optional ``"tags"`` list
            populates ``tags``).
        Returns
        -------
        StructuredCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` does not match the expected shape or this builder
            was already consumed by :meth:`StructuredCreditBuilder.build`.
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
    | RevolvingCredit
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
        means valuation only. Mortgage OAS and CMO Z-spread take clean prices
        per 100 of current face and add settlement accrued interest. MBS
        ``dv01`` and ``bucketed_dv01`` include the same rate-dependent
        prepayments as ``duration_mod``. FI TRS ``duration_dv01`` requires
        ``duration_id`` and a finite signed duration scalar in years.
        Roll specialness is in basis points against the forward curve
        ``repo_curve_id``, or the discount curve when absent; implied
        financing is an ACT/360 decimal rate.
    pricing_options : MetricPricingOverrides or dict or str, optional
        Metric-time overrides merged into the instrument's own
        ``pricing_overrides`` before pricing: ``theta_period`` (``"1D"``,
        ``"1W"``, ``"1M"``), ``breakeven_config``
        (``{"target": "z_spread", "mode": "linear"}``), ``bump_config``,
        ``bond_risk_basis``, ``var_config``. A dict or
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
        If the model or a metric solver fails numerically, or instrument pricing
        wraps a failure with instrument/model context. This includes missing
        quanto inputs, asset-currency mismatches, and an analytical barrier model
        requested for discrete monitoring; the message retains the cause.
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
    | RevolvingCredit
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
    (230, True, True)
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

class TermOutSpec:
    """
    Term-out: months after the revolving period ends over which the drawn
    balance is repaid.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import TermOutSpec
    >>> TermOutSpec(24).months
    24
    """

    def __init__(self, months: int) -> None:
        """
        Construct a term-out.

        Parameters
        ----------
        months : int
            Months after the revolving period (or an earlier amortization
            event) by which the line is repaid.

        Notes
        -----
        This constructor does not raise; any non-negative month count is
        accepted.
        """
        ...

    @staticmethod
    def from_json(json: str) -> TermOutSpec:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``TermOutSpec`` (the shape ``to_json`` writes).

        Returns
        -------
        TermOutSpec
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import TermOutSpec
        >>> try:
        ...     TermOutSpec.from_json("{}")
        ... except ValueError:
        ...     print("rejected")
        rejected
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            Canonical JSON.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return every field as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Serde form of the Rust value.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(TermOutSpec.from_json, (json,))``.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __repr__(self) -> str:
        """
        Return ``repr(self)``.

        Returns
        -------
        str
            A one-line summary of the value.
        """
        ...

    @property
    def months(self) -> int:
        """
        Term-out length after the revolving period.

        Returns
        -------
        int
            Number of months over which the drawn balance is repaid.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class AmortizationEvent:
    """
    An event that ends a facility's revolving period early.

    Built through :meth:`date` (a fixed date), :meth:`cumulative_loss` (a
    cumulative-loss threshold in percent of the original collateral) or
    :meth:`excess_spread` (a trailing three-month excess-spread floor).

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.valuations.instruments import AmortizationEvent
    >>> AmortizationEvent.cumulative_loss(4.0).max_pct
    4.0
    >>> AmortizationEvent.date(datetime.date(2025, 1, 15)).kind
    'date'
    """

    @staticmethod
    def date(date: datetime.date) -> AmortizationEvent:
        """
        The revolving period ends on a fixed date.

        Parameters
        ----------
        date : datetime.date
            Date the event fires (the first payment date at or after it).

        Returns
        -------
        AmortizationEvent
            The dated event.

        Raises
        ------
        ValueError
            If ``date`` is not a valid date.

        Examples
        --------
        >>> import datetime
        >>> from finstack_quant.valuations.instruments import AmortizationEvent
        >>> AmortizationEvent.date(datetime.date(2025, 1, 15)).date_value
        datetime.date(2025, 1, 15)
        """
        ...

    @staticmethod
    def cumulative_loss(max_pct: float) -> AmortizationEvent:
        """
        The revolving period ends once cumulative losses reach a threshold.

        Parameters
        ----------
        max_pct : float
            Cumulative net loss as a percent of the original collateral
            balance (``4.0`` = 4%).

        Returns
        -------
        AmortizationEvent
            The loss event.

        Raises
        ------
        ValueError
            If ``max_pct`` is negative or not finite.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import AmortizationEvent
        >>> AmortizationEvent.cumulative_loss(4.0).kind
        'cumulative_loss'
        """
        ...

    @staticmethod
    def excess_spread(min_3m: float) -> AmortizationEvent:
        """
        The revolving period ends once trailing excess spread falls below a
        floor.

        Parameters
        ----------
        min_3m : float
            Minimum trailing three-month annualized excess spread as a decimal
            (``0.01`` = 1%).

        Returns
        -------
        AmortizationEvent
            The excess-spread event.

        Raises
        ------
        ValueError
            If ``min_3m`` is not finite.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import AmortizationEvent
        >>> AmortizationEvent.excess_spread(0.01).min_3m
        0.01
        """
        ...

    @staticmethod
    def from_json(json: str) -> AmortizationEvent:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``AmortizationEvent`` (the shape ``to_json`` writes).

        Returns
        -------
        AmortizationEvent
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import AmortizationEvent
        >>> try:
        ...     AmortizationEvent.from_json("{}")
        ... except ValueError:
        ...     print("rejected")
        rejected
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            Canonical JSON.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return every field as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Serde form of the Rust value.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(AmortizationEvent.from_json, (json,))``.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __repr__(self) -> str:
        """
        Return ``repr(self)``.

        Returns
        -------
        str
            A one-line summary of the value.
        """
        ...

    @property
    def kind(self) -> str:
        """
        Which early-amortization event this is.

        Returns
        -------
        str
            ``"date"``, ``"cumulative_loss"`` or ``"excess_spread"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def date_value(self) -> datetime.date | None:
        """
        Event date of a dated event.

        Returns
        -------
        datetime.date | None
            ``None`` for the other kinds.

        Raises
        ------
        ValueError
            If the date cannot be converted.
        """
        ...

    @property
    def max_pct(self) -> float | None:
        """
        Cumulative-loss threshold.

        Returns
        -------
        float | None
            Percent, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def min_3m(self) -> float | None:
        """
        Excess-spread floor.

        Returns
        -------
        float | None
            Decimal, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class AssetBackedFacility:
    """
    Typed wrapper for the canonical Rust ``AssetBackedFacility`` instrument:
    a committed warehouse line against a collateral pool.

    Advance rates, eligibility and concentration limits define the borrowing
    base; a borrowing-base coverage test diverts collateral cash to repay the
    facility when the drawn balance exceeds it. Collateral principal recycles
    while revolving and repays the facility sequentially afterwards, the
    undrawn commitment accrues a fee, and a term-out ends in a collateral
    liquidation. The engine runs a synthetic two-class structured-credit deal
    (:meth:`synthesized_deal`); :meth:`project` returns the lender's and the
    residual's flows. Construct via :meth:`builder`, :meth:`example` or
    :meth:`from_json`; instances are accepted directly by
    :func:`price_instrument`. Rates are decimals or basis points (``*_bp``),
    percentages are percent values.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import AssetBackedFacility
    >>> facility = AssetBackedFacility.example()
    >>> (facility.id, facility.drawn.amount, facility.commitment.amount)
    ('ABF-EXAMPLE', 70000000.0, 80000000.0)
    >>> facility.borrowing_base()["borrowing_base"]["currency"]
    'USD'
    """

    @staticmethod
    def builder() -> AssetBackedFacilityBuilder:
        """
        Create a fluent builder (mirrors Rust ``AssetBackedFacility::builder()``).

        Returns
        -------
        AssetBackedFacilityBuilder
            A builder with fluent, consuming setter methods.

        Notes
        -----
        This factory does not raise; it returns an empty builder.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import AssetBackedFacility
        >>> builder = AssetBackedFacility.builder()
        >>> builder.id("WH-1") is builder
        True
        """
        ...
    @staticmethod
    def example() -> AssetBackedFacility:
        """
        Canonical example: the example CLO pool financed by a USD 80M commitment drawn USD 70M at a fixed 6%, 80% advance rate, 20% obligor limit, two-year revolving period and a 24-month term-out (mirrors Rust ``AssetBackedFacility::example``).

        Returns
        -------
        AssetBackedFacility
            The example facility.

        Raises
        ------
        ValueError
            If construction fails (should not occur).

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import AssetBackedFacility
        >>> AssetBackedFacility.example().margin_bp
        600.0
        """
        ...
    @staticmethod
    def from_json(json: str) -> AssetBackedFacility:
        """
        Deserialize from a canonical ``finstack_quant.instrument/1`` envelope.

        Parameters
        ----------
        json : str
            Envelope JSON whose instrument type is ``asset_backed_facility``.

        Returns
        -------
        AssetBackedFacility
            The decoded facility.

        Raises
        ------
        ValueError
            If the JSON is malformed, carries another instrument type or
            fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import AssetBackedFacility
        >>> facility = AssetBackedFacility.example()
        >>> AssetBackedFacility.from_json(facility.to_json()).id
        'ABF-EXAMPLE'
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to the canonical instrument envelope.

        Returns
        -------
        str
            Envelope accepted by :func:`price_instrument` and :meth:`from_json`.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...
    def to_dict(self) -> dict[str, Any]:
        """
        Serde form of the facility as a Python ``dict``.

        Returns
        -------
        dict[str, Any]
            Canonical serde shape of the Rust ``AssetBackedFacility``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...
    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(AssetBackedFacility.from_json, (json,))``.

        Raises
        ------
        ValueError
            If serialization fails.
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
        Price the lender's projected flows (interest, principal and unused fees) and return a :class:`~finstack_quant.valuations.ValuationResult`.

        Same pipeline and keyword surface as :func:`price_instrument`.

        Parameters
        ----------
        market : MarketContext | str
            Market context (discount and index curves plus fixings for the
            note and the collateral) or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date (ISO 8601 strings accepted).
        model : str, default "default"
            Model key; ``"default"`` selects discounting.
        metrics : list[str], optional
            Metric identifiers to compute alongside the value (for example
            ``"abf_borrowing_base_cushion"``, ``"abf_facility_irr"`` or
            ``"dv01"``).
        pricing_options : dict[str, object] | str, optional
            ``MetricPricingOverrides`` merged into the instrument's own overrides.
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
            If an input cannot be interpreted, the facility fails validation
            or a metric cannot be computed.
        KeyError
            If a required curve or metric is missing from ``market``.
        RuntimeError
            If pricing fails.
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
        Compute one scalar metric for this facility (e.g. ``"abf_borrowing_base"``, ``"abf_advance_rate_utilization"`` or ``"dv01"``).

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.
        metric_id : str
            Registered metric identifier (``"abf_borrowing_base"``,
            ``"abf_borrowing_base_cushion"``, ``"abf_advance_rate_utilization"``,
            ``"abf_facility_irr"``, ``"abf_residual_irr"``, ``"dv01"``,
            ``"cs01"`` ...).
        model : str, default "default"
            Model key.

        Returns
        -------
        float
            The metric value.

        Raises
        ------
        ValueError
            If ``metric_id`` is unknown for this instrument or an input cannot
            be interpreted.
        KeyError
            If a required curve is missing from ``market``.
        RuntimeError
            If the metric computation fails.
        """
        ...
    def borrowing_base(self) -> dict[str, Any]:
        """
        Borrowing base on the closing collateral.

        Returns
        -------
        dict[str, Any]
            ``BorrowingBaseReport`` serde dict with ``eligible_collateral``,
            ``concentration_excess`` and ``borrowing_base`` Money values.

        Raises
        ------
        ValueError
            If the borrowing-base rules are malformed.
        """
        ...
    def synthesized_deal(self) -> StructuredCredit:
        """
        The two-class structured-credit deal the engine runs for this facility.

        The deal carries the facility note and the residual class, the
        borrowing-base coverage test, the reinvestment window to the
        effective revolving end, the early-amortization rules and the
        term-out call.

        Returns
        -------
        StructuredCredit
            The synthetic deal.

        Raises
        ------
        ValueError
            If the facility or the synthetic deal fails validation.
        """
        ...
    def project(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
    ) -> FacilityProjection:
        """
        Project the facility through the engine.

        Parameters
        ----------
        market : MarketContext | str
            Market context with the curves and fixings the note and the
            collateral need.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date the projection starts from.

        Returns
        -------
        FacilityProjection
            Facility and residual flows, unused fees and the period record.

        Raises
        ------
        ValueError
            If the facility fails validation or ``as_of`` is invalid.
        KeyError
            If a required curve is missing from ``market``.
        RuntimeError
            If the simulation fails.
        """
        ...
    def facility_irr(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
    ) -> float:
        """
        Lender IRR: XIRR of ``-drawn`` on ``as_of`` against every projected interest, principal and fee receipt.

        Parameters
        ----------
        market : MarketContext | str
            Market context with the curves the projection needs.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date the investment is dated on.

        Returns
        -------
        float
            Annual internal rate of return as a decimal.

        Raises
        ------
        ValueError
            If the projection fails or no rate solves.
        KeyError
            If a required curve is missing from ``market``.
        """
        ...
    def market_dependencies(self) -> dict[str, Any]:
        """
        Market-data dependencies (curves, fixings) as a dict.

        Returns
        -------
        dict[str, Any]
            Serde form of the Rust ``MarketDependencies``.

        Raises
        ------
        ValueError
            If the collateral cannot enumerate its dependencies.
        """
        ...
    @property
    def id(self) -> str:
        """
        Instrument identifier.

        Returns
        -------
        str
            Stable identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def collateral(self) -> AssetPool:
        """
        Collateral pool the facility lends against.

        Returns
        -------
        AssetPool
            The pool (asset rows, rep lines or instrument collateral).

        Notes
        -----
        This accessor does not raise; it returns a copy of the stored pool.
        """
        ...
    @property
    def borrowing_base_rules(self) -> dict[str, Any]:
        """
        Advance rates and concentration limits as their serde ``dict``.

        Returns
        -------
        dict[str, Any]
            ``BorrowingBaseRules`` with ``advance_rates`` and
            ``concentration_limits``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...
    @property
    def commitment(self) -> Money:
        """
        Total commitment.

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
    def drawn(self) -> Money:
        """
        Amount drawn at closing.

        Returns
        -------
        Money
            Currency-tagged drawn balance.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def undrawn(self) -> Money:
        """
        Undrawn commitment at closing (``commitment - drawn``).

        Returns
        -------
        Money
            Currency-tagged undrawn amount.

        Raises
        ------
        ValueError
            If the commitment and the drawn amount differ in currency.
        """
        ...
    @property
    def index_id(self) -> str | None:
        """
        Floating index curve identifier, or ``None`` for a fixed all-in rate.

        Returns
        -------
        str | None
            The forward-curve id.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def margin_bp(self) -> float:
        """
        Margin over the index (or the all-in fixed rate) in basis points.

        Returns
        -------
        float
            Basis points per annum.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def unused_fee_bp(self) -> float:
        """
        Fee on the undrawn commitment in basis points per annum.

        Returns
        -------
        float
            Basis points per annum.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def closing_date(self) -> datetime.date:
        """
        Closing date; the first payment date is one frequency later.

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
    def revolving_end(self) -> datetime.date:
        """
        Scheduled end of the revolving period.

        Returns
        -------
        datetime.date
            The scheduled revolving end.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def effective_revolving_end(self) -> datetime.date:
        """
        Effective revolving end: the scheduled end or the earliest dated amortization event, whichever is first.

        Returns
        -------
        datetime.date
            The effective revolving end.

        Notes
        -----
        This accessor does not raise; it is derived from stored terms.
        """
        ...
    @property
    def repayment_date(self) -> datetime.date:
        """
        Final repayment date: the revolving end plus the term-out window, capped at maturity.

        Returns
        -------
        datetime.date
            The repayment date.

        Notes
        -----
        This accessor does not raise; it is derived from stored terms.
        """
        ...
    @property
    def maturity(self) -> datetime.date:
        """
        Legal final maturity.

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
        Payment frequency of interest, fees and the borrowing-base test.

        Returns
        -------
        Tenor
            The payment frequency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def day_count(self) -> DayCount:
        """
        Accrual day count of the facility interest and the unused fee.

        Returns
        -------
        DayCount
            The accrual convention.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def payment_calendar_id(self) -> str | None:
        """
        Payment calendar identifier, or ``None``.

        Returns
        -------
        str | None
            Holiday calendar id (e.g. ``"nyse"``).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def amortization_events(self) -> list[dict[str, Any]]:
        """
        Events that end revolving early, as ``AmortizationEvent`` serde dicts.

        Returns
        -------
        list[dict[str, Any]]
            Each ``{"kind": "date", "date": ...}``,
            ``{"kind": "cumulative_loss", "max_pct": ...}`` or
            ``{"kind": "excess_spread", "min_3m": ...}``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...
    @property
    def term_out(self) -> dict[str, Any] | None:
        """
        Term-out window as its serde dict (``{"months": ...}``), or ``None``.

        Returns
        -------
        dict[str, Any] | None
            Months after the revolving end at which the collateral is
            liquidated.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...
    @property
    def liquidation_price_pct(self) -> float | None:
        """
        Collateral liquidation price at the term-out end in percent of par, or ``None`` for par.

        Returns
        -------
        float | None
            Percent of par.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def fees(self) -> dict[str, Any] | None:
        """
        Transaction fees paid ahead of the facility's interest as the ``DealFees`` serde dict.

        Returns
        -------
        dict[str, Any] | None
            ``None`` when the facility carries no fees.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def draw_schedule(self) -> list[dict[str, Any]]:
        """
        Scheduled draws after closing as ``FacilityDraw`` serde dicts (``date``, ``amount``).

        Returns
        -------
        list[dict[str, Any]]
            Empty when the line is fully funded at closing.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def readvance_to_borrowing_base(self) -> bool:
        """
        Whether the line is re-advanced up to the borrowing base each revolving period.

        Returns
        -------
        bool
            ``True`` when re-advances apply.

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
    def credit_model(self) -> dict[str, Any]:
        """
        Collateral behavior as its ``CreditModelConfig`` serde ``dict``.

        Returns
        -------
        dict[str, Any]
            Prepayment, default, recovery and delinquency assumptions.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...
    @property
    def prepayment_spec(self) -> PrepaymentModelSpec:
        """
        Deterministic prepayment model of the collateral.

        Returns
        -------
        PrepaymentModelSpec
            The typed spec.

        Notes
        -----
        This accessor does not raise; it returns a copy of the stored spec.
        """
        ...
    @property
    def default_spec(self) -> DefaultModelSpec:
        """
        Deterministic default model of the collateral.

        Returns
        -------
        DefaultModelSpec
            The typed spec.

        Notes
        -----
        This accessor does not raise; it returns a copy of the stored spec.
        """
        ...
    @property
    def recovery_spec(self) -> RecoveryModelSpec:
        """
        Recovery model of the collateral.

        Returns
        -------
        RecoveryModelSpec
            The typed spec.

        Notes
        -----
        This accessor does not raise; it returns a copy of the stored spec.
        """
        ...
    @property
    def attributes(self) -> Attributes:
        """
        Scenario-selection attributes.

        Returns
        -------
        Attributes
            The attribute map.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def default_model(self) -> str:
        """
        Default pricing model key from the ``Instrument`` trait (``"discounting"``).

        Returns
        -------
        str
            The model key.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class AssetBackedFacilityBuilder:
    """
    Fluent builder for :class:`AssetBackedFacility`; wraps the Rust
    ``FinancialBuilder`` output one setter for one setter.

    Builders are consumed by ``build()``; create a new builder per facility.
    Required fields: ``id``, ``collateral``, ``borrowing_base_rules``,
    ``commitment``, ``drawn``, ``margin_bp``, ``closing_date``,
    ``revolving_end``, ``maturity``, ``frequency`` and ``discount_curve_id``.
    ``day_count`` defaults to Act/360 and ``unused_fee_bp`` to zero. Nested
    specs accept a ``dict`` or JSON ``str`` in the Rust serde shape.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.valuations.instruments import AssetBackedFacility
    >>> pool = AssetBackedFacility.example().collateral
    >>> facility = (
    ...     AssetBackedFacility
    ...     .builder()
    ...     .id("WH-1")
    ...     .collateral(pool)
    ...     .borrowing_base_rules({"advance_rates": [{"asset_class": "*", "rate": 0.75}]})
    ...     .commitment(Money(80_000_000.0, Currency("USD")))
    ...     .drawn(60_000_000.0, currency="USD")
    ...     .margin_bp(550.0)
    ...     .unused_fee_bp(50.0)
    ...     .closing_date(datetime.date(2024, 1, 15))
    ...     .revolving_end(datetime.date(2026, 1, 15))
    ...     .maturity(datetime.date(2030, 1, 15))
    ...     .frequency("3M")
    ...     .payment_calendar_id("nyse")
    ...     .term_out(24)
    ...     .discount_curve_id("USD-OIS")
    ...     .build()
    ... )
    >>> (facility.id, facility.undrawn.amount, facility.repayment_date)
    ('WH-1', 20000000.0, datetime.date(2028, 1, 15))
    """

    def id(self, value: str) -> AssetBackedFacilityBuilder:
        """
        Set the facility identifier.

        Parameters
        ----------
        value : str
            Stable facility identifier.

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed.
        """
        ...
    def collateral(self, value: AssetPool) -> AssetBackedFacilityBuilder:
        """
        Set the collateral pool.

        Parameters
        ----------
        value : AssetPool
            Collateral pool (asset rows, rep lines or instrument collateral).

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed.
        """
        ...
    def borrowing_base_rules(self, value: dict[str, Any] | str) -> AssetBackedFacilityBuilder:
        """
        Set the advance rates, eligibility and concentration limits.

        Parameters
        ----------
        value : dict[str, Any] | str
            ``BorrowingBaseRules`` serde object: ``advance_rates`` (each with
            an ``asset_class`` wire name or ``"*"``, a decimal ``rate`` and an
            optional ``eligibility``) and ``concentration_limits`` (each with
            ``scope`` ``"obligor"`` / ``"industry"`` / ``"asset_class"`` and a
            percent ``max_pct``).

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` does not match the ``BorrowingBaseRules`` shape or
            the builder was already consumed.
        """
        ...
    def commitment(self, value: Money | float, currency: str | None = None) -> AssetBackedFacilityBuilder:
        """
        Set the total commitment.

        Parameters
        ----------
        value : Money | float
            Commitment; a bare amount is tagged with ``currency``.
        currency : str, optional
            ISO-4217 code for a bare amount (ignored for ``Money``).

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If a bare amount has no currency or the builder was already
            consumed.
        TypeError
            If ``value`` is neither ``Money`` nor a number.
        """
        ...
    def drawn(self, value: Money | float, currency: str | None = None) -> AssetBackedFacilityBuilder:
        """
        Set the amount drawn at closing.

        Parameters
        ----------
        value : Money | float
            Drawn balance (at most the commitment and below the collateral);
            a bare amount is tagged with ``currency``.
        currency : str, optional
            ISO-4217 code for a bare amount (ignored for ``Money``).

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If a bare amount has no currency or the builder was already
            consumed.
        TypeError
            If ``value`` is neither ``Money`` nor a number.
        """
        ...
    def index_id(self, value: str) -> AssetBackedFacilityBuilder:
        """
        Set the floating index; omit for a fixed all-in rate.

        Parameters
        ----------
        value : str
            Forward-curve identifier (e.g. ``"USD-SOFR-3M"``).

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed.
        """
        ...
    def margin_bp(self, value: float) -> AssetBackedFacilityBuilder:
        """
        Set the margin over the index (or the all-in fixed rate).

        Parameters
        ----------
        value : float
            Basis points per annum.

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed.
        """
        ...
    def unused_fee_bp(self, value: float) -> AssetBackedFacilityBuilder:
        """
        Set the fee on the undrawn commitment.

        Parameters
        ----------
        value : float
            Basis points per annum.

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed.
        """
        ...
    def closing_date(self, value: datetime.date | datetime.datetime | pd.Timestamp | str) -> AssetBackedFacilityBuilder:
        """
        Set the closing date.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pd.Timestamp | str
            Closing date; the first payment date is one frequency later.

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the date is invalid or the builder was already consumed.
        """
        ...
    def revolving_end(
        self, value: datetime.date | datetime.datetime | pd.Timestamp | str
    ) -> AssetBackedFacilityBuilder:
        """
        Set the scheduled end of the revolving period.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pd.Timestamp | str
            Revolving end; after it, collateral principal repays the facility.

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the date is invalid or the builder was already consumed.
        """
        ...
    def maturity(self, value: datetime.date | datetime.datetime | pd.Timestamp | str) -> AssetBackedFacilityBuilder:
        """
        Set the legal final maturity.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pd.Timestamp | str
            Legal final maturity.

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the date is invalid or the builder was already consumed.
        """
        ...
    def frequency(self, value: Tenor | str) -> AssetBackedFacilityBuilder:
        """
        Set the payment frequency.

        Parameters
        ----------
        value : Tenor | str
            Payment frequency of interest, fees and the borrowing-base test
            (``Tenor`` or a string such as ``"3M"``).

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the string is not a tenor or the builder was already consumed.
        """
        ...
    def day_count(self, value: DayCount | str) -> AssetBackedFacilityBuilder:
        """
        Set the accrual day count.

        Parameters
        ----------
        value : DayCount | str
            Accrual convention (``DayCount`` or a name such as
            ``"act_360"``); Act/360 when never set.

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the string is not a day count or the builder was already
            consumed.
        """
        ...
    def payment_calendar_id(self, value: str) -> AssetBackedFacilityBuilder:
        """
        Set the payment calendar.

        Parameters
        ----------
        value : str
            Holiday calendar identifier (e.g. ``"nyse"``); required for
            pricing.

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed.
        """
        ...
    def amortization_events(self, value: list[dict[str, Any]] | str) -> AssetBackedFacilityBuilder:
        """
        Set the events that end revolving early.

        Parameters
        ----------
        value : list[dict[str, Any]] | str
            ``AmortizationEvent`` objects: ``{"kind": "date", "date": ...}``,
            ``{"kind": "cumulative_loss", "max_pct": ...}`` or
            ``{"kind": "excess_spread", "min_3m": ...}``.

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` does not match the event shape or the builder was
            already consumed.
        """
        ...
    def term_out(self, value: int) -> AssetBackedFacilityBuilder:
        """
        Set the term-out window after revolving.

        Parameters
        ----------
        value : int
            Months after the revolving end at which the remaining collateral
            is liquidated to repay the facility.

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed.
        """
        ...

    def fees(self, value: dict[str, Any] | str) -> AssetBackedFacilityBuilder:
        """
        Set the transaction fees paid through the waterfall ahead of the
        facility's interest.

        Parameters
        ----------
        value : dict[str, Any] | str
            ``DealFees`` serde object (``trustee_fee_annual`` Money,
            ``senior_mgmt_fee_bp``, ``subordinated_mgmt_fee_bp``,
            ``servicing_fee_bp``, optional ``master_servicer_fee_bp`` /
            ``workout_fee_pct`` / ``liquidation_fee_pct`` /
            ``special_servicer_fee_bp`` / ``incentive_fee``).

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` does not match the ``DealFees`` shape or this builder
            was already consumed by ``build``.
        """
        ...

    def draw_schedule(self, value: list[dict[str, Any]] | str) -> AssetBackedFacilityBuilder:
        """
        Set the scheduled draws after closing.

        Parameters
        ----------
        value : list[dict[str, Any]] | str
            ``FacilityDraw`` serde objects ``{"date": "2025-01-01", "amount":
            {"amount": "10000000", "currency": "USD"}}``, ascending by date;
            each is applied on the first payment date at or after its date.

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` does not match the ``FacilityDraw`` shape or this
            builder was already consumed by ``build``.
        """
        ...

    def readvance_to_borrowing_base(self, value: bool) -> AssetBackedFacilityBuilder:
        """
        Set whether the line is re-advanced up to the borrowing base each
        revolving period.

        Parameters
        ----------
        value : bool
            ``True`` draws ``min(commitment, borrowing base) − balance`` every
            revolving period.

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If this builder was already consumed by ``build``.
        """
        ...
    def liquidation_price_pct(self, value: float) -> AssetBackedFacilityBuilder:
        """
        Set the collateral liquidation price at the term-out end.

        Parameters
        ----------
        value : float
            Percent of par (``100.0`` = par).

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed.
        """
        ...
    def discount_curve_id(self, value: str) -> AssetBackedFacilityBuilder:
        """
        Set the discount curve.

        Parameters
        ----------
        value : str
            Discount curve identifier.

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed.
        """
        ...
    def credit_model(self, value: dict[str, Any] | str) -> AssetBackedFacilityBuilder:
        """
        Replace the whole collateral behavior model.

        Parameters
        ----------
        value : dict[str, Any] | str
            ``CreditModelConfig`` serde object; the per-field setters
            (:meth:`prepayment_spec` ...) then modify it.

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` does not match the shape or the builder was already
            consumed.
        """
        ...
    def prepayment_spec(self, value: PrepaymentModelSpec | dict[str, Any] | str) -> AssetBackedFacilityBuilder:
        """
        Set the deterministic prepayment model of the collateral.

        Parameters
        ----------
        value : PrepaymentModelSpec | dict[str, Any] | str
            Typed spec or its serde form.

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` does not match the shape or the builder was already
            consumed.
        """
        ...
    def default_spec(self, value: DefaultModelSpec | dict[str, Any] | str) -> AssetBackedFacilityBuilder:
        """
        Set the deterministic default model of the collateral.

        Parameters
        ----------
        value : DefaultModelSpec | dict[str, Any] | str
            Typed spec or its serde form.

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` does not match the shape or the builder was already
            consumed.
        """
        ...
    def recovery_spec(self, value: RecoveryModelSpec | dict[str, Any] | str) -> AssetBackedFacilityBuilder:
        """
        Set the recovery model of the collateral.

        Parameters
        ----------
        value : RecoveryModelSpec | dict[str, Any] | str
            Typed spec or its serde form.

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` does not match the shape or the builder was already
            consumed.
        """
        ...
    def attributes(self, value: Attributes | dict[str, str] | None) -> AssetBackedFacilityBuilder:
        """
        Set scenario-selection attributes.

        Parameters
        ----------
        value : Attributes | dict[str, str] | None
            Attribute map; ``None`` clears it.

        Returns
        -------
        AssetBackedFacilityBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed.
        TypeError
            If ``value`` is neither ``Attributes`` nor a ``dict``.
        """
        ...
    def build(self) -> AssetBackedFacility:
        """
        Consume the builder and validate the facility.

        Returns
        -------
        AssetBackedFacility
            The validated facility.

        Raises
        ------
        ValueError
            If the builder was already consumed, a required field is missing
            (the message names the field), or the facility fails validation.
        """
        ...

class FacilityProjection:
    """
    Facility and residual projection (:meth:`AssetBackedFacility.project`'s
    return value): the lender's interest and principal, the unused-commitment
    fees, the residual class's flows and the synthetic deal's period record.

    Examples
    --------
    >>> from finstack_quant.core.market_data import DiscountCurve, MarketContext
    >>> from finstack_quant.valuations.instruments import AssetBackedFacility
    >>> facility = AssetBackedFacility.example()
    >>> as_of = facility.closing_date
    >>> market = MarketContext().insert(DiscountCurve.flat("USD-OIS", as_of, 0.04))
    >>> projection = facility.project(market, as_of)
    >>> list(projection.to_dataframe().columns)
    ['date', 'interest', 'principal', 'unused_fee', 'draw', 'lender_total', 'residual']
    """

    @staticmethod
    def from_json(json: str) -> FacilityProjection:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``FacilityProjection`` (the shape ``to_json`` writes).

        Returns
        -------
        FacilityProjection
            The decoded projection.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import FacilityProjection
        >>> try:
        ...     FacilityProjection.from_json("{}")
        ... except ValueError:
        ...     print("rejected")
        rejected
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to the JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            JSON-encoded projection.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...
    def to_dict(self) -> dict[str, Any]:
        """
        Every field as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Serde form of the projection.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...
    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(FacilityProjection.from_json, (json,))``.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...
    @property
    def facility(self) -> TrancheCashflows:
        """
        Interest and principal paid to the facility note.

        Returns
        -------
        TrancheCashflows
            The note's flows.

        Notes
        -----
        This accessor does not raise; it returns a copy of the stored flows.
        """
        ...
    @property
    def residual(self) -> TrancheCashflows:
        """
        Cash paid to the residual class.

        Returns
        -------
        TrancheCashflows
            The residual's flows.

        Notes
        -----
        This accessor does not raise; it returns a copy of the stored flows.
        """
        ...
    @property
    def unused_fees(self) -> list[tuple[datetime.date, Money]]:
        """
        Unused-commitment fee per payment date.

        Returns
        -------
        list[tuple[datetime.date, Money]]
            ``(payment date, fee)`` pairs in date order.

        Raises
        ------
        ValueError
            If a date cannot be converted.
        """
        ...

    @property
    def draws(self) -> list[tuple[datetime.date, Money]]:
        """
        Lender draws applied (scheduled draws and re-advances) per payment date.

        Returns
        -------
        list[tuple[datetime.date, Money]]
            ``(date, amount)`` pairs; outflows in :attr:`lender_cashflows`.

        Raises
        ------
        ValueError
            If a date cannot be converted.
        """
        ...
    @property
    def lender_cashflows(self) -> list[tuple[datetime.date, Money]]:
        """
        Every cashflow to the lender (interest, principal and fees) per date.

        Returns
        -------
        list[tuple[datetime.date, Money]]
            ``(payment date, amount)`` pairs in date order.

        Raises
        ------
        ValueError
            If a date cannot be converted.
        """
        ...
    @property
    def diagnostics(self) -> SimulationDiagnostics:
        """
        Per-period record of the synthetic deal.

        Returns
        -------
        SimulationDiagnostics
            Pool, collections, cash-account and coverage-test record.

        Notes
        -----
        This accessor does not raise; it returns a copy of the stored record.
        """
        ...
    def to_dataframe(self) -> pd.DataFrame:
        """
        One row per payment date as a pandas ``DataFrame``.

        Columns: ``date`` (ISO 8601 string), ``interest``, ``principal``,
        ``unused_fee``, ``draw`` (lender advances), ``lender_total`` (the
        first three summed less draws) and ``residual`` (cash to the residual
        class), all in currency units.

        Returns
        -------
        pandas.DataFrame
            The projection in date order.

        Raises
        ------
        ValueError
            If the rows cannot be serialized.
        """
        ...

class RevolvingCredit:
    """
    Typed wrapper for the canonical Rust ``RevolvingCredit`` instrument.

    Construct via :meth:`RevolvingCredit.builder`, the
    :meth:`RevolvingCredit.example` preset or :meth:`RevolvingCredit.from_json`.
    Every public Rust field is readable as a property; :meth:`price` /
    :meth:`metric` run the same pricer as :func:`price_instrument`. Stochastic
    facilities (a ``draw_repay_spec`` of kind ``"stochastic"``) additionally
    expose :meth:`price_with_paths` and their path-averaged
    :meth:`expected_cashflows`. Instances are accepted directly by
    :func:`price_instrument` and :meth:`AssetPool.with_instruments`.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import RevolvingCredit
    >>> facility = RevolvingCredit.example()
    >>> (facility.id, facility.is_stochastic)
    ('RCF-USD-3Y', False)
    """

    @staticmethod
    def builder() -> RevolvingCreditBuilder:
        """
        Create a fluent builder (mirrors Rust ``RevolvingCredit::builder()``).

        Returns
        -------
        RevolvingCreditBuilder
            A builder with fluent, consuming setter methods.

        Notes
        -----
        This factory does not raise; it returns an empty builder.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import RevolvingCredit
        >>> builder = RevolvingCredit.builder()
        >>> builder.id("RCF-1") is builder
        True
        """
        ...
    @staticmethod
    def example() -> RevolvingCredit:
        """
        Canonical example: USD 50M three-year SOFR + 250bp facility with USD 10M drawn and a scheduled draw and repayment (mirrors Rust ``RevolvingCredit::example``).

        Returns
        -------
        RevolvingCredit
            The example facility.

        Raises
        ------
        ValueError
            If construction fails (should not occur).

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import RevolvingCredit
        >>> RevolvingCredit.example().id
        'RCF-USD-3Y'
        """
        ...
    @staticmethod
    def from_json(json: str) -> RevolvingCredit:
        """
        Deserialize a validated facility from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"revolving_credit"`` payload. The UTF-8 input must not exceed
            16 MiB. Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        RevolvingCredit
            The validated facility represented by the payload.

        Raises
        ------
        ValueError
            If the input exceeds 16 MiB, is malformed, has an unsupported
            envelope schema, carries a type other than ``"revolving_credit"``,
            or fails facility validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import RevolvingCredit
        >>> facility = RevolvingCredit.example()
        >>> RevolvingCredit.from_json(facility.to_json()).id == facility.id
        True
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to a canonical ``finstack_quant.instrument/1`` envelope.

        Returns
        -------
        str
            Envelope accepted by :func:`price_instrument` and :meth:`from_json`.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...
    def to_dict(self) -> dict[str, Any]:
        """
        Serde form of the facility as a Python ``dict``.

        Returns
        -------
        dict[str, Any]
            Canonical serde shape of the Rust ``RevolvingCredit``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...
    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(RevolvingCredit.from_json, (json,))``.

        Raises
        ------
        ValueError
            If serialization fails.
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
        Price this facility and return a :class:`~finstack_quant.valuations.ValuationResult`.

        Same pipeline and keyword surface as :func:`price_instrument`; a
        stochastic ``draw_repay_spec`` prices by Monte Carlo, a deterministic
        one on its contractual schedule.

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date (ISO 8601 strings accepted).
        model : str, default "default"
            Model key.
        metrics : list[str], optional
            Metric identifiers to compute (for example ``"dv01"`` or
            ``"draw_option_cost"``).
        pricing_options : dict[str, object] | str, optional
            ``MetricPricingOverrides`` merged into the instrument's own overrides.
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
            If a required curve or metric is missing from ``market``.
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
        Compute one scalar metric for this facility (e.g. ``"dv01"`` or ``"draw_option_cost"``).

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.
        metric_id : str
            Registered metric identifier.
        model : str, default "default"
            Model key.

        Returns
        -------
        float
            The metric value.

        Raises
        ------
        ValueError
            If ``metric_id`` is unknown or an input cannot be interpreted.
        KeyError
            If a required curve is missing from ``market``.
        RuntimeError
            If the metric computation fails.
        """
        ...
    def price_with_paths(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
    ) -> EnhancedMonteCarloResult:
        """
        Run the stochastic Monte Carlo valuation and keep every path.

        Parameters
        ----------
        market : MarketContext | str
            Market context with the discount curve, the index forward curve and
            fixings for floating facilities, and the hazard curve for
            market-anchored spread processes.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date; the simulation starts here with the current drawn
            amount.

        Returns
        -------
        EnhancedMonteCarloResult
            Present value and draw option cost estimates plus every path.

        Raises
        ------
        ValueError
            If the facility has a deterministic ``draw_repay_spec`` or fails
            validation.
        KeyError
            If a required curve is missing from ``market``.
        RuntimeError
            If the simulation fails.
        """
        ...
    def expected_cashflows(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
    ) -> CashFlowSchedule:
        """
        Cashflow schedule of the facility: contractual for a deterministic ``draw_repay_spec``, path-averaged for a stochastic one.

        Parameters
        ----------
        market : MarketContext | str
            Market context with the curves the schedule projects from.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date; flows are projected from this date.

        Returns
        -------
        CashFlowSchedule
            Dated interest, fee and principal flows from the lender's
            perspective (draws negative, repayments positive).

        Raises
        ------
        ValueError
            If the facility fails validation or the schedule cannot be built.
        KeyError
            If a required curve is missing from ``market``.
        """
        ...
    def market_dependencies(self) -> dict[str, Any]:
        """
        Market-data dependencies (curves, fixings) as a dict.

        Returns
        -------
        dict[str, Any]
            Serde form of the Rust ``MarketDependencies``.

        Raises
        ------
        ValueError
            If the instrument cannot enumerate its dependencies.
        """
        ...
    @property
    def id(self) -> str:
        """
        Instrument identifier.

        Returns
        -------
        str
            Stable identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def commitment_amount(self) -> Money:
        """
        Total committed amount.

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
    def drawn_amount(self) -> Money:
        """
        Drawn balance at the simulation anchor (the later of the commitment and valuation dates), in both deterministic and stochastic mode.

        Returns
        -------
        Money
            Currency-tagged drawn balance.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def commitment_date(self) -> datetime.date:
        """
        Date the facility becomes available.

        Returns
        -------
        datetime.date
            The commitment date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def maturity(self) -> datetime.date:
        """
        Expiry of the commitment.

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
    def base_rate_spec(self) -> dict[str, Any]:
        """
        Base-rate specification as its serde ``dict``.

        Returns
        -------
        dict[str, Any]
            ``{"fixed": {"rate": r}}`` or ``{"floating": {...}}``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...
    @property
    def day_count(self) -> DayCount:
        """
        Interest accrual day count.

        Returns
        -------
        DayCount
            The accrual convention.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def frequency(self) -> Tenor:
        """
        Payment frequency for interest and fees.

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
    def fees(self) -> dict[str, Any]:
        """
        Fee structure as its serde ``dict`` (including dated ``steps``).

        Returns
        -------
        dict[str, Any]
            ``upfront_fee``, ``commitment_fee_tiers``, ``usage_fee_tiers``,
            ``facility_fee_bp`` and ``steps``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...
    @property
    def commitment_schedule(self) -> list[dict[str, Any]]:
        """
        Scheduled commitment changes as serde ``dict`` rows (``date``, ``amount``, ``fee_bp``); empty when the commitment is flat.

        Returns
        -------
        list[dict[str, Any]]
            One row per commitment step, in date order.

        Raises
        ------
        ValueError
            If the rows cannot be serialized.
        """
        ...
    @property
    def margin_steps(self) -> list[dict[str, Any]]:
        """
        Dated margin steps as serde ``dict`` rows (``date``, ``delta_bp``); empty when the margin is flat.

        Returns
        -------
        list[dict[str, Any]]
            One row per margin step, in date order.

        Raises
        ------
        ValueError
            If the rows cannot be serialized.
        """
        ...
    @property
    def scheduled_fees(self) -> list[dict[str, Any]]:
        """
        Dated fixed fees as serde ``dict`` rows (``date``, ``amount``); empty when none are scheduled.

        Returns
        -------
        list[dict[str, Any]]
            One row per scheduled fee, in date order.

        Raises
        ------
        ValueError
            If the rows cannot be serialized.
        """
        ...
    @property
    def oid_eir(self) -> dict[str, Any] | None:
        """
        Effective-interest-rate reporting switch (``{"include_fees": bool}``), or ``None`` for the default (fees included).

        Returns
        -------
        dict[str, Any] | None
            The switch, or ``None``.

        Raises
        ------
        ValueError
            If the spec cannot be serialized.
        """
        ...
    @property
    def lc(self) -> dict[str, Any] | None:
        """
        Letter-of-credit sub-facility as its serde ``dict`` (``sublimit``, ``outstanding``, ``events``, ``fee_bp``, ``fronting_fee_bp``, ``leq``), or ``None`` without an LC sublimit.

        Returns
        -------
        dict[str, Any] | None
            The LC sub-facility, or ``None``.

        Raises
        ------
        ValueError
            If the spec cannot be serialized.
        """
        ...
    @property
    def draw_repay_spec(self) -> dict[str, Any]:
        """
        Draw/repay specification as its serde ``dict``.

        Returns
        -------
        dict[str, Any]
            ``{"deterministic": [...]}`` or ``{"stochastic": {...}}``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...
    @property
    def is_stochastic(self) -> bool:
        """
        Whether utilization is simulated (stochastic ``draw_repay_spec``).

        Returns
        -------
        bool
            ``True`` for a stochastic facility.

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
    def credit_curve_id(self) -> str | None:
        """
        Credit (hazard) curve identifier, or ``None``.

        Returns
        -------
        str | None
            The curve id when the facility carries credit risk.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def recovery_rate(self) -> float:
        """
        Recovery rate on default, as a decimal in ``[0, 1]``.

        Returns
        -------
        float
            The recovery fraction.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def leq(self) -> float:
        """
        Loan-equivalent exposure: fraction of the undrawn commitment drawn at default, as a decimal in ``[0, 1]``.

        Returns
        -------
        float
            The loan-equivalent exposure.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def stub(self) -> StubKind:
        """
        Stub rule for schedule generation.

        Returns
        -------
        StubKind
            The stub kind.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def business_day_convention(self) -> str:
        """
        Business-day convention applied to payment dates (serde string, e.g. ``"modified_following"``).

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
        Holiday calendar identifier used for payment, fixing and settlement rolls, or ``None`` for weekends only.

        Returns
        -------
        str | None
            The calendar identifier, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def payment_lag_days(self) -> int:
        """
        Business days between an accrual end and its payment date.

        Returns
        -------
        int
            The payment lag; ``0`` pays on the adjusted accrual end.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def settlement_days(self) -> int:
        """
        Business days from the valuation date to the settlement date used by quote metrics.

        Returns
        -------
        int
            The settlement lag; ``0`` settles on the valuation date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def attributes(self) -> Attributes:
        """
        Scenario-selection attributes.

        Returns
        -------
        Attributes
            The attribute map.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def default_model(self) -> str:
        """
        Default pricing model key from the ``Instrument`` trait.

        Returns
        -------
        str
            The model key.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def expiry(self) -> datetime.date | None:
        """
        Expiry date exposed by the ``Instrument`` trait, or ``None``.

        Returns
        -------
        datetime.date | None
            The expiry date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class RevolvingCreditBuilder:
    """
    Fluent builder for :class:`RevolvingCredit`; wraps the Rust
    ``FinancialBuilder`` output one setter for one setter.

    Builders are consumed by ``build()``; create a new builder per facility.
    Required fields: ``id``, ``commitment_amount``, ``drawn_amount``,
    ``commitment_date``, ``maturity``, ``base_rate_spec``, ``day_count``,
    ``frequency``, ``fees`` (or :meth:`fees_flat`), ``draw_repay_spec``,
    ``discount_curve_id`` and ``recovery_rate``. Nested specs accept a
    ``dict`` or JSON ``str`` in the Rust serde shape.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.valuations.instruments import RevolvingCredit
    >>> facility = (
    ...     RevolvingCredit
    ...     .builder()
    ...     .id("RCF-1")
    ...     .commitment_amount(Money(50_000_000.0, Currency("USD")))
    ...     .drawn_amount(Money(10_000_000.0, Currency("USD")))
    ...     .commitment_date(datetime.date(2024, 1, 15))
    ...     .maturity(datetime.date(2027, 1, 15))
    ...     .base_rate_spec(0.06)
    ...     .day_count("act_360")
    ...     .frequency("3M")
    ...     .fees_flat(25.0, 10.0, 5.0)
    ...     .draw_repay_spec({"deterministic": []})
    ...     .discount_curve_id("USD-OIS")
    ...     .recovery_rate(0.4)
    ...     .build()
    ... )
    >>> facility.is_stochastic
    False
    """

    def id(self, value: str) -> RevolvingCreditBuilder:
        """
        Set the instrument identifier.

        Parameters
        ----------
        value : str
            Unique identifier for the facility.

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def commitment_amount(self, value: Money | float, currency: str | None = None) -> RevolvingCreditBuilder:
        """
        Set the total commitment.

        Parameters
        ----------
        value : Money | float
            Commitment; a bare number needs ``currency``.
        currency : str, optional
            ISO-4217 code applied when ``value`` is a bare number.

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the amount is not finite, a bare number has no currency, or the
            builder was already consumed.
        TypeError
            If ``value`` is neither ``Money`` nor a number.
        """
        ...
    def drawn_amount(self, value: Money | float, currency: str | None = None) -> RevolvingCreditBuilder:
        """
        Set the drawn balance at the simulation anchor, the later of the commitment date and the valuation date, in both modes. Deterministic draw/repay events must be dated after that anchor.

        Parameters
        ----------
        value : Money | float
            Drawn balance; a bare number needs ``currency``.
        currency : str, optional
            ISO-4217 code applied when ``value`` is a bare number.

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the amount is not finite, a bare number has no currency, or the
            builder was already consumed.
        TypeError
            If ``value`` is neither ``Money`` nor a number.
        """
        ...
    def commitment_date(self, value: datetime.date | datetime.datetime | pd.Timestamp | str) -> RevolvingCreditBuilder:
        """
        Set the date the facility becomes available.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pd.Timestamp | str
            Commitment date (ISO 8601 strings accepted).

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` is not a date or the builder was already consumed.
        """
        ...
    def maturity(self, value: datetime.date | datetime.datetime | pd.Timestamp | str) -> RevolvingCreditBuilder:
        """
        Set the expiry of the commitment.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pd.Timestamp | str
            Maturity date (ISO 8601 strings accepted).

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` is not a date or the builder was already consumed.
        """
        ...
    def base_rate_spec(self, value: float | dict[str, Any] | str) -> RevolvingCreditBuilder:
        """
        Set the base rate.

        Parameters
        ----------
        value : float | dict[str, Any] | str
            A bare decimal builds a fixed rate (``0.06`` = 6%); a ``dict`` or
            JSON ``str`` in the ``BaseRateSpec`` serde shape
            (``{"fixed": {"rate": 0.06}}`` or ``{"floating": {...}}`` with a
            ``FloatingRateSpec``) is used verbatim.

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the spec does not match the serde shape or the builder was
            already consumed.
        """
        ...
    def day_count(self, value: DayCount | str) -> RevolvingCreditBuilder:
        """
        Set the interest accrual day count.

        Parameters
        ----------
        value : DayCount | str
            Day count object or serde name (``"act_360"``, ``"act_365f"``,
            ``"30_360"``, ...).

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the name is unknown or the builder was already consumed.
        TypeError
            If ``value`` is neither ``DayCount`` nor ``str``.
        """
        ...
    def frequency(self, value: Tenor | str) -> RevolvingCreditBuilder:
        """
        Set the payment frequency for interest and fees.

        Parameters
        ----------
        value : Tenor | str
            Tenor object or string such as ``"3M"``.

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the tenor cannot be parsed or the builder was already consumed.
        TypeError
            If ``value`` is neither ``Tenor`` nor ``str``.
        """
        ...
    def fees(self, value: dict[str, Any] | str) -> RevolvingCreditBuilder:
        """
        Set the fee structure from its serde shape.

        Parameters
        ----------
        value : dict[str, Any] | str
            ``RevolvingCreditFees`` as a ``dict`` or JSON ``str``
            (``upfront_fee`` as ``None``, ``{"amount": Money-dict}`` or
            ``{"pct_of_commitment": 0.02}``; ``commitment_fee_tiers``,
            ``usage_fee_tiers``, ``facility_fee_bp`` and the dated ``steps`` list of
            ``{"date", "commitment_delta_bp", "usage_delta_bp",
            "facility_delta_bp"}`` rows).

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the spec does not match the serde shape or the builder was
            already consumed.
        """
        ...
    def commitment_schedule(self, value: list[dict[str, Any]] | str) -> RevolvingCreditBuilder:
        """
        Set the scheduled commitment changes.

        Parameters
        ----------
        value : list[dict[str, Any]] | str
            Rows of ``{"date": "YYYY-MM-DD", "amount": Money-dict, "fee_bp":
            float}`` (a ``list`` of dicts or a JSON ``str``), each the
            commitment in force from its date; ``fee_bp`` is the reduction fee
            on a step down, in basis points of the reduced amount. Dates must
            be strictly increasing, after the commitment date and on or before
            maturity.

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If a row does not match the serde shape or the builder was
            already consumed; ordering and feasibility fail at ``build()``.
        """
        ...
    def scheduled_fees(self, value: list[dict[str, Any]] | str) -> RevolvingCreditBuilder:
        """
        Set the dated fixed fees (amendment, waiver, extension, consent).

        Parameters
        ----------
        value : list[dict[str, Any]] | str
            Rows of ``{"date": "YYYY-MM-DD", "amount": Money-dict}`` (a
            ``list`` of dicts or a JSON ``str``), each paid on its date. Dates
            must lie after the commitment date and on or before maturity.

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If a row does not match the serde shape or the builder was
            already consumed; date and sign checks fail at ``build()``.
        """
        ...
    def oid_eir(self, value: dict[str, Any] | str | None) -> RevolvingCreditBuilder:
        """
        Set the effective-interest-rate reporting switch.

        Parameters
        ----------
        value : dict[str, Any] | str | None
            ``{"include_fees": bool}`` as a ``dict`` or JSON ``str``; ``None``
            (the default) includes fees in the effective yield.

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the spec does not match the serde shape or the builder was
            already consumed.
        """
        ...
    def lc(self, value: dict[str, Any] | str | None) -> RevolvingCreditBuilder:
        """
        Set the letter-of-credit sub-facility.

        Parameters
        ----------
        value : dict[str, Any] | str | None
            ``LetterOfCreditSpec`` as a ``dict`` or JSON ``str`` with
            ``sublimit`` and ``outstanding`` (Money dicts), ``events`` (rows
            of ``{"date", "amount", "is_issue"}``), ``fee_bp`` (``None``
            accrues the floating margin), ``fronting_fee_bp`` and ``leq`` (the
            fraction of the LC face drawn at default). ``None`` removes the
            sublimit.

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the spec does not match the serde shape or the builder was
            already consumed; sublimit and capacity checks fail at ``build()``.
        """
        ...
    def margin_steps(self, value: list[dict[str, Any]] | str) -> RevolvingCreditBuilder:
        """
        Set the dated margin steps.

        Parameters
        ----------
        value : list[dict[str, Any]] | str
            Rows of ``{"date": "YYYY-MM-DD", "delta_bp": int}`` (a ``list`` of
            dicts or a JSON ``str``); each delta shifts the floating spread or
            the fixed rate from its date, cumulatively. Dates must be strictly
            increasing and strictly inside the facility life.

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If a row does not match the serde shape or the builder was
            already consumed; ordering fails at ``build()``.
        """
        ...
    def fees_flat(
        self, commitment_fee_bp: float, usage_fee_bp: float, facility_fee_bp: float
    ) -> RevolvingCreditBuilder:
        """
        Set flat (non-tiered) fees in basis points (mirrors Rust ``RevolvingCreditFees::flat``).

        Parameters
        ----------
        commitment_fee_bp : float
            Annual commitment fee on the undrawn amount, in basis points;
            values ``<= 0`` produce no commitment fee.
        usage_fee_bp : float
            Annual usage fee on the drawn amount, in basis points.
        facility_fee_bp : float
            Annual facility fee on the total commitment, in basis points.

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If a rate is not finite or the builder was already consumed.
        """
        ...
    def draw_repay_spec(self, value: dict[str, Any] | str) -> RevolvingCreditBuilder:
        """
        Set the draw/repay specification from its serde shape.

        Parameters
        ----------
        value : dict[str, Any] | str
            ``DrawRepaySpec`` as a ``dict`` or JSON ``str``:
            ``{"deterministic": [{"date": ..., "amount": Money, "is_draw": bool}, ...]}``
            or ``{"stochastic": {"utilization_process": {...}, "num_paths": ..., ...}}``.

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the spec does not match the serde shape or the builder was
            already consumed.
        """
        ...
    def discount_curve_id(self, value: str) -> RevolvingCreditBuilder:
        """
        Set the discount curve identifier.

        Parameters
        ----------
        value : str
            Discount curve id in the market context.

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed.
        """
        ...
    def credit_curve_id(self, value: str | None) -> RevolvingCreditBuilder:
        """
        Set (or clear) the credit curve identifier.

        Parameters
        ----------
        value : str | None
            Hazard curve id used for survival weighting; ``None`` prices
            without credit risk. A stochastic facility with a credit curve must
            use a market-anchored spread process on the same curve.

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed.
        """
        ...
    def recovery_rate(self, value: float) -> RevolvingCreditBuilder:
        """
        Set the recovery rate on default.

        Parameters
        ----------
        value : float
            Recovery fraction as a decimal in ``[0, 1]``.

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed.
        """
        ...
    def leq(self, value: float) -> RevolvingCreditBuilder:
        """
        Set the loan-equivalent exposure drawn at default.

        Parameters
        ----------
        value : float
            Fraction of the undrawn commitment assumed drawn at default, as a
            decimal in ``[0, 1]`` (default ``0.0``).

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed.
        """
        ...
    def stub(self, value: StubKind | str) -> RevolvingCreditBuilder:
        """
        Set the stub rule for schedule generation.

        Parameters
        ----------
        value : StubKind | str
            Stub kind object or serde name (``"short_front"``, ``"short_back"``,
            ``"long_front"``, ``"long_back"``, ``"none"``); default
            ``"short_front"``.

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the name is unknown or the builder was already consumed.
        TypeError
            If ``value`` is neither ``StubKind`` nor ``str``.
        """
        ...
    def business_day_convention(self, value: BusinessDayConvention | str) -> RevolvingCreditBuilder:
        """
        Set the business-day convention for payment dates.

        Parameters
        ----------
        value : BusinessDayConvention | str
            Convention object or serde name (``"modified_following"``,
            ``"following"``, ``"preceding"``, ...); default
            ``"modified_following"``. Accrual boundaries stay unadjusted.

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the name is unknown or the builder was already consumed.
        TypeError
            If ``value`` is neither ``BusinessDayConvention`` nor ``str``.
        """
        ...
    def calendar_id(self, value: str | None) -> RevolvingCreditBuilder:
        """
        Set the holiday calendar used for payment, fixing and settlement rolls.

        Parameters
        ----------
        value : str | None
            Calendar identifier such as ``"usny"``; ``None`` (the default)
            adjusts for weekends only. An unknown identifier fails at
            ``build()``.

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed.
        """
        ...
    def payment_lag_days(self, value: int) -> RevolvingCreditBuilder:
        """
        Set the payment lag in business days after each accrual end.

        Parameters
        ----------
        value : int
            Business days on the facility calendar; ``0`` (the default) pays
            on the adjusted accrual end.

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed.
        """
        ...
    def settlement_days(self, value: int) -> RevolvingCreditBuilder:
        """
        Set the settlement lag used by quote metrics.

        Parameters
        ----------
        value : int
            Business days from the valuation date to settlement; ``0`` (the
            default) settles on the valuation date. The base present value is
            always anchored at the valuation date.

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed.
        """
        ...
    def attributes(self, value: Attributes | dict[str, str] | None) -> RevolvingCreditBuilder:
        """
        Set scenario-selection attributes.

        Parameters
        ----------
        value : Attributes | dict[str, str] | None
            Attribute map; ``None`` clears it.

        Returns
        -------
        RevolvingCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed.
        TypeError
            If ``value`` is neither ``Attributes`` nor a ``dict``.
        """
        ...
    def build(self) -> RevolvingCredit:
        """
        Consume the builder and validate the facility.

        Returns
        -------
        RevolvingCredit
            The validated facility.

        Raises
        ------
        ValueError
            If the builder was already consumed, a required field is missing
            (the message names the field), or the facility fails validation.
        """
        ...

class EnhancedMonteCarloResult:
    """
    Monte Carlo result of a stochastic revolving credit facility with every
    simulated path retained (:meth:`RevolvingCredit.price_with_paths`'s return
    value).

    The present value and the draw option cost are antithetic-aware estimates
    (mean, standard error, 95% interval); :attr:`path_pvs` and
    :attr:`path_draw_option_costs` carry the per-path values in path order and
    :meth:`to_dataframe` tabulates them.

    Examples
    --------
    >>> import datetime
    >>> import json
    >>> from finstack_quant.core.market_data import DiscountCurve, MarketContext
    >>> from finstack_quant.valuations.instruments import RevolvingCredit
    >>> envelope = json.loads(RevolvingCredit.example().to_json())
    >>> spec = envelope["instrument"]["spec"]
    >>> spec["base_rate_spec"] = {"fixed": {"rate": 0.06}}
    >>> spec["draw_repay_spec"] = {
    ...     "stochastic": {
    ...         "utilization_process": {"mean_reverting": {"target_rate": 0.6, "speed": 1.0, "volatility": 0.25}},
    ...         "num_paths": 16,
    ...         "seed": 42,
    ...         "mc_config": {
    ...             "credit_spread_process": {"constant": 0.025},
    ...         },
    ...     }
    ... }
    >>> facility = RevolvingCredit.from_json(json.dumps(envelope))
    >>> as_of = datetime.date(2024, 1, 15)
    >>> market = MarketContext().insert(DiscountCurve.flat("USD-OIS", as_of, 0.03))
    >>> result = facility.price_with_paths(market, as_of)
    >>> (result.num_simulated_paths, len(result.path_pvs), result.pv.currency.code)
    (16, 16, 'USD')
    >>> list(result.to_dataframe().columns)
    ['path', 'pv', 'draw_option_cost']
    """

    @staticmethod
    def from_json(json: str) -> EnhancedMonteCarloResult:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``EnhancedMonteCarloResult`` (the exact shape
            ``to_json`` writes, including every path's cashflow schedule).

        Returns
        -------
        EnhancedMonteCarloResult
            The decoded result.

        Raises
        ------
        ValueError
            If ``json`` is not valid JSON for the result shape.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import EnhancedMonteCarloResult
        >>> try:
        ...     EnhancedMonteCarloResult.from_json("{}")
        ... except ValueError:
        ...     print("rejected")
        rejected
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to the JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            JSON-encoded result, including every path's cashflow schedule and
            factor paths.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def to_dict(self) -> dict[str, Any]:
        """
        Return every field as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Serde form of the Rust result.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...
    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(EnhancedMonteCarloResult.from_json, (json,))``.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...
    def to_dataframe(self) -> pd.DataFrame:
        """
        One row per simulated path as a pandas ``DataFrame``.

        Columns: ``path`` (index in path order), ``pv`` and
        ``draw_option_cost`` (facility currency units).

        Returns
        -------
        pd.DataFrame
            The per-path distribution of present value and draw option cost.

        Raises
        ------
        ValueError
            If the rows cannot be serialized.
        """
        ...
    @property
    def pv(self) -> Money:
        """
        Mean present value across paths (facility currency).

        Returns
        -------
        Money
            Currency-tagged mean present value.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def pv_std_error(self) -> float:
        """
        Standard error of the mean present value, in currency units.

        Returns
        -------
        float
            The standard error.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def pv_ci_95(self) -> tuple[Money, Money]:
        """
        95% confidence interval of the mean present value.

        Returns
        -------
        tuple[Money, Money]
            Lower and upper bounds.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def num_paths(self) -> int:
        """
        Number of independent path estimators (antithetic pairs count once).

        Returns
        -------
        int
            The estimator count.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def num_simulated_paths(self) -> int:
        """
        Number of simulated paths, including both members of antithetic pairs.

        Returns
        -------
        int
            The simulated path count.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def draw_option_cost(self) -> Money:
        """
        Mean draw option cost across paths (facility currency): the value to the lender of the simulated draws having been made at the contractual margin instead of each path's fair spread; negative when spreads widen after draws.

        Returns
        -------
        Money
            Currency-tagged mean draw option cost.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def draw_option_cost_std_error(self) -> float:
        """
        Standard error of the mean draw option cost, in currency units.

        Returns
        -------
        float
            The standard error.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def draw_option_cost_ci_95(self) -> tuple[Money, Money]:
        """
        95% confidence interval of the mean draw option cost.

        Returns
        -------
        tuple[Money, Money]
            Lower and upper bounds.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def path_pvs(self) -> list[float]:
        """
        Present value of every simulated path, in path order.

        Returns
        -------
        list[float]
            Per-path present values in currency units.

        Notes
        -----
        This accessor does not raise; it returns the stored values.
        """
        ...
    @property
    def path_draw_option_costs(self) -> list[float]:
        """
        Draw option cost of every simulated path, in path order.

        Returns
        -------
        list[float]
            Per-path draw option costs in currency units.

        Notes
        -----
        This accessor does not raise; it returns the stored values.
        """
        ...
    @property
    def utilization_paths(self) -> list[list[float]]:
        """
        Simulated utilization trajectories, one list per path, aligned with :attr:`observation_dates`.

        Returns
        -------
        list[list[float]]
            Utilization fractions in ``[0, 1]``.

        Notes
        -----
        This accessor does not raise; it returns the stored values.
        """
        ...
    @property
    def credit_spread_paths(self) -> list[list[float]]:
        """
        Simulated credit-spread trajectories (decimal), one list per path, aligned with :attr:`observation_dates`.

        Returns
        -------
        list[list[float]]
            Spread levels as decimals.

        Notes
        -----
        This accessor does not raise; it returns the stored values.
        """
        ...
    @property
    def observation_dates(self) -> list[str]:
        """
        Observation dates of the factor trajectories (ISO 8601 strings).

        Returns
        -------
        list[str]
            The observation grid.

        Notes
        -----
        This accessor does not raise; it returns the stored values.
        """
        ...

class StochasticPricingResult:
    """
    Stochastic (scenario-waterfall) pricing result of a structured-credit deal
    (:meth:`StructuredCredit.price_stochastic`'s return value).

    Deal-level present value, loss statistics and Monte Carlo error sit next
    to one ``TranchePricingResult`` per tranche (as dicts). Pools of real
    instruments add the reserve-funding diagnostics
    (:attr:`unfunded_draw_path_fraction`, :attr:`expected_collateral_draws`)
    and the draw option cost with its per-path distribution.

    Examples
    --------
    >>> import datetime
    >>> import json
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.dates import DayCount
    >>> from finstack_quant.core.market_data import DiscountCurve, MarketContext
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.valuations.instruments import (
    ...     AssetPool,
    ...     RepLine,
    ...     StructuredCredit,
    ...     Tranche,
    ...     TrancheStructure,
    ... )
    >>> as_of, maturity = datetime.date(2024, 1, 15), datetime.date(2031, 1, 15)
    >>> pool = AssetPool("POOL-1", "abs", Currency("USD")).with_rep_lines([
    ...     RepLine(
    ...         "LINE-1",
    ...         Money(80_000_000.0, Currency("USD")),
    ...         0.07,
    ...         maturity,
    ...         12,
    ...         DayCount.ACT_360,
    ...         asset_type={"type": "first_lien_loan", "industry": None},
    ...     )
    ... ])
    >>> note = (
    ...     Tranche
    ...     .builder()
    ...     .id("A")
    ...     .attachment_point(0.0)
    ...     .detachment_point(100.0)
    ...     .seniority("senior")
    ...     .original_balance(Money(80_000_000.0, Currency("USD")))
    ...     .coupon_fixed(0.05)
    ...     .maturity(maturity)
    ...     .build()
    ... )
    >>> deal = StructuredCredit.new_abs("ABS-1", pool, TrancheStructure([note]), as_of, maturity, "USD-SOFR-DISC")
    >>> envelope = json.loads(deal.to_json())
    >>> envelope["instrument"]["spec"]["payment_calendar_id"] = "nyse"
    >>> deal = StructuredCredit.from_json(json.dumps(envelope))
    >>> market = MarketContext().insert(DiscountCurve.flat("USD-SOFR-DISC", as_of, 0.03))
    >>> result = deal.price_stochastic(market, as_of, num_paths=8)
    >>> (result.num_paths, [t["tranche_id"] for t in result.tranche_results])
    (8, ['A'])
    >>> result.unfunded_draw_path_fraction
    0.0
    """

    @staticmethod
    def from_json(json: str) -> StochasticPricingResult:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``StochasticPricingResult`` (the shape ``to_json`` writes).

        Returns
        -------
        StochasticPricingResult
            The decoded result.

        Raises
        ------
        ValueError
            If ``json`` is not valid JSON for the result shape.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import StochasticPricingResult
        >>> try:
        ...     StochasticPricingResult.from_json("{}")
        ... except ValueError:
        ...     print("rejected")
        rejected
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to the JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            JSON-encoded result.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def to_dict(self) -> dict[str, Any]:
        """
        Return every field as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Serde form of the Rust result.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...
    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(StochasticPricingResult.from_json, (json,))``.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...
    def to_dataframe(self) -> pd.DataFrame:
        """
        One row per tranche as a pandas ``DataFrame``.

        Columns: ``tranche_id``, ``seniority``, ``npv`` (currency units),
        ``price_pct`` (percent of the tranche's current balance),
        ``expected_loss``, ``unexpected_loss``, ``expected_shortfall``
        (currency units),
        ``attachment``, ``detachment`` (decimal), ``average_life`` (years, over
        the paths that returned principal), ``paths_with_principal``,
        ``credit_duration`` and ``draw_option_cost`` (currency units).

        Returns
        -------
        pd.DataFrame
            The tranche summary.

        Raises
        ------
        ValueError
            If the rows cannot be serialized.
        """
        ...
    def draw_option_cost_dataframe(self) -> pd.DataFrame:
        """
        The per-path draw option cost distribution as a pandas ``DataFrame``.

        Columns: ``path`` (index in path order) and ``draw_option_cost``
        (currency units). Empty for pools without stochastic revolvers.

        Returns
        -------
        pd.DataFrame
            One row per path.

        Raises
        ------
        ValueError
            If the rows cannot be serialized.
        """
        ...
    @property
    def npv(self) -> Money:
        """
        Mean deal present value across paths (pool currency).

        Returns
        -------
        Money
            Currency-tagged present value.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def dirty_price(self) -> float:
        """
        Dirty price as a percent of the sum of current tranche balances
        (the same face as the deterministic ``dirty_price`` metric).

        Returns
        -------
        float
            The dirty price.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def expected_loss(self) -> Money:
        """
        Expected (mean) loss across paths.

        Returns
        -------
        Money
            Currency-tagged expected loss.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def unexpected_loss(self) -> Money:
        """
        Unexpected loss (standard deviation of the path losses).

        Returns
        -------
        Money
            Currency-tagged unexpected loss.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def expected_shortfall(self) -> Money:
        """
        Expected shortfall of the path losses at :attr:`es_confidence`.

        Returns
        -------
        Money
            Currency-tagged expected shortfall.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def es_confidence(self) -> float:
        """
        Confidence level of :attr:`expected_shortfall` (decimal).

        Returns
        -------
        float
            The confidence level.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def pv_std_error(self) -> float:
        """
        Standard error of the mean present value, in currency units.

        Returns
        -------
        float
            The standard error.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def pv_confidence_interval(self) -> tuple[float, float]:
        """
        95% confidence interval of the mean present value, in currency units.

        Returns
        -------
        tuple[float, float]
            Lower and upper bounds.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def num_paths(self) -> int:
        """
        Number of scenario paths.

        Returns
        -------
        int
            The path count.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def pricing_mode(self) -> dict[str, Any]:
        """
        Pricing mode used, as its serde ``dict``.

        Returns
        -------
        dict[str, Any]
            The ``PricingMode`` serde shape.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...
    @property
    def unfunded_draw_path_fraction(self) -> float:
        """
        Fraction of paths on which a collateral draw could not be funded from the reserve account and that period's principal collections.

        Returns
        -------
        float
            A fraction in ``[0, 1]``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def expected_collateral_draws(self) -> Money:
        """
        Mean over paths of the collateral draws funded through the reserve account and principal collections.

        Returns
        -------
        Money
            Currency-tagged mean draws.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def draw_option_cost(self) -> Money:
        """
        Mean draw option cost across paths: the value to the deal of its revolvers' draws having been made at the contractual margin instead of each path's fair spread (negative when spreads widen).

        Returns
        -------
        Money
            Currency-tagged mean draw option cost.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def draw_option_cost_paths(self) -> list[float]:
        """
        Per-path draw option cost in path order (empty for pools without stochastic revolvers).

        Returns
        -------
        list[float]
            Per-path costs in currency units.

        Notes
        -----
        This accessor does not raise; it returns the stored values.
        """
        ...
    @property
    def tranche_results(self) -> list[dict[str, Any]]:
        """
        Tranche-level results as a list of dicts (``TranchePricingResult`` serde shape, in capital-structure order).

        Returns
        -------
        list[dict[str, Any]]
            One dict per tranche.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

class SimulationDiagnostics:
    """
    Deal-level accounting of one deterministic simulation
    (:meth:`StructuredCredit.run_simulation_with_diagnostics`'s return value):
    the per-period pool, collections, cash-account and coverage-test record
    plus the reserve-account and draw-funding totals.

    Examples
    --------
    >>> import datetime
    >>> import json
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.dates import DayCount
    >>> from finstack_quant.core.market_data import DiscountCurve, MarketContext
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.valuations.instruments import (
    ...     AssetPool,
    ...     RepLine,
    ...     StructuredCredit,
    ...     Tranche,
    ...     TrancheStructure,
    ... )
    >>> as_of, maturity = datetime.date(2024, 1, 15), datetime.date(2031, 1, 15)
    >>> pool = AssetPool("POOL-1", "abs", Currency("USD")).with_rep_lines([
    ...     RepLine(
    ...         "LINE-1",
    ...         Money(80_000_000.0, Currency("USD")),
    ...         0.07,
    ...         maturity,
    ...         12,
    ...         DayCount.ACT_360,
    ...         asset_type={"type": "first_lien_loan", "industry": None},
    ...     )
    ... ])
    >>> note = (
    ...     Tranche
    ...     .builder()
    ...     .id("A")
    ...     .attachment_point(0.0)
    ...     .detachment_point(100.0)
    ...     .seniority("senior")
    ...     .original_balance(Money(80_000_000.0, Currency("USD")))
    ...     .coupon_fixed(0.05)
    ...     .maturity(maturity)
    ...     .build()
    ... )
    >>> deal = StructuredCredit.new_abs("ABS-1", pool, TrancheStructure([note]), as_of, maturity, "USD-SOFR-DISC")
    >>> envelope = json.loads(deal.to_json())
    >>> envelope["instrument"]["spec"]["payment_calendar_id"] = "nyse"
    >>> deal = StructuredCredit.from_json(json.dumps(envelope))
    >>> market = MarketContext().insert(DiscountCurve.flat("USD-SOFR-DISC", as_of, 0.03))
    >>> diagnostics = deal.run_simulation_with_diagnostics(market, as_of)
    >>> diagnostics.unfunded_draws.amount
    0.0
    >>> list(diagnostics.to_dataframe().columns)[:3]
    ['date', 'pool_balance', 'pool_factor']
    >>> len(diagnostics.periods) == len(diagnostics.to_dataframe())
    True
    """

    @staticmethod
    def from_json(json: str) -> SimulationDiagnostics:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``SimulationDiagnostics`` (the shape ``to_json`` writes).

        Returns
        -------
        SimulationDiagnostics
            The decoded diagnostics.

        Raises
        ------
        ValueError
            If ``json`` is not valid JSON for the diagnostics shape.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import SimulationDiagnostics
        >>> try:
        ...     SimulationDiagnostics.from_json("{}")
        ... except ValueError:
        ...     print("rejected")
        rejected
        """
        ...
    def to_json(self) -> str:
        """
        Serialize to the JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            JSON-encoded diagnostics.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def to_dict(self) -> dict[str, Any]:
        """
        Return every field as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Serde form of the Rust diagnostics.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...
    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(SimulationDiagnostics.from_json, (json,))``.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...
    @property
    def periods(self) -> list[dict[str, Any]]:
        """
        Per-period deal record as ``PeriodDiagnostics`` serde dicts.

        Returns
        -------
        list[dict[str, Any]]
            ``payment_date``, ``pool_balance``, ``pool_factor``, ``weighted_avg_coupon``, ``weighted_avg_spread_bp``, ``warf``, the period's collections, defaults, recoveries, reinvested par, fees paid, the cash-account balances, ``delinquent_balance``, ``servicer_advances_outstanding``, ``excess_spread`` and the ``coverage_tests`` the executor evaluated.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        One row per simulated period as a pandas ``DataFrame``.

        Columns: ``date`` (ISO 8601 string), ``pool_balance``, ``pool_factor``,
        ``weighted_avg_coupon`` (decimal), ``weighted_avg_spread_bp``, ``warf``,
        ``interest_collections``, ``principal_collections``, ``defaults``,
        ``recoveries``, ``reinvested_par``, ``fees_paid``, ``reserve_balance``,
        ``reserve_interest``, ``spread_account``, ``funding_account``,
        ``delinquent_balance``, ``servicer_advances_outstanding`` and
        ``excess_spread`` (annualized decimal);
        amounts in currency units.

        Returns
        -------
        pd.DataFrame
            The period record.

        Raises
        ------
        ValueError
            If the rows cannot be serialized.
        """
        ...

    def coverage_tests_dataframe(self) -> pd.DataFrame:
        """
        Coverage-test evaluations as a long pandas ``DataFrame``.

        Columns: ``date`` (ISO 8601 string), ``test_id``, ``ratio``,
        ``trigger_level``, ``cushion`` (ratio minus trigger) and ``passing``.

        Returns
        -------
        pd.DataFrame
            One row per test per period.

        Raises
        ------
        ValueError
            If the rows cannot be serialized.
        """
        ...
    @property
    def reserve_balance_path(self) -> list[tuple[datetime.date, Money]]:
        """
        Reserve-account balance at the end of each simulated period.

        Returns
        -------
        list[tuple[datetime.date, Money]]
            ``(date, balance)`` pairs in period order.

        Raises
        ------
        ValueError
            If a date cannot be converted.
        """
        ...
    @property
    def reserve_interest_paid(self) -> list[tuple[datetime.date, Money]]:
        """
        Reserve interest earned each period, before routing.

        Returns
        -------
        list[tuple[datetime.date, Money]]
            ``(date, interest)`` pairs in period order.

        Raises
        ------
        ValueError
            If a date cannot be converted.
        """
        ...
    @property
    def draws_from_reserve(self) -> Money:
        """
        Collateral draws funded from the reserve account over the simulation.

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
    def draws_from_principal(self) -> Money:
        """
        Collateral draws funded from principal collections over the simulation.

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
    def unfunded_draws(self) -> Money:
        """
        Collateral draws that could not be funded over the simulation.

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
    def reserve_replenished(self) -> Money:
        """
        Revolver repayments diverted to replenish the reserve over the simulation.

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
    def early_amortization_date(self) -> datetime.date | None:
        """
        Payment date of the early-amortization event (or coverage-test
        acceleration) that ended the revolving period.

        Returns
        -------
        datetime.date | None
            ``None`` when no event fired.

        Raises
        ------
        ValueError
            If the date cannot be converted.
        """
        ...

    @property
    def tranche_draws(self) -> list[tuple[str, datetime.date, Money]]:
        """
        Lender draws applied to notes.

        Returns
        -------
        list[tuple[str, datetime.date, Money]]
            ``(tranche_id, payment date, amount)`` triples in date order.

        Raises
        ------
        ValueError
            If a date cannot be converted.
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
        Model price at the solved OAS, as a percentage of current balance.

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
        Target market price, as a percentage of current balance.

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
        current balance.

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
        ``market_price`` (percentage of current balance), ``num_paths``,
        ``price_std_error`` (percentage of current balance).

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
    ...     "factor": 1.0,
    ...     "wal": 3.0,
    ...     "z_spread_bp": 0.0,
    ...     "cs01": -1.0,
    ...     "spread_duration": 3.0,
    ...     "spread_convexity": 12.0,
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
        ...     "factor": 1.0,
        ...     "wal": 3.0,
        ...     "z_spread_bp": 0.0,
        ...     "cs01": -1.0,
        ...     "spread_duration": 3.0,
        ...     "spread_convexity": 12.0,
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
        Model clean price, as a percentage of the tranche's current balance
        (the factor-adjusted secondary-market quote basis).

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
    def factor(self) -> float:
        """
        Pool factor of the note: current balance over original balance, so a
        price on original face is ``price_pct * factor``.

        Returns
        -------
        float
            The pool factor in ``[0, 1]`` for an amortizing note (``1.0`` at
            new issue; above ``1.0`` only after PIK accretion).

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
    def spread_convexity(self) -> float:
        """
        Spread convexity at the solved z-spread.

        Returns
        -------
        float
            Second-order z-spread sensitivity of the projected cashflows, in
            years squared, on the same kernel as ``spread_duration``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def modified_duration(self) -> float:
        """
        Effective (rate) duration of the tranche.

        Returns
        -------
        float
            Years. The cashflows are re-projected with every rate curve
            (discount and forward) bumped ±1 bp in parallel, so a floater's
            coupon resets move with the curve and its duration is short; a
            fixed-coupon note's equals its modified duration.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def convexity(self) -> float:
        """
        Effective convexity of the tranche.

        Returns
        -------
        float
            Years squared, from the same ±1 bp re-projection as
            ``modified_duration``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def target_price_pct(self) -> float:
        """
        Price the z-spread/CS01 were solved against, as a percentage of
        current balance.

        Returns
        -------
        float
            The target price.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def wal_to_call(self) -> float | None:
        """
        Weighted-average life to the assumed call, in years.

        Returns
        -------
        float | None
            ``None`` when the deal carries no call covering this tranche.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def z_spread_to_call_bp(self) -> float | None:
        """
        Z-spread of the to-call flows to ``target_price_pct``, in basis points.

        Returns
        -------
        float | None
            ``None`` without a call.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def dm_to_call_bp(self) -> float | None:
        """
        Discount margin to call for a floating-rate tranche, in basis points.

        Returns
        -------
        float | None
            ``None`` for fixed-rate tranches or without a call.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def dm_bp(self) -> float | None:
        """
        Discount margin to maturity at ``target_price_pct``.

        Returns
        -------
        float | None
            Basis points of constant spread over the deal discount curve that
            reprices the floater's projected cashflows; ``None`` for
            fixed-rate tranches.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export as a single-row pandas DataFrame.

        Columns: ``tranche_id``, ``currency``, ``pv``, ``price_pct``, ``factor``, ``wal``,
        ``z_spread_bp``, ``cs01``, ``spread_duration``, ``spread_convexity``,
        ``modified_duration``, ``convexity``, ``target_price_pct``, ``wal_to_call``,
        ``z_spread_to_call_bp``, ``dm_to_call_bp``, ``dm_bp`` -- the same fields and
        units as the properties of the same name (the ``*_to_call`` columns are
        ``NaN`` without a call, ``dm_bp`` for fixed-rate tranches).

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
        (percentage of current balance), ``wal`` (years), ``writedown``
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
        Positive dirty settlement value in the tranche's currency, including
        accrued interest once. Settlement is the deal's ``quote_settlement_date``
        or the valuation date when omitted. Values above model value
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
        Clean settlement price as a percentage of current balance (100.0 = par).
        Accrued interest is added once at the deal's ``quote_settlement_date``
        (valuation date when omitted); earlier payments belong to the seller.
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
        Clean settlement price as a percentage of current balance. When omitted,
        the deal's market quote is used, or its model clean settlement price
        if no quote is supplied. PV remains measured at valuation.

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
    >>> (opts.theta_period, opts.bond_risk_basis)
    ('1W', 'callable_oas')
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
