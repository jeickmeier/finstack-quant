"""Stub declarations mirroring ``finstack-quant-py/src/bindings/valuations/typed_structured_credit/pool_asset.rs``."""

from __future__ import annotations

import datetime
from typing import Any
from finstack_quant.core.dates import DayCount
from finstack_quant.core.money import Money

class PoolAsset:
    """
    One loan-level collateral row of an :class:`AssetPool`: the contractual
    terms (balance, coupon or index + spread, maturity, day count,
    amortization type), the credit state (rating, default, recovery, purchase
    price) and the behavioural overrides (SMM / MDR / recovery overrides,
    delinquency buckets, commercial-mortgage balloon, prepayment penalty,
    special servicing, NOI).

    Percent fields (``market_price_pct``) are percent values; ``rate``,
    ``smm_override``, ``mdr_override`` and ``recovery_rate`` are decimals;
    ``spread_bp`` is in basis points.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.dates import DayCount
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.valuations.instruments import PoolAsset
    >>> loan = PoolAsset(
    ...     "LOAN-1",
    ...     {"type": "first_lien_loan"},
    ...     Money(10_000_000.0, Currency("USD")),
    ...     0.08,
    ...     datetime.date(2031, 1, 15),
    ...     day_count=DayCount.ACT_360,
    ...     rating="B",
    ...     balloon={"extension_prob": 0.3, "extension_months": 24},
    ... )
    >>> loan.rating, loan.balloon["extension_months"]
    ('B', 24)
    """

    def __init__(
        self,
        id: str,
        asset_type: dict[str, Any] | str,
        balance: Money,
        rate: float,
        maturity: datetime.date,
        *,
        day_count: DayCount,
        spread_bp: float | None = None,
        forward_curve_id: str | None = None,
        index_floor_bp: float | None = None,
        rating: str | None = None,
        industry: str | None = None,
        obligor_id: str | None = None,
        defaulted: bool = False,
        recovery_amount: Money | None = None,
        default_date: datetime.date | None = None,
        purchase_price: Money | None = None,
        acquisition_date: datetime.date | None = None,
        origination_date: datetime.date | None = None,
        smm_override: float | None = None,
        mdr_override: float | None = None,
        recovery_rate: float | None = None,
        commitment: Money | None = None,
        contractual_payment: Money | None = None,
        amortization_term_months: int | None = None,
        io_months: int | None = None,
        market_price_pct: float | None = None,
        delinquency_buckets: list[Money] | None = None,
        balloon: dict[str, Any] | str | None = None,
        prepayment_penalty: dict[str, Any] | str | None = None,
        special_servicing: dict[str, Any] | str | None = None,
        noi: Money | None = None,
        liquidation: dict[str, Any] | str | None = None,
    ) -> None:
        """
        Construct a collateral row from its contractual terms.

        Parameters
        ----------
        id : str
            Stable asset identifier, unique within the pool.
        asset_type : dict[str, Any] | str
            ``AssetType`` serde object such as ``{"type": "first_lien_loan"}`` or
            ``{"type": "commercial_mortgage", "ltv": 0.6}``; the type decides
            whether the row amortizes (level pay) or pays as a bullet. The
            obligor industry is the separate ``industry`` keyword.
        balance : Money
            Current principal balance in the pool currency.
        rate : float
            Annual coupon as a decimal (``0.08`` = 8%). For floating rows the
            engine adds ``spread_bp`` to the ``forward_curve_id`` projection.
        maturity : datetime.date
            Contractual maturity (balloon date for commercial mortgages).
        day_count : DayCount
            Accrual convention of the row (keyword-only, required as in Rust).
        spread_bp : float, optional
            Floating spread over ``forward_curve_id`` in basis points.
        forward_curve_id : str, optional
            Forward-curve identifier of the floating index; ``None`` for a
            fixed-rate row.
        index_floor_bp : float, optional
            Floor on the floating index in basis points (``100`` = 1%),
            applied before ``spread_bp`` is added; ignored on fixed-rate rows.
        rating : str, optional
            Credit rating (``"BB"``, ``"CCC"``, ``"NR"`` ...), used by the
            coverage-test haircuts and the CCC bucket.
        industry : str, optional
            Industry label for concentration reporting.
        obligor_id : str, optional
            Obligor identifier for exposure aggregation.
        defaulted : bool, optional
            ``True`` marks the row defaulted at closing; ``recovery_amount`` and
            ``default_date`` describe its state.
        recovery_amount : Money, optional
            Recovery still expected on a defaulted row.
        default_date : datetime.date, optional
            Date of default for a defaulted row.
        purchase_price : Money, optional
            Price paid for the row (discount-obligation test).
        acquisition_date : datetime.date, optional
            Date the row entered the pool (anchors non-performing-loan
            timelines).
        origination_date : datetime.date, optional
            Date the loan was originated; anchors the seasoning-dependent
            prepayment/default curves (PSA, SDA, ABS, vector) and the
            amortization schedule. Falls back to ``acquisition_date``, then
            to the deal closing date.
        smm_override : float, optional
            Row-level single-month mortality (decimal) overriding the deal
            prepayment model.
        mdr_override : float, optional
            Row-level monthly default rate (decimal) overriding the deal default
            model.
        recovery_rate : float, optional
            Row-level recovery rate (decimal) overriding the deal recovery model.
        commitment : Money, optional
            Total commitment for revolving rows (drawn balance is ``balance``).
        contractual_payment : Money, optional
            Monthly level payment of an amortizing row; derived from the terms
            when omitted.
        amortization_term_months : int, optional
            Schedule length in months from origination (``origination_date``,
            else ``acquisition_date``, else closing) for level-pay rows; the unamortized balance pays as
            the balloon at maturity. ``None`` amortizes fully by maturity.
        io_months : int, optional
            Interest-only window in months from origination: no scheduled
            principal while the loan is younger than this.
        market_price_pct : float, optional
            Market price in percent of par for market-value coverage rules.
        delinquency_buckets : list[Money], optional
            Seeded delinquent balances per bucket (30/60/90 ...); requires a
            ``credit_model.delinquency`` model on the deal.
        balloon : dict[str, Any] | str, optional
            ``BalloonSpec`` (``extension_prob`` decimal, ``extension_months``,
            optional ``extension_rate`` decimal, and ``loss_prob`` decimal,
            ``severity_pct`` percent, ``workout_months`` for the share that
            defaults at maturity) for balloon extension and workout.
        prepayment_penalty : dict[str, Any] | str, optional
            ``PrepaymentPenalty``: ``{"kind": "lockout", "through":
            "2025-12-31"}`` (no voluntary prepayment inside the window),
            ``{"kind": "fixed", "pct": 3.0, "through": "2026-01-01"}``,
            ``{"kind": "step_down", "schedule": [{"through": "2025-12-31",
            "pct": 5.0}, ...]}`` or ``{"kind": "yield_maintenance",
            "reinvestment_rate": 0.05, "discount_curve_id": "USD-OIS",
            "floor_pct": 1.0, "through": None}`` (the curve discounts the lost
            coupons and supplies the reinvestment rate when it is omitted).
        special_servicing : dict[str, Any] | str, optional
            ``SpecialServicingSpec`` (``appraisal_reduction_pct`` percent).
        noi : Money, optional
            Annual net operating income of the property for the CMBS DSCR.
        liquidation : dict[str, Any] | str, optional
            ``LiquidationSpec`` for a non-performing loan
            (``months_to_resolution``, ``proceeds_pct`` and ``carry_cost_pct``
            percents, ``reperformance_prob`` decimal, optional
            ``modified_rate`` decimal); leave ``defaulted`` false.

        Raises
        ------
        ValueError
            If a sub-spec does not match its serde shape, a date is invalid or
            ``rating`` is not a known rating.
        """
        ...

    @staticmethod
    def fixed_rate_bond(
        id: str, balance: Money, rate: float, maturity: datetime.date, day_count: DayCount
    ) -> PoolAsset:
        """
        Fixed-rate bullet bond row (mirrors Rust ``PoolAsset::fixed_rate_bond``).

        Parameters
        ----------
        id : str
            Stable asset identifier.
        balance : Money
            Current principal balance.
        rate : float
            Annual fixed coupon as a decimal.
        maturity : datetime.date
            Bullet maturity.
        day_count : DayCount
            Accrual convention.

        Returns
        -------
        PoolAsset
            A performing, unrated bond row.

        Raises
        ------
        ValueError
            If ``maturity`` is not a valid date.

        Examples
        --------
        >>> import datetime
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.dates import DayCount
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.instruments import PoolAsset
        >>> bond = PoolAsset.fixed_rate_bond(
        ...     "B1", Money(1_000_000.0, Currency("USD")), 0.06, datetime.date(2030, 1, 1), DayCount.THIRTY_360
        ... )
        >>> bond.asset_type["type"], bond.rate
        ('high_yield_bond', 0.06)
        """
        ...

    @staticmethod
    def floating_rate_loan(
        id: str, balance: Money, forward_curve_id: str, spread_bp: float, maturity: datetime.date, day_count: DayCount
    ) -> PoolAsset:
        """
        Floating-rate first-lien loan row (mirrors Rust ``PoolAsset::floating_rate_loan``).

        Parameters
        ----------
        id : str
            Stable asset identifier.
        balance : Money
            Current principal balance.
        forward_curve_id : str
            Forward-curve identifier of the floating index (e.g. ``"USD-SOFR-3M"``).
        spread_bp : float
            Spread over the index in basis points.
        maturity : datetime.date
            Loan maturity.
        day_count : DayCount
            Accrual convention.

        Returns
        -------
        PoolAsset
            A performing first-lien loan row.

        Raises
        ------
        ValueError
            If ``maturity`` is not a valid date.

        Examples
        --------
        >>> import datetime
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.dates import DayCount
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.instruments import PoolAsset
        >>> loan = PoolAsset.floating_rate_loan(
        ...     "L1",
        ...     Money(1_000_000.0, Currency("USD")),
        ...     "USD-SOFR-3M",
        ...     350.0,
        ...     datetime.date(2030, 1, 1),
        ...     DayCount.ACT_360,
        ... )
        >>> loan.forward_curve_id, loan.spread_bp
        ('USD-SOFR-3M', 350.0)
        """
        ...

    @staticmethod
    def from_json(json: str) -> PoolAsset:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``PoolAsset`` (the shape ``to_json`` writes).

        Returns
        -------
        PoolAsset
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import PoolAsset
        >>> try:
        ...     PoolAsset.from_json("{}")
        ... except ValueError:
        ...     print("rejected")
        rejected
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            JSON-encoded ``PoolAsset``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return every field as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Serde form of the Rust value.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(PoolAsset.from_json, (json,))``.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __repr__(self) -> str:
        """
        Return ``repr(self)``.

        Returns
        -------
        str
            A one-line summary of the value.
        """
        ...

    @property
    def id(self) -> str:
        """
        Asset identifier.

        Returns
        -------
        str
            The stable identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def asset_type(self) -> dict[str, Any]:
        """
        ``AssetType`` serde object (``{"type": ..., ...}``).

        Returns
        -------
        dict[str, Any]
            The asset type and its variant fields.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def balance(self) -> Money:
        """
        Current principal balance.

        Returns
        -------
        Money
            The balance in the pool currency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def rate(self) -> float:
        """
        Annual coupon as a decimal.

        Returns
        -------
        float
            The coupon (spread-only for floating rows).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def spread_bp(self) -> float | None:
        """
        Floating spread in basis points.

        Returns
        -------
        float | None
            ``None`` for a fixed-rate row.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def forward_curve_id(self) -> str | None:
        """
        Floating index curve identifier.

        Returns
        -------
        str | None
            ``None`` for a fixed-rate row.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def index_floor_bp(self) -> float | None:
        """
        Floor on the floating index in basis points, applied before the spread.

        Returns
        -------
        float | None
            ``None`` when the row is unfloored or fixed-rate.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def maturity(self) -> datetime.date:
        """
        Contractual maturity.

        Returns
        -------
        datetime.date
            The maturity date.

        Raises
        ------
        ValueError
            If the date cannot be converted.
        """
        ...

    @property
    def rating(self) -> str | None:
        """
        Credit rating string.

        Returns
        -------
        str | None
            ``None`` when unrated.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def industry(self) -> str | None:
        """
        Industry label used for concentration reporting.

        Returns
        -------
        str | None
            ``None`` when unset.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def obligor_id(self) -> str | None:
        """
        Obligor identifier.

        Returns
        -------
        str | None
            ``None`` when unset.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def defaulted(self) -> bool:
        """
        Whether the row is defaulted.

        Returns
        -------
        bool
            ``True`` for a defaulted row.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def recovery_amount(self) -> Money | None:
        """
        Expected recovery on a defaulted row.

        Returns
        -------
        Money | None
            ``None`` when unset.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def default_date(self) -> datetime.date | None:
        """
        Date the row defaulted, for rows marked defaulted.

        Returns
        -------
        datetime.date | None
            ``None`` when the row is performing.

        Raises
        ------
        ValueError
            If the date cannot be converted.
        """
        ...

    @property
    def purchase_price(self) -> Money | None:
        """
        Price paid for the row, used by the discount-obligation test.

        Returns
        -------
        Money | None
            ``None`` when unset.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def acquisition_date(self) -> datetime.date | None:
        """
        Acquisition date.

        Returns
        -------
        datetime.date | None
            ``None`` when unset.

        Raises
        ------
        ValueError
            If the date cannot be converted.
        """
        ...

    @property
    def origination_date(self) -> datetime.date | None:
        """
        Origination date anchoring the row's seasoning.

        Returns
        -------
        datetime.date | None
            ``None`` when unset (the row then ages from ``acquisition_date``,
            else the closing date).

        Raises
        ------
        ValueError
            If the date cannot be converted.
        """
        ...

    @property
    def day_count(self) -> DayCount:
        """
        Accrual day count.

        Returns
        -------
        DayCount
            The convention.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def smm_override(self) -> float | None:
        """
        Row-level SMM override (decimal).

        Returns
        -------
        float | None
            ``None`` when the deal model applies.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def mdr_override(self) -> float | None:
        """
        Row-level MDR override (decimal).

        Returns
        -------
        float | None
            ``None`` when the deal model applies.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def recovery_rate(self) -> float | None:
        """
        Row-level recovery rate (decimal).

        Returns
        -------
        float | None
            ``None`` when the deal model applies.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def commitment(self) -> Money | None:
        """
        Total commitment of a revolving row.

        Returns
        -------
        Money | None
            ``None`` for term rows.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def contractual_payment(self) -> Money | None:
        """
        Monthly level payment.

        Returns
        -------
        Money | None
            ``None`` when derived from the terms.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def amortization_term_months(self) -> int | None:
        """
        Amortization schedule length in months from origination.

        Returns
        -------
        int | None
            ``None`` when unset.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def io_months(self) -> int | None:
        """
        Interest-only window in months from origination.

        Returns
        -------
        int | None
            ``None`` when unset.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def market_price_pct(self) -> float | None:
        """
        Market price in percent of par.

        Returns
        -------
        float | None
            ``None`` when unset.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def delinquency_buckets(self) -> list[Money] | None:
        """
        Seeded delinquent balances per bucket.

        Returns
        -------
        list[Money] | None
            ``None`` when the row is current.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def balloon(self) -> dict[str, Any] | None:
        """
        ``BalloonSpec`` serde dict.

        Returns
        -------
        dict[str, Any] | None
            ``None`` without balloon terms.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def prepayment_penalty(self) -> dict[str, Any] | None:
        """
        ``PrepaymentPenalty`` serde dict.

        Returns
        -------
        dict[str, Any] | None
            ``None`` without a penalty.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def special_servicing(self) -> dict[str, Any] | None:
        """
        ``SpecialServicingSpec`` serde dict.

        Returns
        -------
        dict[str, Any] | None
            ``None`` when not specially serviced.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def noi(self) -> Money | None:
        """
        Annual net operating income.

        Returns
        -------
        Money | None
            ``None`` when unset.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def liquidation(self) -> dict[str, Any] | None:
        """
        ``LiquidationSpec`` serde dict of a non-performing loan.

        Returns
        -------
        dict[str, Any] | None
            ``None`` for a performing loan.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...
