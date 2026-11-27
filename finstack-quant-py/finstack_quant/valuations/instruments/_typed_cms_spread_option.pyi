"""Typed CMS spread option and its builder."""

from __future__ import annotations

from datetime import date
from typing import Any
from decimal import Decimal
from finstack_quant.core.money import Money
from finstack_quant.core.dates import DayCount
from finstack_quant.core.dates import Tenor
from finstack_quant.core.types import Attributes
from finstack_quant.valuations.instruments._pricing import MetricPricingOverrides

class CmsSpreadOption:
    """Option on the difference between two constant-maturity swap rates.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import CmsSpreadOption
    >>> isinstance(CmsSpreadOption.example(), CmsSpreadOption)
    True
    """

    @staticmethod
    def builder() -> CmsSpreadOptionBuilder:
        """Start authoring a CMS spread option.

        Returns
        -------
        CmsSpreadOptionBuilder
            Empty builder for the exact contractual terms.

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CmsSpreadOption
        >>> _ = CmsSpreadOption.builder().id("example-cms-spread-option")
        """
        ...

    def to_builder(self) -> CmsSpreadOptionBuilder:
        """Copy the current terms into an editable builder.

        Returns
        -------
        CmsSpreadOptionBuilder
            Builder retaining contractual terms, market identifiers and pricing overrides.

        Notes
        -----
        This method does not raise.
        """
        ...

    @staticmethod
    def example() -> CmsSpreadOption:
        """Construct the built-in CMS spread option example.

        Returns
        -------
        CmsSpreadOption
            Validated contract with its lifecycle and market dependencies.

        Raises
        ------
        ValueError
            If the built-in instrument fixture is invalid.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CmsSpreadOption
        >>> CmsSpreadOption.example().id != ""
        True
        """
        ...

    @classmethod
    def from_json(cls, json: str) -> CmsSpreadOption:
        """
        Deserialize a validated CmsSpreadOption from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"cms_spread_option"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        CmsSpreadOption
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import CmsSpreadOption
        >>> future = CmsSpreadOption.example()
        >>> CmsSpreadOption.from_json(future.to_json()).id == future.id != ""
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
            and :meth:`CmsSpreadOption.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    def to_dict(self) -> dict[str, object]:
        """
        Canonical Rust terms for this CMS spread option as a plain Python ``dict``.

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
    def long_cms_tenor(self) -> dict[str, object] | Tenor:
        """Read the contract's long cms tenor.

        Returns
        -------
        dict[str, object] | Tenor
            Long CMS tenor (e.g., 10Y).

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def short_cms_tenor(self) -> dict[str, object] | Tenor:
        """Read the contract's short cms tenor.

        Returns
        -------
        dict[str, object] | Tenor
            Short CMS tenor (e.g., 2Y).

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def strike(self) -> Decimal:
        """Read the contract's strike.

        Returns
        -------
        Decimal
            Strike on the CMS spread as a decimal rate (0.005 = 50bp).

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
            Call or put on the spread CMS_long - CMS_short: a call pays max(spread - strike, 0), a put pays max(strike - spread, 0).

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
            Notional amount.

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
    def payment_date(self) -> date:
        """Read the contract's payment date.

        Returns
        -------
        date
            Payment date (may differ from expiry).

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def long_vol_surface_id(self) -> str:
        """Read the contract's long vol surface id.

        Returns
        -------
        str
            Black swaption volatility surface for the long tenor. The selected expiry slice must be flat across strikes; tenor-axis ATM surfaces are valid.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def short_vol_surface_id(self) -> str:
        """Read the contract's short vol surface id.

        Returns
        -------
        str
            Black swaption volatility surface for the short tenor, subject to the same flat-strike restriction as the long-tenor surface.

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
    def forward_curve_id(self) -> str:
        """Read the contract's forward curve id.

        Returns
        -------
        str
            Forward curve ID (for swap rate projection).

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def correlation(self) -> float:
        """Read the contract's correlation.

        Returns
        -------
        float
            Gaussian-copula correlation between the two CMS rates, a decimal in [-1, 1].

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
            Day count convention.

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
            Rate-index convention-registry key of the underlying CMS swaps (e.g. EUR-ESTR-OIS). Required for USD CMS; other supported currencies use their registered overnight-index default when omitted. Supplies calendar, settlement lag and default leg conventions. Individual leg fields override the index.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def long_cms_fixing_index_id(self) -> str | None:
        """Read the contract's long cms fixing index id.

        Returns
        -------
        str | None
            Explicit CMS benchmark fixing identifier for the long-tenor leg (FIXING:<long_cms_fixing_index_id>), distinct from forward_curve_id, index_id, and short_cms_fixing_index_id.

        Notes
        -----
        This property does not raise.
        """
        ...

    @property
    def short_cms_fixing_index_id(self) -> str | None:
        """Read the contract's short cms fixing index id.

        Returns
        -------
        str | None
            Explicit CMS benchmark fixing identifier for the short-tenor leg (FIXING:<short_cms_fixing_index_id>), distinct from forward_curve_id, index_id, and long_cms_fixing_index_id.

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
            Fixed leg frequency of the underlying CMS swaps (overrides convention).

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
            Floating leg frequency of the underlying CMS swaps (overrides convention).

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
            Fixed leg day count of the underlying CMS swaps (overrides convention).

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
            Floating leg day count of the underlying CMS swaps (overrides convention).

        Notes
        -----
        This property does not raise.
        """
        ...

