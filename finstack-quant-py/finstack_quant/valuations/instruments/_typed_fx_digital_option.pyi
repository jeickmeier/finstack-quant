"""Typed FX digital option and its builder."""

from __future__ import annotations

from datetime import date
from typing import Any
from finstack_quant.core.dates import DayCount
from finstack_quant.core.money import Money
from finstack_quant.core.currency import Currency
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.types import Attributes
from finstack_quant.valuations.instruments._pricing import PriceExtras, MetricPricingOverrides

class FxDigitalOption:
    """Binary currency option linked to an exchange-rate threshold and payout terms.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import FxDigitalOption
    >>> isinstance(FxDigitalOption.example(), FxDigitalOption)
    True
    """

    @staticmethod
    def builder() -> FxDigitalOptionBuilder:
        """Start authoring an FX digital option.

        Returns
        -------
        FxDigitalOptionBuilder
            Empty builder for the exact contractual terms.

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import FxDigitalOption
        >>> _ = FxDigitalOption.builder().id("example-fx-digital-option")
        """
        ...

    def to_builder(self) -> FxDigitalOptionBuilder:
        """Copy the current terms into an editable builder.

        Returns
        -------
        FxDigitalOptionBuilder
            Builder retaining contractual terms, market identifiers and pricing overrides.

        Notes
        -----
        This method does not raise.
        """
        ...

    @staticmethod
    def example() -> FxDigitalOption:
        """Construct the built-in FX digital option example.

        Returns
        -------
        FxDigitalOption
            Validated contract with its lifecycle and market dependencies.

        Raises
        ------
        ValueError
            If the built-in instrument fixture is invalid.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import FxDigitalOption
        >>> FxDigitalOption.example().id != ""
        True
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> FxDigitalOption:
        """
        Deserialize a validated FxDigitalOption from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"fx_digital_option"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        FxDigitalOption
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import FxDigitalOption
        >>> contract = FxDigitalOption.example()
        >>> FxDigitalOption.from_json(contract.to_json()).id == contract.id != ""
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
            and :meth:`FxDigitalOption.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def to_dict(self) -> dict[str, object]:
        """
        Canonical Rust terms for this FX digital option as a plain Python ``dict``.

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

    def greeks(
        self,
        market: MarketContext,
        as_of: date | str,
        model: str = "default",
        extras: PriceExtras | dict[str, Any] | str | None = None,
    ) -> dict[str, float]:
        """Compute the first- and second-order price sensitivities supported by the selected model.

        Parameters
        ----------
        market : MarketContext
            Snapshot containing the configured FX, carry and volatility inputs.
        as_of : date or str
            Valuation date used to determine the remaining option life and observed state.
        model : str
            Registered engine name. The default uses the instrument's standard model.
        extras : PriceExtras or dict or str, optional
            Request configuration and overrides accepted by ``price_instrument``.
            Omit to use the stored instrument controls.

        Returns
        -------
        dict[str, float]
            Applicable Greeks keyed by their native metric identifiers and in their native units.

        Raises
        ------
        ValueError
            If the date or engine is invalid.
        KeyError
            If a required market input is missing.
        RuntimeError
            If the selected pricing or sensitivity calculation fails.
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
            Base currency (foreign currency)

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
            Quote currency (domestic currency)

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def strike(self) -> float:
        """Read the contract's strike.

        Returns
        -------
        float
            Strike exchange rate (quote per base)

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def option_type(self) -> object:
        """Read the contract's option type.

        Returns
        -------
        object
            Option type (call or put on base currency)

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def payout_type(self) -> object:
        """Read the contract's payout type.

        Returns
        -------
        object
            Payout type (cash-or-nothing or asset-or-nothing)

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def payout_amount(self) -> Money:
        """Read the contract's payout amount.

        Returns
        -------
        Money
            Fixed payout amount (used for cash-or-nothing; for asset-or-nothing this is the notional of foreign currency delivered)

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def expiry(self) -> date:
        """Read the contract's expiry.

        Returns
        -------
        date
            Option expiry date

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def day_count(self) -> DayCount:
        """Read the contract's day count.

        Returns
        -------
        DayCount
            Day count convention (defaults to ACT/365F).

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
            Notional amount in base currency

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
            Domestic currency discount curve ID

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
            Foreign currency discount curve ID

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def vol_surface_id(self) -> str:
        """Read the contract's vol surface id.

        Returns
        -------
        str
            FX volatility surface ID

        Notes
        -----
        This property does not raise.
        """
        ...

class FxDigitalOptionBuilder:
    """Fluent builder for contracts with explicit lifecycle terms.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import FxDigitalOption
    >>> builder = FxDigitalOption.example().to_builder()
    >>> builder.id("RISK-TIPS").build().id
    'RISK-TIPS'
    """

    def build(self) -> FxDigitalOption:
        """Validate the contractual terms and consume the builder.

        Returns
        -------
        FxDigitalOption
            Native instrument ready for pricing against the selected market.

        Raises
        ------
        ValueError
            If a required field is missing, the terms are inconsistent, or the
            builder has already been consumed.
        """
        ...

    def id(self, value: str) -> FxDigitalOptionBuilder:
        """Set the FX digital option identifier.

        Parameters
        ----------
        value : str
            Stable contract identifier used for pricing and metric keys.

        Returns
        -------
        FxDigitalOptionBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def base_currency(self, value: Currency | str) -> FxDigitalOptionBuilder:
        """Set the contract's base currency.

        Parameters
        ----------
        value : Currency | str
            Base currency (foreign currency)

        Returns
        -------
        FxDigitalOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def quote_currency(self, value: Currency | str) -> FxDigitalOptionBuilder:
        """Set the contract's quote currency.

        Parameters
        ----------
        value : Currency | str
            Quote currency (domestic currency)

        Returns
        -------
        FxDigitalOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def strike(self, value: float) -> FxDigitalOptionBuilder:
        """Set the contract's strike.

        Parameters
        ----------
        value : float
            Strike exchange rate (quote per base)

        Returns
        -------
        FxDigitalOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def option_type(self, value: object) -> FxDigitalOptionBuilder:
        """Set the contract's option type.

        Parameters
        ----------
        value : object
            Option type (call or put on base currency)

        Returns
        -------
        FxDigitalOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def payout_type(self, value: object) -> FxDigitalOptionBuilder:
        """Set the contract's payout type.

        Parameters
        ----------
        value : object
            Payout type (cash-or-nothing or asset-or-nothing)

        Returns
        -------
        FxDigitalOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def payout_amount(self, value: Money) -> FxDigitalOptionBuilder:
        """Set the contract's payout amount.

        Parameters
        ----------
        value : Money
            Fixed payout amount (used for cash-or-nothing; for asset-or-nothing this is the notional of foreign currency delivered)

        Returns
        -------
        FxDigitalOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def expiry(self, value: date | str) -> FxDigitalOptionBuilder:
        """Set the contract's expiry.

        Parameters
        ----------
        value : date | str
            Option expiry date

        Returns
        -------
        FxDigitalOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def day_count(self, value: DayCount | str) -> FxDigitalOptionBuilder:
        """Set the contract's day count.

        Parameters
        ----------
        value : DayCount | str
            Day count convention (defaults to ACT/365F).

        Returns
        -------
        FxDigitalOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def notional(self, value: Money) -> FxDigitalOptionBuilder:
        """Set the contract's notional.

        Parameters
        ----------
        value : Money
            Notional amount in base currency

        Returns
        -------
        FxDigitalOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def domestic_discount_curve_id(self, value: str) -> FxDigitalOptionBuilder:
        """Set the contract's domestic discount curve id.

        Parameters
        ----------
        value : str
            Domestic currency discount curve ID

        Returns
        -------
        FxDigitalOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def foreign_discount_curve_id(self, value: str) -> FxDigitalOptionBuilder:
        """Set the contract's foreign discount curve id.

        Parameters
        ----------
        value : str
            Foreign currency discount curve ID

        Returns
        -------
        FxDigitalOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def vol_surface_id(self, value: str) -> FxDigitalOptionBuilder:
        """Set the contract's vol surface id.

        Parameters
        ----------
        value : str
            FX volatility surface ID

        Returns
        -------
        FxDigitalOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def attributes(self, value: Attributes | dict[str, Any]) -> FxDigitalOptionBuilder:
        """Set the FX digital option grouping and filter attributes.

        Parameters
        ----------
        value : Attributes | dict[str, Any]
            Tags and literal metadata carried with the contract.

        Returns
        -------
        FxDigitalOptionBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def instrument_pricing_overrides(self, value: dict[str, object] | str) -> FxDigitalOptionBuilder:
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
        FxDigitalOptionBuilder
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
    ) -> FxDigitalOptionBuilder:
        """
        Set the metric-time pricing configuration stored on the instrument.

        Parameters
        ----------
        value : MetricPricingOverrides | dict[str, object] | str
            Rust ``MetricPricingOverrides`` as the typed class, a ``dict`` or a
            JSON string.

        Returns
        -------
        FxDigitalOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``MetricPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def scenario_pricing_overrides(self, value: dict[str, object] | str) -> FxDigitalOptionBuilder:
        """
        Set the scenario-only pricing adjustments.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``ScenarioPricingOverrides`` in serde form (``dict`` or JSON
            string), e.g. ``{"scenario_price_shock_decimal": -0.02}``.

        Returns
        -------
        FxDigitalOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``ScenarioPricingOverrides`` (unknown fields are rejected).
        """
        ...
