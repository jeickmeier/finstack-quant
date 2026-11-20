"""Stub declarations mirroring ``finstack-quant-py/src/bindings/portfolio/fi_attribution.rs``."""

from __future__ import annotations

from datetime import date
from decimal import Decimal
from typing import Any
import pandas as pd
from finstack_quant.core.currency import Currency
from finstack_quant.core.money import Money
from finstack_quant.portfolio._attribution import ReconciliationReport

class FiAttributionResult:
    """
    Single-period Campisi benchmark-relative attribution result.

    Returned by :func:`campisi_attribution`. The five effect totals sum to
    :attr:`active_return`; sector effects preserve declared sector order.

    Examples
    --------
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.portfolio import (
    ...     FiAttributionConfig,
    ...     FiPositionSnapshot,
    ...     FiReportingBasis,
    ...     campisi_attribution,
    ... )
    >>> basis = FiReportingBasis(
    ...     "portfolio", "benchmark", "reinvested", "dirty-settlement", "USD-native", "net-of-fees"
    ... )
    >>> config = FiAttributionConfig("2025-01-01", "2025-03-15", "USD", basis, ["GOVT"])
    >>> snap = FiPositionSnapshot("UST.1", "GOVT", "1", 0.02, Money(80, "USD"), Money(5, "USD"))
    >>> report = campisi_attribution([snap], [snap], config)
    >>> report.active_return
    0.0
    """

    @property
    def sectors(self) -> list[dict[str, object]]:
        """
        Per-sector benchmark-relative effects as records.

        Returns
        -------
        list[dict[str, object]]
            JSON-shaped view of the canonical Rust field.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def portfolio_components(self) -> dict[str, object]:
        """
        Portfolio-side absolute Campisi split (carry, treasury, spread, residual, total).

        Returns
        -------
        dict[str, object]
            JSON-shaped view of the canonical Rust field.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def benchmark_components(self) -> dict[str, object]:
        """
        Benchmark-side absolute Campisi split (carry, treasury, spread, residual, total).

        Returns
        -------
        dict[str, object]
            JSON-shaped view of the canonical Rust field.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def portfolio_return(self) -> float:
        """
        Portfolio total return for the period.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def benchmark_return(self) -> float:
        """
        Benchmark total return for the period.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def active_return(self) -> float:
        """
        Active return; portfolio minus benchmark total return.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def total_allocation(self) -> float:
        """
        Sum of sector allocation effects.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def total_active_carry(self) -> float:
        """
        Sum of sector active carry effects.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def total_active_treasury(self) -> float:
        """
        Sum of sector active treasury effects.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def total_active_spread(self) -> float:
        """
        Sum of sector active spread effects.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def total_residual(self) -> float:
        """
        Sum of sector residual effects.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Per-sector effects as a pandas DataFrame.

        Columns: ``sector``, ``portfolio_weight``, ``benchmark_weight``, ``portfolio_return``, ``benchmark_return``, ``allocation``, ``active_carry``, ``active_treasury``, ``active_spread``, ``residual``, ``total_active``. The schema is pinned, so an empty result keeps the
        same dtypes as a populated one.

        Returns
        -------
        pd.DataFrame
            One row per entry, with the pinned column schema above.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def reconciliation_check(self, tolerance: float) -> ReconciliationReport:
        """Compare the displayed active return with the sum of displayed effects.

        Parameters
        ----------
        tolerance : float
            Finite nonnegative absolute tolerance in decimal return units.

        Returns
        -------
        ReconciliationReport
            Signed difference, comparison tolerance and pass indicator.

        Raises
        ------
        PortfolioError
            If the tolerance is negative or nonfinite, or the displayed
            reconciliation difference cannot be represented.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this result to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation, suitable for a matching
            :meth:`from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @staticmethod
    def from_json(json: str) -> FiAttributionResult:
        """
        Deserialize a ``FiAttributionResult`` from canonical JSON.

        Parameters
        ----------
        json : str
            Canonical payload produced by :meth:`to_json`, including its
            source observations.

        Returns
        -------
        FiAttributionResult
            Validated instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or does not match the serialized
            ``FiAttributionResult`` schema.

        Examples
        --------
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.portfolio import (
        ...     FiAttributionConfig,
        ...     FiPositionSnapshot,
        ...     FiReportingBasis,
        ...     campisi_attribution,
        ... )
        >>> basis = FiReportingBasis("pf", "bm", "reinvested", "dirty", "native", "net")
        >>> holding = FiPositionSnapshot("X", "GOVT", "1", 0.02, Money(100, "USD"), Money(0, "USD"))
        >>> first = FiAttributionConfig("2025-01-01", "2025-02-01", "USD", basis, ["GOVT"])
        >>> period = campisi_attribution([holding], [holding], first)
        >>> from finstack_quant.portfolio import FiAttributionResult
        >>> FiAttributionResult.from_json(period.to_json()).active_return
        0.0
        """
        ...

class FiCarinoLinkedResult:
    """
    Multi-period Carino-linked Campisi attribution result.

    Returned by :func:`campisi_carino_link`. The five linked totals sum to
    the geometrically compounded active return.

    Examples
    --------
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.portfolio import (
    ...     FiAttributionConfig,
    ...     FiPositionSnapshot,
    ...     FiReportingBasis,
    ...     campisi_attribution,
    ... )
    >>> basis = FiReportingBasis("pf", "bm", "reinvested", "dirty", "native", "net")
    >>> holding = FiPositionSnapshot("X", "GOVT", "1", 0.02, Money(100, "USD"), Money(0, "USD"))
    >>> first = FiAttributionConfig("2025-01-01", "2025-02-01", "USD", basis, ["GOVT"])
    >>> period = campisi_attribution([holding], [holding], first)
    >>> from finstack_quant.portfolio import campisi_carino_link
    >>> second = FiAttributionConfig("2025-02-01", "2025-03-01", "USD", basis, ["GOVT"])
    >>> later = campisi_attribution([holding], [holding], second)
    >>> linked = campisi_carino_link([period, later])
    >>> linked.active_return_compounded
    0.0
    """

    @property
    def periods(self) -> list[dict[str, object]]:
        """
        Per-period single-period results as records, in chronological order.

        Returns
        -------
        list[dict[str, object]]
            JSON-shaped view of the canonical Rust field.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def portfolio_return_compounded(self) -> float:
        """
        Geometrically compounded portfolio return.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def benchmark_return_compounded(self) -> float:
        """
        Geometrically compounded benchmark return.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def active_return_compounded(self) -> float:
        """Return compounded active return computed before projecting the two side returns.

        Returns
        -------
        float
            Portfolio less benchmark compounded growth in decimal units.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def linked_sectors(self) -> list[dict[str, object]]:
        """
        Per-sector linked effects as records.

        Returns
        -------
        list[dict[str, object]]
            JSON-shaped view of the canonical Rust field.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def linked_allocation(self) -> float:
        """
        Sum of linked allocation effects.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def linked_active_carry(self) -> float:
        """
        Sum of linked active carry effects.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def linked_active_treasury(self) -> float:
        """
        Sum of linked active treasury effects.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def linked_active_spread(self) -> float:
        """
        Sum of linked active spread effects.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def linked_residual(self) -> float:
        """
        Sum of linked residual effects.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Linked per-sector effects as a pandas DataFrame.

        Columns: ``sector``, ``allocation``, ``active_carry``, ``active_treasury``, ``active_spread``, ``residual``, ``total_active``. The schema is pinned, so an empty result keeps the
        same dtypes as a populated one.

        Returns
        -------
        pd.DataFrame
            One row per entry, with the pinned column schema above.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this result to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation, suitable for a matching
            :meth:`from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @staticmethod
    def from_json(json: str) -> FiCarinoLinkedResult:
        """
        Deserialize a ``FiCarinoLinkedResult`` from canonical JSON.

        Parameters
        ----------
        json : str
            Canonical payload produced by :meth:`to_json`, including its
            source observations.

        Returns
        -------
        FiCarinoLinkedResult
            Validated instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or does not match the serialized
            ``FiCarinoLinkedResult`` schema.

        Examples
        --------
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.portfolio import (
        ...     FiAttributionConfig,
        ...     FiPositionSnapshot,
        ...     FiReportingBasis,
        ...     campisi_attribution,
        ... )
        >>> basis = FiReportingBasis("pf", "bm", "reinvested", "dirty", "native", "net")
        >>> holding = FiPositionSnapshot("X", "GOVT", "1", 0.02, Money(100, "USD"), Money(0, "USD"))
        >>> first = FiAttributionConfig("2025-01-01", "2025-02-01", "USD", basis, ["GOVT"])
        >>> period = campisi_attribution([holding], [holding], first)
        >>> from finstack_quant.portfolio import campisi_carino_link
        >>> second = FiAttributionConfig("2025-02-01", "2025-03-01", "USD", basis, ["GOVT"])
        >>> later = campisi_attribution([holding], [holding], second)
        >>> linked = campisi_carino_link([period, later])
        >>> from finstack_quant.portfolio import FiCarinoLinkedResult
        >>> FiCarinoLinkedResult.from_json(linked.to_json()).active_return_compounded
        0.0
        """
        ...

