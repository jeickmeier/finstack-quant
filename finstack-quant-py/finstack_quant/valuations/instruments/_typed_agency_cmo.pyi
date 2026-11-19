"""Typed agency CMO and its builder."""

from __future__ import annotations

import datetime
from datetime import date
from typing import Any
import pandas as pd

from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.types import Attributes
from finstack_quant.valuations import ValuationResult
from finstack_quant.valuations.instruments._pricing import PriceExtras, MetricPricingOverrides

class AgencyCmo:
    """Agency collateralized mortgage obligation backed by mortgage cashflows.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import AgencyCmo
    >>> isinstance(AgencyCmo.example(), AgencyCmo)
    True
    """

    @staticmethod
    def builder() -> AgencyCmoBuilder:
        """Start authoring an agency CMO.

        Returns
        -------
        AgencyCmoBuilder
            Empty builder for the exact contractual terms.

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import AgencyCmo
        >>> _ = AgencyCmo.builder().id("example-agency-cmo")
        """
        ...

    def to_builder(self) -> AgencyCmoBuilder:
        """Copy the current terms into an editable builder.

        Returns
        -------
        AgencyCmoBuilder
            Builder retaining contractual terms, market identifiers and pricing overrides.

        Notes
        -----
        This method does not raise.
        """
        ...

    @staticmethod
    def example() -> AgencyCmo:
        """Construct the built-in agency CMO example.

        Returns
        -------
        AgencyCmo
            Validated contract with its lifecycle and market dependencies.

        Raises
        ------
        ValueError
            If the built-in instrument fixture is invalid.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import AgencyCmo
        >>> AgencyCmo.example().id != ""
        True
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> AgencyCmo:
        """
        Deserialize a validated AgencyCmo from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"agency_cmo"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        AgencyCmo
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import AgencyCmo
        >>> future = AgencyCmo.example()
        >>> AgencyCmo.from_json(future.to_json()).id == future.id != ""
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
            and :meth:`AgencyCmo.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def to_dict(self) -> dict[str, object]:
        """
        Canonical Rust terms for this agency CMO as a plain Python ``dict``.

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
        extras: PriceExtras | dict[str, Any] | str | None = None,
    ) -> ValuationResult:
        """
        Price this agency CMO and return a :class:`~finstack_quant.valuations.ValuationResult`.

        Same pipeline and keyword surface as :func:`price_instrument`.

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date (ISO 8601 strings accepted).
        model : str, default "default"
            Registered model key for these terms. The default uses default_model.
        metrics : list[str], optional
            Metric identifiers to compute (see :func:`list_standard_metrics`).
        extras : PriceExtras or dict[str, Any] or str, optional
            The rarer pricing inputs, as a dict or JSON text in the shape of the
            Rust ``PriceExtras``; every key is optional and an unknown key raises
            ``ValueError``. ``metric_pricing_overrides`` (a ``MetricPricingOverrides``
            or dict) is merged into ``instrument.spec.metric_pricing_overrides``
            before pricing (e.g. ``{"theta_period": {"count": 1, "unit": "days"}}``):
            only the keys present replace stored controls, ``bump_config`` keys
            merge into the stored bump configuration and an explicit ``None``
            clears a control; omitted keeps the instrument's overrides. ``market_history`` (a ``MarketHistory`` or dict)
            holds the historical scenarios ``hvar`` and ``expected_shortfall``
            require; omitted prices without history. ``explain`` (bool, default
            ``False``) attaches a step trace as ``result.explanation`` when
            ``True``. ``None`` (the default) means no extras.

        Returns
        -------
        ValuationResult
            Typed valuation envelope with price, currency and metrics.

        Raises
        ------
        ValueError
            If an input cannot be interpreted or the instrument fails validation.
        KeyError
            If a required market observation or requested metric key is missing.
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
        Compute a registered scalar metric for this agency CMO.

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
            If an observation required by the selected metric is missing.
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
            The registered pricing model key selected for these contract terms.

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
            ``price(..., extras={"metric_pricing_overrides": ...})`` patch is
            merged over it per call.

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
        """Return the contractual expiry reported by the native instrument.

        Returns
        -------
        datetime.date or None
            Contract expiry where the instrument defines an expiry date.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Reconstruct the exact contract through its canonical instrument JSON.

        Returns
        -------
        tuple[Any, tuple[str]]
            Reconstruction callable and the serialized contract argument.

        Raises
        ------
        ValueError
            If serialization of the contract fails.
        """
        ...

    @property
    def deal_name(self) -> dict[str, object] | str:
        """Read the contract's deal name.

        Returns
        -------
        dict[str, object] | str
            Deal name (e.g., "FNR 2024-1")

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def agency(self) -> dict[str, object] | str:
        """Read the contract's agency.

        Returns
        -------
        dict[str, object] | str
            Agency program

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def issue_date(self) -> date:
        """Read the contract's issue date.

        Returns
        -------
        date
            Contractual issuance date used for the initial accrual period.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def waterfall(self) -> dict[str, object] | str:
        """Read the contract's waterfall.

        Returns
        -------
        dict[str, object] | str
            Waterfall configuration with tranches

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def reference_tranche_id(self) -> str:
        """Read the contract's reference tranche id.

        Returns
        -------
        str
            Reference tranche ID for pricing (which tranche to value)

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def collateral(self) -> dict[str, object] | str | None:
        """Read the contract's collateral.

        Returns
        -------
        dict[str, object] | str | None
            Collateral pool (optional - for detailed cashflow projection)

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def collateral_wac(self) -> float | None:
        """Read the contract's collateral wac.

        Returns
        -------
        float | None
            Collateral WAC (if no explicit collateral)

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def collateral_wam_months(self) -> int | None:
        """Read the contract's collateral wam months.

        Returns
        -------
        int | None
            Remaining collateral WAM in months (if no explicit collateral).

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def discount_curve_id(self) -> str:
        """Read the contract's discount curve id.

        Returns
        -------
        str
            Discount curve identifier.

        Notes
        -----
        This property does not raise.
        """
        ...

class AgencyCmoBuilder:
    """Fluent builder for contracts with explicit lifecycle terms.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import AgencyCmo
    >>> builder = AgencyCmo.example().to_builder()
    >>> builder.id("RISK-TIPS").build().id
    'RISK-TIPS'
    """

    def build(self) -> AgencyCmo:
        """Validate the contractual terms and consume the builder.

        Returns
        -------
        AgencyCmo
            Native instrument ready for pricing against the selected market.

        Raises
        ------
        ValueError
            If a required field is missing, the terms are inconsistent, or the
            builder has already been consumed.
        """
        ...

    def id(self, value: str) -> AgencyCmoBuilder:
        """Set the agency CMO identifier.

        Parameters
        ----------
        value : str
            Stable contract identifier used for pricing and metric keys.

        Returns
        -------
        AgencyCmoBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def deal_name(self, value: dict[str, object] | str) -> AgencyCmoBuilder:
        """Set the contract's deal name.

        Parameters
        ----------
        value : dict[str, object] | str
            Deal name (e.g., "FNR 2024-1")

        Returns
        -------
        AgencyCmoBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def agency(self, value: dict[str, object] | str) -> AgencyCmoBuilder:
        """Set the contract's agency.

        Parameters
        ----------
        value : dict[str, object] | str
            Agency program

        Returns
        -------
        AgencyCmoBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def issue_date(self, value: date | str) -> AgencyCmoBuilder:
        """Set the contract's issue date.

        Parameters
        ----------
        value : date | str
            Issue date

        Returns
        -------
        AgencyCmoBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def waterfall(self, value: dict[str, object] | str) -> AgencyCmoBuilder:
        """Set the contract's waterfall.

        Parameters
        ----------
        value : dict[str, object] | str
            Waterfall configuration with tranches

        Returns
        -------
        AgencyCmoBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def reference_tranche_id(self, value: str) -> AgencyCmoBuilder:
        """Set the contract's reference tranche id.

        Parameters
        ----------
        value : str
            Reference tranche ID for pricing (which tranche to value)

        Returns
        -------
        AgencyCmoBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def collateral(self, value: dict[str, object] | str | None) -> AgencyCmoBuilder:
        """Set the contract's collateral.

        Parameters
        ----------
        value : dict[str, object] | str | None
            Collateral pool (optional - for detailed cashflow projection)

        Returns
        -------
        AgencyCmoBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def collateral_wac(self, value: float | None) -> AgencyCmoBuilder:
        """Set the contract's collateral wac.

        Parameters
        ----------
        value : float | None
            Collateral WAC (if no explicit collateral)

        Returns
        -------
        AgencyCmoBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def collateral_wam_months(self, value: int | None) -> AgencyCmoBuilder:
        """Set the contract's collateral wam months.

        Parameters
        ----------
        value : int | None
            Remaining collateral WAM in months (if no explicit collateral).

        Returns
        -------
        AgencyCmoBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def discount_curve_id(self, value: str) -> AgencyCmoBuilder:
        """Set the contract's discount curve id.

        Parameters
        ----------
        value : str
            Discount curve identifier.

        Returns
        -------
        AgencyCmoBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def attributes(self, value: Attributes | dict[str, Any]) -> AgencyCmoBuilder:
        """Set the agency CMO grouping and filter attributes.

        Parameters
        ----------
        value : Attributes | dict[str, Any]
            Tags and literal metadata carried with the contract.

        Returns
        -------
        AgencyCmoBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def instrument_pricing_overrides(self, value: dict[str, object] | str) -> AgencyCmoBuilder:
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
        AgencyCmoBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``InstrumentPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def metric_pricing_overrides(self, value: MetricPricingOverrides | dict[str, object] | str) -> AgencyCmoBuilder:
        """
        Set the metric-time pricing configuration stored on the instrument.

        Parameters
        ----------
        value : MetricPricingOverrides | dict[str, object] | str
            Rust ``MetricPricingOverrides`` as the typed class, a ``dict`` or a
            JSON string.

        Returns
        -------
        AgencyCmoBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``MetricPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def scenario_pricing_overrides(self, value: dict[str, object] | str) -> AgencyCmoBuilder:
        """
        Set the scenario-only pricing adjustments.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``ScenarioPricingOverrides`` in serde form (``dict`` or JSON
            string), e.g. ``{"scenario_price_shock_decimal": -0.02}``.

        Returns
        -------
        AgencyCmoBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``ScenarioPricingOverrides`` (unknown fields are rejected).
        """
        ...
