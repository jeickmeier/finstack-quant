"""Stub declarations mirroring ``finstack-quant-py/src/bindings/statements_analytics/capital_structure.rs``."""

from __future__ import annotations

import datetime
from typing import Any, Literal
import pandas as pd
from finstack_quant.core.money import Money
from finstack_quant.statements import FinancialModelSpec, StatementResult

class CapitalStructureTranche:
    """
    One debt instrument's row in a :class:`CapitalStructureSummary`.

    Tranches in one payment class are pari passu, so the figures through a class
    are the same on every tranche of the class: they count the whole class and
    every class senior to it.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import CapitalStructureTranche
    >>> tranche = CapitalStructureTranche.from_json(
    ...     '{"id": "TLB", "kind": "term_loan", "rank": 0, "payment_class": "1L",'
    ...     ' "balance": {"amount": "400", "currency": "USD"}, "share_of_debt": 0.5,'
    ...     ' "maturity": "2029-06-30", "cumulative_debt": {"amount": "800", "currency": "USD"},'
    ...     ' "leverage": 2.0, "interest_coverage": 10.0,'
    ...     ' "ltm_interest": {"amount": "20", "currency": "USD"}}'
    ... )
    >>> tranche.id, tranche.kind, tranche.rank, tranche.payment_class
    ('TLB', 'term_loan', 0, '1L')
    >>> tranche.balance.amount, tranche.share_of_debt, tranche.maturity.isoformat()
    (400.0, 0.5, '2029-06-30')
    >>> tranche.cumulative_debt.amount, tranche.leverage, tranche.interest_coverage, tranche.ltm_interest.amount
    (800.0, 2.0, 10.0, 20.0)
    """

    @staticmethod
    def from_json(json: str) -> CapitalStructureTranche:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``CapitalStructureTranche`` (the shape ``to_json`` writes).

        Returns
        -------
        CapitalStructureTranche
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import CapitalStructureTranche
        >>> try:
        ...     CapitalStructureTranche.from_json("{}")
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
            The tranche as canonical JSON.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return the tranche's wire form as a dictionary.

        Returns
        -------
        dict[str, Any]
            The parsed form of :meth:`to_json`.

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
            :meth:`from_json` and the ``to_json`` text.

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
            A one-line summary of the tranche.
        """
        ...

    @property
    def id(self) -> str:
        """
        Instrument identifier in the capital structure.

        Returns
        -------
        str
            The identifier.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def kind(self) -> Literal["bond", "convertible_bond", "term_loan", "revolving_credit"]:
        """
        Instrument type.

        Returns
        -------
        Literal["bond", "convertible_bond", "term_loan", "revolving_credit"]
            Bonds, convertibles, term loans and revolvers are debt; hedges never appear.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def rank(self) -> int:
        """
        Seniority rank of the payment class; ``0`` is most senior.

        Returns
        -------
        int
            The rank, ``0`` on every tranche when the waterfall declares no payment classes.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def payment_class(self) -> str | None:
        """
        Identifier of the payment class.

        Returns
        -------
        str | None
            The class id, or ``None`` when the waterfall declares no payment classes.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def balance(self) -> Money:
        """
        Debt balance at the end of the reporting period.

        Returns
        -------
        Money
            The balance; for a revolver this is the drawn balance, not the commitment.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def share_of_debt(self) -> float | None:
        """
        Balance over total debt, as a decimal (``0.25`` means 25%).

        Returns
        -------
        float | None
            The share, or ``None`` when total debt is not positive.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def maturity(self) -> datetime.date:
        """
        Final contractual maturity of the instrument.

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
    def cumulative_debt(self) -> Money:
        """
        Total balance of this payment class and every class senior to it.

        Returns
        -------
        Money
            The balance through this class.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def leverage(self) -> float | None:
        """
        Cumulative debt over LTM EBITDA, in turns (``3.5`` means 3.5x).

        Returns
        -------
        float | None
            The multiple, or ``None`` when LTM EBITDA is partial or not positive.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def interest_coverage(self) -> float | None:
        """
        LTM EBITDA over the LTM interest of this class and every class senior to it, in turns.

        Returns
        -------
        float | None
            The multiple, or ``None`` when LTM EBITDA is partial or not positive, or that interest is unavailable or not positive.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def ltm_interest(self) -> Money | None:
        """
        This tranche's cash plus PIK interest expense over the LTM window.

        Returns
        -------
        Money | None
            The sum, or ``None`` when the window is short or the instrument has no cashflows in one of its periods.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class MaturityWallBucket:
    """
    Outstanding balance falling due in one calendar year, a bar of the maturity wall.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import MaturityWallBucket
    >>> bucket = MaturityWallBucket.from_json(
    ...     '{"year": 2029, "amount": {"amount": "500", "currency": "USD"}, "share_of_debt": 0.5}'
    ... )
    >>> bucket.year, bucket.amount.amount, bucket.share_of_debt
    (2029, 500.0, 0.5)
    """

    @staticmethod
    def from_json(json: str) -> MaturityWallBucket:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``MaturityWallBucket`` (the shape ``to_json`` writes).

        Returns
        -------
        MaturityWallBucket
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import MaturityWallBucket
        >>> try:
        ...     MaturityWallBucket.from_json("{}")
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
            The bucket as canonical JSON.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return the bucket's wire form as a dictionary.

        Returns
        -------
        dict[str, Any]
            The parsed form of :meth:`to_json`.

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
            :meth:`from_json` and the ``to_json`` text.

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
            A one-line summary of the bucket.
        """
        ...

    @property
    def year(self) -> int:
        """
        Calendar year of the final maturities in this bucket.

        Returns
        -------
        int
            The four-digit calendar year, such as ``2029``.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def amount(self) -> Money:
        """
        Balance at the reporting period of the tranches maturing in :attr:`year`.

        Returns
        -------
        Money
            The amount.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def share_of_debt(self) -> float | None:
        """
        Amount over total debt, as a decimal.

        Returns
        -------
        float | None
            The share, or ``None`` when total debt is not positive.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class CapitalStructureSummary:
    """
    Debt stack by seniority at one period, returned by :func:`capital_structure_summary`.

    Examples
    --------
    >>> from datetime import date
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.statements import Evaluator, ModelBuilder
    >>> from finstack_quant.statements_analytics import capital_structure_summary
    >>> quarters = ["2025Q1", "2025Q2", "2025Q3", "2025Q4"]
    >>> builder = ModelBuilder("borrower")
    >>> _ = builder.periods("2025Q1..Q4", None)
    >>> _ = builder.value("ebitda", [(quarter, 50_000.0) for quarter in quarters])
    >>> _ = builder.value("cash", [(quarter, 100_000.0) for quarter in quarters])
    >>> usd = Currency("USD")
    >>> _ = builder.add_bond("SENIOR", Money(1_000_000.0, usd), 0.05, date(2024, 1, 1), date(2029, 6, 1), "USD-OIS")
    >>> model = builder.build()
    >>> summary = capital_structure_summary(
    ...     Evaluator().evaluate(model), model, {"period": "2025Q4", "ebitda_node": "ebitda", "cash_node": "cash"}
    ... )
    >>> summary.as_of, summary.currency, summary.total_debt.amount
    ('2025Q4', 'USD', 1000000.0)
    >>> summary.cash.amount, summary.net_debt.amount, summary.ltm_ebitda.amount
    (100000.0, 900000.0, 200000.0)
    >>> summary.ltm_periods_required, summary.ltm_periods_found
    (4, 4)
    >>> summary.total_leverage, summary.net_leverage, summary.total_coverage
    (5.0, 4.5, 4.0)
    >>> [tranche.id for tranche in summary.tranches], [bucket.year for bucket in summary.maturity_wall]
    (['SENIOR'], [2029])
    """

    @staticmethod
    def from_json(json: str) -> CapitalStructureSummary:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``CapitalStructureSummary`` (the shape ``to_json`` writes).

        Returns
        -------
        CapitalStructureSummary
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import CapitalStructureSummary
        >>> try:
        ...     CapitalStructureSummary.from_json("{}")
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
            The summary as canonical JSON.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return the summary's wire form as a dictionary.

        Returns
        -------
        dict[str, Any]
            The parsed form of :meth:`to_json`.

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
            :meth:`from_json` and the ``to_json`` text.

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
            A one-line summary of the summary.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        One row per tranche, by rank then model declaration order.

        Returns
        -------
        pd.DataFrame
            Columns ``id``, ``kind``, ``rank``, ``payment_class``, ``balance``,
            ``share_of_debt``, ``maturity`` (ISO string), ``cumulative_debt``,
            ``leverage``, ``interest_coverage`` and ``ltm_interest``, with amounts in
            :attr:`currency` as floats and undefined figures as ``NaN``.

        Raises
        ------
        ValueError
            If a row cannot be serialized.
        """
        ...

    @property
    def as_of(self) -> str:
        """
        Reporting period identifier, such as ``2025Q4``.

        Returns
        -------
        str
            The period the balances are read at and the LTM window ends at.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def currency(self) -> str:
        """
        ISO-4217 code of the currency every tranche and monetary node is in.

        Returns
        -------
        str
            The currency code.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def total_debt(self) -> Money:
        """
        Sum of tranche balances at the reporting period.

        Returns
        -------
        Money
            The total, with revolvers at their drawn balance and hedges excluded.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def cash(self) -> Money | None:
        """
        Cash balance at the reporting period.

        Returns
        -------
        Money | None
            The balance, or ``None`` without a ``cash_node``.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def net_debt(self) -> Money | None:
        """
        Total debt less cash.

        Returns
        -------
        Money | None
            The amount, negative when cash exceeds debt, or ``None`` without a ``cash_node``.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def ltm_ebitda(self) -> Money | None:
        """
        EBITDA summed over the LTM window.

        Returns
        -------
        Money | None
            The sum, reported when zero or negative, or ``None`` when the window is short.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def ltm_periods_required(self) -> int:
        """
        Periods a full LTM window holds at the reporting period's frequency.

        Returns
        -------
        int
            4 for quarters, 12 for months.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def ltm_periods_found(self) -> int:
        """
        Periods of the LTM window that are in the model and hold an EBITDA value.

        Returns
        -------
        int
            The count; below :attr:`ltm_periods_required` the ratios are ``None``.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def total_leverage(self) -> float | None:
        """
        Total debt over LTM EBITDA, in turns.

        Returns
        -------
        float | None
            The multiple, or ``None`` when LTM EBITDA is partial or not positive.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def net_leverage(self) -> float | None:
        """
        Net debt over LTM EBITDA, in turns.

        Returns
        -------
        float | None
            The multiple, negative when cash exceeds debt, or ``None`` without a ``cash_node`` or when LTM EBITDA is partial or not positive.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def total_coverage(self) -> float | None:
        """
        LTM EBITDA over the LTM interest of every tranche, in turns.

        Returns
        -------
        float | None
            The multiple, or ``None`` when LTM EBITDA is partial or not positive, or that interest is unavailable or not positive.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def tranches(self) -> list[CapitalStructureTranche]:
        """
        Tranches by rank, then model declaration order.

        Returns
        -------
        list[CapitalStructureTranche]
            The tranches, zero balances included.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def maturity_wall(self) -> list[MaturityWallBucket]:
        """
        Non-zero balances by calendar year of final maturity, ascending.

        Returns
        -------
        list[MaturityWallBucket]
            The buckets, using the balance at the reporting period and ignoring any scheduled amortization after it.
        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

