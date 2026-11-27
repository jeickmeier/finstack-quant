"""Typed FX spot trade and its builder."""

from __future__ import annotations

import datetime
from datetime import date
from typing import Any
from finstack_quant.core.money import Money
from finstack_quant.core.currency import Currency
from finstack_quant.core.types import Attributes
from finstack_quant.valuations.instruments._pricing import MetricPricingOverrides

class FxSpot:
    """Spot transaction in a currency pair quoted as quote currency per unit of base currency.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import FxSpot
    >>> isinstance(FxSpot.example(), FxSpot)
    True
    """

    @staticmethod
    def builder() -> FxSpotBuilder:
        """Start authoring an FX spot trade.

        Returns
        -------
        FxSpotBuilder
            Empty builder for the exact contractual terms.

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import FxSpot
        >>> _ = FxSpot.builder().id("example-fx-spot")
        """
        ...

    def to_builder(self) -> FxSpotBuilder:
        """Copy the current terms into an editable builder.

        Returns
        -------
        FxSpotBuilder
            Builder retaining contractual terms, market identifiers and pricing overrides.

        Notes
        -----
        This method does not raise.
        """
        ...

    @staticmethod
    def example() -> FxSpot:
        """Construct the built-in FX spot trade example.

        Returns
        -------
        FxSpot
            Validated contract with its lifecycle and market dependencies.

        Raises
        ------
        ValueError
            If the built-in instrument fixture is invalid.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import FxSpot
        >>> FxSpot.example().id != ""
        True
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> FxSpot:
        """
        Deserialize a validated FxSpot from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"fx_spot"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        FxSpot
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import FxSpot
        >>> future = FxSpot.example()
        >>> FxSpot.from_json(future.to_json()).id == future.id != ""
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
            and :meth:`FxSpot.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def to_dict(self) -> dict[str, object]:
        """
        Canonical Rust terms for this FX spot trade as a plain Python ``dict``.

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
            Base currency (the currency being priced)

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
            Quote currency (the currency used for pricing)

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def settlement_date(self) -> date | None:
        """Read the contract's settlement date.

        Returns
        -------
        date | None
            Actual settlement (value) date, used as given; when set, settlement_days is ignored.

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
            Contractual spot lag in business days on the joint calendar, required when settlement_date is not provided.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def quoted_spot(self) -> float | None:
        """Read the contract's quoted spot.

        Returns
        -------
        float | None
            Optional quoted FX spot rate (quote per base); None reads the FxMatrix.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def domestic_discount_curve_id(self) -> str | None:
        """Read the contract's domestic discount curve id.

        Returns
        -------
        str | None
            Optional quote-currency discount curve for PV-ing the settlement cashflow. When absent the settlement amount is reported undiscounted (a 1–2 day effect for standard spot lags).

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
            Notional amount in base currency.

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
            Optional base currency calendar for joint calendar settlement adjustment. Per market convention, FX settlement uses the joint calendar of both currencies. A date is a good business day only if it's valid in both calendars.

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
            Optional quote currency calendar for joint calendar settlement adjustment. Per market convention, FX settlement uses the joint calendar of both currencies. A date is a good business day only if it's valid in both calendars.

        Notes
        -----
        This property does not raise.
        """
        ...

class FxSpotBuilder:
    """Fluent builder for contracts with explicit lifecycle terms.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import FxSpot
    >>> builder = FxSpot.example().to_builder()
    >>> builder.id("RISK-TIPS").build().id
    'RISK-TIPS'
    """

    def build(self) -> FxSpot:
        """Validate the contractual terms and consume the builder.

        Returns
        -------
        FxSpot
            Native instrument ready for pricing against the selected market.

        Raises
        ------
        ValueError
            If a required field is missing, the terms are inconsistent, or the
            builder has already been consumed.
        """
        ...

    def id(self, value: str) -> FxSpotBuilder:
        """Set the FX spot trade identifier.

        Parameters
        ----------
        value : str
            Stable contract identifier used for pricing and metric keys.

        Returns
        -------
        FxSpotBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def base_currency(self, value: Currency | str) -> FxSpotBuilder:
        """Set the contract's base currency.

        Parameters
        ----------
        value : Currency | str
            Base currency (the currency being priced)

        Returns
        -------
        FxSpotBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def quote_currency(self, value: Currency | str) -> FxSpotBuilder:
        """Set the contract's quote currency.

        Parameters
        ----------
        value : Currency | str
            Quote currency (the currency used for pricing)

        Returns
        -------
        FxSpotBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def settlement_date(self, value: date | str | None) -> FxSpotBuilder:
        """Set the contract's settlement date.

        Parameters
        ----------
        value : date | str | None
            Actual settlement (value) date, used as given; when set, settlement_days is ignored.

        Returns
        -------
        FxSpotBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def settlement_days(self, value: int | None) -> FxSpotBuilder:
        """Set the contract's settlement days.

        Parameters
        ----------
        value : int | None
            Contractual spot lag in business days on the joint calendar, required when settlement_date is not provided.

        Returns
        -------
        FxSpotBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def quoted_spot(self, value: float | None) -> FxSpotBuilder:
        """Set the contract's quoted spot.

        Parameters
        ----------
        value : float | None
            Optional quoted FX spot rate (quote per base); None reads the FxMatrix.

        Returns
        -------
        FxSpotBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def domestic_discount_curve_id(self, value: str | None) -> FxSpotBuilder:
        """Set the contract's domestic discount curve id.

        Parameters
        ----------
        value : str | None
            Optional quote-currency discount curve for PV-ing the settlement cashflow. When absent the settlement amount is reported undiscounted (a 1–2 day effect for standard spot lags).

        Returns
        -------
        FxSpotBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def notional(self, value: Money) -> FxSpotBuilder:
        """Set the contract's notional.

        Parameters
        ----------
        value : Money
            Notional amount in base currency.

        Returns
        -------
        FxSpotBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def base_calendar_id(self, value: str | None) -> FxSpotBuilder:
        """Set the contract's base calendar id.

        Parameters
        ----------
        value : str | None
            Optional base currency calendar for joint calendar settlement adjustment. Per market convention, FX settlement uses the joint calendar of both currencies. A date is a good business day only if it's valid in both calendars.

        Returns
        -------
        FxSpotBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def quote_calendar_id(self, value: str | None) -> FxSpotBuilder:
        """Set the contract's quote calendar id.

        Parameters
        ----------
        value : str | None
            Optional quote currency calendar for joint calendar settlement adjustment. Per market convention, FX settlement uses the joint calendar of both currencies. A date is a good business day only if it's valid in both calendars.

        Returns
        -------
        FxSpotBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def attributes(self, value: Attributes | dict[str, Any]) -> FxSpotBuilder:
        """Set the FX spot trade grouping and filter attributes.

        Parameters
        ----------
        value : Attributes | dict[str, Any]
            Tags and literal metadata carried with the contract.

        Returns
        -------
        FxSpotBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def instrument_pricing_overrides(self, value: dict[str, object]) -> FxSpotBuilder:
        """
        Set the instrument-owned pricing inputs.

        Parameters
        ----------
        value : dict[str, object]
            Rust ``InstrumentPricingOverrides`` in serde form (a ``dict``): ``{"market_quotes": {...}, "model_config": {...}}``, e.g.
            ``{"market_quotes": {"quoted_clean_price_pct": 99.0}}``.

        Returns
        -------
        FxSpotBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``InstrumentPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def metric_pricing_overrides(self, value: MetricPricingOverrides | dict[str, object]) -> FxSpotBuilder:
        """
        Set the metric-time pricing configuration stored on the instrument.

        Parameters
        ----------
        value : MetricPricingOverrides | dict[str, object]
            Rust ``MetricPricingOverrides`` as the typed class or a ``dict``.

        Returns
        -------
        FxSpotBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``MetricPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def scenario_pricing_overrides(self, value: dict[str, object]) -> FxSpotBuilder:
        """
        Set the scenario-only pricing adjustments.

        Parameters
        ----------
        value : dict[str, object]
            Rust ``ScenarioPricingOverrides`` in serde form (a ``dict``), e.g. ``{"scenario_price_shock_decimal": -0.02}``.

        Returns
        -------
        FxSpotBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``ScenarioPricingOverrides`` (unknown fields are rejected).
        """
        ...
