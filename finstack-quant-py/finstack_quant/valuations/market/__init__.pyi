"""Market conventions and listed-market coverage metadata.

``ConventionRegistry`` mirrors the Rust ``ConventionRegistry::try_global()``:
read-only lookups of the embedded rate-index, CDS, swaption, inflation-swap,
IR-future and cross-currency convention tables. Every record returned is a
frozen serde-backed value object with ``to_json`` / ``from_json`` / ``to_dict``
and ``pickle`` support.

Examples
--------
>>> from finstack_quant.valuations.market import ConventionRegistry, listed_product_catalog
>>> any(row["exchange"] == "eurex" for row in listed_product_catalog())
True
>>> registry = ConventionRegistry()
>>> registry.require_rate_index("USD-SOFR").currency
'USD'
>>> registry.primary_cds_family("USD")
'isda_na'
"""

from __future__ import annotations

from typing import Any, Literal

__all__ = [
    "CdsConventionSpec",
    "ConventionRegistry",
    "InflationSwapConventions",
    "IrFutureConventions",
    "RateIndexConventions",
    "SwaptionConventions",
    "XccyConventions",
    "listed_product_catalog",
]

CdsDocClause = Literal["cr14", "mr14", "mm14", "xr14", "isda_na", "isda_eu", "isda_as", "isda_au", "isda_nz"]

