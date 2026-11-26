"""Stub declarations mirroring ``finstack-quant-py/src/bindings/valuations/instruments/bond.rs``."""

from __future__ import annotations

from finstack_quant.cashflows.builder import CashFlowSchedule
import datetime
from typing import Any, Literal
import pandas as pd
from finstack_quant.core.dates import DayCount, StubKind, Tenor
from finstack_quant.core.money import Money
from finstack_quant.core.types import Attributes, Bps, Rate
from finstack_quant.valuations.instruments._merton_mc import MertonMcConfig, MertonMcResult
from finstack_quant.valuations.instruments._pricing import MarketHistory, MetricPricingOverrides

class Bond:
    """
    Typed wrapper for the canonical Rust ``Bond`` instrument.

    Construct via :meth:`Bond.fixed` (US corporate), :meth:`Bond.with_convention`
    (a named preset; override its stub with :meth:`Bond.with_stub`), :meth:`Bond.floating` /
    :meth:`Bond.floating_with_convention`, :meth:`Bond.zero_coupon`, the
    :meth:`Bond.builder` fluent builder (callable schedules, credit curve,
    custom cashflow specs and settlement conventions), the ``Bond.example*``
    presets or :meth:`Bond.from_json`. Every public Rust field is readable
    as a property. Instances are accepted directly by
    :func:`price_instrument` and :func:`instrument_cashflows`.

    Examples
    --------
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.valuations.instruments import Bond
    >>> bond = Bond.fixed(
    ...     "BOND-1",
    ...     Money(1_000_000.0, "USD"),
    ...     0.05,
    ...     "2024-01-01",
    ...     "2034-01-01",
    ...     "none",
    ...     "USD-OIS",
    ... )
    >>> bond.id
    'BOND-1'
    >>> bond.notional.amount
    1000000.0
    """

    @staticmethod
    def from_cashflows(
        id: str,
        schedule: CashFlowSchedule,
        quote_compounding: str,
        discount_curve_id: str,
        quoted_clean_price_pct: float | None = None,
    ) -> Bond:
        """Construct a bond from an explicit cashflow schedule.

        Parameters
        ----------
        id : str
            Stable bond identifier used for pricing and metric keys.
        schedule : CashFlowSchedule
            Validated principal and coupon schedule, copied into the bond.
        quote_compounding : str
            Compounding of spread quotes. Use continuous, simple, semi_annual,
            quarterly, monthly, or periodic(n). Supply it independently of
            coupon frequency.
        discount_curve_id : str
            Market discount curve used to value the scheduled cashflows.
        quoted_clean_price_pct : float or None
            Observed clean price in percent of par (99.5 means 99.5%).
            Omit to calculate prices without an observed clean-price override.

        Returns
        -------
        Bond
            Typed bond ready to price against the discount curve.

        Raises
        ------
        ValueError
            If the schedule is invalid or the quote convention or bond terms are invalid.

        Examples
        --------
        >>> from datetime import date
        >>> from finstack_quant.cashflows.builder import CashFlowSchedule
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.instruments import Bond
        >>> schedule = (
        ...     CashFlowSchedule.builder().principal(Money(100, "USD"), date(2025, 1, 1), date(2026, 1, 1)).build()
        ... )
        >>> Bond.from_cashflows("custom", schedule, "semi_annual", "USD-OIS").id
        'custom'
        """
        ...

    @staticmethod
    def builder() -> BondBuilder:
        """
        Create a fluent builder (mirrors Rust ``Bond::builder()``).

        Returns
        -------
        BondBuilder
            A builder with fluent, consuming setter methods.

        Notes
        -----
        This factory does not raise; it returns an empty builder.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Bond
        >>> builder = Bond.builder()
        >>> builder.id("EXAMPLE") is builder
        True
        """
        ...
    @staticmethod
    def fixed(
        id: str,
        notional: Money,
        coupon_rate: float | Rate,
        issue_date: datetime.date | datetime.datetime | pd.Timestamp | str,
        maturity: datetime.date | datetime.datetime | pd.Timestamp | str,
        stub: StubKind | Literal["none", "short_front", "long_front", "short_back", "long_back"],
        discount_curve_id: str,
    ) -> Bond:
        """
        Create a US corporate fixed-rate bond (semi-annual, 30/360, T+1).

        Mirrors Rust ``Bond::fixed``. For another market's conventions use
        :meth:`Bond.with_convention` and, to override the preset's stub rule,
        chain :meth:`Bond.with_stub`.

        Parameters
        ----------
        id : str
            Unique instrument identifier.
        notional : Money
            Principal amount; its currency is the coupon and redemption currency.
        coupon_rate : float | Rate
            Annual coupon as a decimal (``0.05`` = 5%) or a ``Rate``.
        issue_date : datetime.date | datetime.datetime | pd.Timestamp | str
            Issue date (ISO 8601 strings accepted).
        maturity : datetime.date | datetime.datetime | pd.Timestamp | str
            Maturity date.
        stub : StubKind | str
            Placement and length policy for an irregular coupon period, as a
            ``StubKind`` or its serde name (``"none"``, ``"short_front"``, ...).
        discount_curve_id : str
            Discount curve identifier used for pricing.

        Returns
        -------
        Bond
            A validated fixed-rate bond.

        Raises
        ------
        ValueError
            If ``stub`` is not a recognized name or validation fails (e.g.
            maturity not after issue_date).
        TypeError
            If ``notional`` is not a ``Money``, ``coupon_rate`` has an
            unsupported type or a date cannot be interpreted.

        Examples
        --------
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.instruments import Bond
        >>> bond = Bond.fixed("BOND-1", Money(1_000_000.0, "USD"), 0.05, "2024-01-01", "2034-01-01", "none", "USD-OIS")
        >>> bond.settlement_days
        1
        """
        ...
    def with_stub(
        self, stub: StubKind | Literal["none", "short_front", "long_front", "short_back", "long_back"]
    ) -> Bond:
        """
        Return a copy of this bond with a different coupon-schedule stub rule.

        Mirrors Rust ``Bond::with_stub``; the receiver is not modified.

        Parameters
        ----------
        stub : StubKind | str
            Placement and length policy for an irregular coupon period, as a
            ``StubKind`` or its serde name (``"none"``, ``"short_front"``, ...).

        Returns
        -------
        Bond
            A new bond whose coupon schedule uses ``stub``.

        Raises
        ------
        ValueError
            If ``stub`` is not a recognized stub name.

        Examples
        --------
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.instruments import Bond
        >>> bund = Bond.with_convention(
        ...     "BUND", Money(1_000_000.0, "EUR"), 0.025, "2024-01-15", "2034-01-15", "german_bund", "EUR-OIS"
        ... ).with_stub("long_back")
        >>> bund.to_dict()["cashflow_spec"]["fixed"]["stub"]
        'long_back'
        """
        ...
    @staticmethod
    def with_convention(
        id: str,
        notional: Money,
        coupon_rate: float | Rate,
        issue_date: datetime.date | datetime.datetime | pd.Timestamp | str,
        maturity: datetime.date | datetime.datetime | pd.Timestamp | str,
        convention: Literal[
            "us_treasury", "us_agency", "german_bund", "uk_gilt", "french_oat", "jgb", "us_corporate", "eur_corporate"
        ],
        discount_curve_id: str,
    ) -> Bond:
        """
        Create a fixed-rate bond from a named market convention.

        Mirrors Rust ``Bond::with_convention``; the stub rule is the preset's own
        (chain :meth:`Bond.with_stub` to override it).

        Parameters
        ----------
        id : str
            Unique instrument identifier.
        notional : Money
            Principal amount; its currency is the coupon and redemption currency.
        coupon_rate : float | Rate
            Annual coupon as a decimal (``0.05`` = 5%) or a ``Rate``.
        issue_date : datetime.date | datetime.datetime | pd.Timestamp | str
            Issue date.
        maturity : datetime.date | datetime.datetime | pd.Timestamp | str
            Maturity date.
        convention : str
            Bond convention preset (``"us_treasury"``, ``"us_agency"``,
            ``"german_bund"``, ``"uk_gilt"``, ``"french_oat"``, ``"jgb"``,
            ``"us_corporate"``, ``"eur_corporate"``).
        discount_curve_id : str
            Discount curve identifier used for pricing.

        Returns
        -------
        Bond
            A validated fixed-rate bond.

        Raises
        ------
        ValueError
            If ``convention`` is unknown or validation fails.
        TypeError
            If ``notional`` is not a ``Money``, ``coupon_rate`` has an
            unsupported type or a date cannot be interpreted.

        Examples
        --------
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.instruments import Bond
        >>> gilt = Bond.with_convention(
        ...     "GILT",
        ...     Money(1_000_000.0, "GBP"),
        ...     0.04,
        ...     "2024-01-01",
        ...     "2034-01-01",
        ...     "uk_gilt",
        ...     "GBP-OIS",
        ... )
        >>> gilt.settlement_days
        1
        """
        ...
    @staticmethod
    def floating(params: dict[str, Any] | str) -> Bond:
        """
        Create a floating-rate bond (FRN) linked to a forward index.

        Mirrors Rust ``Bond::floating``. Settlement, calendar, and business-day
        convention come from the notional currency: USD ``us_corporate`` (T+1,
        ``usny``), EUR ``eur_corporate`` (T+2, ``target2``), GBP ``uk_gilt``
        (T+1), JPY ``jgb`` (T+2). Other currencies raise ``ValueError``; use
        :meth:`Bond.floating_with_convention` to name the preset explicitly.

        Parameters
        ----------
        params : dict[str, Any] | str
            ``FloatingBondParams`` as a dict or JSON text. Every key is
            required:

            - ``id`` (str): unique instrument identifier.
            - ``notional`` (Money | dict): principal amount, a ``Money`` or its
              wire dict ``{"amount": "1000000", "currency": "USD"}``; its
              currency is the coupon and redemption currency.
            - ``forward_curve_id`` (str): forward curve of the coupon index
              (e.g. ``"USD-SOFR-3M"``).
            - ``spread_bp``: spread over the index in basis points. The exact wire form is a decimal string such as ``"25.125"``. Python mappings also accept Decimal, Rate, Bps, or a finite number.
              (``125`` = 1.25%); a fractional value raises ``ValueError`` (use
              :meth:`Bond.from_json` for sub-bp margins).
            - ``issue_date`` / ``maturity`` (datetime.date | str): start of the
              first coupon period and redemption date (ISO 8601 strings).
            - ``frequency`` (Tenor | dict): coupon frequency, a ``Tenor``
              (``Tenor.quarterly()``) or its wire dict
              ``{"count": 3, "unit": "months"}``.
            - ``day_count`` (DayCount | str): coupon accrual day count
              (``DayCount.ACT_360`` or its label ``"act_360"``).
            - ``discount_curve_id`` (str): discount curve used for pricing.

        Returns
        -------
        Bond
            A validated floating-rate note.

        Raises
        ------
        ValueError
            If ``params`` has a missing, unknown or malformed key, the notional
            currency has no mapped settlement convention, ``notional`` is not
            finite and positive, or ``issue_date`` is not strictly before
            ``maturity``.

        Examples
        --------
        >>> from finstack_quant.core.dates import DayCount, Tenor
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.instruments import Bond
        >>> frn = Bond.floating({
        ...     "id": "FRN",
        ...     "notional": Money(1000.0, "USD"),
        ...     "forward_curve_id": "USD-SOFR-3M",
        ...     "spread_bp": 125,
        ...     "issue_date": "2024-01-01",
        ...     "maturity": "2029-01-01",
        ...     "frequency": Tenor.quarterly(),
        ...     "day_count": DayCount.ACT_360,
        ...     "quote_compounding": {"periodic": 4},
        ...     "discount_curve_id": "USD-OIS",
        ... })
        >>> frn.has_floating_coupons
        True
        """
        ...
    @staticmethod
    def floating_with_convention(
        params: dict[str, Any] | str,
        convention: Literal[
            "us_treasury", "us_agency", "german_bund", "uk_gilt", "french_oat", "jgb", "us_corporate", "eur_corporate"
        ],
    ) -> Bond:
        """
        Create a floating-rate bond with an explicit convention preset.

        Mirrors Rust ``Bond::floating_with_convention``: the preset supplies
        settlement lag, business-day convention and calendar.

        Parameters
        ----------
        params : dict[str, Any] | str
            ``FloatingBondParams`` as a dict or JSON text. Every key is
            required:

            - ``id`` (str): unique instrument identifier.
            - ``notional`` (Money | dict): principal amount, a ``Money`` or its
              wire dict ``{"amount": "1000000", "currency": "USD"}``; its
              currency is the coupon and redemption currency.
            - ``forward_curve_id`` (str): forward curve of the coupon index
              (e.g. ``"USD-SOFR-3M"``).
            - ``spread_bp``: spread over the index in basis points. The exact wire form is a decimal string such as ``"25.125"``. Python mappings also accept Decimal, Rate, Bps, or a finite number.
              (``125`` = 1.25%); a fractional value raises ``ValueError`` (use
              :meth:`Bond.from_json` for sub-bp margins).
            - ``issue_date`` / ``maturity`` (datetime.date | str): start of the
              first coupon period and redemption date (ISO 8601 strings).
            - ``frequency`` (Tenor | dict): coupon frequency, a ``Tenor``
              (``Tenor.quarterly()``) or its wire dict
              ``{"count": 3, "unit": "months"}``.
            - ``day_count`` (DayCount | str): coupon accrual day count
              (``DayCount.ACT_360`` or its label ``"act_360"``).
            - ``discount_curve_id`` (str): discount curve used for pricing.
        convention : str
            Bond convention preset (see :meth:`Bond.with_convention`).

        Returns
        -------
        Bond
            A validated floating-rate note.

        Raises
        ------
        ValueError
            If ``params`` has a missing, unknown or malformed key,
            ``convention`` is unknown, or validation fails.

        Examples
        --------
        >>> from finstack_quant.core.dates import DayCount, Tenor
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.instruments import Bond
        >>> frn = Bond.floating_with_convention(
        ...     {
        ...         "id": "FRN-EUR",
        ...         "notional": Money(1000.0, "EUR"),
        ...         "forward_curve_id": "EUR-EURIBOR-3M",
        ...         "spread_bp": 80,
        ...         "issue_date": "2024-01-01",
        ...         "maturity": "2029-01-01",
        ...         "frequency": Tenor.quarterly(),
        ...         "day_count": DayCount.ACT_360,
        ...         "quote_compounding": {"periodic": 4},
        ...         "discount_curve_id": "EUR-OIS",
        ...     },
        ...     "eur_corporate",
        ... )
        >>> frn.settlement_days
        2
        """
        ...
    @staticmethod
    def zero_coupon(
        id: str,
        notional: Money,
        issue_date: datetime.date | datetime.datetime | pd.Timestamp | str,
        maturity: datetime.date | datetime.datetime | pd.Timestamp | str,
        quote_compounding: str,
        discount_curve_id: str,
    ) -> Bond:
        """
        Create a zero-coupon bond (single principal redemption at maturity).

        Mirrors Rust ``Bond::zero_coupon``.

        Parameters
        ----------
        id : str
            Unique instrument identifier.
        notional : Money
            Redemption amount; its currency is the bond currency.
        issue_date : datetime.date | datetime.datetime | pd.Timestamp | str
            Issue date.
        maturity : datetime.date | datetime.datetime | pd.Timestamp | str
            Maturity (redemption) date.
        quote_compounding : str
            Compounding of the bond's z-spread quotes: ``"continuous"``,
            ``"simple"``, ``"semi_annual"``, ``"quarterly"``, ``"monthly"`` or
            ``"periodic(n)"``. Required; never derived from the coupon grid.
        discount_curve_id : str
            Discount curve identifier used for pricing.

        Returns
        -------
        Bond
            A validated zero-coupon bond.

        Raises
        ------
        ValueError
            If ``maturity`` is not after ``issue_date`` or ``quote_compounding``
            is not a known label.
        TypeError
            If ``notional`` is not a ``Money`` or a date cannot be interpreted.

        Examples
        --------
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.instruments import Bond
        >>> zc = Bond.zero_coupon("ZC", Money(1_000_000.0, "USD"), "2024-01-01", "2029-01-01", "periodic(1)", "USD-OIS")
        >>> zc.has_floating_coupons
        False
        """
        ...
    @staticmethod
    def example() -> Bond:
        """
        Canonical example: 5-year USD 5% semi-annual fixed-rate bond discounted on ``USD-OIS`` (mirrors Rust ``Bond::example``).

        Returns
        -------
        Bond
            The example bond.

        Raises
        ------
        ValueError
            If construction fails (should not occur).

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Bond
        >>> Bond.example().discount_curve_id
        'USD-TREASURY'
        """
        ...
    @staticmethod
    def example_floating() -> Bond:
        """
        Canonical example: USD SOFR-linked floating-rate note (mirrors Rust ``Bond::example_floating``).

        Returns
        -------
        Bond
            The example bond.

        Raises
        ------
        ValueError
            If construction fails (should not occur).

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Bond
        >>> Bond.example_floating().has_floating_coupons
        True
        """
        ...
    @staticmethod
    def example_callable() -> Bond:
        """
        Canonical example: fixed-rate bond carrying a call schedule (mirrors Rust ``Bond::example_callable``).

        Returns
        -------
        Bond
            The example bond.

        Raises
        ------
        ValueError
            If construction fails (should not occur).

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Bond
        >>> Bond.example_callable().call_put is not None
        True
        """
        ...
    @staticmethod
    def example_amortizing() -> Bond:
        """
        Canonical example: fixed-rate bond with a principal amortization schedule (mirrors Rust ``Bond::example_amortizing``).

        Returns
        -------
        Bond
            The example bond.

        Raises
        ------
        ValueError
            If construction fails (should not occur).

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Bond
        >>> Bond.example_amortizing().cashflow_spec.keys() >= {"amortizing"}
        True
        """
        ...
    @classmethod
    def from_json(cls, json: str) -> Bond:
        """
        Deserialize a validated Bond from its canonical v1 envelope.

        Parameters
        ----------
        json : str
            A ``finstack_quant.instrument/1`` envelope containing an exact
            ``"bond"`` payload. The UTF-8 input must not exceed 16 MiB.
            Bare payloads and cross-type coercion are rejected.

        Returns
        -------
        Bond
            The validated instrument.

        Raises
        ------
        ValueError
            If input exceeds 16 MiB, is malformed, uses an unsupported
            envelope schema, carries another type, or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Bond
        >>> Bond.from_json(Bond.example().to_json()).id == Bond.example().id
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
            and :meth:`Bond.from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    def to_dict(self) -> dict[str, object]:
        """
        Serde form of the bond spec as a plain Python ``dict``.

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
    def to_builder(self) -> BondBuilder:
        """
        Convert this bond into an editable :class:`BondBuilder`.

        Returns
        -------
        BondBuilder
            Builder pre-populated with this bond's fields for incremental editing.

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Bond
        >>> b = Bond.example().to_builder().id("BOND-EDIT").build()
        >>> b.id
        'BOND-EDIT'
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
    def min_moic(self, multiple: float) -> Bond:
        """
        Return a copy with a minimum MOIC return floor on early redemption
        (mirrors Rust ``Bond::min_moic``).

        Parameters
        ----------
        multiple : float
            Minimum multiple of invested capital (e.g. ``1.25``).

        Returns
        -------
        Bond
            A new bond with ``return_floor`` set; ``self`` is unchanged.

        Notes
        -----
        This method does not raise; the floor is validated at pricing time.
        """
        ...
    def min_xirr(self, rate: float | Rate) -> Bond:
        """
        Return a copy with a minimum XIRR return floor on early redemption
        (mirrors Rust ``Bond::min_xirr``).

        Parameters
        ----------
        rate : float | Rate
            Target annualized IRR as a decimal (``0.12`` = 12%) or a ``Rate``.

        Returns
        -------
        Bond
            A new bond with ``return_floor`` set; ``self`` is unchanged.

        Raises
        ------
        TypeError
            If ``rate`` is neither a number nor a ``Rate``.
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
    def notional(self) -> Money:
        """
        Principal amount of the bond.

        Returns
        -------
        Money
            Currency-tagged principal.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def issue_date(self) -> datetime.date:
        """
        Issue date of the bond.

        Returns
        -------
        datetime.date
            The contractual issue date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def maturity(self) -> datetime.date:
        """
        Maturity (final redemption) date.

        Returns
        -------
        datetime.date
            The contractual maturity date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def cashflow_spec(self) -> dict[str, object]:
        """
        Coupon/cashflow specification in serde form.

        Returns
        -------
        dict[str, object]
            One-key dict: ``{"fixed": {...}}``, ``{"floating": {...}}``, ``{"step_up": {...}}`` or ``{"amortizing": {...}}``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def quote_compounding(self) -> str:
        """
        Quote compounding of the z-spread.

        Shared by spread pricing, quote inversion and CS01; stated
        independently of the coupon frequency, never derived from it.

        Returns
        -------
        str
            ``"continuous"``, ``"simple"``, ``"semi_annual"``, ``"quarterly"``,
            ``"monthly"`` or ``"periodic(n)"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def discount_curve_id(self) -> str:
        """
        Discount curve identifier.

        Returns
        -------
        str
            Curve id used for discounting.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def credit_curve_id(self) -> str | None:
        """
        Hazard curve identifier for ``hazard_rate`` and ``rates_credit`` pricing.

        Returns
        -------
        str | None
            Curve id, or ``None`` when no credit-consuming model is configured.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def repo_curve_id(self) -> str | None:
        """
        Repo (financing) discount curve identifier.

        Returns
        -------
        str | None
            Curve id, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def call_put(self) -> dict[str, object] | None:
        """
        Call/put schedule in serde form.

        Returns
        -------
        dict[str, object] | None
            ``{"calls": [...], "puts": [...]}`` or ``None`` for a bullet bond.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def return_floor(self) -> dict[str, object] | None:
        """
        Return-floor specification (minimum MOIC / XIRR) in serde form.

        Returns
        -------
        dict[str, object] | None
            The spec dict, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def custom_cashflows(self) -> dict[str, object] | None:
        """
        Explicit cashflow schedule overriding generated coupons, in serde form.

        Returns
        -------
        dict[str, object] | None
            The schedule dict, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def accrual_method(self) -> str:
        """
        Accrual method (serde string).

        Returns
        -------
        str
            ``"linear"`` unless overridden.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def settlement_convention(self) -> dict[str, object] | None:
        """
        Settlement convention (settlement lag, ex-coupon period) in serde form.

        Returns
        -------
        dict[str, object] | None
            ``{"settlement_days": ..., "ex_coupon_days": ..., "ex_coupon_calendar_id": ...}`` or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def settlement_days(self) -> int | None:
        """
        Settlement lag in business days.

        Returns
        -------
        int | None
            The lag, or ``None`` when no settlement convention is set.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def ex_coupon_days(self) -> int | None:
        """
        Ex-coupon period in business days.

        Returns
        -------
        int | None
            The period, or ``None`` when no settlement convention is set.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def has_floating_coupons(self) -> bool:
        """
        Whether coupons depend on forward-curve projection (FRNs).

        Returns
        -------
        bool
            ``True`` for floating (or amortizing-floating) cashflow specs.

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
            Registered model key such as ``"discounting"`` or ``"black76"``.

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
        """
        Expiry date exposed by the Rust ``Instrument`` trait.

        Returns
        -------
        datetime.date | None
            The expiry/maturity date, or ``None`` when the instrument type reports none.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    def price_merton_mc(
        self,
        config: MertonMcConfig,
        discount_rate: float,
        as_of: datetime.date | datetime.datetime | pd.Timestamp | str,
    ) -> MertonMcResult:
        """
        Price this bond with the Merton Monte Carlo structural credit engine.

        Uses geometric Brownian motion asset dynamics only. Floating-rate and
        amortizing cashflow specs raise ``ValueError``. When the config's PIK
        schedule is the default uniform cash mode, the bond's ``CouponType``
        overrides the schedule; otherwise the config schedule takes precedence.
        The independent-estimator count, antithetic flag and seed label come
        from this bond's ``instrument_pricing_overrides.model_config``
        (``mc_paths``, ``mc_antithetic``, ``mc_seed_scenario``), defaulting to
        5,000 antithetic estimators seeded from the bond id.

        Parameters
        ----------
        config : MertonMcConfig
            Structural model, PIK schedule, recovery and time grid.
        discount_rate : float
            Flat continuously compounded risk-free rate as a decimal used to
            discount simulated cashflows (unless term-structure discount factors
            are set on the config).
        as_of : datetime.date | datetime.datetime | pd.Timestamp | str
            Valuation date.

        Returns
        -------
        MertonMcResult
            Monte Carlo pricing result with clean/dirty prices and path stats.

        Raises
        ------
        ValueError
            If ``as_of`` is invalid, the bond has floating or amortizing
            cashflows, or simulation parameters fail validation.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Bond, MertonMcConfig, PikMode, PikSchedule
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.models.credit import MertonModel
        >>> bond = Bond.fixed(
        ...     "BOND-MC",
        ...     Money(1_000_000.0, "USD"),
        ...     0.08,
        ...     "2024-01-01",
        ...     "2029-01-01",
        ...     "none",
        ...     "USD-OIS",
        ... )
        >>> merton = MertonModel(100.0, 0.25, 80.0, 0.04)
        >>> config = MertonMcConfig(merton, 0.40).pik_schedule(PikSchedule.uniform(PikMode.pik()))
        >>> bond.price_merton_mc(config, 0.04, "2024-01-01").num_paths
        5000
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            Reconstruction callable and canonical instrument JSON arguments.

        Raises
        ------
        ValueError
            If serialization of the stored instrument fails.
        """
        ...

class BondBuilder:
    """
    Fluent builder for :class:`Bond`; wraps the Rust ``FinancialBuilder``
    output one setter for one setter.

    Builders are consumed by ``build()``; create a new builder per
    instrument. Required fields: ``id``, ``notional``, ``issue_date``, ``maturity``,
    ``cashflow_spec``, ``discount_curve_id``. Nested specs accept a ``dict`` or JSON ``str``
    in the Rust serde shape; ``Bond.example().to_dict()`` shows the exact
    field names.

    Examples
    --------
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.valuations.instruments import Bond
    >>> base = Bond.example().to_dict()
    >>> callable_bond = (
    ...     Bond
    ...     .builder()
    ...     .id("CALLABLE")
    ...     .notional(Money(1_000_000.0, "USD"))
    ...     .issue_date("2024-01-15")
    ...     .maturity("2034-01-15")
    ...     .cashflow_spec(base["cashflow_spec"])
    ...     .discount_curve_id("USD-OIS")
    ...     .credit_curve_id("ACME-HZD")
    ...     .call_put({
    ...         "calls": [{"start": "2029-01-15", "end": "2034-01-15", "price_pct_of_par": 100.0}],
    ...         "puts": [],
    ...     })
    ...     .quote_compounding("semi_annual")
    ...     .build()
    ... )
    >>> callable_bond.credit_curve_id
    'ACME-HZD'
    """

    def id(self, value: str) -> BondBuilder:
        """
        Set the instrument identifier.

        Parameters
        ----------
        value : str
            Instrument identifier.

        Returns
        -------
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def notional(self, value: Money) -> BondBuilder:
        """
        Set the principal amount.

        Parameters
        ----------
        value : Money
            Principal amount; its currency is the bond currency.

        Returns
        -------
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        TypeError
            If ``value`` is not a ``finstack_quant.core.money.Money``.
        """
        ...
    def issue_date(self, value: datetime.date | datetime.datetime | pd.Timestamp | str) -> BondBuilder:
        """
        Set the required contractual issue date.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pd.Timestamp | str
            Required contractual issue date (ISO 8601 strings accepted). Omission raises ValueError on build().

        Returns
        -------
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or the date cannot be interpreted.
        """
        ...
    def maturity(self, value: datetime.date | datetime.datetime | pd.Timestamp | str) -> BondBuilder:
        """
        Set the maturity date.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pd.Timestamp | str
            Maturity date (ISO 8601 strings accepted).

        Returns
        -------
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or the date cannot be interpreted.
        """
        ...
    def cashflow_spec(self, value: dict[str, object] | str) -> BondBuilder:
        """
        Set the coupon/cashflow specification.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``CashflowSpec`` in serde form (``dict`` or JSON string), e.g. ``{"fixed": {"coupon_type": "cash", "rate": "0.05", "schedule": {...}}}`` (copy ``Bond.example().cashflow_spec``).

        Returns
        -------
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does not deserialize as a ``CashflowSpec``.
        """
        ...
    def quote_compounding(self, value: str) -> BondBuilder:
        """
        Set the quote compounding of the z-spread (required).

        Parameters
        ----------
        value : str
            ``"continuous"``, ``"simple"``, ``"semi_annual"``, ``"quarterly"``,
            ``"monthly"`` or ``"periodic(n)"``; stated independently of the
            coupon frequency.

        Returns
        -------
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the label is unknown or the builder was already consumed by ``build()``.
        """
        ...
    def discount_curve_id(self, value: str) -> BondBuilder:
        """
        Set the discount curve identifier.

        Parameters
        ----------
        value : str
            Discount curve identifier.

        Returns
        -------
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def credit_curve_id(self, value: str) -> BondBuilder:
        """
        Set the hazard curve identifier for ``hazard_rate`` and ``rates_credit`` pricing.

        Parameters
        ----------
        value : str
            Hazard curve identifier for scalar or joint rates-credit pricing.

        Returns
        -------
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def repo_curve_id(self, value: str) -> BondBuilder:
        """
        Set the repo (financing) discount curve identifier.

        Parameters
        ----------
        value : str
            Repo (financing) discount curve identifier.

        Returns
        -------
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()``.
        """
        ...
    def call_put(self, value: dict[str, object] | str) -> BondBuilder:
        """
        Set the call/put schedule.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``CallPutSchedule`` in serde form (``dict`` or JSON string), e.g. ``{"calls": [{"start": "2027-01-15", "end": "2029-01-15", "price_pct_of_par": 100.0}], "puts": []}``.

        Returns
        -------
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does not deserialize as a ``CallPutSchedule``.
        """
        ...
    def return_floor(self, value: dict[str, object] | str) -> BondBuilder:
        """
        Set the return-floor specification (minimum MOIC / XIRR on early redemption).

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``ReturnFloorSpec`` in serde form (``dict`` or JSON string), e.g. ``Bond.example().min_moic(1.25).return_floor``.

        Returns
        -------
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does not deserialize as a ``ReturnFloorSpec``.
        """
        ...
    def custom_cashflows(self, value: dict[str, object] | str) -> BondBuilder:
        """
        Set the explicit cashflow schedule that overrides generated coupons.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``CashFlowSchedule`` in serde form (``dict`` or JSON string), e.g. the ``custom_cashflows`` value of a bond built from cashflows.

        Returns
        -------
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does not deserialize as a ``CashFlowSchedule``.
        """
        ...
    def accrual_method(self, value: str) -> BondBuilder:
        """
        Set the accrual method.

        Parameters
        ----------
        value : str
            Accrual method (serde string). ``"linear"`` is the default.

        Returns
        -------
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` is not a recognized name.
        """
        ...
    def settlement_convention(self, value: dict[str, object] | str) -> BondBuilder:
        """
        Set the settlement convention (settlement lag and ex-coupon period).

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``BondSettlementConvention`` in serde form (``dict`` or JSON string), e.g. ``{"settlement_days": 2, "ex_coupon_days": 0, "ex_coupon_calendar_id": None}``.

        Returns
        -------
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does not deserialize as a ``BondSettlementConvention``.
        """
        ...
    def instrument_pricing_overrides(self, value: dict[str, object] | str) -> BondBuilder:
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
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``InstrumentPricingOverrides`` (unknown fields are rejected).
        """
        ...
    def metric_pricing_overrides(self, value: MetricPricingOverrides | dict[str, object] | str) -> BondBuilder:
        """
        Set the metric-time pricing configuration stored on the instrument.

        Parameters
        ----------
        value : MetricPricingOverrides | dict[str, object] | str
            Rust ``MetricPricingOverrides`` as the typed class, a ``dict`` or a
            JSON string.

        Returns
        -------
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``MetricPricingOverrides`` (unknown fields are rejected).
        """
        ...
    def scenario_pricing_overrides(self, value: dict[str, object] | str) -> BondBuilder:
        """
        Set the scenario-only pricing adjustments.

        Parameters
        ----------
        value : dict[str, object] | str
            Rust ``ScenarioPricingOverrides`` in serde form (``dict`` or JSON
            string), e.g. ``{"scenario_price_shock_decimal": -0.02}``.

        Returns
        -------
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If the builder was already consumed by ``build()`` or ``value`` does
            not deserialize as ``ScenarioPricingOverrides`` (unknown fields are rejected).
        """
        ...
    def attributes(self, value: Attributes | dict[str, Any] | None) -> BondBuilder:
        """
        Set instrument attributes (tags and metadata).

        Parameters
        ----------
        value : Attributes | dict | None
            Attribute bag: an ``Attributes`` or its serde ``dict`` form
            (``{"tags": [...], "meta": {...}}``, as ``to_dict()`` returns it);
            ``None`` clears it.

        Returns
        -------
        BondBuilder
            ``self``, for chaining.

        Raises
        ------
        TypeError
            If ``value`` is neither ``Attributes``, a ``dict`` nor ``None``.
        ValueError
            If the ``dict`` has a key other than ``tags`` / ``meta`` or a non-string tag or value.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style rendering of the fields set so far, e.g.
        ``BondBuilder(id='X', notional=Money(1000000.0, 'USD'))``.

        Returns
        -------
        str
            ``BondBuilder(<field>=<value>, ...)``.

        Notes
        -----
        This method does not raise; it renders the recorded setter calls.
        """
        ...
    def build(self) -> Bond:
        """
        Build the validated bond.

        Runs the same validation as the Rust ``BondBuilder::build`` (structural
        invariants only); pricing-time checks run in :func:`price_instrument`.

        Returns
        -------
        Bond
            The validated instrument.

        Raises
        ------
        ValueError
            If the builder was already consumed, a required field is missing
            (the message names the builder and every missing field, e.g.
            ``BondBuilder: missing required field 'id'``), or the instrument
            fails validation.
        """
        ...
