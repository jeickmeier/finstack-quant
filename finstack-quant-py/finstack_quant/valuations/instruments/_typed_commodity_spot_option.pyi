"""Typed commodity spot option and its builder."""

from __future__ import annotations

from datetime import date
from typing import Any
from finstack_quant.core.dates import DayCount
from finstack_quant.core.types import Attributes
from finstack_quant.valuations.instruments._pricing import MetricPricingOverrides

class CommoditySpotOption:
    """Option on a spot commodity price.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import CommoditySpotOption
    >>> isinstance(CommoditySpotOption.example(), CommoditySpotOption)
    True
    """

    @staticmethod
    def builder() -> CommoditySpotOptionBuilder:
        """Start authoring a commodity spot option.

        Returns
        -------
        CommoditySpotOptionBuilder
            Empty builder for the exact contractual terms.

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CommoditySpotOption
        >>> _ = CommoditySpotOption.builder().id("example-commodity-spot-option")
        """
        ...

    def to_builder(self) -> CommoditySpotOptionBuilder:
        """Copy the current terms into an editable builder.

        Returns
        -------
        CommoditySpotOptionBuilder
            Builder retaining contractual terms, market identifiers and pricing overrides.

        Notes
        -----
        This method does not raise.
        """
        ...

    @staticmethod
    def example() -> CommoditySpotOption:
        """Construct the built-in commodity spot option example.

        Returns
        -------
        CommoditySpotOption
            Validated contract with its lifecycle and market dependencies.

        Raises
        ------
        ValueError
            If the built-in instrument fixture is invalid.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CommoditySpotOption
        >>> CommoditySpotOption.example().id != ""
        True
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> CommoditySpotOption:
        """
        Deserialize a validated CommoditySpotOption from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"commodity_spot_option"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        CommoditySpotOption
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CommoditySpotOption
        >>> future = CommoditySpotOption.example()
        >>> CommoditySpotOption.from_json(future.to_json()).id == future.id != ""
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
            and :meth:`CommoditySpotOption.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def to_dict(self) -> dict[str, object]:
        """
        Canonical Rust terms for this commodity spot option as a plain Python ``dict``.

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
    def strike(self) -> float:
        """Read the contract's strike.

        Returns
        -------
        float
            Strike price per unit.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def option_type(self) -> dict[str, object] | str:
        """Read the contract's option type.

        Returns
        -------
        dict[str, object] | str
            Option type (call or put).

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def exercise_style(self) -> dict[str, object] | str:
        """Read the contract's exercise style.

        Returns
        -------
        dict[str, object] | str
            Exercise style (European or American).

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def exercise_dates(self) -> list[date] | None:
        """Read the contract's exercise dates.

        Returns
        -------
        list[date] | None
            Optional Bermudan exercise schedule. Required when exercise_style == ExerciseStyle::Bermudan.

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
            Option expiry date.

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
            Contract multiplier (typically 1.0 for OTC options).

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
            Settlement type (physical or cash). Cash fixes the payoff on exercise; physical retains the marked delivery claim until its recorded settlement date.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def exercise(self) -> dict[str, object] | str | None:
        """Read the contract's exercise.

        Returns
        -------
        dict[str, object] | str | None
            Recorded exercise or expiry; required after expiry while settlement remains unpaid.

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
            Forward/futures curve ID for price interpolation.

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
            Discount curve ID for present value.

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
            Volatility surface ID for implied vol.

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
            Optional spot price ID (for spot-based pricing and American options). Numeric prices use the underlying currency; Money prices must match it.

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
            Optional quoted forward price (overrides curve lookup).

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
            Day count convention for time to expiry.

        Notes
        -----
        This property does not raise.
        """
        ...

class CommoditySpotOptionBuilder:
    """Fluent builder for contracts with explicit lifecycle terms.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import CommoditySpotOption
    >>> builder = CommoditySpotOption.example().to_builder()
    >>> builder.id("RISK-TIPS").build().id
    'RISK-TIPS'
    """

    def build(self) -> CommoditySpotOption:
        """Validate the contractual terms and consume the builder.

        Returns
        -------
        CommoditySpotOption
            Native instrument ready for pricing against the selected market.

        Raises
        ------
        ValueError
            If a required field is missing, the terms are inconsistent, or the
            builder has already been consumed.
        """
        ...

    def id(self, value: str) -> CommoditySpotOptionBuilder:
        """Set the commodity spot option identifier.

        Parameters
        ----------
        value : str
            Stable contract identifier used for pricing and metric keys.

        Returns
        -------
        CommoditySpotOptionBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def underlying(self, value: dict[str, object] | str) -> CommoditySpotOptionBuilder:
        """Set the contract's underlying.

        Parameters
        ----------
        value : dict[str, object] | str
            Commodity underlying parameters (commodity_type, underlying_ticker, unit, currency).

        Returns
        -------
        CommoditySpotOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def strike(self, value: float) -> CommoditySpotOptionBuilder:
        """Set the contract's strike.

        Parameters
        ----------
        value : float
            Strike price per unit.

        Returns
        -------
        CommoditySpotOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def option_type(self, value: dict[str, object] | str) -> CommoditySpotOptionBuilder:
        """Set the contract's option type.

        Parameters
        ----------
        value : dict[str, object] | str
            Option type (call or put).

        Returns
        -------
        CommoditySpotOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def exercise_style(self, value: dict[str, object] | str) -> CommoditySpotOptionBuilder:
        """Set the contract's exercise style.

        Parameters
        ----------
        value : dict[str, object] | str
            Exercise style (European or American).

        Returns
        -------
        CommoditySpotOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def exercise_dates(self, value: list[date | str] | None) -> CommoditySpotOptionBuilder:
        """Set the contract's exercise dates.

        Parameters
        ----------
        value : list[date | str] | None
            Optional Bermudan exercise schedule. Required when exercise_style == ExerciseStyle::Bermudan.

        Returns
        -------
        CommoditySpotOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def expiry(self, value: date | str) -> CommoditySpotOptionBuilder:
        """Set the contract's expiry.

        Parameters
        ----------
        value : date | str
            Option expiry date.

        Returns
        -------
        CommoditySpotOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def quantity(self, value: float) -> CommoditySpotOptionBuilder:
        """Set the contract's quantity.

        Parameters
        ----------
        value : float
            Contract quantity in units.

        Returns
        -------
        CommoditySpotOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def multiplier(self, value: float) -> CommoditySpotOptionBuilder:
        """Set the contract's multiplier.

        Parameters
        ----------
        value : float
            Contract multiplier (typically 1.0 for OTC options).

        Returns
        -------
        CommoditySpotOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def settlement(self, value: dict[str, object] | str) -> CommoditySpotOptionBuilder:
        """Set the contract's settlement.

        Parameters
        ----------
        value : dict[str, object] | str
            Settlement type (physical or cash). Cash fixes the payoff on exercise; physical retains the marked delivery claim until its recorded settlement date.

        Returns
        -------
        CommoditySpotOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def exercise(self, value: dict[str, object] | str | None) -> CommoditySpotOptionBuilder:
        """Set the contract's exercise.

        Parameters
        ----------
        value : dict[str, object] | str | None
            Recorded exercise or expiry; required after expiry while settlement remains unpaid.

        Returns
        -------
        CommoditySpotOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def forward_curve_id(self, value: str) -> CommoditySpotOptionBuilder:
        """Set the contract's forward curve id.

        Parameters
        ----------
        value : str
            Forward/futures curve ID for price interpolation.

        Returns
        -------
        CommoditySpotOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def discount_curve_id(self, value: str) -> CommoditySpotOptionBuilder:
        """Set the contract's discount curve id.

        Parameters
        ----------
        value : str
            Discount curve ID for present value.

        Returns
        -------
        CommoditySpotOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def vol_surface_id(self, value: str) -> CommoditySpotOptionBuilder:
        """Set the contract's vol surface id.

        Parameters
        ----------
        value : str
            Volatility surface ID for implied vol.

        Returns
        -------
        CommoditySpotOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def spot_id(self, value: str | None) -> CommoditySpotOptionBuilder:
        """Set the contract's spot id.

        Parameters
        ----------
        value : str | None
            Optional spot price ID (for spot-based pricing and American options). Numeric prices use the underlying currency; Money prices must match it.

        Returns
        -------
        CommoditySpotOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def quoted_forward(self, value: float | None) -> CommoditySpotOptionBuilder:
        """Set the contract's quoted forward.

        Parameters
        ----------
        value : float | None
            Optional quoted forward price (overrides curve lookup).

        Returns
        -------
        CommoditySpotOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def day_count(self, value: DayCount | str) -> CommoditySpotOptionBuilder:
        """Set the contract's day count.

        Parameters
        ----------
        value : DayCount | str
            Day count convention for time to expiry.

        Returns
        -------
        CommoditySpotOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def attributes(self, value: Attributes | dict[str, Any]) -> CommoditySpotOptionBuilder:
        """Set the commodity spot option grouping and filter attributes.

        Parameters
        ----------
        value : Attributes | dict[str, Any]
            Tags and literal metadata carried with the contract.

        Returns
        -------
        CommoditySpotOptionBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def instrument_pricing_overrides(self, value: dict[str, object] | str) -> CommoditySpotOptionBuilder:
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
        CommoditySpotOptionBuilder
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
    ) -> CommoditySpotOptionBuilder:
        """
        Set the metric-time pricing configuration stored on the instrument.

        Parameters
        ----------
        value : MetricPricingOverrides | dict[str, object] | str
            Rust ``MetricPricingOverrides`` as the typed class, a ``dict`` or a
            JSON string.

        Returns
        -------
        CommoditySpotOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``MetricPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def scenario_pricing_overrides(self, value: dict[str, object] | str) -> CommoditySpotOptionBuilder:
        """
        Set the scenario-only pricing adjustments.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``ScenarioPricingOverrides`` in serde form (``dict`` or JSON
            string), e.g. ``{"scenario_price_shock_decimal": -0.02}``.

        Returns
        -------
        CommoditySpotOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``ScenarioPricingOverrides`` (unknown fields are rejected).
        """
        ...