class RateIndexConventions:
    """
    Market conventions for one floating-rate index (SOFR, EURIBOR-3M, ...).

    Returned by :meth:`ConventionRegistry.require_rate_index`. Every field is a
    read-only property; enum-valued fields are serde strings so they can be
    passed straight back into builders and leg specs.

    Examples
    --------
    >>> from finstack_quant.valuations.market import ConventionRegistry
    >>> conv = ConventionRegistry().require_rate_index("USD-SOFR")
    >>> conv.currency, conv.default_reset_lag_days >= 0
    ('USD', True)
    """

    @staticmethod
    def from_json(json: str) -> RateIndexConventions:
        """
        Deserialize from the JSON produced by :meth:`to_json`.

        Parameters
        ----------
        json : str
            Strict JSON object with exactly the fields ``to_json`` writes.

        Returns
        -------
        RateIndexConventions
            The reconstructed record.

        Raises
        ------
        ValueError
            If the JSON is malformed or has the wrong shape.

        Examples
        --------
        >>> from finstack_quant.valuations.market import ConventionRegistry, RateIndexConventions
        >>> conv = ConventionRegistry().require_rate_index("USD-SOFR")
        >>> RateIndexConventions.from_json(conv.to_json()) == conv
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON wire form.

        Returns
        -------
        str
            Strict JSON accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If canonical serialization fails.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return every field as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Same content as ``json.loads(self.to_json())``.

        Raises
        ------
        ValueError
            If canonical serialization fails.
        """
        ...

    def __eq__(self, other: object) -> bool:
        """
        Structural equality on every field.

        Parameters
        ----------
        other : object
            Value to compare with.

        Returns
        -------
        bool
            ``True`` when ``other`` is a ``RateIndexConventions`` with identical fields.
        """
        ...

    @property
    def currency(self) -> str:
        """
        ISO-4217 currency code of the index.

        Returns
        -------
        str
            Three-letter code such as ``"USD"``.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

    @property
    def kind(self) -> str:
        """
        Index family (serde string of ``RateIndexKind``).

        Returns
        -------
        str
            e.g. ``"overnight_rfr"`` or ``"term_ibor"``.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

    @property
    def tenor(self) -> str | None:
        """
        Index tenor, or ``None`` for overnight indices.

        Returns
        -------
        str | None
            Tenor string such as ``"3M"``.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

    @property
    def day_count(self) -> str:
        """
        Accrual day count of the floating leg (serde string).

        Returns
        -------
        str
            e.g. ``"act_360"``.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

    @property
    def default_payment_frequency(self) -> str:
        """
        Default floating-leg payment frequency.

        Returns
        -------
        str
            Tenor string such as ``"3M"`` or ``"1Y"``.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

    @property
    def default_payment_lag_days(self) -> int:
        """
        Default payment lag in business days after period end.

        Returns
        -------
        int
            Lag in business days.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

    @property
    def default_reset_lag_days(self) -> int:
        """
        Default fixing (reset) lag in business days before accrual start.

        Returns
        -------
        int
            Lag in business days.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

    @property
    def ois_compounding(self) -> str | None:
        """
        OIS compounding style for overnight indices, or ``None``.

        Returns
        -------
        str | None
            Serde string of ``FloatingLegCompounding``.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

    @property
    def market_calendar_id(self) -> str:
        """
        Calendar identifier used for fixings and payments.

        Returns
        -------
        str
            Registered calendar id.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

    @property
    def market_settlement_days(self) -> int:
        """
        Spot settlement lag in business days (T+n).

        Returns
        -------
        int
            Settlement lag.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

    @property
    def market_business_day_convention(self) -> str:
        """
        Business-day convention for date rolls (serde string).

        Returns
        -------
        str
            e.g. ``"modified_following"``.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

    @property
    def default_fixed_leg_day_count(self) -> str:
        """
        Default fixed-leg day count for a standard swap on this index.

        Returns
        -------
        str
            Serde day-count name.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

    @property
    def default_fixed_leg_frequency(self) -> str:
        """
        Default fixed-leg payment frequency for a standard swap on this index.

        Returns
        -------
        str
            Tenor string.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

class CdsConventionSpec:
    """
    Schedule conventions for one CDS family (``isda_na``, ``isda_eu``, ...).

    Returned by :meth:`ConventionRegistry.resolve_cds`.

    Examples
    --------
    >>> from finstack_quant.valuations.market import ConventionRegistry
    >>> spec = ConventionRegistry().resolve_cds("USD", "isda_na")
    >>> spec.family
    'isda_na'
    """

    @staticmethod
    def from_json(json: str) -> CdsConventionSpec:
        """
        Deserialize from the JSON produced by :meth:`to_json`.

        Parameters
        ----------
        json : str
            Strict JSON object with exactly the fields ``to_json`` writes.

        Returns
        -------
        CdsConventionSpec
            The reconstructed record.

        Raises
        ------
        ValueError
            If the JSON is malformed or has the wrong shape.

        Examples
        --------
        >>> from finstack_quant.valuations.market import CdsConventionSpec, ConventionRegistry
        >>> spec = ConventionRegistry().resolve_cds("USD", "isda_na")
        >>> CdsConventionSpec.from_json(spec.to_json()) == spec
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON wire form.

        Returns
        -------
        str
            Strict JSON accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If canonical serialization fails.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return every field as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Same content as ``json.loads(self.to_json())``.

        Raises
        ------
        ValueError
            If canonical serialization fails.
        """
        ...

    def __eq__(self, other: object) -> bool:
        """
        Structural equality on every field.

        Parameters
        ----------
        other : object
            Value to compare with.

        Returns
        -------
        bool
            ``True`` when ``other`` is a ``CdsConventionSpec`` with identical fields.
        """
        ...

    @property
    def family(self) -> str:
        """
        Convention family (serde string).

        Returns
        -------
        str
            ``"isda_na"``, ``"isda_eu"``, ``"isda_as"`` or ``"custom"``.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

    @property
    def calendar_id(self) -> str:
        """
        Calendar identifier for premium-leg date rolls.

        Returns
        -------
        str
            Registered calendar id.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

    @property
    def day_count(self) -> str:
        """
        Premium-leg accrual day count (serde string).

        Returns
        -------
        str
            e.g. ``"act_360"``.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

    @property
    def business_day_convention(self) -> str:
        """
        Business-day convention for premium dates (serde string).

        Returns
        -------
        str
            e.g. ``"following"``.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

    @property
    def stub(self) -> str:
        """
        Stub rule for the first premium period (serde string).

        Returns
        -------
        str
            e.g. ``"short_front"``.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

    @property
    def settlement_days(self) -> int:
        """
        Settlement lag in business days (T+n).

        Returns
        -------
        int
            Settlement lag.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

    @property
    def frequency(self) -> str:
        """
        Premium payment frequency.

        Returns
        -------
        str
            Tenor string, ``"3M"`` for standard IMM-roll CDS.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