class FiQuote:
    """An opening or closing quote for a selected Campisi rate experiment.

    Examples
    --------
    >>> from finstack_quant.portfolio import FiQuote
    >>> quote = FiQuote("UST-5Y-par", 0.035)
    """

    def __init__(self, basis: str, rate: float) -> None:
        """Record a qualified absolute decimal annual rate.

        Parameters
        ----------
        basis : str
            Stable quote experiment identifier including the relevant curve,
            tenor or claim, compounding, valuation clock and settlement basis.
        rate : float
            Observed annual yield or spread in decimal units. A zero or
            negative spread is admissible.

        Raises
        ------
        TypeError
            If the basis cannot be read as text or the rate as a number.
        """
        ...

class FiQuoteLeg:
    """Opening normalized duration and two observations on the same basis.

    Examples
    --------
    >>> from finstack_quant.portfolio import FiQuote, FiQuoteLeg
    >>> leg = FiQuoteLeg("UST-5Y-par", 4.5, FiQuote("UST-5Y-par", 0.03), FiQuote("UST-5Y-par", 0.031))
    """

    def __init__(self, sensitivity_basis: str, duration: float, opening: FiQuote, closing: FiQuote) -> None:
        """Record the valuation sensitivity and the opening and closing rates.

        Parameters
        ----------
        sensitivity_basis : str
            The exact experiment used to estimate opening normalized duration.
        duration : float
            Years of price sensitivity per decimal rate change at the opening mark.
        opening : FiQuote
            Opening quote on the selected sensitivity basis.
        closing : FiQuote
            Closing quote on the unchanged opening basis.

        Raises
        ------
        TypeError
            If a term has the wrong host type. Basis and finiteness are checked
            when the report is calculated.
        """
        ...