def capital_structure_summary(
    results: StatementResult | str,
    model: FinancialModelSpec | str,
    config: dict[str, Any] | str,
) -> CapitalStructureSummary:
    """
    Summarize the debt stack by seniority at one period.

    Only bonds, convertible bonds, term loans and revolvers are debt; swaps,
    cap/floors and swaptions are left out of balances and interest, including when a
    payment class names them. Seniority is the rank of the waterfall's payment classes.
    A model with no waterfall or no payment classes has one implicit class: every
    tranche has rank ``0`` and no class id.

    LTM EBITDA and LTM interest sum the trailing periods of one year ending at the
    reporting period (4 quarters, 12 months), the slots ``ttm()`` reads in a formula. A
    window with a slot missing from the model or a slot where the EBITDA node has no
    value is never annualised or partially summed: LTM EBITDA and every ratio are
    ``None``, and ``ltm_periods_found`` says how many slots held. A zero or negative LTM
    EBITDA also gives ``None`` ratios, not a negative multiple. Interest is cash plus
    PIK. Where an instrument has no cashflows in an earlier slot, its LTM interest and
    the coverage through its class and every class junior to it are ``None``.

    Parameters
    ----------
    results : StatementResult or str
        Evaluated statement results, typed or as JSON, holding ``cs_cashflows`` and both
        configured nodes. A scalar EBITDA or cash node is read as an amount in the debt
        currency.
    model : FinancialModelSpec or str
        Model, typed or as JSON, whose capital structure gives the instruments, their
        declaration order and the payment classes, and whose periods bound the LTM
        window.
    config : dict or str
        ``CapitalStructureSummaryConfig`` document, a dict or JSON text: ``period`` (the
        reporting period, such as ``"2025Q4"``), ``ebitda_node`` (required; the node
        holding EBITDA for each single period, never inferred from a conventional name)
        and ``cash_node`` (optional; the node holding the period-end cash balance,
        omitted means no cash, net debt or net leverage).

    Returns
    -------
    CapitalStructureSummary
        Tranches by rank and declaration order, the ratios through each class and the
        maturity wall.

    Raises
    ------
    ValueError
        If ``config``, ``results`` or ``model`` is malformed; the model has no capital
        structure, or no debt instruments once hedges are excluded; ``cs_cashflows`` is
        absent; the period is not in the model; a configured node is not in ``results``,
        or the cash node has no value at the period; an instrument has no cashflows at
        the period or is in no class while payment classes are declared; a consumed value
        or ratio is non-finite; tranches are in more than one currency (no FX is applied);
        or the EBITDA or cash node is monetary in a currency other than the debt's.

    Examples
    --------
    >>> from datetime import date
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.statements import Evaluator, ModelBuilder
    >>> from finstack_quant.statements_analytics import capital_structure_summary
    >>> quarters = ["2025Q1", "2025Q2", "2025Q3", "2025Q4"]
    >>> builder = ModelBuilder("borrower")
    >>> _ = builder.periods("2025Q1..Q4", None)
    >>> _ = builder.value("ebitda", [(quarter, 50_000.0) for quarter in quarters])
    >>> _ = builder.value("cash", [(quarter, 100_000.0) for quarter in quarters])
    >>> usd = Currency("USD")
    >>> _ = builder.add_bond("SENIOR", Money(1_000_000.0, usd), 0.05, date(2024, 1, 1), date(2029, 6, 1), "USD-OIS")
    >>> model = builder.build()
    >>> results = Evaluator().evaluate(model)
    >>> summary = capital_structure_summary(results, model, {"period": "2025Q4", "ebitda_node": "ebitda"})
    >>> summary.total_leverage
    5.0
    >>> summary.net_leverage is None
    True
    >>> short = capital_structure_summary(results, model, {"period": "2025Q3", "ebitda_node": "ebitda"})
    >>> short.ltm_periods_found, short.total_leverage is None
    (3, True)
    """
    ...