class SwaptionConventions:
    """
    Market conventions for a swaption family (settlement, fixed-leg schedule, index).

    Returned by :meth:`ConventionRegistry.require_swaption`.

    Examples
    --------
    >>> from finstack_quant.valuations.market import ConventionRegistry
    >>> conv = ConventionRegistry().require_swaption("USD")
    >>> isinstance(conv.float_leg_index, str)
    True
    """

    @staticmethod
    def from_json(json: str) -> SwaptionConventions:
        """
        Deserialize from the JSON produced by :meth:`to_json`.

        Parameters
        ----------
        json : str
            Strict JSON object with exactly the fields ``to_json`` writes.

        Returns
        -------
        SwaptionConventions
            The reconstructed record.

        Raises
        ------
        ValueError
            If the JSON is malformed or has the wrong shape.

        Examples
        --------
        >>> from finstack_quant.valuations.market import ConventionRegistry, SwaptionConventions
        >>> conv = ConventionRegistry().require_swaption("USD")
        >>> SwaptionConventions.from_json(conv.to_json()) == conv
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON wire form.

        Returns
        -------
        str
            Strict JSON accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If canonical serialization fails.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return every field as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Same content as ``json.loads(self.to_json())``.

        Raises
        ------
        ValueError
            If canonical serialization fails.
        """
        ...

    def __eq__(self, other: object) -> bool:
        """
        Structural equality on every field.

        Parameters
        ----------
        other : object
            Value to compare with.

        Returns
        -------
        bool
            ``True`` when ``other`` is a ``SwaptionConventions`` with identical fields.
        """
        ...

    @property
    def calendar_id(self) -> str:
        """
        Calendar identifier for expiry and settlement rolls.

        Returns
        -------
        str
            Registered calendar id.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

    @property
    def settlement_days(self) -> int:
        """
        Settlement lag in business days from expiry to swap start.

        Returns
        -------
        int
            Settlement lag.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

    @property
    def business_day_convention(self) -> str:
        """
        Business-day convention for schedule rolls (serde string).

        Returns
        -------
        str
            e.g. ``"modified_following"``.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

    @property
    def fixed_leg_frequency(self) -> str:
        """
        Fixed-leg payment frequency of the underlying swap.

        Returns
        -------
        str
            Tenor string.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

    @property
    def fixed_leg_day_count(self) -> str:
        """
        Fixed-leg day count of the underlying swap (serde string).

        Returns
        -------
        str
            Serde day-count name.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

    @property
    def float_leg_index(self) -> str:
        """
        Floating-leg index identifier of the underlying swap.

        Returns
        -------
        str
            Index id such as ``"USD-SOFR"``.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