class FiReportingBasis:
    """Common valuation and realized-return conventions for a Campisi period.

    Examples
    --------
    >>> from finstack_quant.portfolio import FiReportingBasis
    >>> basis = FiReportingBasis("portfolio", "benchmark", "reinvested", "dirty-settlement", "USD-close", "net-of-fees")
    """

    def __init__(
        self, portfolio_series_id: str, benchmark_series_id: str, return_policy: str, valuation: str, fx: str, fees: str
    ) -> None:
        """Identify the same reporting conventions for both sides.

        Parameters
        ----------
        portfolio_series_id : str
            Nonempty identifier of the portfolio realized return series.
        benchmark_series_id : str
            Nonempty identifier of the benchmark realized return series.
        return_policy : str
            Named total-return convention, including reinvestment.
        valuation : str
            Named valuation cut, settlement and observed price convention.
        fx : str
            Named FX translation policy and basis.
        fees : str
            Named fee inclusion and treatment policy.

        Raises
        ------
        TypeError
            If any convention is not supplied as text.
        """
        ...

class FiAttributionConfig:
    """Dated reporting window, currency and sector axis for Campisi attribution.

    Examples
    --------
    >>> from finstack_quant.portfolio import FiAttributionConfig, FiReportingBasis
    >>> basis = FiReportingBasis("portfolio", "benchmark", "reinvested", "dirty-settlement", "USD-close", "net-of-fees")
    >>> config = FiAttributionConfig("2025-01-01", "2025-03-15", "USD", basis, ["Treasuries"])
    """

    def __init__(
        self,
        start: date | str,
        end: date | str,
        currency: Currency | str,
        basis: FiReportingBasis,
        sectors: list[str],
    ) -> None:
        """Record the reporting dates and ordered calculation axis.

        Parameters
        ----------
        start : date or str
            Opening valuation date, as a date or ISO calendar date text.
        end : date or str
            Closing valuation date strictly after the opening date.
        currency : Currency or str
            Currency of the return basis, opening marks and annual cash coupons.
        basis : FiReportingBasis
            Shared conventions supplied by the portfolio and benchmark.
        sectors : list[str]
            Unique ordered sector labels, including applicable cash and financing.

        Raises
        ------
        TypeError
            If a date, currency, basis or sector axis has the wrong host type.
        ValueError
            If a date or currency cannot be parsed.
        """
        ...

