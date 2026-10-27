"""Stub declarations mirroring ``finstack-quant-py/src/bindings/valuations/typed_structured_credit/structured_credit.rs``."""

from __future__ import annotations

import datetime
from typing import Any, Literal
import pandas as pd
from finstack_quant.cashflows.builder import DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec
from finstack_quant.core.dates import Tenor
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.types import Attributes
from finstack_quant.valuations import ValuationResult
from finstack_quant.valuations.instruments._pricing import MarketHistory, MetricPricingOverrides
from finstack_quant.valuations.instruments._typed_structured_credit.asset_pool import AssetPool
from finstack_quant.valuations.instruments._typed_structured_credit.call_assumption import CallAssumption
from finstack_quant.valuations.instruments._typed_structured_credit.coverage_rules import CoverageRules
from finstack_quant.valuations.instruments._typed_structured_credit.equity_metrics import EquityMetrics
from finstack_quant.valuations.instruments._typed_structured_credit.hedge_swap import HedgeSwap
from finstack_quant.valuations.instruments._typed_structured_credit.simulation_run import SimulationRun
from finstack_quant.valuations.instruments._typed_structured_credit.standard_deal_params import StandardDealParams
from finstack_quant.valuations.instruments._typed_structured_credit.stochastic_result import StochasticPricingResult
from finstack_quant.valuations.instruments._typed_structured_credit.tranche_cashflows import TrancheCashflows
from finstack_quant.valuations.instruments._typed_structured_credit.tranche_structure import TrancheStructure
from finstack_quant.valuations.instruments._typed_structured_credit.waterfall import Waterfall

