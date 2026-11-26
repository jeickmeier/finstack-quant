"""Typed private markets fund position and its builder."""

from __future__ import annotations

import datetime
from datetime import date
from typing import Any
from finstack_quant.core.money import Money
from finstack_quant.core.currency import Currency
from finstack_quant.core.types import Attributes
from finstack_quant.valuations.instruments._pricing import MetricPricingOverrides

class PrivateMarketsFund:
    """Private markets fund interest valued from its capital and distribution cashflows.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import PrivateMarketsFund
    >>> isinstance(PrivateMarketsFund.example(), PrivateMarketsFund)
    True
    """

    @staticmethod
    def builder() -> PrivateMarketsFundBuilder:
        """Start authoring a private markets fund position.

        Returns
        -------
        PrivateMarketsFundBuilder
            Empty builder for the exact contractual terms.

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import PrivateMarketsFund
        >>> _ = PrivateMarketsFund.builder().id("example-private-markets-fund")
        """
        ...

    def to_builder(self) -> PrivateMarketsFundBuilder:
        """Copy the current terms into an editable builder.

        Returns
        -------
        PrivateMarketsFundBuilder
            Builder retaining contractual terms, market identifiers and pricing overrides.

        Notes
        -----
        This method does not raise.
        """
        ...

    @staticmethod
    def example() -> PrivateMarketsFund:
        """Construct the built-in private markets fund position example.

        Returns
        -------
        PrivateMarketsFund
            Validated contract with its lifecycle and market dependencies.

        Raises
        ------
        ValueError
            If the built-in instrument fixture is invalid.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import PrivateMarketsFund
        >>> PrivateMarketsFund.example().id != ""
        True
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> PrivateMarketsFund:
        """
        Deserialize a validated PrivateMarketsFund from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"private_markets_fund"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        PrivateMarketsFund
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import PrivateMarketsFund
        >>> future = PrivateMarketsFund.example()
        >>> PrivateMarketsFund.from_json(future.to_json()).id == future.id != ""
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
            and :meth:`PrivateMarketsFund.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def to_dict(self) -> dict[str, object]:
        """
        Canonical Rust terms for this private markets fund position as a plain Python ``dict``.

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
    def currency(self) -> str:
        """Read the contract's currency.

        Returns
        -------
        str
            Functional currency of the fund.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def waterfall_spec(self) -> dict[str, object] | str:
        """Read the contract's waterfall spec.

        Returns
        -------
        dict[str, object] | str
            Waterfall specification defining LP/GP allocation tiers.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def events(self) -> list[dict[str, object]]:
        """Read the contract's events.

        Returns
        -------
        list[dict[str, object]]
            Time-ordered list of fund events (contributions, proceeds, distributions).

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def discount_curve_id(self) -> str | None:
        """Read the contract's discount curve id.

        Returns
        -------
        str | None
            Optional discount curve for future LP cashflows. When None, future cashflows are included undiscounted relative to the caller's valuation date.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def unrealized_nav(self) -> Money | None:
        """Read the contract's unrealized nav.

        Returns
        -------
        Money | None
            Unrealized net asset value attributable to the LP, stated as of the fund's valuation date. Holder-view residual value : the fund's present value is the PV of LP cashflows strictly after the valuation date plus this NAV. When None, the residual value is zero — a fully realized fund prices to ~0. **Do not combine a NAV mark with forecast distribution events after the valuation date.** The NAV mark already embeds the value of the fund's future realizations, so supplying both double counts them: model either realized history + NAV mark (marked fund), or realized history + projected future events with no NAV (cashflow projection). The NAV is added undiscounted, taken as stated at the valuation date.

        Notes
        -----
        This property does not raise.
        """
        ...

class PrivateMarketsFundBuilder:
    """Fluent builder for contracts with explicit lifecycle terms.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import PrivateMarketsFund
    >>> builder = PrivateMarketsFund.example().to_builder()
    >>> builder.id("RISK-TIPS").build().id
    'RISK-TIPS'
    """

    def build(self) -> PrivateMarketsFund:
        """Validate the contractual terms and consume the builder.

        Returns
        -------
        PrivateMarketsFund
            Native instrument ready for pricing against the selected market.

        Raises
        ------
        ValueError
            If a required field is missing, the terms are inconsistent, or the
            builder has already been consumed.
        """
        ...

    def id(self, value: str) -> PrivateMarketsFundBuilder:
        """Set the private markets fund position identifier.

        Parameters
        ----------
        value : str
            Stable contract identifier used for pricing and metric keys.

        Returns
        -------
        PrivateMarketsFundBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def currency(self, value: Currency | str) -> PrivateMarketsFundBuilder:
        """Set the contract's currency.

        Parameters
        ----------
        value : Currency | str
            Functional currency of the fund.

        Returns
        -------
        PrivateMarketsFundBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def waterfall_spec(self, value: dict[str, object] | str) -> PrivateMarketsFundBuilder:
        """Set the contract's waterfall spec.

        Parameters
        ----------
        value : dict[str, object] | str
            Waterfall specification defining LP/GP allocation tiers.

        Returns
        -------
        PrivateMarketsFundBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def events(self, value: list[dict[str, object]]) -> PrivateMarketsFundBuilder:
        """Set the contract's events.

        Parameters
        ----------
        value : list[dict[str, object]]
            Time-ordered list of fund events (contributions, proceeds, distributions).

        Returns
        -------
        PrivateMarketsFundBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def discount_curve_id(self, value: str | None) -> PrivateMarketsFundBuilder:
        """Set the contract's discount curve id.

        Parameters
        ----------
        value : str | None
            Optional discount curve for future LP cashflows. When None, future cashflows are included undiscounted relative to the caller's valuation date.

        Returns
        -------
        PrivateMarketsFundBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def unrealized_nav(self, value: Money | None) -> PrivateMarketsFundBuilder:
        """Set the contract's unrealized nav.

        Parameters
        ----------
        value : Money | None
            Unrealized net asset value attributable to the LP, stated as of the fund's valuation date. Holder-view residual value : the fund's present value is the PV of LP cashflows strictly after the valuation date plus this NAV. When None, the residual value is zero — a fully realized fund prices to ~0. **Do not combine a NAV mark with forecast distribution events after the valuation date.** The NAV mark already embeds the value of the fund's future realizations, so supplying both double counts them: model either realized history + NAV mark (marked fund), or realized history + projected future events with no NAV (cashflow projection). The NAV is added undiscounted, taken as stated at the valuation date.

        Returns
        -------
        PrivateMarketsFundBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def attributes(self, value: Attributes | dict[str, Any]) -> PrivateMarketsFundBuilder:
        """Set the private markets fund position grouping and filter attributes.

        Parameters
        ----------
        value : Attributes | dict[str, Any]
            Tags and literal metadata carried with the contract.

        Returns
        -------
        PrivateMarketsFundBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def instrument_pricing_overrides(self, value: dict[str, object] | str) -> PrivateMarketsFundBuilder:
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
        PrivateMarketsFundBuilder
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
    ) -> PrivateMarketsFundBuilder:
        """
        Set the metric-time pricing configuration stored on the instrument.

        Parameters
        ----------
        value : MetricPricingOverrides | dict[str, object] | str
            Rust ``MetricPricingOverrides`` as the typed class, a ``dict`` or a
            JSON string.

        Returns
        -------
        PrivateMarketsFundBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``MetricPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def scenario_pricing_overrides(self, value: dict[str, object] | str) -> PrivateMarketsFundBuilder:
        """
        Set the scenario-only pricing adjustments.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``ScenarioPricingOverrides`` in serde form (``dict`` or JSON
            string), e.g. ``{"scenario_price_shock_decimal": -0.02}``.

        Returns
        -------
        PrivateMarketsFundBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``ScenarioPricingOverrides`` (unknown fields are rejected).
        """
        ...
