"""Typed FX swap and its builder."""

from __future__ import annotations

import datetime
from datetime import date
from typing import Any
from finstack_quant.core.money import Money
from finstack_quant.core.currency import Currency
from finstack_quant.core.types import Attributes
from finstack_quant.valuations.instruments._pricing import MetricPricingOverrides

class FxSwap:
    """Paired currency exchanges on a near date and a far date.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import FxSwap
    >>> isinstance(FxSwap.example(), FxSwap)
    True
    """

    @staticmethod
    def builder() -> FxSwapBuilder:
        """Start authoring an FX swap.

        Returns
        -------
        FxSwapBuilder
            Empty builder for the exact contractual terms.

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import FxSwap
        >>> _ = FxSwap.builder().id("example-fx-swap")
        """
        ...

    def to_builder(self) -> FxSwapBuilder:
        """Copy the current terms into an editable builder.

        Returns
        -------
        FxSwapBuilder
            Builder retaining contractual terms, market identifiers and pricing overrides.

        Notes
        -----
        This method does not raise.
        """
        ...

    @staticmethod
    def example() -> FxSwap:
        """Construct the built-in FX swap example.

        Returns
        -------
        FxSwap
            Validated contract with its lifecycle and market dependencies.

        Raises
        ------
        ValueError
            If the built-in instrument fixture is invalid.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import FxSwap
        >>> FxSwap.example().id != ""
        True
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> FxSwap:
        """
        Deserialize a validated FxSwap from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"fx_swap"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        FxSwap
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import FxSwap
        >>> contract = FxSwap.example()
        >>> FxSwap.from_json(contract.to_json()).id == contract.id != ""
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
            and :meth:`FxSwap.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def to_dict(self) -> dict[str, object]:
        """
        Canonical Rust terms for this FX swap as a plain Python ``dict``.

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
    def base_currency(self) -> str:
        """Read the contract's base currency.

        Returns
        -------
        str
            Base currency (foreign)

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def quote_currency(self) -> str:
        """Read the contract's quote currency.

        Returns
        -------
        str
            Quote currency (domestic)

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def near_date(self) -> date:
        """Read the contract's near date.

        Returns
        -------
        date
            Near leg settlement date (spot leg)

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def far_date(self) -> date:
        """Read the contract's far date.

        Returns
        -------
        date
            Far leg settlement date (forward leg)

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def notional(self) -> Money:
        """Read the contract's notional.

        Returns
        -------
        Money
            Notional amount in base currency (exchanged on near, reversed on far)

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def domestic_discount_curve_id(self) -> str:
        """Read the contract's domestic discount curve id.

        Returns
        -------
        str
            Domestic discount curve id (quote currency)

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def foreign_discount_curve_id(self) -> str:
        """Read the contract's foreign discount curve id.

        Returns
        -------
        str
            Foreign discount curve id (base currency)

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def near_rate(self) -> float | None:
        """Read the contract's near rate.

        Returns
        -------
        float | None
            Optional near leg FX rate (quote per base). If absent, use the market outright forward from the valuation date to near settlement.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def far_rate(self) -> float | None:
        """Read the contract's far rate.

        Returns
        -------
        float | None
            Optional far leg FX rate (quote per base). If absent, use the market outright forward from the valuation date to far settlement.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def base_calendar_id(self) -> str | None:
        """Read the contract's base calendar id.

        Returns
        -------
        str | None
            Optional base currency calendar for spot/settlement adjustment metadata.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def quote_calendar_id(self) -> str | None:
        """Read the contract's quote calendar id.

        Returns
        -------
        str | None
            Optional quote currency calendar for spot/settlement adjustment metadata.

        Notes
        -----
        This property does not raise.
        """
        ...

class FxSwapBuilder:
    """Fluent builder for contracts with explicit lifecycle terms.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import FxSwap
    >>> builder = FxSwap.example().to_builder()
    >>> builder.id("RISK-TIPS").build().id
    'RISK-TIPS'
    """

    def build(self) -> FxSwap:
        """Validate the contractual terms and consume the builder.

        Returns
        -------
        FxSwap
            Native instrument ready for pricing against the selected market.

        Raises
        ------
        ValueError
            If a required field is missing, the terms are inconsistent, or the
            builder has already been consumed.
        """
        ...

    def id(self, value: str) -> FxSwapBuilder:
        """Set the FX swap identifier.

        Parameters
        ----------
        value : str
            Stable contract identifier used for pricing and metric keys.

        Returns
        -------
        FxSwapBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def base_currency(self, value: Currency | str) -> FxSwapBuilder:
        """Set the contract's base currency.

        Parameters
        ----------
        value : Currency | str
            Base currency (foreign)

        Returns
        -------
        FxSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def quote_currency(self, value: Currency | str) -> FxSwapBuilder:
        """Set the contract's quote currency.

        Parameters
        ----------
        value : Currency | str
            Quote currency (domestic)

        Returns
        -------
        FxSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def near_date(self, value: date | str) -> FxSwapBuilder:
        """Set the contract's near date.

        Parameters
        ----------
        value : date | str
            Near leg settlement date (spot leg)

        Returns
        -------
        FxSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def far_date(self, value: date | str) -> FxSwapBuilder:
        """Set the contract's far date.

        Parameters
        ----------
        value : date | str
            Far leg settlement date (forward leg)

        Returns
        -------
        FxSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def notional(self, value: Money) -> FxSwapBuilder:
        """Set the contract's notional.

        Parameters
        ----------
        value : Money
            Notional amount in base currency (exchanged on near, reversed on far)

        Returns
        -------
        FxSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def domestic_discount_curve_id(self, value: str) -> FxSwapBuilder:
        """Set the contract's domestic discount curve id.

        Parameters
        ----------
        value : str
            Domestic discount curve id (quote currency)

        Returns
        -------
        FxSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def foreign_discount_curve_id(self, value: str) -> FxSwapBuilder:
        """Set the contract's foreign discount curve id.

        Parameters
        ----------
        value : str
            Foreign discount curve id (base currency)

        Returns
        -------
        FxSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def near_rate(self, value: float | None) -> FxSwapBuilder:
        """Set the contract's near rate.

        Parameters
        ----------
        value : float | None
            Optional near leg FX rate (quote per base). If absent, use the market outright forward from the valuation date to near settlement.

        Returns
        -------
        FxSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def far_rate(self, value: float | None) -> FxSwapBuilder:
        """Set the contract's far rate.

        Parameters
        ----------
        value : float | None
            Optional far leg FX rate (quote per base). If absent, use the market outright forward from the valuation date to far settlement.

        Returns
        -------
        FxSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def base_calendar_id(self, value: str | None) -> FxSwapBuilder:
        """Set the contract's base calendar id.

        Parameters
        ----------
        value : str | None
            Optional base currency calendar for spot/settlement adjustment metadata.

        Returns
        -------
        FxSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def quote_calendar_id(self, value: str | None) -> FxSwapBuilder:
        """Set the contract's quote calendar id.

        Parameters
        ----------
        value : str | None
            Optional quote currency calendar for spot/settlement adjustment metadata.

        Returns
        -------
        FxSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def attributes(self, value: Attributes | dict[str, Any]) -> FxSwapBuilder:
        """Set the FX swap grouping and filter attributes.

        Parameters
        ----------
        value : Attributes | dict[str, Any]
            Tags and literal metadata carried with the contract.

        Returns
        -------
        FxSwapBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def instrument_pricing_overrides(self, value: dict[str, object] | str) -> FxSwapBuilder:
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
        FxSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``InstrumentPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def metric_pricing_overrides(self, value: MetricPricingOverrides | dict[str, object] | str) -> FxSwapBuilder:
        """
        Set the metric-time pricing configuration stored on the instrument.

        Parameters
        ----------
        value : MetricPricingOverrides | dict[str, object] | str
            Rust ``MetricPricingOverrides`` as the typed class, a ``dict`` or a
            JSON string.

        Returns
        -------
        FxSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``MetricPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def scenario_pricing_overrides(self, value: dict[str, object] | str) -> FxSwapBuilder:
        """
        Set the scenario-only pricing adjustments.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``ScenarioPricingOverrides`` in serde form (``dict`` or JSON
            string), e.g. ``{"scenario_price_shock_decimal": -0.02}``.

        Returns
        -------
        FxSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``ScenarioPricingOverrides`` (unknown fields are rejected).
        """
        ...