class StructuredCredit:
    """
    Structured-credit deal (ABS/CLO/CMBS/RMBS) with pool, tranches, and waterfall.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import StandardDealParams, StructuredCredit
    >>> builder = StructuredCredit.builder()
    >>> builder.id("EXAMPLE") is builder
    True

    """

    @staticmethod
    def builder() -> StructuredCreditBuilder:
        """
        Create a fluent builder (mirrors Rust ``StructuredCredit::builder()``).

        The builder pre-seeds ``market_conditions``, ``deal_metadata`` and
        ``hedge_swaps`` with their Rust ``Default`` values (the Rust
        builder fields have no default), which the corresponding setters
        (``market_conditions``, ``waterfall_rules``, ``fees``,
        ``credit_model``, ``hedge_swaps`` ...) can override with typed
        objects, dicts or JSON strings. Prefer :meth:`new_standard` for registry-calibrated deal-type
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
        >>> from finstack_quant.valuations.instruments import StandardDealParams, StructuredCredit
        >>> builder = StructuredCredit.builder()
        >>> builder.id("EXAMPLE") is builder
        True
        """
        ...

    @staticmethod
    def new_standard(deal_type: str, params: StandardDealParams) -> StructuredCredit:
        """Construct a validated deal using a standard registry profile.

        Parameters
        ----------
        deal_type : str
            Standard family: ``abs``, ``clo``, ``cmbs`` or ``rmbs``.
        params : StandardDealParams
            Collateral, liabilities, dates and market identifiers. A missing
            payment calendar is accepted here but must be supplied before simulation.

        Returns
        -------
        StructuredCredit
            The deal with its registry assumptions and validated structure.

        Raises
        ------
        ValueError
            If the family is unsupported or the deal fails pricing validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import StandardDealParams, StructuredCredit
        >>> example = StructuredCredit.example()
        >>> params = StandardDealParams(
        ...     example.id,
        ...     example.pool,
        ...     example.tranches,
        ...     example.closing_date,
        ...     example.maturity,
        ...     example.discount_curve_id,
        ...     example.calendar_id,
        ... )
        >>> StructuredCredit.new_standard("clo", params).id == example.id
        True
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
        >>> from finstack_quant.valuations.instruments import StandardDealParams, StructuredCredit
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
        >>> from finstack_quant.valuations.instruments import StructuredCredit
        >>> deal = StructuredCredit.example()
        >>> deal.fees is None
        True
        >>> deal.with_standard_fees().fees is not None
        True
        """
        ...

    def enable_stochastic(self) -> StructuredCredit:
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
    def calendar_id(self) -> str | None:
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
    def business_day_convention(self) -> str | None:
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
    def cleanup_call_decimal(self) -> float | None:
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
    def instrument_pricing_overrides(self) -> dict[str, object]:
        """
        Instrument-owned pricing inputs (``InstrumentPricingOverrides``) in serde form.

        Returns
        -------
        dict[str, object]
            ``market_quotes`` (quoted clean price, yield, spreads, implied
            volatility, premium) and ``model_config`` (tree model and steps, Monte
            Carlo and Hull-White parameters). Empty sections are omitted; ``{}``
            when none is set.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def metric_pricing_overrides(self) -> dict[str, object]:
        """
        Metric-time pricing configuration (``MetricPricingOverrides``) in serde form.

        Returns
        -------
        dict[str, object]
            Bump sizes, theta period and the other metric settings stored on
            the instrument; unset fields are ``None`` or omitted. A
            ``metric_pricing_overrides`` argument to ``price`` is merged over it
            per call.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def scenario_pricing_overrides(self) -> dict[str, object]:
        """
        Scenario-only pricing adjustments (``ScenarioPricingOverrides``) in serde form.

        Returns
        -------
        dict[str, object]
            ``scenario_price_shock_decimal`` and ``scenario_spread_shock_bp``;
            ``{}`` when none is set.

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
        antithetic: bool | None = None,
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
            Number of independent Monte Carlo estimators; must be at least
            two. Defaults to the deal's configured ``mc_paths`` override or
            5,000. With ``antithetic`` each estimator averages a mirrored pair,
            so ``2 * num_paths`` physical paths run while the statistical
            sample size remains ``num_paths``.
        antithetic : bool, optional
            Pair each estimator's path with its sign-flipped mirror; defaults
            to the deal's configured ``mc_antithetic`` override or ``True``.

        Returns
        -------
        StochasticPricingResult
            Deal and tranche present values, loss statistics, Monte Carlo
            error, draw diagnostics and the draw option cost. Sampling error
            uses the sample standard deviation and a 95% Student-t confidence
            interval with ``num_paths - 1`` degrees of freedom.

        Raises
        ------
        ValueError
            If the deal fails validation or the resolved ``num_paths`` is less
            than two independent estimators.
        KeyError
            If a required curve is missing from ``market``.
        RuntimeError
            If the simulation fails.
        """
        ...
    def run_simulation(
        self,
        market: MarketContext | str,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
    ) -> SimulationRun:
        """
        Run deterministic simulation and return all tranche flows and deal accounting.

        Parameters
        ----------
        market : MarketContext | str
            Market context with the deal's discount curve and every curve the
            collateral references.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date (ISO 8601 strings accepted).

        Returns
        -------
        SimulationRun
            Tranche cashflows keyed by identifier and complete reserve, draw,
            pool, coverage-test and cash-account diagnostics.

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

    @staticmethod
    def example() -> StructuredCredit:
        """
        Canonical example deal (mirrors Rust ``StructuredCredit::example``).

        A USD 100M CLO with one 7% fixed-rate collateral bond and one 6%
        senior note, closing 2024-01-01, legal final 2034-01-01, discounted
        on ``USD-OIS`` with the ``nyse`` calendar.

        Returns
        -------
        StructuredCredit
            The example deal.

        Raises
        ------
        ValueError
            If construction fails (does not occur for a released build).

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import StandardDealParams, StructuredCredit
        >>> StructuredCredit.example().id
        'CLO-EXAMPLE'
        """
        ...

    def price(
        self,
        market: MarketContext | str,
        as_of: datetime.date | str,
        model: str = "default",
        metrics: list[str] | None = None,
        metric_pricing_overrides: MetricPricingOverrides | dict[str, Any] | str | None = None,
        market_history: MarketHistory | dict[str, Any] | str | None = None,
        explain: bool = False,
    ) -> ValuationResult:
        """
        Price this deal and return a typed ``ValuationResult``.

        Delegates to the same canonical Rust pricer entry point as
        ``price_instrument(self, market, as_of, model)``.

        Parameters
        ----------
        market : MarketContext | str
            Market context object or JSON string supplying the discount and
            forward curves.
        as_of : datetime.date | str
            Valuation date, either a date-like object or an ISO 8601 string.
        model : str, default "default"
            Model key; ``"default"`` is the deal's ``default_model``, the
            deterministic ``"discounting"`` waterfall.
        metrics : list[str], optional
            Metric identifiers to compute (e.g. ``["wal", "clo_warf"]``);
            omitted means valuation only.
        metric_pricing_overrides : MetricPricingOverrides | dict | str | None
            Metric-time overrides merged into
            ``instrument.spec.metric_pricing_overrides`` before pricing.
        market_history : MarketHistory | dict | str | None
            Historical scenarios required by ``hvar`` and
            ``expected_shortfall``.
        explain : bool, default False
            When ``True``, attach a step trace as ``result.explanation``.

        Returns
        -------
        ValuationResult
            Typed valuation envelope carrying value, currency and metrics.

        Raises
        ------
        ValueError
            If an input cannot be interpreted or the deal fails validation.
        KeyError
            If a required curve or metric is missing.
        RuntimeError
            If pricing or a metric computation fails.
        """
        ...

    def metric(
        self,
        market: MarketContext | str,
        as_of: datetime.date | str,
        metric_id: str,
        model: str = "default",
    ) -> float:
        """
        Compute one scalar metric (e.g. ``"wal"`` or ``"clo_warf"``).

        Mirrors Rust ``pricer::metric_value``: the deal is priced under
        ``model`` and the single metric ``metric_id`` is returned.

        Parameters
        ----------
        market : MarketContext | str
            Market context object or JSON string.
        as_of : datetime.date | str
            Valuation date, either a date-like object or an ISO 8601 string.
        metric_id : str
            Registered metric identifier.
        model : str, default "default"
            Model key.

        Returns
        -------
        float
            The metric value in the metric's documented unit.

        Raises
        ------
        ValueError
            If ``metric_id`` is unknown or an input cannot be interpreted.
        KeyError
            If a required curve is missing.
        RuntimeError
            If the metric computation fails.
        """
        ...

    def market_dependencies(self) -> dict[str, Any]:
        """
        Market-data dependencies (discount and forward curves, fixing series).

        Returns
        -------
        dict[str, Any]
            Serde form of the Rust ``MarketDependencies``.

        Raises
        ------
        ValueError
            If the deal cannot enumerate its dependencies (for example a pool
            that cannot be normalised).
        """
        ...

    def disable_stochastic(self) -> StructuredCredit:
        """
        Return a copy with the stochastic prepayment, default, recovery and correlation specs cleared.

        Mirrors Rust ``StructuredCredit::disable_stochastic``, the inverse of
        ``enable_stochastic``; this also clears ``stochastic_recovery_spec``.

        Returns
        -------
        StructuredCredit
            A new deal priced deterministically; the receiver is unchanged.

        Notes
        -----
        This method does not raise.
        """
        ...

    @property
    def default_model(self) -> str:
        """
        Default pricing model key from the ``Instrument`` trait.

        Returns
        -------
        str
            ``"discounting"`` (the deterministic waterfall).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def expiry(self) -> datetime.date | None:
        """
        Expiry date exposed by the ``Instrument`` trait.

        Returns
        -------
        datetime.date | None
            The expiry date, or ``None`` when the deal has none.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def is_stochastic(self) -> bool:
        """
        Whether a stochastic prepayment, default, recovery or correlation spec is set.

        Returns
        -------
        bool
            ``True`` when ``price_stochastic`` has specs to simulate.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def effective_loss_allocation(self) -> str:
        """
        Loss-allocation policy in force: ``loss_allocation`` when set, else the deal-type convention.

        Returns
        -------
        str
            ``"write_down"`` or ``"par_preserving"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def effective_loss_recognition(self) -> str:
        """
        Loss-recognition timing in force: ``loss_recognition`` when set, else the deal-type convention.

        Returns
        -------
        str
            ``"at_liquidation"`` for RMBS and CMBS by default, ``"at_default"`` otherwise.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def effective_principal_covers_senior_interest(self) -> bool:
        """
        Whether principal pays senior fees and note interest when interest proceeds fall short.

        Returns
        -------
        bool
            ``principal_covers_senior_interest`` when set, else ``True`` for CLO/CBO and ``False`` otherwise.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            Reconstruction callable and canonical instrument JSON arguments.

        Raises
        ------
        ValueError
            If serialization of the stored instrument fails.
        """
        ...

