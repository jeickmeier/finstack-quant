"""Stub declarations mirroring ``finstack-quant-py/src/bindings/valuations/typed_structured_credit/tranche.rs``."""

from __future__ import annotations

import datetime
from typing import Any, Literal
import pandas as pd
from finstack_quant.core.dates import DayCount, Tenor
from finstack_quant.core.money import Money
from finstack_quant.core.types import Attributes, Rate

class Tranche:
    """
    Structured-credit tranche with attachment/detachment points.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.valuations.instruments import Tranche
    >>> tranche = (
    ...     Tranche
    ...     .builder()
    ...     .id("A")
    ...     .attach_pct(0.0)
    ...     .detach_pct(100.0)
    ...     .seniority("senior")
    ...     .original_balance(Money(100.0, Currency("USD")))
    ...     .coupon_fixed(0.05)
    ...     .maturity(datetime.date(2029, 1, 1))
    ...     .build()
    ... )
    >>> tranche.seniority, tranche.attach_pct, tranche.detach_pct
    ('senior', 0.0, 100.0)

    """

    @staticmethod
    def builder() -> TrancheBuilder:
        """
        Create a fluent builder (mirrors Rust ``Tranche::builder()``).

        Returns
        -------
        TrancheBuilder
            A builder with fluent, consuming setter methods.

        Notes
        -----
        This factory does not raise; it returns a new instance with the documented defaults.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import Tranche
        >>> builder = Tranche.builder()
        >>> builder.id("EXAMPLE") is builder
        True
        """
        ...

    @staticmethod
    def from_json(json: str) -> Tranche:
        """
        Deserialize from the JSON produced by :meth:`to_json`.

        Parameters
        ----------
        json : str
            Strict JSON object with exactly the fields ``to_json`` writes.

        Returns
        -------
        Tranche
            The reconstructed value.

        Raises
        ------
        ValueError
            If the JSON is malformed or has the wrong shape.

        Examples
        --------
        >>> import datetime
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.instruments import Tranche
        >>> tranche = (
        ...     Tranche
        ...     .builder()
        ...     .id("A")
        ...     .attach_pct(0.0)
        ...     .detach_pct(100.0)
        ...     .seniority("senior")
        ...     .original_balance(Money(100.0, Currency("USD")))
        ...     .coupon_fixed(0.05)
        ...     .maturity(datetime.date(2029, 1, 1))
        ...     .build()
        ... )
        >>> restored = Tranche.from_json(tranche.to_json())
        >>> restored.to_json() == tranche.to_json()
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON wire form (also used by ``pickle``).

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

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` via the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(from_json, (payload,))``.

        Raises
        ------
        ValueError
            If the value cannot be reconstructed from its JSON form.
        """
        ...

    @property
    def id(self) -> str:
        """
        Tranche identifier.

        Returns
        -------
        str
            Unique identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def attach_pct(self) -> float | None:
        """
        Attachment point in percent (0-100 scale).

        ``None`` for a note built without points until a ``TrancheStructure``
        derives it from the balance shares.

        Returns
        -------
        float | None
            e.g. ``10.0``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def detach_pct(self) -> float | None:
        """
        Detachment point in percent (0-100 scale).

        ``None`` for a note built without points until a ``TrancheStructure``
        derives it from the balance shares.

        Returns
        -------
        float | None
            e.g. ``100.0``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def seniority(self) -> str:
        """
        Seniority (serde string, e.g. ``"Senior"``).

        Returns
        -------
        str
            Wire spelling of the seniority.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def rating(self) -> str | None:
        """
        Credit rating (serde string) or ``None``.

        Returns
        -------
        str | None
            Rating, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def original_balance(self) -> Money:
        """
        Original (issuance) balance.

        Returns
        -------
        Money
            Currency-tagged amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def current_balance(self) -> Money:
        """
        Current outstanding balance.

        Returns
        -------
        Money
            Currency-tagged amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def coupon(self) -> dict[str, Any]:
        """
        Coupon definition (``RateSpec`` serde shape).

        Returns
        -------
        dict[str, Any]
            Tagged coupon dict.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def frequency(self) -> Tenor:
        """
        Coupon payment frequency of the tranche premium leg.

        Returns
        -------
        Tenor
            Tenor giving the period between payments, such as
            ``Tenor.quarterly()`` or ``Tenor.semi_annual()``. Payment
            dates are rolled from this frequency using the deal's
            business-day convention.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def day_count(self) -> DayCount:
        """
        Accrual day count.

        Returns
        -------
        DayCount
            e.g. ``DayCount.ACT_360``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def deferred_interest(self) -> Money:
        """
        Accumulated deferred (PIK) interest.

        Returns
        -------
        Money
            Currency-tagged amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def pik_enabled(self) -> bool:
        """
        Whether interest may be deferred (PIK).

        Returns
        -------
        bool
            ``True`` when PIK is enabled.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def non_deferrable(self) -> bool | None:
        """
        Explicit non-deferrable flag on the coupon.

        Returns
        -------
        bool | None
            ``None`` for the seniority convention (senior notes are
            non-deferrable, every other class defers).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def is_non_deferrable(self) -> bool:
        """
        Whether the coupon is a non-deferrable claim the template pays from
        principal proceeds when interest falls short.

        Returns
        -------
        bool
            The explicit flag when set, else ``True`` for senior notes only.

        Notes
        -----
        This accessor does not raise; it returns the resolved convention.
        """
        ...

    @property
    def maturity(self) -> datetime.date:
        """
        Legal final maturity date.

        Returns
        -------
        datetime.date
            The maturity date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def payment_priority(self) -> int:
        """
        Payment priority rank (1 = most senior).

        Derived state rather than a wire field: ``TrancheStructure`` ranks
        its tranches ``1..=n`` by seniority, input order breaking ties, and
        a standalone ``Tranche`` reports ``0`` (unassigned).

        Returns
        -------
        int
            Priority rank, or ``0`` outside a ``TrancheStructure``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def attributes(self) -> Attributes:
        """
        User attributes (tags and metadata).

        Returns
        -------
        Attributes
            Attribute bag.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def oc_trigger(self) -> dict[str, Any] | None:
        """
        Per-tranche overcollateralization trigger as its ``CoverageTrigger`` serde ``dict``.

        Returns
        -------
        dict[str, Any] | None
            ``None`` without a trigger.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def ic_trigger(self) -> dict[str, Any] | None:
        """
        Per-tranche interest-coverage trigger as its ``CoverageTrigger`` serde ``dict``.

        Returns
        -------
        dict[str, Any] | None
            ``None`` without a trigger.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

