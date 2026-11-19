"""Optional conversion policy, FX volatility and correlation inputs for quanto valuation. Pass null to clear it.."""

from __future__ import annotations

import datetime
from datetime import date
from typing import Any
import pandas as pd
from decimal import Decimal
from finstack_quant.core.money import Money
from finstack_quant.core.dates import DayCount
from finstack_quant.core.dates import Tenor
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.types import Attributes
from finstack_quant.valuations import ValuationResult
from finstack_quant.valuations.instruments._pricing import PriceExtras, MetricPricingOverrides

class CmsOption:
    """Optional conversion policy, FX volatility and correlation inputs for quanto valuation. Pass null to clear it..

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import CmsOption
    >>> isinstance(CmsOption.example(), CmsOption)
    True
    """

    @staticmethod
    def builder() -> CmsOptionBuilder:
        """Start specifying an bond future.

        Returns
        -------
        CmsOptionBuilder
            Empty builder for the exact contractual terms.

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CmsOption
        >>> _ = CmsOption.builder().id("TYU5")
        """
        ...

    def to_builder(self) -> CmsOptionBuilder:
        """Copy the current terms into an editable builder.

        Returns
        -------
        CmsOptionBuilder
            Builder retaining contractual terms, market identifiers and pricing overrides.

        Notes
        -----
        This method does not raise.
        """
        ...

    @staticmethod
    def example() -> CmsOption:
        """Construct the canonical bond future fixture.

        Returns
        -------
        CmsOption
            Validated contract with its lifecycle and market dependencies.

        Raises
        ------
        ValueError
            If the built-in instrument fixture is invalid.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CmsOption
        >>> CmsOption.example().id != ""
        True
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> CmsOption:
        """
        Deserialize a validated CmsOption from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"cms_option"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        CmsOption
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CmsOption
        >>> future = CmsOption.example()
        >>> CmsOption.from_json(future.to_json()).id == future.id != ""
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
            and :meth:`CmsOption.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def to_dict(self) -> dict[str, object]:
        """
        Serde form of the bond future spec as a plain Python ``dict``.

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
        Price this bond future and return a :class:`~finstack_quant.valuations.ValuationResult`.

        Same pipeline and keyword surface as :func:`price_instrument`.

        Parameters
        ----------
        market : MarketContext | str
            Market context object or its JSON string.
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date (ISO 8601 strings accepted).
        model : str, default "default"
            Model key (``"discounting"``, ``"hull_white_1f"``, ...).
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
            Cash flow valuation requires the bond future’s contractual fixed rate.
        KeyError
            If the discount curve or requested metric is missing from ``market``.
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
        Compute one scalar metric for this bond future (e.g. ``"dv01"``).

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
            If the discount curve is missing from ``market``.
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
            The registered bond future model key ``"discounting"``.

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
    def strike(self) -> Decimal:
        """Read the contract's strike.

        Returns
        -------
        Decimal
            Strike (fixed rate for CMS option)

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def cms_tenor(self) -> dict[str, object] | Tenor:
        """Read the contract's cms tenor.

        Returns
        -------
        dict[str, object] | Tenor
            Tenor of the CMS reference swap (e.g. 10Y); must be month- or year-based.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def fixing_dates(self) -> list[date]:
        """Read the contract's fixing dates.

        Returns
        -------
        list[date]
            Observation/fixing dates for CMS rate

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def payment_dates(self) -> list[date]:
        """Read the contract's payment dates.

        Returns
        -------
        list[date]
            Payment dates for each period (usually fixing date + lag or period end)

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def accrual_fractions(self) -> dict[str, object] | str:
        """Read the contract's accrual fractions.

        Returns
        -------
        dict[str, object] | str
            Finite nonnegative accrual fractions for each period; zero gives a zero coupon.

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
            Option type (call or put on CMS rate)

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
            Notional amount

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
            Day count convention for the option accrual

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def index_id(self) -> str | None:
        """Read the contract's index id.

        Returns
        -------
        str | None
            Rate-index convention-registry key of the underlying swap (e.g. USD-SOFR-OIS). When set, provides default values for swap_fixed_frequency, swap_float_frequency, swap_fixed_day_count, and swap_float_day_count. Individual fields still override the convention when explicitly set. Required for USD CMS.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def cms_fixing_index_id(self) -> str | None:
        """Read the contract's cms fixing index id.

        Returns
        -------
        str | None
            Explicit CMS benchmark fixing identifier used for historical fixings (FIXING:<cms_fixing_index_id>), distinct from forward_curve_id and index_id.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def swap_fixed_frequency(self) -> dict[str, object] | Tenor | None:
        """Read the contract's swap fixed frequency.

        Returns
        -------
        dict[str, object] | Tenor | None
            Fixed leg frequency of the underlying swap (overrides convention if set)

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def swap_float_frequency(self) -> dict[str, object] | Tenor | None:
        """Read the contract's swap float frequency.

        Returns
        -------
        dict[str, object] | Tenor | None
            Floating leg frequency of the underlying swap (overrides convention if set)

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def swap_fixed_day_count(self) -> DayCount | None:
        """Read the contract's swap fixed day count.

        Returns
        -------
        DayCount | None
            Day count convention of the underlying swap fixed leg (overrides convention if set)

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def swap_float_day_count(self) -> DayCount | None:
        """Read the contract's swap float day count.

        Returns
        -------
        DayCount | None
            Optional day count convention of the underlying swap floating leg

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
            Discount curve ID for present value calculations

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
            Forward/projection curve ID for CMS rate projection

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
            Volatility surface ID for CMS rates

        Notes
        -----
        This property does not raise.
        """
        ...

class CmsOptionBuilder:
    """Fluent builder for contracts with explicit lifecycle terms.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import CmsOption
    >>> builder = CmsOption.example().to_builder()
    >>> builder.id("RISK-TIPS").build().id
    'RISK-TIPS'
    """

    def build(self) -> CmsOption:
        """Validate the contractual terms and consume the builder.

        Returns
        -------
        CmsOption
            Native instrument ready for pricing against the selected market.

        Raises
        ------
        ValueError
            If a required field is missing, the terms are inconsistent, or the
            builder has already been consumed.
        """
        ...

    def id(self, value: str) -> CmsOptionBuilder:
        """Set the bond's id.

        Parameters
        ----------
        value : str
            Stable contract identifier used for pricing and metric keys.

        Returns
        -------
        CmsOptionBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def strike(self, value: Decimal | str | int) -> CmsOptionBuilder:
        """Set the contract's strike.

        Parameters
        ----------
        value : Decimal | str | int
            Strike (fixed rate for CMS option)

        Returns
        -------
        CmsOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def cms_tenor(self, value: dict[str, object] | Tenor) -> CmsOptionBuilder:
        """Set the contract's cms tenor.

        Parameters
        ----------
        value : dict[str, object] | Tenor
            Tenor of the CMS reference swap (e.g. 10Y); must be month- or year-based.

        Returns
        -------
        CmsOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def fixing_dates(self, value: list[date | str]) -> CmsOptionBuilder:
        """Set the contract's fixing dates.

        Parameters
        ----------
        value : list[date | str]
            Observation/fixing dates for CMS rate

        Returns
        -------
        CmsOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def payment_dates(self, value: list[date | str]) -> CmsOptionBuilder:
        """Set the contract's payment dates.

        Parameters
        ----------
        value : list[date | str]
            Payment dates for each period (usually fixing date + lag or period end)

        Returns
        -------
        CmsOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def accrual_fractions(self, value: dict[str, object] | str) -> CmsOptionBuilder:
        """Set the contract's accrual fractions.

        Parameters
        ----------
        value : dict[str, object] | str
            Finite nonnegative accrual fractions for each period; zero gives a zero coupon.

        Returns
        -------
        CmsOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def option_type(self, value: dict[str, object] | str) -> CmsOptionBuilder:
        """Set the contract's option type.

        Parameters
        ----------
        value : dict[str, object] | str
            Option type (call or put on CMS rate)

        Returns
        -------
        CmsOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def notional(self, value: Money) -> CmsOptionBuilder:
        """Set the contract's notional.

        Parameters
        ----------
        value : Money
            Notional amount

        Returns
        -------
        CmsOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def day_count(self, value: DayCount | str) -> CmsOptionBuilder:
        """Set the contract's day count.

        Parameters
        ----------
        value : DayCount | str
            Day count convention for the option accrual

        Returns
        -------
        CmsOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def index_id(self, value: str | None) -> CmsOptionBuilder:
        """Set the contract's index id.

        Parameters
        ----------
        value : str | None
            Rate-index convention-registry key of the underlying swap (e.g. USD-SOFR-OIS). When set, provides default values for swap_fixed_frequency, swap_float_frequency, swap_fixed_day_count, and swap_float_day_count. Individual fields still override the convention when explicitly set. Required for USD CMS.

        Returns
        -------
        CmsOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def cms_fixing_index_id(self, value: str | None) -> CmsOptionBuilder:
        """Set the contract's cms fixing index id.

        Parameters
        ----------
        value : str | None
            Explicit CMS benchmark fixing identifier used for historical fixings (FIXING:<cms_fixing_index_id>), distinct from forward_curve_id and index_id.

        Returns
        -------
        CmsOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def swap_fixed_frequency(self, value: dict[str, object] | Tenor | None) -> CmsOptionBuilder:
        """Set the contract's swap fixed frequency.

        Parameters
        ----------
        value : dict[str, object] | Tenor | None
            Fixed leg frequency of the underlying swap (overrides convention if set)

        Returns
        -------
        CmsOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def swap_float_frequency(self, value: dict[str, object] | Tenor | None) -> CmsOptionBuilder:
        """Set the contract's swap float frequency.

        Parameters
        ----------
        value : dict[str, object] | Tenor | None
            Floating leg frequency of the underlying swap (overrides convention if set)

        Returns
        -------
        CmsOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def swap_fixed_day_count(self, value: DayCount | str | None) -> CmsOptionBuilder:
        """Set the contract's swap fixed day count.

        Parameters
        ----------
        value : DayCount | str | None
            Day count convention of the underlying swap fixed leg (overrides convention if set)

        Returns
        -------
        CmsOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def swap_float_day_count(self, value: DayCount | str | None) -> CmsOptionBuilder:
        """Set the contract's swap float day count.

        Parameters
        ----------
        value : DayCount | str | None
            Optional day count convention of the underlying swap floating leg

        Returns
        -------
        CmsOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def discount_curve_id(self, value: str) -> CmsOptionBuilder:
        """Set the contract's discount curve id.

        Parameters
        ----------
        value : str
            Discount curve ID for present value calculations

        Returns
        -------
        CmsOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def forward_curve_id(self, value: str) -> CmsOptionBuilder:
        """Set the contract's forward curve id.

        Parameters
        ----------
        value : str
            Forward/projection curve ID for CMS rate projection

        Returns
        -------
        CmsOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def vol_surface_id(self, value: str) -> CmsOptionBuilder:
        """Set the contract's vol surface id.

        Parameters
        ----------
        value : str
            Volatility surface ID for CMS rates

        Returns
        -------
        CmsOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def attributes(self, value: Attributes | dict[str, Any]) -> CmsOptionBuilder:
        """Set the bond's attributes.

        Parameters
        ----------
        value : Attributes | dict[str, Any]
            Tags and literal metadata carried with the contract.

        Returns
        -------
        CmsOptionBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def instrument_pricing_overrides(self, value: dict[str, object] | str) -> CmsOptionBuilder:
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
        CmsOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``InstrumentPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def metric_pricing_overrides(self, value: MetricPricingOverrides | dict[str, object] | str) -> CmsOptionBuilder:
        """
        Set the metric-time pricing configuration stored on the instrument.

        Parameters
        ----------
        value : MetricPricingOverrides | dict[str, object] | str
            Rust ``MetricPricingOverrides`` as the typed class, a ``dict`` or a
            JSON string.

        Returns
        -------
        CmsOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``MetricPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def scenario_pricing_overrides(self, value: dict[str, object] | str) -> CmsOptionBuilder:
        """
        Set the scenario-only pricing adjustments.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``ScenarioPricingOverrides`` in serde form (``dict`` or JSON
            string), e.g. ``{"scenario_price_shock_decimal": -0.02}``.

        Returns
        -------
        CmsOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``ScenarioPricingOverrides`` (unknown fields are rejected).
        """
        ...