class InflationSwapConventions:
    """
    Market conventions for a zero-coupon inflation swap family.

    Returned by :meth:`ConventionRegistry.require_inflation_swap`.

    Examples
    --------
    >>> from finstack_quant.valuations.market import ConventionRegistry, InflationSwapConventions
    >>> registry = ConventionRegistry()
    >>> isinstance(registry, ConventionRegistry)
    True
    """

    @staticmethod
    def from_json(json: str) -> InflationSwapConventions:
        """
        Deserialize from the JSON produced by :meth:`to_json`.

        Parameters
        ----------
        json : str
            Strict JSON object with exactly the fields ``to_json`` writes.

        Returns
        -------
        InflationSwapConventions
            The reconstructed record.

        Raises
        ------
        ValueError
            If the JSON is malformed or has the wrong shape.

        Examples
        --------
        >>> from finstack_quant.valuations.market import InflationSwapConventions
        >>> payload = (
        ...     '{"calendar_id":"nyse","settlement_days":2,'
        ...     '"business_day_convention":"modified_following",'
        ...     '"day_count":"one_one","inflation_lag":{"count":3,"unit":"months"},"interpolation":"linear"}'
        ... )
        >>> InflationSwapConventions.from_json(payload).inflation_lag
        '3M'
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON wire form.

        Returns
        -------
        str
            Strict JSON accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If canonical serialization fails.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return every field as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Same content as ``json.loads(self.to_json())``.

        Raises
        ------
        ValueError
            If canonical serialization fails.
        """
        ...

    def __eq__(self, other: object) -> bool:
        """
        Structural equality on every field.

        Parameters
        ----------
        other : object
            Value to compare with.

        Returns
        -------
        bool
            ``True`` when ``other`` is an ``InflationSwapConventions`` with identical fields.
        """
        ...

    @property
    def interpolation(self) -> str:
        """Monthly reference-index interpolation convention.

        Returns
        -------
        str
            ``linear`` for daily weights between monthly CPI observations, or
            ``step`` for the single lagged calendar-month observation.

        Raises
        ------
        No exceptions are raised when reading this property.
        """
        ...

    @property
    def calendar_id(self) -> str:
        """
        Calendar identifier for schedule rolls.

        Returns
        -------
        str
            Registered calendar id.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

    @property
    def settlement_days(self) -> int:
        """
        Settlement lag in business days.

        Returns
        -------
        int
            Settlement lag.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

    @property
    def business_day_convention(self) -> str:
        """
        Business-day convention for schedule rolls (serde string).

        Returns
        -------
        str
            e.g. ``"modified_following"``.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

    @property
    def day_count(self) -> str:
        """
        Accrual day count (serde string).

        Returns
        -------
        str
            Serde day-count name.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

    @property
    def inflation_lag(self) -> str:
        """
        Index observation lag.

        Returns
        -------
        str
            Tenor string, ``"3M"`` for the standard 3-month lag.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

class XccyConventions:
    """
    Market conventions for a cross-currency basis swap pair.

    Returned by :meth:`ConventionRegistry.require_xccy`.

    Examples
    --------
    >>> from finstack_quant.valuations.market import ConventionRegistry, XccyConventions
    >>> registry = ConventionRegistry()
    >>> isinstance(registry, ConventionRegistry)
    True
    """

    @staticmethod
    def from_json(json: str) -> XccyConventions:
        """
        Deserialize from the JSON produced by :meth:`to_json`.

        Parameters
        ----------
        json : str
            Strict JSON object with exactly the fields ``to_json`` writes.

        Returns
        -------
        XccyConventions
            The reconstructed record.

        Raises
        ------
        ValueError
            If the JSON is malformed or has the wrong shape.

        Examples
        --------
        >>> from finstack_quant.valuations.market import XccyConventions
        >>> try:
        ...     XccyConventions.from_json("{}")
        ... except ValueError:
        ...     print("rejected")
        rejected
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON wire form.

        Returns
        -------
        str
            Strict JSON accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If canonical serialization fails.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return every field as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Same content as ``json.loads(self.to_json())``.

        Raises
        ------
        ValueError
            If canonical serialization fails.
        """
        ...

    def __eq__(self, other: object) -> bool:
        """
        Structural equality on every field.

        Parameters
        ----------
        other : object
            Value to compare with.

        Returns
        -------
        bool
            ``True`` when ``other`` is an ``XccyConventions`` with identical fields.
        """
        ...

    @property
    def base_currency(self) -> str:
        """
        ISO-4217 code of the base (first) currency.

        Returns
        -------
        str
            Three-letter code.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

    @property
    def quote_currency(self) -> str:
        """
        ISO-4217 code of the quote (second) currency.

        Returns
        -------
        str
            Three-letter code.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

    @property
    def base_index_id(self) -> str:
        """
        Floating index identifier of the base-currency leg.

        Returns
        -------
        str
            Registry key of the rate index paid on the base-currency leg,
            for example ``"USD-SOFR"``. It must resolve in the same
            :class:`ConventionRegistry` and in the market context used for
            pricing.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

    @property
    def quote_index_id(self) -> str:
        """
        Floating index identifier of the quote-currency leg.

        Returns
        -------
        str
            Registry key of the rate index paid on the quote-currency leg,
            for example ``"EUR-ESTR"``. This is the leg that carries the
            cross-currency basis spread.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

    @property
    def spot_lag_days(self) -> int:
        """
        Settlement lag from trade date to the swap effective date.

        Returns
        -------
        int
            Number of business days (typically ``2`` for major pairs),
            counted on the joint base and quote calendars, not calendar
            days.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

    @property
    def payment_frequency(self) -> str:
        """
        Payment frequency of both legs.

        Returns
        -------
        str
            Tenor string.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

    @property
    def day_count(self) -> str:
        """
        Accrual day count (serde string).

        Returns
        -------
        str
            Serde day-count name.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

    @property
    def business_day_convention(self) -> str:
        """
        Business-day convention for schedule rolls (serde string).

        Returns
        -------
        str
            e.g. ``"modified_following"``.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

    @property
    def base_calendar_id(self) -> str:
        """
        Calendar identifier of the base-currency leg.

        Returns
        -------
        str
            Registered calendar id.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

    @property
    def quote_calendar_id(self) -> str:
        """
        Calendar identifier of the quote-currency leg.

        Returns
        -------
        str
            Registered calendar id.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

    @property
    def notional_exchange(self) -> str:
        """
        Notional-exchange style (serde string of ``NotionalExchange``).

        Returns
        -------
        str
            Serde variant name.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

