"""Typed commodity forward and its builder."""

from __future__ import annotations

import datetime
from datetime import date
from typing import Any

from finstack_quant.core.types import Attributes
from finstack_quant.valuations.instruments._pricing import MetricPricingOverrides

class CommodityForward:
    """Forward contract on the price of a commodity for a specified delivery date.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import CommodityForward
    >>> isinstance(CommodityForward.example(), CommodityForward)
    True
    """

    @staticmethod
    def builder() -> CommodityForwardBuilder:
        """Start authoring a commodity forward.

        Returns
        -------
        CommodityForwardBuilder
            Empty builder for the exact contractual terms.

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CommodityForward
        >>> _ = CommodityForward.builder().id("example-commodity-forward")
        """
        ...

    def to_builder(self) -> CommodityForwardBuilder:
        """Copy the current terms into an editable builder.

        Returns
        -------
        CommodityForwardBuilder
            Builder retaining contractual terms, market identifiers and pricing overrides.

        Notes
        -----
        This method does not raise.
        """
        ...

    @staticmethod
    def example() -> CommodityForward:
        """Construct the built-in commodity forward example.

        Returns
        -------
        CommodityForward
            Validated contract with its lifecycle and market dependencies.

        Raises
        ------
        ValueError
            If the built-in instrument fixture is invalid.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CommodityForward
        >>> CommodityForward.example().id != ""
        True
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> CommodityForward:
        """
        Deserialize a validated CommodityForward from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"commodity_forward"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        CommodityForward
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CommodityForward
        >>> future = CommodityForward.example()
        >>> CommodityForward.from_json(future.to_json()).id == future.id != ""
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
            and :meth:`CommodityForward.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def to_dict(self) -> dict[str, object]:
        """
        Canonical Rust terms for this commodity forward as a plain Python ``dict``.

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
        Canonical model key used when ``model="default"`` is passed to ``price_instrument``.

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
    def underlying(self) -> dict[str, object] | str:
        """Read the contract's underlying.

        Returns
        -------
        dict[str, object] | str
            Commodity underlying parameters (commodity_type, underlying_ticker, unit, currency).

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def quantity(self) -> float:
        """Read the contract's quantity.

        Returns
        -------
        float
            Contract quantity in units.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def multiplier(self) -> float:
        """Read the contract's multiplier.

        Returns
        -------
        float
            Contract multiplier (typically 1.0 for OTC forwards, defaults to 1.0).

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def maturity(self) -> date:
        """Read the contract's maturity.

        Returns
        -------
        date
            Settlement/delivery date.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def settlement(self) -> dict[str, object] | str:
        """Read the contract's settlement.

        Returns
        -------
        dict[str, object] | str
            Settlement type (physical or cash). Defaults to cash settlement when omitted in serialized payloads.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def position(self) -> dict[str, object] | str:
        """Read the contract's position.

        Returns
        -------
        dict[str, object] | str
            Position direction (long or short). - Long: buyer of the commodity at settlement - Short: seller of the commodity at settlement

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def contract_price(self) -> float | None:
        """Read the contract's contract price.

        Returns
        -------
        float | None
            Contract price (entry/trade price K). If None, the forward is treated as **at-market** (K = F), meaning NPV ≈ 0 at inception (like a newly opened futures position). If Some(K), the forward is **off-market** and NPV reflects the mark-to-market difference: sign × (F - K) × Q × M × DF.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def quoted_forward(self) -> float | None:
        """Read the contract's quoted forward.

        Returns
        -------
        float | None
            Optional quoted forward price (overrides curve lookup for F). This is a market price override, not the contract entry price. Use contract_price for the trade entry price K.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def forward_curve_id(self) -> str:
        """Read the contract's forward curve id.

        Returns
        -------
        str
            Forward/futures price curve ID for price interpolation. Should reference a PriceCurve in the MarketContext.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def spot_id(self) -> str | None:
        """Read the contract's spot id.

        Returns
        -------
        str | None
            Optional observed spot identifier. At maturity the scalar is the delivery observation. After maturity use the exact delivery observation in the corresponding FIXING:{spot_id} series. Before maturity an explicitly identified spot can supply the cost-of-carry input if the forward curve is absent.

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
            Discount curve ID.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def exchange(self) -> str | None:
        """Read the contract's exchange.

        Returns
        -------
        str | None
            Optional exchange identifier (e.g., "NYMEX", "ICE").

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def contract_month(self) -> str | None:
        """Read the contract's contract month.

        Returns
        -------
        str | None
            Optional contract month (e.g., "2025M03" for March 2025).

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def convention(self) -> dict[str, object] | str | None:
        """Read the contract's convention.

        Returns
        -------
        dict[str, object] | str | None
            Optional market convention for this commodity. When set, provides default settlement days and calendar if not explicitly specified. See CommodityConvention for available options.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def settlement_days(self) -> int | None:
        """Read the contract's settlement days.

        Returns
        -------
        int | None
            Settlement lag in business days (T+N). Defaults to 2 for most commodity markets (T+2). If convention is set, uses the convention's default unless explicitly overridden here. # Market Standards | Market | Settlement | |--------|------------| | Energy (WTI, Brent, NG) | T+2 | | Precious metals | T+2 | | Base metals (LME) | T+2 | | Power | T+1 |

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def settlement_calendar_id(self) -> str | None:
        """Read the contract's settlement calendar id.

        Returns
        -------
        str | None
            The contract's settlement calendar ID. Required whenever the settlement (payment) date is calculated: no calendar is inferred from convention or defaulted to weekends only. nerc is a power-market label calendar, not a settlement calendar, and is rejected.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def settlement_business_day_convention(self) -> str | None:
        """Read the contract's settlement business day convention.

        Returns
        -------
        str | None
            Business day convention for settlement date adjustment. Defaults to Following for energy commodities, ModifiedFollowing for precious metals.

        Notes
        -----
        This property does not raise.
        """
        ...

class CommodityForwardBuilder:
    """Fluent builder for contracts with explicit lifecycle terms.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import CommodityForward
    >>> builder = CommodityForward.example().to_builder()
    >>> builder.id("RISK-TIPS").build().id
    'RISK-TIPS'
    """

    def build(self) -> CommodityForward:
        """Validate the contractual terms and consume the builder.

        Returns
        -------
        CommodityForward
            Native instrument ready for pricing against the selected market.

        Raises
        ------
        ValueError
            If a required field is missing, the terms are inconsistent, or the
            builder has already been consumed.
        """
        ...

    def id(self, value: str) -> CommodityForwardBuilder:
        """Set the commodity forward identifier.

        Parameters
        ----------
        value : str
            Stable contract identifier used for pricing and metric keys.

        Returns
        -------
        CommodityForwardBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def underlying(self, value: dict[str, object] | str) -> CommodityForwardBuilder:
        """Set the contract's underlying.

        Parameters
        ----------
        value : dict[str, object] | str
            Commodity underlying parameters (commodity_type, underlying_ticker, unit, currency).

        Returns
        -------
        CommodityForwardBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def quantity(self, value: float) -> CommodityForwardBuilder:
        """Set the contract's quantity.

        Parameters
        ----------
        value : float
            Contract quantity in units.

        Returns
        -------
        CommodityForwardBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def multiplier(self, value: float) -> CommodityForwardBuilder:
        """Set the contract's multiplier.

        Parameters
        ----------
        value : float
            Contract multiplier (typically 1.0 for OTC forwards, defaults to 1.0).

        Returns
        -------
        CommodityForwardBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def maturity(self, value: date | str) -> CommodityForwardBuilder:
        """Set the contract's maturity.

        Parameters
        ----------
        value : date | str
            Settlement/delivery date.

        Returns
        -------
        CommodityForwardBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def settlement(self, value: dict[str, object] | str) -> CommodityForwardBuilder:
        """Set the contract's settlement.

        Parameters
        ----------
        value : dict[str, object] | str
            Settlement type (physical or cash). Defaults to cash settlement when omitted in serialized payloads.

        Returns
        -------
        CommodityForwardBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def position(self, value: dict[str, object] | str) -> CommodityForwardBuilder:
        """Set the contract's position.

        Parameters
        ----------
        value : dict[str, object] | str
            Position direction (long or short). - Long: buyer of the commodity at settlement - Short: seller of the commodity at settlement

        Returns
        -------
        CommodityForwardBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def contract_price(self, value: float | None) -> CommodityForwardBuilder:
        """Set the contract's contract price.

        Parameters
        ----------
        value : float | None
            Contract price (entry/trade price K). If None, the forward is treated as **at-market** (K = F), meaning NPV ≈ 0 at inception (like a newly opened futures position). If Some(K), the forward is **off-market** and NPV reflects the mark-to-market difference: sign × (F - K) × Q × M × DF.

        Returns
        -------
        CommodityForwardBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def quoted_forward(self, value: float | None) -> CommodityForwardBuilder:
        """Set the contract's quoted forward.

        Parameters
        ----------
        value : float | None
            Optional quoted forward price (overrides curve lookup for F). This is a market price override, not the contract entry price. Use contract_price for the trade entry price K.

        Returns
        -------
        CommodityForwardBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def forward_curve_id(self, value: str) -> CommodityForwardBuilder:
        """Set the contract's forward curve id.

        Parameters
        ----------
        value : str
            Forward/futures price curve ID for price interpolation. Should reference a PriceCurve in the MarketContext.

        Returns
        -------
        CommodityForwardBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def spot_id(self, value: str | None) -> CommodityForwardBuilder:
        """Set the contract's spot id.

        Parameters
        ----------
        value : str | None
            Optional observed spot identifier. At maturity the scalar is the delivery observation. After maturity use the exact delivery observation in the corresponding FIXING:{spot_id} series. Before maturity an explicitly identified spot can supply the cost-of-carry input if the forward curve is absent.

        Returns
        -------
        CommodityForwardBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def discount_curve_id(self, value: str) -> CommodityForwardBuilder:
        """Set the contract's discount curve id.

        Parameters
        ----------
        value : str
            Discount curve ID.

        Returns
        -------
        CommodityForwardBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def exchange(self, value: str | None) -> CommodityForwardBuilder:
        """Set the contract's exchange.

        Parameters
        ----------
        value : str | None
            Optional exchange identifier (e.g., "NYMEX", "ICE").

        Returns
        -------
        CommodityForwardBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def contract_month(self, value: str | None) -> CommodityForwardBuilder:
        """Set the contract's contract month.

        Parameters
        ----------
        value : str | None
            Optional contract month (e.g., "2025M03" for March 2025).

        Returns
        -------
        CommodityForwardBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def convention(self, value: dict[str, object] | str | None) -> CommodityForwardBuilder:
        """Set the contract's convention.

        Parameters
        ----------
        value : dict[str, object] | str | None
            Optional market convention for this commodity. When set, provides default settlement days and calendar if not explicitly specified. See CommodityConvention for available options.

        Returns
        -------
        CommodityForwardBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def settlement_days(self, value: int | None) -> CommodityForwardBuilder:
        """Set the contract's settlement days.

        Parameters
        ----------
        value : int | None
            Settlement lag in business days (T+N). Defaults to 2 for most commodity markets (T+2). If convention is set, uses the convention's default unless explicitly overridden here. # Market Standards | Market | Settlement | |--------|------------| | Energy (WTI, Brent, NG) | T+2 | | Precious metals | T+2 | | Base metals (LME) | T+2 | | Power | T+1 |

        Returns
        -------
        CommodityForwardBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def settlement_calendar_id(self, value: str | None) -> CommodityForwardBuilder:
        """Set the contract's settlement calendar id.

        Parameters
        ----------
        value : str | None
            The contract's settlement calendar ID. Required whenever the settlement (payment) date is calculated: no calendar is inferred from convention or defaulted to weekends only. nerc is a power-market label calendar, not a settlement calendar, and is rejected.

        Returns
        -------
        CommodityForwardBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def settlement_business_day_convention(self, value: str | None) -> CommodityForwardBuilder:
        """Set the contract's settlement business day convention.

        Parameters
        ----------
        value : str | None
            Business day convention for settlement date adjustment. Defaults to Following for energy commodities, ModifiedFollowing for precious metals.

        Returns
        -------
        CommodityForwardBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def attributes(self, value: Attributes | dict[str, Any]) -> CommodityForwardBuilder:
        """Set the commodity forward grouping and filter attributes.

        Parameters
        ----------
        value : Attributes | dict[str, Any]
            Tags and literal metadata carried with the contract.

        Returns
        -------
        CommodityForwardBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def instrument_pricing_overrides(self, value: dict[str, object] | str) -> CommodityForwardBuilder:
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
        CommodityForwardBuilder
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
    ) -> CommodityForwardBuilder:
        """
        Set the metric-time pricing configuration stored on the instrument.

        Parameters
        ----------
        value : MetricPricingOverrides | dict[str, object] | str
            Rust ``MetricPricingOverrides`` as the typed class, a ``dict`` or a
            JSON string.

        Returns
        -------
        CommodityForwardBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``MetricPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def scenario_pricing_overrides(self, value: dict[str, object] | str) -> CommodityForwardBuilder:
        """
        Set the scenario-only pricing adjustments.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``ScenarioPricingOverrides`` in serde form (``dict`` or JSON
            string), e.g. ``{"scenario_price_shock_decimal": -0.02}``.

        Returns
        -------
        CommodityForwardBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``ScenarioPricingOverrides`` (unknown fields are rejected).
        """
        ...
