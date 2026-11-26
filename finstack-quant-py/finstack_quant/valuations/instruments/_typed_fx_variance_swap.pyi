"""Typed FX variance swap and its builder."""

from __future__ import annotations

import datetime
from datetime import date
from typing import Any
from finstack_quant.core.money import Money
from finstack_quant.core.currency import Currency
from finstack_quant.core.dates import DayCount
from finstack_quant.core.dates import Tenor
from finstack_quant.core.types import Attributes
from finstack_quant.valuations.instruments._pricing import MetricPricingOverrides

class FxVarianceSwap:
    """Swap that exchanges realized exchange-rate variance for contractual variance.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import FxVarianceSwap
    >>> isinstance(FxVarianceSwap.example(), FxVarianceSwap)
    True
    """

    @staticmethod
    def builder() -> FxVarianceSwapBuilder:
        """Start authoring an FX variance swap.

        Returns
        -------
        FxVarianceSwapBuilder
            Empty builder for the exact contractual terms.

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import FxVarianceSwap
        >>> _ = FxVarianceSwap.builder().id("example-fx-variance-swap")
        """
        ...

    def to_builder(self) -> FxVarianceSwapBuilder:
        """Copy the current terms into an editable builder.

        Returns
        -------
        FxVarianceSwapBuilder
            Builder retaining contractual terms, market identifiers and pricing overrides.

        Notes
        -----
        This method does not raise.
        """
        ...

    @staticmethod
    def example() -> FxVarianceSwap:
        """Construct the built-in FX variance swap example.

        Returns
        -------
        FxVarianceSwap
            Validated contract with its lifecycle and market dependencies.

        Raises
        ------
        ValueError
            If the built-in instrument fixture is invalid.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import FxVarianceSwap
        >>> FxVarianceSwap.example().id != ""
        True
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> FxVarianceSwap:
        """
        Deserialize a validated FxVarianceSwap from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"fx_variance_swap"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        FxVarianceSwap
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import FxVarianceSwap
        >>> future = FxVarianceSwap.example()
        >>> FxVarianceSwap.from_json(future.to_json()).id == future.id != ""
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
            and :meth:`FxVarianceSwap.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def to_dict(self) -> dict[str, object]:
        """
        Canonical Rust terms for this FX variance swap as a plain Python ``dict``.

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
    def spot_id(self) -> str | None:
        """Read the contract's spot id.

        Returns
        -------
        str | None
            Optional spot identifier used to look up historical series.

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
            Variance notional (in quote currency units)

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def strike_variance(self) -> float:
        """Read the contract's strike variance.

        Returns
        -------
        float
            Strike variance (annualized)

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def start_date(self) -> date:
        """Read the contract's start date.

        Returns
        -------
        date
            Start date of observation period

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
            Contractual end of the observation period.

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
            Optional cash-settlement date. Defaults to the adjusted final observation date.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def observation_frequency(self) -> dict[str, object]:
        """Read the contract's observation frequency.

        Returns
        -------
        dict[str, object]
            Observation frequency

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def base_calendar_id(self) -> str:
        """Read the contract's base calendar id.

        Returns
        -------
        str
            Base-currency calendar used in the joint observation calendar.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def quote_calendar_id(self) -> str:
        """Read the contract's quote calendar id.

        Returns
        -------
        str
            Quote-currency calendar used in the joint observation calendar.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def observation_business_day_convention(self) -> str:
        """Read the contract's observation business day convention.

        Returns
        -------
        str
            Business-day convention applied to observation dates.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def observation_end_of_month(self) -> bool:
        """Read the contract's observation end of month.

        Returns
        -------
        bool
            Preserve month-end rolls for month/year observation frequencies.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def realized_var_method(self) -> dict[str, object] | str:
        """Read the contract's realized var method.

        Returns
        -------
        dict[str, object] | str
            Method for calculating realized variance (defaults to CloseToClose)

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def trading_days_per_year(self) -> float:
        """Read the contract's trading days per year.

        Returns
        -------
        float
            Trading days per year used to annualise daily realized variance. A contract term of the variance-swap confirmation: daily observations are annualised by trading_days_per_year / observation_frequency.count(). Must be finite and positive. Defaults to finstack_quant_valuations_core::constants::TRADING_DAYS_PER_YEAR (252).

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def open_series_id(self) -> str | None:
        """Read the contract's open series id.

        Returns
        -------
        str | None
            Series ID for open prices (required for Parkinson, GarmanKlass, RogersSatchell, YangZhang). Defaults to spot_id (or currency-pair string) when absent.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def high_series_id(self) -> str | None:
        """Read the contract's high series id.

        Returns
        -------
        str | None
            Series ID for high prices (required for Parkinson, GarmanKlass, RogersSatchell, YangZhang). Defaults to spot_id (or currency-pair string) when absent.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def low_series_id(self) -> str | None:
        """Read the contract's low series id.

        Returns
        -------
        str | None
            Series ID for low prices (required for Parkinson, GarmanKlass, RogersSatchell, YangZhang). Defaults to spot_id (or currency-pair string) when absent.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def close_series_id(self) -> str | None:
        """Read the contract's close series id.

        Returns
        -------
        str | None
            Series ID for close prices. Defaults to spot_id (or currency-pair string) when absent.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def previous_close_date(self) -> date | None:
        """Read the contract's previous close date.

        Returns
        -------
        date | None
            Contractual closing fixing immediately before the first Yang–Zhang target bar. Resolved exactly in the same close series as the target bars.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def side(self) -> dict[str, object] | str:
        """Read the contract's side.

        Returns
        -------
        dict[str, object] | str
            Pay/receive variance

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

    @property
    def day_count(self) -> DayCount:
        """Read the contract's day count.

        Returns
        -------
        DayCount
            Day count convention for time calculations (defaults to ACT/365F).

        Notes
        -----
        This property does not raise.
        """
        ...

class FxVarianceSwapBuilder:
    """Fluent builder for contracts with explicit lifecycle terms.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import FxVarianceSwap
    >>> builder = FxVarianceSwap.example().to_builder()
    >>> builder.id("RISK-TIPS").build().id
    'RISK-TIPS'
    """

    def build(self) -> FxVarianceSwap:
        """Validate the contractual terms and consume the builder.

        Returns
        -------
        FxVarianceSwap
            Native instrument ready for pricing against the selected market.

        Raises
        ------
        ValueError
            If a required field is missing, the terms are inconsistent, or the
            builder has already been consumed.
        """
        ...

    def id(self, value: str) -> FxVarianceSwapBuilder:
        """Set the FX variance swap identifier.

        Parameters
        ----------
        value : str
            Stable contract identifier used for pricing and metric keys.

        Returns
        -------
        FxVarianceSwapBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def base_currency(self, value: Currency | str) -> FxVarianceSwapBuilder:
        """Set the contract's base currency.

        Parameters
        ----------
        value : Currency | str
            Base currency (foreign)

        Returns
        -------
        FxVarianceSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def quote_currency(self, value: Currency | str) -> FxVarianceSwapBuilder:
        """Set the contract's quote currency.

        Parameters
        ----------
        value : Currency | str
            Quote currency (domestic)

        Returns
        -------
        FxVarianceSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def spot_id(self, value: str | None) -> FxVarianceSwapBuilder:
        """Set the contract's spot id.

        Parameters
        ----------
        value : str | None
            Optional spot identifier used to look up historical series.

        Returns
        -------
        FxVarianceSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def notional(self, value: Money) -> FxVarianceSwapBuilder:
        """Set the contract's notional.

        Parameters
        ----------
        value : Money
            Variance notional (in quote currency units)

        Returns
        -------
        FxVarianceSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def strike_variance(self, value: float) -> FxVarianceSwapBuilder:
        """Set the contract's strike variance.

        Parameters
        ----------
        value : float
            Strike variance (annualized)

        Returns
        -------
        FxVarianceSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def start_date(self, value: date | str) -> FxVarianceSwapBuilder:
        """Set the contract's start date.

        Parameters
        ----------
        value : date | str
            Start date of observation period

        Returns
        -------
        FxVarianceSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def maturity(self, value: date | str) -> FxVarianceSwapBuilder:
        """Set the contract's maturity.

        Parameters
        ----------
        value : date | str
            Contractual end of the observation period.

        Returns
        -------
        FxVarianceSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def settlement_date(self, value: date | str | None) -> FxVarianceSwapBuilder:
        """Set the contract's settlement date.

        Parameters
        ----------
        value : date | str | None
            Optional cash-settlement date. Defaults to the adjusted final observation date.

        Returns
        -------
        FxVarianceSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def observation_frequency(self, value: dict[str, object] | Tenor) -> FxVarianceSwapBuilder:
        """Set the contract's observation frequency.

        Parameters
        ----------
        value : dict[str, object] | Tenor
            Observation frequency

        Returns
        -------
        FxVarianceSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def base_calendar_id(self, value: str) -> FxVarianceSwapBuilder:
        """Set the contract's base calendar id.

        Parameters
        ----------
        value : str
            Base-currency calendar used in the joint observation calendar.

        Returns
        -------
        FxVarianceSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def quote_calendar_id(self, value: str) -> FxVarianceSwapBuilder:
        """Set the contract's quote calendar id.

        Parameters
        ----------
        value : str
            Quote-currency calendar used in the joint observation calendar.

        Returns
        -------
        FxVarianceSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def observation_business_day_convention(self, value: str) -> FxVarianceSwapBuilder:
        """Set the contract's observation business day convention.

        Parameters
        ----------
        value : str
            Business-day convention applied to observation dates.

        Returns
        -------
        FxVarianceSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def observation_end_of_month(self, value: bool) -> FxVarianceSwapBuilder:
        """Set the contract's observation end of month.

        Parameters
        ----------
        value : bool
            Preserve month-end rolls for month/year observation frequencies.

        Returns
        -------
        FxVarianceSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def realized_var_method(self, value: dict[str, object] | str) -> FxVarianceSwapBuilder:
        """Set the contract's realized var method.

        Parameters
        ----------
        value : dict[str, object] | str
            Method for calculating realized variance (defaults to CloseToClose)

        Returns
        -------
        FxVarianceSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def trading_days_per_year(self, value: float) -> FxVarianceSwapBuilder:
        """Set the contract's trading days per year.

        Parameters
        ----------
        value : float
            Trading days per year used to annualise daily realized variance. A contract term of the variance-swap confirmation: daily observations are annualised by trading_days_per_year / observation_frequency.count(). Must be finite and positive. Defaults to finstack_quant_valuations_core::constants::TRADING_DAYS_PER_YEAR (252).

        Returns
        -------
        FxVarianceSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def open_series_id(self, value: str | None) -> FxVarianceSwapBuilder:
        """Set the contract's open series id.

        Parameters
        ----------
        value : str | None
            Series ID for open prices (required for Parkinson, GarmanKlass, RogersSatchell, YangZhang). Defaults to spot_id (or currency-pair string) when absent.

        Returns
        -------
        FxVarianceSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def high_series_id(self, value: str | None) -> FxVarianceSwapBuilder:
        """Set the contract's high series id.

        Parameters
        ----------
        value : str | None
            Series ID for high prices (required for Parkinson, GarmanKlass, RogersSatchell, YangZhang). Defaults to spot_id (or currency-pair string) when absent.

        Returns
        -------
        FxVarianceSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def low_series_id(self, value: str | None) -> FxVarianceSwapBuilder:
        """Set the contract's low series id.

        Parameters
        ----------
        value : str | None
            Series ID for low prices (required for Parkinson, GarmanKlass, RogersSatchell, YangZhang). Defaults to spot_id (or currency-pair string) when absent.

        Returns
        -------
        FxVarianceSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def close_series_id(self, value: str | None) -> FxVarianceSwapBuilder:
        """Set the contract's close series id.

        Parameters
        ----------
        value : str | None
            Series ID for close prices. Defaults to spot_id (or currency-pair string) when absent.

        Returns
        -------
        FxVarianceSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def previous_close_date(self, value: date | str | None) -> FxVarianceSwapBuilder:
        """Set the contract's previous close date.

        Parameters
        ----------
        value : date | str | None
            Contractual closing fixing immediately before the first Yang–Zhang target bar. Resolved exactly in the same close series as the target bars.

        Returns
        -------
        FxVarianceSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def side(self, value: dict[str, object] | str) -> FxVarianceSwapBuilder:
        """Set the contract's side.

        Parameters
        ----------
        value : dict[str, object] | str
            Pay/receive variance

        Returns
        -------
        FxVarianceSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def domestic_discount_curve_id(self, value: str) -> FxVarianceSwapBuilder:
        """Set the contract's domestic discount curve id.

        Parameters
        ----------
        value : str
            Domestic currency discount curve ID

        Returns
        -------
        FxVarianceSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def foreign_discount_curve_id(self, value: str) -> FxVarianceSwapBuilder:
        """Set the contract's foreign discount curve id.

        Parameters
        ----------
        value : str
            Foreign currency discount curve ID

        Returns
        -------
        FxVarianceSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def vol_surface_id(self, value: str) -> FxVarianceSwapBuilder:
        """Set the contract's vol surface id.

        Parameters
        ----------
        value : str
            FX volatility surface ID

        Returns
        -------
        FxVarianceSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def day_count(self, value: DayCount | str) -> FxVarianceSwapBuilder:
        """Set the contract's day count.

        Parameters
        ----------
        value : DayCount | str
            Day count convention for time calculations (defaults to ACT/365F).

        Returns
        -------
        FxVarianceSwapBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def attributes(self, value: Attributes | dict[str, Any]) -> FxVarianceSwapBuilder:
        """Set the FX variance swap grouping and filter attributes.

        Parameters
        ----------
        value : Attributes | dict[str, Any]
            Tags and literal metadata carried with the contract.

        Returns
        -------
        FxVarianceSwapBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def instrument_pricing_overrides(self, value: dict[str, object] | str) -> FxVarianceSwapBuilder:
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
        FxVarianceSwapBuilder
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
    ) -> FxVarianceSwapBuilder:
        """
        Set the metric-time pricing configuration stored on the instrument.

        Parameters
        ----------
        value : MetricPricingOverrides | dict[str, object] | str
            Rust ``MetricPricingOverrides`` as the typed class, a ``dict`` or a
            JSON string.

        Returns
        -------
        FxVarianceSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``MetricPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def scenario_pricing_overrides(self, value: dict[str, object] | str) -> FxVarianceSwapBuilder:
        """
        Set the scenario-only pricing adjustments.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``ScenarioPricingOverrides`` in serde form (``dict`` or JSON
            string), e.g. ``{"scenario_price_shock_decimal": -0.02}``.

        Returns
        -------
        FxVarianceSwapBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``ScenarioPricingOverrides`` (unknown fields are rejected).
        """
        ...
