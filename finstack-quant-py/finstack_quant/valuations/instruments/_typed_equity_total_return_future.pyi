"""Typed equity total-return future and its builder."""

from __future__ import annotations

import datetime
from datetime import date
from typing import Any
from finstack_quant.core.types import Attributes
from finstack_quant.valuations.instruments._pricing import MetricPricingOverrides

from finstack_quant.core.dates import DayCount
from finstack_quant.valuations.instruments._typed_listed import ListedFutureTerms

class EquityTotalReturnFuture:
    """Exchange-listed future on the total return of an equity or index.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import EquityTotalReturnFuture
    >>> isinstance(EquityTotalReturnFuture.example(), EquityTotalReturnFuture)
    True
    """

    @staticmethod
    def builder() -> EquityTotalReturnFutureBuilder:
        """Start authoring an equity total-return future.

        Returns
        -------
        EquityTotalReturnFutureBuilder
            Empty builder for the exact contractual terms.

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import EquityTotalReturnFuture
        >>> _ = EquityTotalReturnFuture.builder().id("example-equity-total-return-future")
        """
        ...

    def to_builder(self) -> EquityTotalReturnFutureBuilder:
        """Copy the current terms into an editable builder.

        Returns
        -------
        EquityTotalReturnFutureBuilder
            Builder retaining contractual terms, market identifiers and pricing overrides.

        Notes
        -----
        This method does not raise.
        """
        ...

    @staticmethod
    def example() -> EquityTotalReturnFuture:
        """Construct the built-in equity total-return future example.

        Returns
        -------
        EquityTotalReturnFuture
            Validated contract with its lifecycle and market dependencies.

        Raises
        ------
        ValueError
            If the built-in instrument fixture is invalid.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import EquityTotalReturnFuture
        >>> EquityTotalReturnFuture.example().id != ""
        True
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> EquityTotalReturnFuture:
        """
        Deserialize a validated EquityTotalReturnFuture from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"equity_total_return_future"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        EquityTotalReturnFuture
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import EquityTotalReturnFuture
        >>> future = EquityTotalReturnFuture.example()
        >>> EquityTotalReturnFuture.from_json(future.to_json()).id == future.id != ""
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
            and :meth:`EquityTotalReturnFuture.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def to_dict(self) -> dict[str, object]:
        """
        Canonical Rust terms for this equity total-return future as a plain Python ``dict``.

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
    def underlying_ticker(self) -> str:
        """Read the contract's underlying ticker.

        Returns
        -------
        str
            Equity or equity-index ticker.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def terms(self) -> ListedFutureTerms:
        """Read the contract's terms.

        Returns
        -------
        ListedFutureTerms
            Standard listed position and lifecycle terms.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def spot_id(self) -> str:
        """Read the contract's spot id.

        Returns
        -------
        str
            Current underlying close or exchange-prescribed reference level.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def accrued_distributions_id(self) -> str:
        """Read the contract's accrued distributions id.

        Returns
        -------
        str
            Cumulative distribution points published by the exchange.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def accrued_funding_id(self) -> str:
        """Read the contract's accrued funding id.

        Returns
        -------
        str
            Cumulative funding points published by the exchange.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def spread_bp_id(self) -> str:
        """Read the contract's spread bp id.

        Returns
        -------
        str
            Current annualized TRF spread scalar expressed in basis points.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def spread_day_count(self) -> DayCount:
        """Read the contract's spread day count.

        Returns
        -------
        DayCount
            Day-count basis used to convert the annualized spread into index points.

        Notes
        -----
        This property does not raise.
        """
        ...

class EquityTotalReturnFutureBuilder:
    """Fluent builder for contracts with explicit lifecycle terms.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import EquityTotalReturnFuture
    >>> builder = EquityTotalReturnFuture.example().to_builder()
    >>> builder.id("RISK-TIPS").build().id
    'RISK-TIPS'
    """

    def build(self) -> EquityTotalReturnFuture:
        """Validate the contractual terms and consume the builder.

        Returns
        -------
        EquityTotalReturnFuture
            Native instrument ready for pricing against the selected market.

        Raises
        ------
        ValueError
            If a required field is missing, the terms are inconsistent, or the
            builder has already been consumed.
        """
        ...

    def id(self, value: str) -> EquityTotalReturnFutureBuilder:
        """Set the equity total-return future identifier.

        Parameters
        ----------
        value : str
            Stable contract identifier used for pricing and metric keys.

        Returns
        -------
        EquityTotalReturnFutureBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def underlying_ticker(self, value: str) -> EquityTotalReturnFutureBuilder:
        """Set the contract's underlying ticker.

        Parameters
        ----------
        value : str
            Equity or equity-index ticker.

        Returns
        -------
        EquityTotalReturnFutureBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def terms(self, value: ListedFutureTerms) -> EquityTotalReturnFutureBuilder:
        """Set the contract's terms.

        Parameters
        ----------
        value : ListedFutureTerms
            Standard listed position and lifecycle terms.

        Returns
        -------
        EquityTotalReturnFutureBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def spot_id(self, value: str) -> EquityTotalReturnFutureBuilder:
        """Set the contract's spot id.

        Parameters
        ----------
        value : str
            Current underlying close or exchange-prescribed reference level.

        Returns
        -------
        EquityTotalReturnFutureBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def accrued_distributions_id(self, value: str) -> EquityTotalReturnFutureBuilder:
        """Set the contract's accrued distributions id.

        Parameters
        ----------
        value : str
            Cumulative distribution points published by the exchange.

        Returns
        -------
        EquityTotalReturnFutureBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def accrued_funding_id(self, value: str) -> EquityTotalReturnFutureBuilder:
        """Set the contract's accrued funding id.

        Parameters
        ----------
        value : str
            Cumulative funding points published by the exchange.

        Returns
        -------
        EquityTotalReturnFutureBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def spread_bp_id(self, value: str) -> EquityTotalReturnFutureBuilder:
        """Set the contract's spread bp id.

        Parameters
        ----------
        value : str
            Current annualized TRF spread scalar expressed in basis points.

        Returns
        -------
        EquityTotalReturnFutureBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def spread_day_count(self, value: DayCount | str) -> EquityTotalReturnFutureBuilder:
        """Set the contract's spread day count.

        Parameters
        ----------
        value : DayCount | str
            Day-count basis used to convert the annualized spread into index points.

        Returns
        -------
        EquityTotalReturnFutureBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def attributes(self, value: Attributes | dict[str, Any]) -> EquityTotalReturnFutureBuilder:
        """Set the equity total-return future grouping and filter attributes.

        Parameters
        ----------
        value : Attributes | dict[str, Any]
            Tags and literal metadata carried with the contract.

        Returns
        -------
        EquityTotalReturnFutureBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def instrument_pricing_overrides(self, value: dict[str, object]) -> EquityTotalReturnFutureBuilder:
        """
        Set the instrument-owned pricing inputs.

        Parameters
        ----------
        value : dict[str, object]
            Rust ``InstrumentPricingOverrides`` in serde form (a ``dict``): ``{"market_quotes": {...}, "model_config": {...}}``, e.g.
            ``{"market_quotes": {"quoted_clean_price_pct": 99.0}}``.

        Returns
        -------
        EquityTotalReturnFutureBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``InstrumentPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def metric_pricing_overrides(
        self, value: MetricPricingOverrides | dict[str, object]
    ) -> EquityTotalReturnFutureBuilder:
        """
        Set the metric-time pricing configuration stored on the instrument.

        Parameters
        ----------
        value : MetricPricingOverrides | dict[str, object]
            Rust ``MetricPricingOverrides`` as the typed class or a ``dict``.

        Returns
        -------
        EquityTotalReturnFutureBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``MetricPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def scenario_pricing_overrides(self, value: dict[str, object]) -> EquityTotalReturnFutureBuilder:
        """
        Set the scenario-only pricing adjustments.

        Parameters
        ----------
        value : dict[str, object]
            Rust ``ScenarioPricingOverrides`` in serde form (a ``dict``), e.g. ``{"scenario_price_shock_decimal": -0.02}``.

        Returns
        -------
        EquityTotalReturnFutureBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``ScenarioPricingOverrides`` (unknown fields are rejected).
        """
        ...