class StructuredCreditBuilder:
    """
    Fluent builder returned by :meth:`StructuredCredit.builder`.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import StandardDealParams, StructuredCredit
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

    def frequency(self, value: Tenor | str) -> StructuredCreditBuilder:
        """
        Set the payment frequency for the structure.

        Parameters
        ----------
        value : Tenor | str
            Payment frequency.

        Returns
        -------
        StructuredCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If this builder was already consumed by a prior call to
            :meth:`StructuredCreditBuilder.build`, or a string ``value`` is not a recognized tenor string.
        """
        ...

    def calendar_id(self, value: str) -> StructuredCreditBuilder:
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

    def business_day_convention(self, value: str) -> StructuredCreditBuilder:
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
            the pool currency) and ``fixed_allocation_decimal`` (decimal in
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
            ``currency``, optional ``coverage_rules``).
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

    def cleanup_call_decimal(self, value: float) -> StructuredCreditBuilder:
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

    def instrument_pricing_overrides(self, value: dict[str, object] | str) -> StructuredCreditBuilder:
        """
        Set the instrument-owned pricing inputs.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``InstrumentPricingOverrides`` in serde form (``dict`` or JSON
            string): ``{"market_quotes": {...}, "model_config": {...}}``, e.g.
            ``{"market_quotes": {"quoted_clean_price_pct": 99.0}}``.

        Returns
        -------
        StructuredCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``InstrumentPricingOverrides`` (unknown fields are rejected).
        """
        ...
    def metric_pricing_overrides(
        self, value: MetricPricingOverrides | dict[str, object] | str
    ) -> StructuredCreditBuilder:
        """
        Set the metric-time pricing configuration stored on the instrument.

        Parameters
        ----------
        value : MetricPricingOverrides | dict[str, object] | str
            Rust ``MetricPricingOverrides`` as the typed class, a ``dict`` or a
            JSON string.

        Returns
        -------
        StructuredCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``MetricPricingOverrides`` (unknown fields are rejected).
        """
        ...
    def scenario_pricing_overrides(self, value: dict[str, object] | str) -> StructuredCreditBuilder:
        """
        Set the scenario-only pricing adjustments.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``ScenarioPricingOverrides`` in serde form (``dict`` or JSON
            string), e.g. ``{"scenario_price_shock_decimal": -0.02}``.

        Returns
        -------
        StructuredCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``ScenarioPricingOverrides`` (unknown fields are rejected).
        """
        ...
    def attributes(self, value: Attributes | dict[str, Any] | None) -> StructuredCreditBuilder:
        """
        Set free-form attributes (tags and metadata) on the deal.

        Parameters
        ----------
        value : Attributes | dict | None
            Attribute bag: an ``Attributes`` or its serde ``dict`` form
            (``{"tags": [...], "meta": {...}}``, as ``to_dict()`` returns it);
            ``None`` clears it.
        Returns
        -------
        StructuredCreditBuilder
            ``self``, for chaining.

        Raises
        ------
        TypeError
            If ``value`` is neither ``Attributes``, a ``dict`` nor ``None``.
        ValueError
            If ``value`` does not match the expected shape or this builder
            was already consumed by :meth:`StructuredCreditBuilder.build`.
        ValueError
            If the ``dict`` has a key other than ``tags`` / ``meta`` or a non-string tag or value.
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
            If the builder was already consumed, a required field is missing,
            or the completed deal fails pricing validation.
        """
        ...