class TrancheBuilder:
    """
    Fluent builder returned by :meth:`Tranche.builder`.

    ``attach_pct`` and ``detach_pct`` are tracked separately from
    the wrapped Rust builder (which only exposes a combined
    ``attach_detach(a, d)`` setter) and applied together on
    :meth:`build`, so either call order works. Both may be omitted: a
    ``TrancheStructure`` then derives them from the balance shares in
    payment-priority order (the first-loss class attaches at 0).

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import Tranche
    >>> isinstance(Tranche.builder(), Tranche.builder().__class__)
    True
    """

    def id(self, value: str) -> TrancheBuilder:
        """
        Set the tranche identifier.

        Parameters
        ----------
        value : str
            Unique identifier for the tranche.

        Returns
        -------
        TrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If this builder was already consumed by a prior call to
            :meth:`TrancheBuilder.build`.
        """
        ...

    def attach_pct(self, value: float) -> TrancheBuilder:
        """
        Set the attachment point.

        Parameters
        ----------
        value : float
            Attachment point quoted in percent on a 0-100 scale (e.g. ``0.0``
            for equity, ``10.0`` for a tranche attaching at 10%).

        Returns
        -------
        TrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If this builder was already consumed by a prior call to
            :meth:`TrancheBuilder.build`.
        """
        ...

    def detach_pct(self, value: float) -> TrancheBuilder:
        """
        Set the detachment point.

        Parameters
        ----------
        value : float
            Detachment point quoted in percent on a 0-100 scale (e.g.
            ``100.0`` for the most senior tranche).

        Returns
        -------
        TrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If this builder was already consumed by a prior call to
            :meth:`TrancheBuilder.build`.
        """
        ...

    def seniority(self, value: Literal["senior", "mezzanine", "subordinated", "equity"]) -> TrancheBuilder:
        """
        Set the tranche seniority.

        Parameters
        ----------
        value : {"senior", "mezzanine", "subordinated", "equity"}
            Structural seniority of the tranche.

        Returns
        -------
        TrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` is not a recognized seniority.
        """
        ...

    def original_balance(self, value: Money) -> TrancheBuilder:
        """
        Set the original tranche balance.

        Maps to the Rust ``TrancheBuilder::balance`` setter; named
        ``original_balance`` here to match the ``Tranche.original_balance``
        field it populates.

        Parameters
        ----------
        value : Money
            Original tranche balance. Must be positive.

        Returns
        -------
        TrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If this builder was already consumed by a prior call to
            :meth:`TrancheBuilder.build`.
        """
        ...

    def coupon_fixed(self, rate: float | Rate) -> TrancheBuilder:
        """
        Set a fixed-rate coupon.

        Parameters
        ----------
        rate : float | Rate
            Fixed interest rate as an annual decimal (e.g. ``0.05`` = 5%).

        Returns
        -------
        TrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If this builder was already consumed by a prior call to
            :meth:`TrancheBuilder.build`.
        """
        ...

    def coupon_floating(self, value: dict[str, Any] | str) -> TrancheBuilder:
        """
        Set a floating-rate coupon from a JSON ``RateSpec::Floating`` payload.

        The floating-rate spec (``FloatingRateSpec``: index, spread, gearing,
        floors/caps, reset conventions) stays JSON per the nested-spec rule —
        the typed cashflows plan owns that shape.

        Parameters
        ----------
        value : dict[str, Any] | str
            JSON-encoded, externally-tagged ``RateSpec`` value, e.g.
            ``{"floating": {...FloatingRateSpec fields...}}``.

        Returns
        -------
        TrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` is not valid JSON for the ``RateSpec`` shape.
        """
        ...

    def maturity(self, value: datetime.date | datetime.datetime | pd.Timestamp | str) -> TrancheBuilder:
        """
        Set the legal final maturity date.

        Parameters
        ----------
        value : datetime.date | datetime.datetime | pandas.Timestamp | str
            Legal final maturity date.

        Returns
        -------
        TrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If this builder was already consumed by a prior call to
            :meth:`TrancheBuilder.build`.
        """
        ...

    def frequency(self, value: Tenor | str) -> TrancheBuilder:
        """
        Set the payment frequency.

        Parameters
        ----------
        value : Tenor | str
            Payment frequency. Defaults to quarterly when never set.

        Returns
        -------
        TrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If this builder was already consumed by a prior call to
            :meth:`TrancheBuilder.build`, or a string ``value`` is not a recognized tenor string.
        """
        ...

    def day_count(self, value: DayCount | str) -> TrancheBuilder:
        """
        Set the day count convention for interest accrual.

        Parameters
        ----------
        value : DayCount | str
            Day count convention. Defaults to Act/360 when never set.

        Returns
        -------
        TrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If this builder was already consumed by a prior call to
            :meth:`TrancheBuilder.build`, or a string ``value`` is not a recognized day-count name.
        """
        ...

    def current_balance(self, value: Money) -> TrancheBuilder:
        """
        Set the current (factored) balance.

        Parameters
        ----------
        value : Money
            Outstanding principal today, at most the original balance. Defaults
            to the original balance when never set.
        Returns
        -------
        TrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` is invalid or this builder was already consumed by a
            prior call to :meth:`TrancheBuilder.build`.
        """
        ...

    def deferred_interest(self, value: Money) -> TrancheBuilder:
        """
        Set interest already deferred (unpaid, still owed) at closing.

        Parameters
        ----------
        value : Money
            Deferred interest carried into the projection; zero when never set.
        Returns
        -------
        TrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` is invalid or this builder was already consumed by a
            prior call to :meth:`TrancheBuilder.build`.
        """
        ...

    def pik_enabled(self, value: bool) -> TrancheBuilder:
        """
        Enable payment-in-kind accretion of interest shortfalls.

        Parameters
        ----------
        value : bool
            ``True`` capitalizes unpaid interest into the balance; ``False`` (the
            default) defers it as a claim.
        Returns
        -------
        TrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` is invalid or this builder was already consumed by a
            prior call to :meth:`TrancheBuilder.build`.
        """
        ...

    def non_deferrable(self, value: bool) -> TrancheBuilder:
        """
        Mark the coupon non-deferrable or deferrable, overriding the seniority
        convention.

        Parameters
        ----------
        value : bool
            ``True`` for a coupon the template pays from principal proceeds when
            interest falls short (the senior default); ``False`` for one that
            defers (the default for every other class).

        Returns
        -------
        TrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If this builder was already consumed by a prior call to
            :meth:`TrancheBuilder.build`.
        """
        ...

    def rating(self, value: str) -> TrancheBuilder:
        """
        Set the credit rating.

        Parameters
        ----------
        value : str
            Rating string (``"AAA"``, ``"BBB"``, ``"NR"`` ...).
        Returns
        -------
        TrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` is invalid or this builder was already consumed by a
            prior call to :meth:`TrancheBuilder.build`.
        """
        ...

    def oc_trigger(self, value: dict[str, Any] | str) -> TrancheBuilder:
        """
        Attach a per-tranche overcollateralization trigger.

        Parameters
        ----------
        value : dict[str, Any] | str
            ``CoverageTrigger`` serde object: ``trigger_level`` (ratio), optional
            ``cure_level``, ``consequence`` and breach memory fields.
        Returns
        -------
        TrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` is invalid or this builder was already consumed by a
            prior call to :meth:`TrancheBuilder.build`.
        """
        ...

    def ic_trigger(self, value: dict[str, Any] | str) -> TrancheBuilder:
        """
        Attach a per-tranche interest-coverage trigger.

        Parameters
        ----------
        value : dict[str, Any] | str
            ``CoverageTrigger`` serde object (see :meth:`oc_trigger`).
        Returns
        -------
        TrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        ValueError
            If ``value`` is invalid or this builder was already consumed by a
            prior call to :meth:`TrancheBuilder.build`.
        """
        ...

    def attributes(self, value: Attributes | dict[str, Any] | None) -> TrancheBuilder:
        """
        Set free-form attributes (tags and metadata).

        Parameters
        ----------
        value : Attributes | dict | None
            Attribute bag: an ``Attributes`` or its serde ``dict`` form
            (``{"tags": [...], "meta": {...}}``, as ``to_dict()`` returns it);
            ``None`` clears it.
        Returns
        -------
        TrancheBuilder
            ``self``, for chaining.

        Raises
        ------
        TypeError
            If ``value`` is neither ``Attributes``, a ``dict`` nor ``None``.
        ValueError
            If ``value`` is invalid or this builder was already consumed by a
            prior call to :meth:`TrancheBuilder.build`.
        ValueError
            If the ``dict`` has a key other than ``tags`` / ``meta`` or a non-string tag or value.
        """
        ...

    def __repr__(self) -> str:
        """
        Return ``repr(self)`` showing the tracked attachment/detachment points.

        Returns
        -------
        str
            ``TrancheBuilder(attach_pct=..., detach_pct=..., consumed=...)``.
        """
        ...

    def build(self) -> Tranche:
        """
        Build the validated tranche.

        Returns
        -------
        Tranche
            The validated tranche.

        Raises
        ------
        ValueError
            If a required field is missing, or attachment/detachment points
            are invalid (negative, out of the ``[0, 100]`` range, or
            detachment not strictly above attachment).
        """
        ...