class CmsSpreadOptionBuilder:
    """Fluent builder for contracts with explicit lifecycle terms.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import CmsSpreadOption
    >>> builder = CmsSpreadOption.example().to_builder()
    >>> builder.id("RISK-TIPS").build().id
    'RISK-TIPS'
    """

    def build(self) -> CmsSpreadOption:
        """Validate the contractual terms and consume the builder.

        Returns
        -------
        CmsSpreadOption
            Native instrument ready for pricing against the selected market.

        Raises
        ------
        ValueError
            If a required field is missing, the terms are inconsistent, or the
            builder has already been consumed.
        """
        ...

    def id(self, value: str) -> CmsSpreadOptionBuilder:
        """Set the CMS spread option identifier.

        Parameters
        ----------
        value : str
            Stable contract identifier used for pricing and metric keys.

        Returns
        -------
        CmsSpreadOptionBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def long_cms_tenor(self, value: dict[str, object] | Tenor) -> CmsSpreadOptionBuilder:
        """Set the contract's long cms tenor.

        Parameters
        ----------
        value : dict[str, object] | Tenor
            Long CMS tenor (e.g., 10Y).

        Returns
        -------
        CmsSpreadOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def short_cms_tenor(self, value: dict[str, object] | Tenor) -> CmsSpreadOptionBuilder:
        """Set the contract's short cms tenor.

        Parameters
        ----------
        value : dict[str, object] | Tenor
            Short CMS tenor (e.g., 2Y).

        Returns
        -------
        CmsSpreadOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def strike(self, value: Decimal | str | int) -> CmsSpreadOptionBuilder:
        """Set the contract's strike.

        Parameters
        ----------
        value : Decimal | str | int
            Strike on the CMS spread as a decimal rate (0.005 = 50bp).

        Returns
        -------
        CmsSpreadOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def option_type(self, value: dict[str, object] | str) -> CmsSpreadOptionBuilder:
        """Set the contract's option type.

        Parameters
        ----------
        value : dict[str, object] | str
            Call or put on the spread CMS_long - CMS_short: a call pays max(spread - strike, 0), a put pays max(strike - spread, 0).

        Returns
        -------
        CmsSpreadOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def notional(self, value: Money) -> CmsSpreadOptionBuilder:
        """Set the contract's notional.

        Parameters
        ----------
        value : Money
            Notional amount.

        Returns
        -------
        CmsSpreadOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def expiry(self, value: date | str) -> CmsSpreadOptionBuilder:
        """Set the contract's expiry.

        Parameters
        ----------
        value : date | str
            Option expiry date.

        Returns
        -------
        CmsSpreadOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def payment_date(self, value: date | str) -> CmsSpreadOptionBuilder:
        """Set the contract's payment date.

        Parameters
        ----------
        value : date | str
            Payment date (may differ from expiry).

        Returns
        -------
        CmsSpreadOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def long_vol_surface_id(self, value: str) -> CmsSpreadOptionBuilder:
        """Set the contract's long vol surface id.

        Parameters
        ----------
        value : str
            Black swaption volatility surface for the long tenor. The selected expiry slice must be flat across strikes; tenor-axis ATM surfaces are valid.

        Returns
        -------
        CmsSpreadOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def short_vol_surface_id(self, value: str) -> CmsSpreadOptionBuilder:
        """Set the contract's short vol surface id.

        Parameters
        ----------
        value : str
            Black swaption volatility surface for the short tenor, subject to the same flat-strike restriction as the long-tenor surface.

        Returns
        -------
        CmsSpreadOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def discount_curve_id(self, value: str) -> CmsSpreadOptionBuilder:
        """Set the contract's discount curve id.

        Parameters
        ----------
        value : str
            Discount curve ID.

        Returns
        -------
        CmsSpreadOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def forward_curve_id(self, value: str) -> CmsSpreadOptionBuilder:
        """Set the contract's forward curve id.

        Parameters
        ----------
        value : str
            Forward curve ID (for swap rate projection).

        Returns
        -------
        CmsSpreadOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def correlation(self, value: float) -> CmsSpreadOptionBuilder:
        """Set the contract's correlation.

        Parameters
        ----------
        value : float
            Gaussian-copula correlation between the two CMS rates, a decimal in [-1, 1].

        Returns
        -------
        CmsSpreadOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def day_count(self, value: DayCount | str) -> CmsSpreadOptionBuilder:
        """Set the contract's day count.

        Parameters
        ----------
        value : DayCount | str
            Day count convention.

        Returns
        -------
        CmsSpreadOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def index_id(self, value: str | None) -> CmsSpreadOptionBuilder:
        """Set the contract's index id.

        Parameters
        ----------
        value : str | None
            Rate-index convention-registry key of the underlying CMS swaps (e.g. EUR-ESTR-OIS). Required for USD CMS; other supported currencies use their registered overnight-index default when omitted. Supplies calendar, settlement lag and default leg conventions. Individual leg fields override the index.

        Returns
        -------
        CmsSpreadOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def long_cms_fixing_index_id(self, value: str | None) -> CmsSpreadOptionBuilder:
        """Set the contract's long cms fixing index id.

        Parameters
        ----------
        value : str | None
            Explicit CMS benchmark fixing identifier for the long-tenor leg (FIXING:<long_cms_fixing_index_id>), distinct from forward_curve_id, index_id, and short_cms_fixing_index_id.

        Returns
        -------
        CmsSpreadOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def short_cms_fixing_index_id(self, value: str | None) -> CmsSpreadOptionBuilder:
        """Set the contract's short cms fixing index id.

        Parameters
        ----------
        value : str | None
            Explicit CMS benchmark fixing identifier for the short-tenor leg (FIXING:<short_cms_fixing_index_id>), distinct from forward_curve_id, index_id, and long_cms_fixing_index_id.

        Returns
        -------
        CmsSpreadOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def swap_fixed_frequency(self, value: dict[str, object] | Tenor | None) -> CmsSpreadOptionBuilder:
        """Set the contract's swap fixed frequency.

        Parameters
        ----------
        value : dict[str, object] | Tenor | None
            Fixed leg frequency of the underlying CMS swaps (overrides convention).

        Returns
        -------
        CmsSpreadOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def swap_float_frequency(self, value: dict[str, object] | Tenor | None) -> CmsSpreadOptionBuilder:
        """Set the contract's swap float frequency.

        Parameters
        ----------
        value : dict[str, object] | Tenor | None
            Floating leg frequency of the underlying CMS swaps (overrides convention).

        Returns
        -------
        CmsSpreadOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def swap_fixed_day_count(self, value: DayCount | str | None) -> CmsSpreadOptionBuilder:
        """Set the contract's swap fixed day count.

        Parameters
        ----------
        value : DayCount | str | None
            Fixed leg day count of the underlying CMS swaps (overrides convention).

        Returns
        -------
        CmsSpreadOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def swap_float_day_count(self, value: DayCount | str | None) -> CmsSpreadOptionBuilder:
        """Set the contract's swap float day count.

        Parameters
        ----------
        value : DayCount | str | None
            Floating leg day count of the underlying CMS swaps (overrides convention).

        Returns
        -------
        CmsSpreadOptionBuilder
            Active builder carrying the supplied value.

        Raises
        ------
        ValueError
            If the value cannot be converted or the builder has been consumed.
        """
        ...

    def attributes(self, value: Attributes | dict[str, Any]) -> CmsSpreadOptionBuilder:
        """Set the CMS spread option grouping and filter attributes.

        Parameters
        ----------
        value : Attributes | dict[str, Any]
            Tags and literal metadata carried with the contract.

        Returns
        -------
        CmsSpreadOptionBuilder
            The same builder for additional contract edits.

        Raises
        ------
        ValueError
            If the value cannot be decoded or the builder has been consumed.
        TypeError
            If the input has an incompatible host type.
        """
        ...

    def instrument_pricing_overrides(self, value: dict[str, object]) -> CmsSpreadOptionBuilder:
        """
        Set the instrument-owned pricing inputs.

        Parameters
        ----------
        value : dict[str, object]
            Rust ``InstrumentPricingOverrides`` in serde form (a ``dict``): ``{"market_quotes": {...}, "model_config": {...}}``, e.g.
            ``{"market_quotes": {"quoted_clean_price_pct": 99.0}}``.

        Returns
        -------
        CmsSpreadOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``InstrumentPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def metric_pricing_overrides(self, value: MetricPricingOverrides | dict[str, object]) -> CmsSpreadOptionBuilder:
        """
        Set the metric-time pricing configuration stored on the instrument.

        Parameters
        ----------
        value : MetricPricingOverrides | dict[str, object]
            Rust ``MetricPricingOverrides`` as the typed class or a ``dict``.

        Returns
        -------
        CmsSpreadOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``MetricPricingOverrides`` (unknown fields are rejected).
        """
        ...

    def scenario_pricing_overrides(self, value: dict[str, object]) -> CmsSpreadOptionBuilder:
        """
        Set the scenario-only pricing adjustments.

        Parameters
        ----------
        value : dict[str, object]
            Rust ``ScenarioPricingOverrides`` in serde form (a ``dict``), e.g. ``{"scenario_price_shock_decimal": -0.02}``.

        Returns
        -------
        CmsSpreadOptionBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``ScenarioPricingOverrides`` (unknown fields are rejected).
        """
        ...
