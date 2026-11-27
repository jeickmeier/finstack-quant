"""Typed basis swap and its builder."""

from __future__ import annotations

import datetime
from datetime import date
from typing import Any
from finstack_quant.core.money import Money
from finstack_quant.valuations.instruments._typed_legs import FloatLegSpec
from finstack_quant.core.types import Attributes
from finstack_quant.valuations.instruments._pricing import MetricPricingOverrides

class BasisSwap:
    """Swap that exchanges two floating-rate legs with separate index conventions.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import BasisSwap
    >>> isinstance(BasisSwap.example(), BasisSwap)
    True
    """

    @staticmethod
    def builder() -> BasisSwapBuilder:
        """Start authoring a basis swap.

        Returns
        -------
        BasisSwapBuilder
            Empty builder for the exact contractual terms.

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import BasisSwap
        >>> _ = BasisSwap.builder().id("example-basis-swap")
        """
        ...

    def to_builder(self) -> BasisSwapBuilder:
        """Copy the current terms into an editable builder.

        Returns
        -------
        BasisSwapBuilder
            Builder retaining contractual terms, market identifiers and pricing overrides.

        Notes
        -----
        This method does not raise.
        """
        ...

    @staticmethod
    def example() -> BasisSwap:
        """Construct the built-in basis swap example.

        Returns
        -------
        BasisSwap
            Validated contract with its lifecycle and market dependencies.

        Raises
        ------
        ValueError
            If the built-in instrument fixture is invalid.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import BasisSwap
        >>> BasisSwap.example().id != ""
        True
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> BasisSwap:
        """
        Deserialize a validated BasisSwap from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"basis_swap"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        BasisSwap
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import BasisSwap
        >>> future = BasisSwap.example()
        >>> BasisSwap.from_json(future.to_json()).id == future.id != ""
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
            and :meth:`BasisSwap.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def to_dict(self) -> dict[str, object]:
        """
        Canonical Rust terms for this basis swap as a plain Python ``dict``.

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
    def notional(self) -> Money:
        """Read the contract's notional.

        Returns
        -------
        Money
            Notional amount for both legs.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def primary_leg(self) -> FloatLegSpec:
        """Read the contract's primary leg.

        Returns
        -------
        FloatLegSpec
            Primary leg that typically receives the spread.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def reference_leg(self) -> FloatLegSpec:
        """Read the contract's reference leg.

        Returns
        -------
        FloatLegSpec
            Reference leg that typically pays flat.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def allow_calendar_fallback(self) -> bool:
        """Read the contract's allow calendar fallback.

        Returns
        -------
        bool
            Allow calendar-day fallback when the calendar cannot be resolved. When false (default), missing calendars are treated as an input error to avoid silently misaligning schedule and payment-lag conventions.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def allow_same_curve(self) -> bool:
        """Read the contract's allow same curve.

        Returns
        -------
        bool
            Allow both legs to reference the same forward curve. When false (default), having identical forward curves on both legs produces a validation error, as this is almost always a configuration mistake (NPV would equal spread × annuity by construction). Set to true only for testing or deliberate same-index spread trades.

        Notes
        -----
        This property does not raise.
        """
        ...

class BasisSwapBuilder:
    """Fluent builder for contracts with explicit lifecycle terms.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import BasisSwap
    >>> builder = BasisSwap.example().to_builder()
    >>> builder.id("RISK-TIPS").build().id
    'RISK-TIPS'
    """

    def build(self) -> BasisSwap:
        """Validate the contractual terms and consume the builder.

        Returns
        -------
        BasisSwap
            Native instrument ready for pricing against the selected market.

        Raises
        ------
        ValueError
            If a required field is missing, the terms are inconsistent, or the
            builder has already been consumed.
        """
        ...

    def id(self, value: str) -> BasisSwapBuilder:
        """Set the basis swap identifier.

        Parameters
        ----------
        value : str
            Stable contract identifier used for pricing and metric keys.

        Returns
        -------
        BasisSwapBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def notional(self, value: Money) -> BasisSwapBuilder:
        """Set the contract's notional.

        Parameters
        ----------
        value : Money
            Notional amount for both legs.

        Returns
        -------
        BasisSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def primary_leg(self, value: FloatLegSpec) -> BasisSwapBuilder:
        """Set the contract's primary leg.

        Parameters
        ----------
        value : FloatLegSpec
            Primary leg that typically receives the spread.

        Returns
        -------
        BasisSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def reference_leg(self, value: FloatLegSpec) -> BasisSwapBuilder:
        """Set the contract's reference leg.

        Parameters
        ----------
        value : FloatLegSpec
            Reference leg that typically pays flat.

        Returns
        -------
        BasisSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def allow_calendar_fallback(self, value: bool) -> BasisSwapBuilder:
        """Set the contract's allow calendar fallback.

        Parameters
        ----------
        value : bool
            Allow calendar-day fallback when the calendar cannot be resolved. When false (default), missing calendars are treated as an input error to avoid silently misaligning schedule and payment-lag conventions.

        Returns
        -------
        BasisSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def allow_same_curve(self, value: bool) -> BasisSwapBuilder:
        """Set the contract's allow same curve.

        Parameters
        ----------
        value : bool
            Allow both legs to reference the same forward curve. When false (default), having identical forward curves on both legs produces a validation error, as this is almost always a configuration mistake (NPV would equal spread × annuity by construction). Set to true only for testing or deliberate same-index spread trades.

        Returns
        -------
        BasisSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def attributes(self, value: Attributes | dict[str, Any]) -> BasisSwapBuilder:
        """Set the basis swap grouping and filter attributes.

        Parameters
        ----------
        value : Attributes | dict[str, Any]
            Tags and literal metadata carried with the contract.

        Returns
        -------
        BasisSwapBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def instrument_pricing_overrides(self, value: dict[str, object]) -> BasisSwapBuilder:
        """
        Set the instrument-owned pricing inputs.

        Parameters
        ----------
        value : dict[str, object]
            Rust ``InstrumentPricingOverrides`` in serde form (a ``dict``): ``{"market_quotes": {...}, "model_config": {...}}``, e.g.
            ``{"market_quotes": {"quoted_clean_price_pct": 99.0}}``.

        Returns
        -------
        BasisSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``InstrumentPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def metric_pricing_overrides(self, value: MetricPricingOverrides | dict[str, object]) -> BasisSwapBuilder:
        """
        Set the metric-time pricing configuration stored on the instrument.

        Parameters
        ----------
        value : MetricPricingOverrides | dict[str, object]
            Rust ``MetricPricingOverrides`` as the typed class or a ``dict``.

        Returns
        -------
        BasisSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``MetricPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def scenario_pricing_overrides(self, value: dict[str, object]) -> BasisSwapBuilder:
        """
        Set the scenario-only pricing adjustments.

        Parameters
        ----------
        value : dict[str, object]
            Rust ``ScenarioPricingOverrides`` in serde form (a ``dict``), e.g. ``{"scenario_price_shock_decimal": -0.02}``.

        Returns
        -------
        BasisSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``ScenarioPricingOverrides`` (unknown fields are rejected).
        """
        ...