class IrFutureConventions:
    """
    Contract conventions for a listed interest-rate future.

    Returned by :meth:`ConventionRegistry.require_ir_future`.

    Examples
    --------
    >>> from finstack_quant.valuations.market import ConventionRegistry, IrFutureConventions
    >>> registry = ConventionRegistry()
    >>> isinstance(registry, ConventionRegistry)
    True
    """

    @staticmethod
    def from_json(json: str) -> IrFutureConventions:
        """
        Deserialize from the JSON produced by :meth:`to_json`.

        Parameters
        ----------
        json : str
            Strict JSON object with exactly the fields ``to_json`` writes.

        Returns
        -------
        IrFutureConventions
            The reconstructed record.

        Raises
        ------
        ValueError
            If the JSON is malformed or has the wrong shape.

        Examples
        --------
        >>> from finstack_quant.valuations.market import IrFutureConventions
        >>> try:
        ...     IrFutureConventions.from_json("{}")
        ... except ValueError:
        ...     print("rejected")
        rejected
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON wire form.

        Returns
        -------
        str
            Strict JSON accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If canonical serialization fails.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return every field as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Same content as ``json.loads(self.to_json())``.

        Raises
        ------
        ValueError
            If canonical serialization fails.
        """
        ...

    def __eq__(self, other: object) -> bool:
        """
        Structural equality on every field.

        Parameters
        ----------
        other : object
            Value to compare with.

        Returns
        -------
        bool
            ``True`` when ``other`` is an ``IrFutureConventions`` with identical fields.
        """
        ...

    @property
    def index_id(self) -> str:
        """
        Underlying floating index identifier.

        Returns
        -------
        str
            Registry key of the reference index the futures contract settles
            against, for example ``"USD-SOFR"`` for SOFR futures. It must
            resolve in the market context used to build the futures
            reference period.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

    @property
    def rate_averaging(self) -> str:
        """
        Rate averaging method over the reference period (serde string).

        Returns
        -------
        str
            Serde variant of ``RateAveragingMethod``.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

    @property
    def reference_period(self) -> str:
        """
        Reference-period placement relative to expiry (serde string).

        Returns
        -------
        str
            ``"forward_starting"`` or the other ``IrFutureReferencePeriod`` variant.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

    @property
    def calendar_id(self) -> str:
        """
        Exchange calendar identifier.

        Returns
        -------
        str
            Registered calendar id.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

    @property
    def settlement_days(self) -> int:
        """
        Settlement lag in business days.

        Returns
        -------
        int
            Settlement lag.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

    @property
    def delivery_months(self) -> int:
        """
        Number of listed delivery months.

        Returns
        -------
        int
            Count of delivery months.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

    @property
    def face_value(self) -> float:
        """
        Contract face value in the contract currency.

        Returns
        -------
        float
            Face value.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

    @property
    def tick_size(self) -> float:
        """
        Minimum price increment in price points.

        Returns
        -------
        float
            Tick size.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

    @property
    def tick_value(self) -> float:
        """
        Currency value of one tick.

        Returns
        -------
        float
            Tick value.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

    @property
    def convexity_adjustment(self) -> float | None:
        """
        Fixed convexity adjustment in decimal rate, or ``None``.

        Returns
        -------
        float | None
            Adjustment, or ``None`` when the pricer derives it.

        This accessor does not raise; it returns the stored convention value.
        """
        ...

class ConventionRegistry:
    """
    Process-global registry of embedded market conventions.

    Mirrors Rust ``ConventionRegistry::try_global()``: the tables (rate
    indices, CDS families, swaptions, inflation swaps, IR futures,
    cross-currency pairs) are loaded once from the crate's embedded JSON and
    shared. Lookups are read-only; missing identifiers raise ``KeyError``.

    Examples
    --------
    >>> from finstack_quant.valuations.market import ConventionRegistry
    >>> registry = ConventionRegistry()
    >>> registry.require_rate_index("USD-SOFR").currency
    'USD'
    >>> registry.primary_cds_family("USD")
    'isda_na'
    """

    def __init__(self) -> None:
        """
        Open the process-global convention registry.

        Raises
        ------
        RuntimeError
            If the embedded convention tables fail to load (a packaging error).

        Examples
        --------
        >>> from finstack_quant.valuations.market import ConventionRegistry
        >>> isinstance(ConventionRegistry(), ConventionRegistry)
        True
        """
        ...

    def require_rate_index(self, id: str) -> RateIndexConventions:
        """
        Look up the conventions of a floating-rate index.

        Parameters
        ----------
        id : str
            Index identifier as used on curves and legs, e.g. ``"USD-SOFR"``
            or ``"EUR-EURIBOR-3M"``.

        Returns
        -------
        RateIndexConventions
            Day count, frequencies, lags, calendar and settlement conventions.

        Raises
        ------
        KeyError
            If ``id`` is not in the embedded rate-index table.

        Examples
        --------
        >>> from finstack_quant.valuations.market import ConventionRegistry
        >>> ConventionRegistry().require_rate_index("USD-SOFR").kind
        'overnight_rfr'
        """
        ...

    def resolve_cds(self, currency: str, doc_clause: CdsDocClause) -> CdsConventionSpec:
        """
        Resolve the CDS schedule conventions for a currency and doc clause.

        Parameters
        ----------
        currency : str
            ISO-4217 code of the contract currency (``"USD"``, ``"EUR"``).
        doc_clause : {"cr14", "mr14", "mm14", "xr14", "isda_na", "isda_eu", "isda_as", "isda_au", "isda_nz"}
            ISDA doc clause or family name. The 2014 clauses map to the
            currency's primary family; ``"isda_na"`` is the SNAC /
            post-Big-Bang North American standard.

        Returns
        -------
        CdsConventionSpec
            Calendar, day count, roll, stub, settlement and frequency conventions.

        Raises
        ------
        ValueError
            If ``currency`` or ``doc_clause`` is not recognised.
        KeyError
            If no convention exists for the resolved family.

        Examples
        --------
        >>> from finstack_quant.valuations.market import ConventionRegistry
        >>> ConventionRegistry().resolve_cds("USD", "cr14").frequency
        '3M'
        """
        ...

    def primary_cds_family(self, currency: str) -> str | None:
        """
        Return the primary CDS convention family for a currency.

        Parameters
        ----------
        currency : str
            ISO-4217 code (``"USD"`` -> ``"isda_na"``, ``"EUR"`` -> ``"isda_eu"``).

        Returns
        -------
        str | None
            Family serde name, or ``None`` when the currency has no primary family.

        Raises
        ------
        ValueError
            If ``currency`` is not a valid ISO-4217 code.

        Examples
        --------
        >>> from finstack_quant.valuations.market import ConventionRegistry
        >>> ConventionRegistry().primary_cds_family("EUR")
        'isda_eu'
        """
        ...

    def require_swaption(self, id: str) -> SwaptionConventions:
        """
        Look up swaption conventions.

        Parameters
        ----------
        id : str
            Swaption convention identifier (typically the underlying index id,
            e.g. ``"USD"`` or ``"EUR"``).

        Returns
        -------
        SwaptionConventions
            Settlement, roll and fixed-leg conventions of the underlying swap.

        Raises
        ------
        KeyError
            If ``id`` is not in the embedded swaption table.

        Examples
        --------
        >>> from finstack_quant.valuations.market import ConventionRegistry
        >>> try:
        ...     ConventionRegistry().require_swaption("NOPE")
        ... except KeyError:
        ...     print("missing")
        missing
        """
        ...

    def require_inflation_swap(self, id: str) -> InflationSwapConventions:
        """
        Look up zero-coupon inflation-swap conventions.

        Parameters
        ----------
        id : str
            Inflation-swap convention identifier (e.g. ``"USD-CPI"``, ``"EUR-HICP"``, ``"UK-RPI"``).

        Returns
        -------
        InflationSwapConventions
            Calendar, settlement, day count and observation-lag conventions.

        Raises
        ------
        KeyError
            If ``id`` is not in the embedded inflation-swap table.

        Examples
        --------
        >>> from finstack_quant.valuations.market import ConventionRegistry
        >>> try:
        ...     ConventionRegistry().require_inflation_swap("NOPE")
        ... except KeyError:
        ...     print("missing")
        missing
        """
        ...

    def require_ir_future(self, id: str) -> IrFutureConventions:
        """
        Look up listed interest-rate-future contract conventions.

        Parameters
        ----------
        id : str
            Contract identifier (e.g. ``"CME:SR3"`` for CME 3M SOFR futures).

        Returns
        -------
        IrFutureConventions
            Index, averaging, reference period, tick and settlement conventions.

        Raises
        ------
        KeyError
            If ``id`` is not in the embedded IR-future table.

        Examples
        --------
        >>> from finstack_quant.valuations.market import ConventionRegistry
        >>> try:
        ...     ConventionRegistry().require_ir_future("NOPE")
        ... except KeyError:
        ...     print("missing")
        missing
        """
        ...

    def require_xccy(self, id: str) -> XccyConventions:
        """
        Look up cross-currency basis-swap conventions.

        Parameters
        ----------
        id : str
            Pair identifier (e.g. ``"EUR/USD-XCCY"``).

        Returns
        -------
        XccyConventions
            Leg indices, calendars, lags, frequency and notional-exchange style.

        Raises
        ------
        KeyError
            If ``id`` is not in the embedded cross-currency table.

        Examples
        --------
        >>> from finstack_quant.valuations.market import ConventionRegistry
        >>> try:
        ...     ConventionRegistry().require_xccy("NOPE")
        ... except KeyError:
        ...     print("missing")
        missing
        """
        ...

def listed_product_catalog(
    exchange: Literal["cme", "eurex", "montreal", "sgx"] | None = None,
) -> list[dict[str, object]]:
    """Return the maintained liquid listed-derivatives coverage catalog.

    Parameters
    ----------
    exchange : {"cme", "eurex", "montreal", "sgx"} | None, optional
        Exact venue filter. ``None`` returns all four exchanges.

    Returns
    -------
    list[dict[str, object]]
        Product-family rows containing the canonical instrument type, covered
        exchange features, source URL, and any residual modelling gap.

    Raises
    ------
    ValueError
        If ``exchange`` is not one of the accepted canonical venue names, or
        if the embedded listed-product sidecar is invalid.

    Examples
    --------
    >>> from finstack_quant.valuations.market import listed_product_catalog
    >>> rows = listed_product_catalog("cme")
    >>> all(row["exchange"] == "cme" for row in rows)
    True
    """
    ...
