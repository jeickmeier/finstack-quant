"""Stub declarations mirroring ``finstack-quant-py/src/bindings/valuations/typed_structured_credit/asset_pool.rs``."""

from __future__ import annotations

import datetime
from typing import Any, Literal
from finstack_quant.core.currency import Currency
from finstack_quant.core.money import Money
from finstack_quant.valuations.instruments._instruments import Bond, TermLoan
from finstack_quant.valuations.instruments._typed_revolving_credit.revolving_credit import RevolvingCredit
from finstack_quant.valuations.instruments._typed_structured_credit.pool_asset import PoolAsset
from finstack_quant.valuations.instruments._typed_structured_credit.rep_line import RepLine

class AssetPool:
    """
    Structured-credit collateral pool.

    Examples
    --------
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.valuations.instruments import AssetPool
    >>> pool = AssetPool("POOL-1", "abs", Currency("USD"))
    >>> "POOL-1" in repr(pool)
    True
    """

    def __init__(
        self,
        id: str,
        deal_type: Literal["clo", "cbo", "abs", "rmbs", "cmbs", "auto", "card"],
        currency: Currency | str,
    ) -> None:
        """
        Structured-credit collateral pool.

        Parameters
        ----------
        id : str
            Pool identifier.
        deal_type : {"clo", "cbo", "abs", "rmbs", "cmbs", "auto", "card"}
            Deal classification for pool-level assumptions.
        currency : Currency | str
            Currency (``Currency`` or ISO-4217 code) every asset and
            pool-level account is denominated in.

        Returns
        -------
        AssetPool
            A new, empty asset pool. Use either :meth:`with_rep_lines` or
            :meth:`assets` to attach collateral.

        Raises
        ------
        ValueError
            If ``deal_type`` is not a recognized deal type.

        Examples
        --------
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.valuations.instruments import AssetPool
        >>> pool = AssetPool("POOL-1", "abs", Currency("USD"))
        >>> "POOL-1" in repr(pool)
        True
        """
        ...

    def with_rep_lines(self, rep_lines: list[RepLine]) -> AssetPool:
        """
        Attach representative pool lines, returning a new pool.

        Parameters
        ----------
        rep_lines : list[RepLine]
            Aggregated representative lines the pricing engine will use
            instead of individual assets.

        Returns
        -------
        AssetPool
            A new pool with ``rep_lines`` set (the original is unchanged).

        Raises
        ------
        TypeError
            If an element of ``rep_lines`` is not a ``RepLine``.

        Examples
        --------
        >>> import datetime
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.dates import DayCount
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.instruments import AssetPool, RepLine
        >>> pool = AssetPool("POOL-1", "abs", Currency("USD")).with_rep_lines([
        ...     RepLine(
        ...         "LINE-1",
        ...         Money(80_000_000.0, Currency("USD")),
        ...         0.07,
        ...         datetime.date(2031, 1, 15),
        ...         12,
        ...         DayCount.ACT_360,
        ...         asset_type={"type": "first_lien_loan"},
        ...     )
        ... ])
        >>> "POOL-1" in repr(pool)
        True
        """
        ...

    def with_assets(self, value: list[PoolAsset | dict[str, Any]] | str) -> AssetPool:
        """
        Attach loan-level assets, returning a new pool.

        Parameters
        ----------
        value : list[PoolAsset | dict[str, Any]] | str
            Typed :class:`PoolAsset` rows, their serde dicts, or a JSON array
            string (mixing typed rows and dicts is allowed).

        Returns
        -------
        AssetPool
            A new pool with ``assets`` set (the original is unchanged).

        Raises
        ------
        ValueError
            If ``value`` does not match the ``PoolAsset`` list shape.
        """
        ...
    def with_instruments(
        self,
        bonds: list[Bond] | None = None,
        term_loans: list[TermLoan] | None = None,
        revolvers: list[RevolvingCredit] | None = None,
        call_exercise: str | dict[str, Any] | None = None,
        put_exercise: str | dict[str, Any] | None = None,
        overrides: list[dict[str, Any]] | None = None,
    ) -> AssetPool:
        """
        Attach real instruments as the collateral, returning a new pool.

        The pool then holds ``Bond``, ``TermLoan`` and ``RevolvingCredit``
        instruments instead of asset rows or representative lines; each
        instrument's own cashflow schedule drives the deal, defaults come from
        the instrument's credit curve when present, and collateral draws are
        funded from the reserve account (see :meth:`with_reserve`).

        Parameters
        ----------
        bonds : list[Bond], optional
            Bonds in any form (fixed, floating, step-up, amortizing, callable).
        term_loans : list[TermLoan], optional
            Term loans, including delayed-draw facilities.
        revolvers : list[RevolvingCredit], optional
            Revolving facilities; stochastic ones simulate draws on the paths.
        call_exercise : str | dict[str, Any], optional
            Default issuer-call policy: ``"contractual"`` (never, the default),
            ``"first_call"``, ``"worst"`` (yield-to-worst, needs a quoted clean
            price) or ``{"policy": "refinancing_incentive", "threshold_bp": 50.0}``.
        put_exercise : str | dict[str, Any], optional
            Default holder-put policy: ``"never"`` (default), ``"first_put"``, or the
            dict ``{"policy": "reinvestment_incentive", "threshold_bp": 50.0}`` (put
            when the reinvestment rate exceeds the coupon by more than the threshold).
        overrides : list[dict[str, Any]], optional
            Per-instrument overrides ``{"id": ..., "call": {...}, "put": {...}}``.

        Returns
        -------
        AssetPool
            A new pool with ``instruments`` set (the original is unchanged).

        Raises
        ------
        ValueError
            If a policy or override does not match its serde shape.
        TypeError
            If a list element is not the expected typed instrument.

        Examples
        --------
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.valuations.instruments import AssetPool, Bond, RevolvingCredit
        >>> pool = AssetPool("POOL-1", "clo", Currency("USD")).with_instruments(
        ...     bonds=[Bond.example()],
        ...     revolvers=[RevolvingCredit.example()],
        ...     call_exercise="first_call",
        ... )
        >>> sorted(pool.instruments)
        ['bonds', 'call_exercise', 'overrides', 'put_exercise', 'revolvers', 'term_loans']
        """
        ...
    def with_reserve(
        self,
        reserve_account: Money | float,
        reserve_account_rate: float = 0.0,
        reserve_target: Money | float | None = None,
        reserve_interest_destination: str | dict[str, Any] | None = None,
        currency: str | None = None,
    ) -> AssetPool:
        """
        Configure the reserve account, returning a new pool.

        The reserve funds collateral draws (revolver utilization increases,
        delayed draws and loan-equivalent draws at default), is replenished by
        revolver repayments up to ``reserve_target``, and earns
        ``reserve_account_rate`` routed per ``reserve_interest_destination``.

        Parameters
        ----------
        reserve_account : Money | float
            Opening reserve balance; a bare number needs ``currency``.
        reserve_account_rate : float, default 0.0
            Annual interest rate earned by the reserve, as a decimal (simple
            ACT/360 on the opening balance each period).
        reserve_target : Money | float, optional
            Balance revolver repayments replenish toward; ``None`` disables
            replenishment.
        reserve_interest_destination : str | dict[str, Any], optional
            ``"waterfall"`` (default, interest proceeds), ``"retain"``
            (capitalized into the reserve) or
            ``{"kind": "tranche", "tranche_id": "EQ"}`` (paid directly to that
            tranche).
        currency : str, optional
            ISO-4217 code applied when a bare number is passed.

        Returns
        -------
        AssetPool
            A new pool with the reserve configured (the original is unchanged).

        Raises
        ------
        ValueError
            If an amount is not finite, a bare number has no currency, or the
            destination does not match its serde shape.
        TypeError
            If an amount is neither ``Money`` nor a number.

        Examples
        --------
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.instruments import AssetPool
        >>> pool = AssetPool("POOL-1", "clo", Currency("USD")).with_reserve(
        ...     Money(5_000_000.0, Currency("USD")),
        ...     reserve_account_rate=0.03,
        ...     reserve_interest_destination={"kind": "tranche", "tranche_id": "EQ"},
        ... )
        >>> (pool.reserve_account_rate, pool.reserve_interest_destination["kind"])
        (0.03, 'tranche')
        """
        ...

    def with_reinvestment_period(self, value: dict[str, Any] | str) -> AssetPool:
        """
        Configure the deal-level reinvestment period, returning a new pool.

        Principal proceeds collected while the period is active are recycled
        into collateral instead of repaying the notes; every note is held flat
        except those listed in ``amortizing_tranches``, which are paid down
        first. Instrument-collateral pools cannot reinvest.

        Parameters
        ----------
        value : dict[str, Any] | str
            ``ReinvestmentPeriod`` in its serde shape, or that JSON as a
            string: ISO ``end`` (inclusive), ``is_active``, ``criteria``
            (``max_price_pct`` percent of par, ``min_yield`` annual decimal current
            yield below which a surviving asset is skipped), optional
            ``amortizing_tranches`` (note ids paid down inside the window) and
            optional ``assumptions`` (``spread_bp``, ``price_pct``,
            ``maturity_months``, ``forward_curve_id``, ``all_in_floor_bp``) describing the
            replacement collateral; omitted assumptions clone the surviving
            pool pro rata.

        Returns
        -------
        AssetPool
            A new pool with the reinvestment period set (the original is
            unchanged).

        Raises
        ------
        ValueError
            If ``value`` does not match the ``ReinvestmentPeriod`` serde shape.
            Tranche ids, dates and assumption ranges are validated when the
            deal is built or priced.

        Examples
        --------
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.valuations.instruments import AssetPool
        >>> pool = AssetPool("POOL-1", "clo", Currency("USD")).with_reinvestment_period({
        ...     "end": "2028-01-01",
        ...     "is_active": True,
        ...     "criteria": {
        ...         "max_price_pct": 100.0,
        ...         "min_yield": 0.0,
        ...     },
        ...     "amortizing_tranches": ["A"],
        ... })
        >>> pool.reinvestment_period["amortizing_tranches"]
        ['A']
        """
        ...

    def with_accounts(
        self,
        *,
        cumulative_defaults: Money | None = None,
        cumulative_recoveries: Money | None = None,
        cumulative_prepayments: Money | None = None,
        cumulative_scheduled_amortization: Money | None = None,
        collection_account: Money | None = None,
        excess_spread_account: Money | None = None,
        original_balance: Money | None = None,
    ) -> AssetPool:
        """
        Set the pool's historical tallies and cash accounts, returning a new
        pool (seasoned-deal inputs).

        Parameters
        ----------
        cumulative_defaults : Money, optional
            Defaulted par to date; unchanged when omitted.
        cumulative_recoveries : Money, optional
            Recoveries received to date; unchanged when omitted.
        cumulative_prepayments : Money, optional
            Prepayments received to date; unchanged when omitted.
        cumulative_scheduled_amortization : Money, optional
            Scheduled principal received to date; unchanged when omitted.
        collection_account : Money, optional
            Undistributed collections held at closing; unchanged when omitted.
        excess_spread_account : Money, optional
            Trapped excess spread held at closing; unchanged when omitted.
        original_balance : Money, optional
            Original (cut-off) pool balance the cumulative-loss triggers,
            clean-up call factor and loss curves are stated against; when
            omitted the engine reconstructs it as the current balance plus
            the cumulative defaults, prepayments and scheduled amortization.
            Must be at least the current balance (``ValueError`` at pricing).

        Returns
        -------
        AssetPool
            A new pool with the supplied balances (the original is unchanged).

        Notes
        -----
        This method does not raise.

        Examples
        --------
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.instruments import AssetPool
        >>> pool = AssetPool("POOL-1", "clo", Currency("USD")).with_accounts(
        ...     cumulative_defaults=Money(8_000_000.0, Currency("USD"))
        ... )
        >>> pool.cumulative_defaults.amount
        8000000.0
        """
        ...

    @staticmethod
    def from_json(json: str) -> AssetPool:
        """
        Deserialize from the JSON produced by :meth:`to_json`.

        Parameters
        ----------
        json : str
            Strict JSON object with exactly the fields ``to_json`` writes.
            Defaulted assets specify economic ``default_date`` and outstanding,
            unreceived ``recovery_amount`` in asset currency. Their par is excluded
            from performing collateral. ``reinvestment_period`` owns the active
            flag, inclusive end date, maximum percent-of-par purchase price and
            minimum annual decimal current yield.

        Returns
        -------
        AssetPool
            The reconstructed value.

        Raises
        ------
        ValueError
            If the JSON is malformed or has the wrong shape.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import AssetPool
        >>> restored = AssetPool.from_json(AssetPool("POOL-1", "abs", "USD").to_json())
        >>> restored.to_json() == AssetPool("POOL-1", "abs", "USD").to_json()
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
        Pool identifier.

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
    def deal_type(self) -> str:
        """
        Deal classification (serde string).

        Returns
        -------
        str
            ``"abs"``, ``"clo"``, ...

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def currency(self) -> str:
        """
        ISO-4217 currency code every pool amount is denominated in.

        Returns
        -------
        str
            Three-letter code.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def asset_records(self) -> list[dict[str, Any]]:
        """
        Loan-level assets in their ``PoolAsset`` serde shape.

        Returns
        -------
        list[dict[str, Any]]
            One dict per asset (empty when rep lines are used).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def assets(self) -> list[PoolAsset]:
        """
        Loan-level assets as typed :class:`PoolAsset` rows.

        Returns
        -------
        list[PoolAsset]
            One row per asset (empty when rep lines or instruments are used).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def rep_lines(self) -> list[RepLine] | None:
        """
        Representative lines, or ``None`` when the pool is modelled loan-level.

        Returns
        -------
        list[RepLine] | None
            Rep lines, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def cumulative_defaults(self) -> Money:
        """
        Cumulative defaults to date.

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
    def cumulative_recoveries(self) -> Money:
        """
        Cumulative recoveries to date.

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
    def cumulative_prepayments(self) -> Money:
        """
        Cumulative prepayments to date.

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
    def cumulative_scheduled_amortization(self) -> Money:
        """
        Cumulative scheduled amortization to date.

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
    def original_balance(self) -> Money | None:
        """
        Original (cut-off) pool balance when supplied.

        Returns
        -------
        Money or None
            The explicit original balance, or ``None`` when the engine
            reconstructs it from the current balance and cumulative tallies.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def collection_account(self) -> Money:
        """
        Collection account balance.

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
    def reserve_account(self) -> Money:
        """
        Reserve account balance.

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
    def excess_spread_account(self) -> Money:
        """
        Excess-spread account balance.

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
    def reserve_account_rate(self) -> float:
        """
        Annual interest rate earned by the reserve account, as a decimal.

        Returns
        -------
        float
            The reserve rate (``0.0`` when the reserve earns nothing).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def reserve_target(self) -> Money | None:
        """
        Reserve balance revolver repayments replenish toward, or ``None``.

        Returns
        -------
        Money | None
            The target balance, or ``None`` when replenishment is disabled.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def reserve_interest_destination(self) -> dict[str, Any]:
        """
        Destination of the reserve interest as its serde ``dict``.

        Returns
        -------
        dict[str, Any]
            ``{"kind": "waterfall"}``, ``{"kind": "retain"}`` or
            ``{"kind": "tranche", "tranche_id": ...}``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def instruments(self) -> dict[str, Any] | None:
        """
        Instrument collateral as its serde ``dict``, or ``None``.

        Returns
        -------
        dict[str, Any] | None
            ``bonds``, ``term_loans``, ``revolvers``, ``call_exercise``,
            ``put_exercise`` and ``overrides``; ``None`` when the pool is
            modelled with asset rows or representative lines.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def reinvestment_period(self) -> dict[str, Any] | None:
        """
        Reinvestment period as its serde ``dict``, or ``None``.

        Returns
        -------
        dict[str, Any] | None
            ``end_date`` (ISO), ``is_active``, ``criteria``,
            ``amortizing_tranches`` and ``assumptions`` as set with
            :meth:`with_reinvestment_period`; ``None`` when the deal does not
            reinvest.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def total_balance(self) -> Money:
        """
        Current balance of the whole pool (mirrors Rust ``AssetPool::total_balance``).

        Sums the loan-level assets, the representative lines or the
        instrument collateral, whichever represents the pool; defaulted
        assets are included.

        Returns
        -------
        Money
            Pool balance in the pool currency.

        Raises
        ------
        ValueError
            If the pool carries both asset rows and representative lines, or
            balances in more than one currency.
        """
        ...

    def performing_balance(self) -> Money:
        """
        Balance of the performing (non-defaulted) collateral.

        Returns
        -------
        Money
            Performing balance in the pool currency; equals ``total_balance()``
            when the pool has no loan-level assets.

        Raises
        ------
        ValueError
            If the pool representation or currencies are inconsistent.
        """
        ...

    def wac(self) -> float:
        """
        Weighted-average coupon of the performing fixed-rate collateral.

        Returns
        -------
        float
            Balance-weighted coupon as an annual decimal (``0.07`` = 7%);
            ``0.0`` when no performing fixed-rate collateral exists.

        Notes
        -----
        This method does not raise.
        """
        ...

    def weighted_avg_spread_bp(self) -> float:
        """
        Weighted-average spread of the performing collateral that carries an explicit spread.

        Returns
        -------
        float
            Balance-weighted spread in basis points; ``0.0`` when no
            performing asset carries a spread.

        Notes
        -----
        This method does not raise.
        """
        ...

    def weighted_avg_maturity(self, as_of: datetime.date | str) -> float:
        """
        Balance-weighted average remaining maturity of the pool.

        Parameters
        ----------
        as_of : datetime.date | str
            Date the remaining maturities are measured from, either a
            date-like object or an ISO 8601 string.

        Returns
        -------
        float
            Weighted average maturity in years (ACT/365F from ``as_of``);
            ``0.0`` for an empty pool.

        Raises
        ------
        ValueError
            If ``as_of`` is not a date or the pool balances are inconsistent.
        """
        ...

    def diversity_score(self) -> float:
        """
        Moody's diversity score of the pool.

        Returns
        -------
        float
            Diversity score from the obligor and industry concentrations;
            ``0.0`` when the pool balance cannot be computed.

        Notes
        -----
        This method does not raise.
        """
        ...

    def assets_by_obligor(self, obligor_id: str) -> list[PoolAsset]:
        """
        Loan-level assets of one obligor.

        Parameters
        ----------
        obligor_id : str
            Obligor identifier matched exactly against each asset's
            ``obligor_id``.

        Returns
        -------
        list[PoolAsset]
            Copies of the matching assets in pool order; empty when none match.

        Notes
        -----
        This method does not raise.
        """
        ...

    def assets_by_industry(self, industry: str) -> list[PoolAsset]:
        """
        Loan-level assets in one industry.

        Parameters
        ----------
        industry : str
            Industry label matched exactly against each asset's ``industry``.

        Returns
        -------
        list[PoolAsset]
            Copies of the matching assets in pool order; empty when none match.

        Notes
        -----
        This method does not raise.
        """
        ...