class FiPositionSnapshot:
    """Exact signed opening weight, cash coupon and realized return for a holding.

    Examples
    --------
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.portfolio import FiPositionSnapshot
    >>> snapshot = FiPositionSnapshot("UST", "Treasuries", "1", 0.02, Money(80, "USD"), Money(5, "USD"))
    """

    def __init__(
        self,
        id: str,
        sector: str,
        weight: Decimal | str | int,
        total_return: float,
        opening_market_value: Money,
        annual_coupon: Money,
    ) -> None:
        """Record one holding on the request's reporting basis.

        Parameters
        ----------
        id : str
            Stable identifier unique on one side of the report.
        sector : str
            Label on the declared ordered sector axis.
        weight : Decimal or str or int
            Exact signed fraction of opening net capital. Each side must sum to one.
        total_return : float
            Realized total return over the request window in decimal units.
        opening_market_value : Money
            Positive magnitude of the opening mark in reporting currency.
        annual_coupon : Money
            Annual opening cash coupon income in the same currency, including an
            exact zero for a holding with no cash coupon.

        Raises
        ------
        TypeError
            If fields have the wrong host types.
        ValueError
            If the exact opening weight cannot be represented.
        """
        ...

    def with_quote_legs(
        self,
        *,
        treasury: FiQuoteLeg | None = None,
        spread: FiQuoteLeg | None = None,
    ) -> FiPositionSnapshot:
        """Return a copy of the holding with elected opening rate sensitivities.

        Parameters
        ----------
        treasury : FiQuoteLeg or None
            Opening-selected government reference quote experiment or an
            explicitly inapplicable leg.
        spread : FiQuoteLeg or None
            Qualified settlement Z-spread experiment or an explicitly
            inapplicable leg.

        Returns
        -------
        FiPositionSnapshot
            Copy whose Treasury and spread legs are set to the supplied terms.

        Raises
        ------
        TypeError
            If a supplied leg has the wrong host type.
        """
        ...

def campisi_attribution(
    portfolio: list[FiPositionSnapshot],
    benchmark: list[FiPositionSnapshot],
    config: FiAttributionConfig,
) -> FiAttributionResult:
    """Decompose dated active fixed-income return into five effects.

    Parameters
    ----------
    portfolio : list[FiPositionSnapshot]
        Portfolio holdings with exact signed opening weights and realized returns.
    benchmark : list[FiPositionSnapshot]
        Benchmark holdings on the same valuation, currency, FX and fee basis.
    config : FiAttributionConfig
        Positive-length window and ordered sector axis on which both sides are measured.

    Returns
    -------
    FiAttributionResult
        Allocation, cash coupon carry, Treasury, spread and residual. Rates are
        decimals. Sector returns are absent when that side has no holding.

    Raises
    ------
    PortfolioError
        If a side is empty, exact weights do not sum to one, positions or rates
        fail validation, or a required result is not representable.
    TypeError
        If the inputs are not the expected typed objects.

    Examples
    --------
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.portfolio import (
    ...     FiAttributionConfig,
    ...     FiPositionSnapshot,
    ...     FiReportingBasis,
    ...     campisi_attribution,
    ... )
    >>> basis = FiReportingBasis(
    ...     "portfolio", "benchmark", "reinvested", "dirty-settlement", "USD-native", "net-of-fees"
    ... )
    >>> config = FiAttributionConfig("2025-01-01", "2025-03-15", "USD", basis, ["GOVT"])
    >>> snap = FiPositionSnapshot("UST.1", "GOVT", "1", 0.02, Money(80, "USD"), Money(5, "USD"))
    >>> report = campisi_attribution([snap], [snap], config)
    >>> report.active_return
    0.0
    """
    ...

def campisi_carino_link(periods: list[FiAttributionResult]) -> FiCarinoLinkedResult:
    """Link contiguous checked Campisi periods with Carino smoothing.

    Parameters
    ----------
    periods : list[FiAttributionResult]
        Nonempty chronological sequence from :func:`campisi_attribution`.
        Each interval must start when the preceding one ends, with the
        same series identities, reporting basis, currency and sector axis.

    Returns
    -------
    FiCarinoLinkedResult
        Linked decimal effects and the independently compounded active
        return, with the single-period sources retained for restoration.

    Raises
    ------
    PortfolioError
        If the periods disagree on basis or dates, an underlying growth
        factor is nonpositive, or linked results fail numerical checks.
    TypeError
        If a supplied period is not a typed Campisi result.

    Examples
    --------
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.portfolio import (
    ...     FiAttributionConfig,
    ...     FiPositionSnapshot,
    ...     FiReportingBasis,
    ...     campisi_attribution,
    ... )
    >>> basis = FiReportingBasis("pf", "bm", "reinvested", "dirty", "native", "net")
    >>> holding = FiPositionSnapshot("X", "GOVT", "1", 0.02, Money(100, "USD"), Money(0, "USD"))
    >>> first = FiAttributionConfig("2025-01-01", "2025-02-01", "USD", basis, ["GOVT"])
    >>> period = campisi_attribution([holding], [holding], first)
    >>> from finstack_quant.portfolio import campisi_carino_link
    >>> second = FiAttributionConfig("2025-02-01", "2025-03-01", "USD", basis, ["GOVT"])
    >>> later = campisi_attribution([holding], [holding], second)
    >>> linked = campisi_carino_link([period, later])
    >>> round(linked.portfolio_return_compounded, 4)
    0.0404
    """
    ...
