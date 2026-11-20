"""Credit scoring on admitted annual monetary statement observations."""

from datetime import date
from finstack_quant.core.money import Money
from finstack_quant.models.credit.scoring import ScoringResult
from finstack_quant.statements import StatementResult

class StatementScoringMapping:
    """Identify the financial statement nodes used to construct credit ratios.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import StatementScoringMapping
    >>> mapping = StatementScoringMapping(total_assets="assets", total_liabilities="liabilities")
    >>> isinstance(mapping, StatementScoringMapping)
    True
    """

    def __init__(
        self,
        *,
        total_assets: str,
        total_liabilities: str,
        working_capital: str | None = None,
        retained_earnings: str | None = None,
        ebit: str | None = None,
        sales: str | None = None,
        book_equity: str | None = None,
        current_assets: str | None = None,
        current_liabilities: str | None = None,
        net_income: str | None = None,
    ) -> None:
        """Map model-specific financial inputs by exact node identifier.

        Parameters
        ----------
        total_assets : str
            Positive closing total assets in reporting currency.
        total_liabilities : str
            Closing total liabilities in reporting currency.
        working_capital : str or None
            Closing net working capital, required by the Altman models.
        retained_earnings : str or None
            Closing retained earnings, required by the Altman models.
        ebit : str or None
            Full-year EBIT, required by the Altman models.
        sales : str or None
            Full-year sales, required by the manufacturing Altman models.
        book_equity : str or None
            Closing book equity, required by the book-value Altman models.
        current_assets : str or None
            Closing current assets, required by Zmijewski.
        current_liabilities : str or None
            Positive closing current liabilities, required by Zmijewski.
        net_income : str or None
            Full-year net income, required by Zmijewski.

        Raises
        ------
        ValueError
            If any supplied field cannot be decoded as a node identifier.
        """
        ...

class CreditScoringModel:
    """Choose a native score and any matching non-statement observation.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import CreditScoringModel
    >>> isinstance(CreditScoringModel.zmijewski(), CreditScoringModel)
    True
    """

    @staticmethod
    def altman_z(market_equity: Money, as_of: date | str) -> CreditScoringModel:
        """Elect the original manufacturing score for public companies.

        Parameters
        ----------
        market_equity : Money
            Nonnegative closing market cap in the statement currency.
        as_of : date or str
            Date of the share valuation, required to match the annual statement close.

        Returns
        -------
        CreditScoringModel
            Original Altman model with its dated market equity.

        Raises
        ------
        ValueError
            If the date spelling is invalid.

        Examples
        --------
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.statements_analytics import CreditScoringModel
        >>> isinstance(CreditScoringModel.altman_z(Money(900, "USD"), "2024-12-31"), CreditScoringModel)
        True
        """
        ...

    @staticmethod
    def altman_z_prime() -> CreditScoringModel:
        """Elect the Altman manufacturing score for privately held companies.

        This method does not raise.

        Returns
        -------
        CreditScoringModel
            A model that reads its inputs from annual statement observations.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import CreditScoringModel
        >>> isinstance(CreditScoringModel.altman_z_prime(), CreditScoringModel)
        True
        """
        ...

    @staticmethod
    def altman_z_double_prime() -> CreditScoringModel:
        """Elect the four-factor Altman score for nonmanufacturers.

        This method does not raise.

        Returns
        -------
        CreditScoringModel
            A model that reads its inputs from annual statement observations.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import CreditScoringModel
        >>> isinstance(CreditScoringModel.altman_z_double_prime(), CreditScoringModel)
        True
        """
        ...

    @staticmethod
    def altman_em() -> CreditScoringModel:
        """Elect the four-factor Altman emerging-market score.

        This method does not raise.

        Returns
        -------
        CreditScoringModel
            A model that reads its inputs from annual statement observations.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import CreditScoringModel
        >>> isinstance(CreditScoringModel.altman_em(), CreditScoringModel)
        True
        """
        ...

    @staticmethod
    def zmijewski() -> CreditScoringModel:
        """Elect the Zmijewski accounting-ratio probit model.

        This method does not raise.

        Returns
        -------
        CreditScoringModel
            A model that reads its inputs from annual statement observations.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import CreditScoringModel
        >>> isinstance(CreditScoringModel.zmijewski(), CreditScoringModel)
        True
        """
        ...

def score_statement(
    results: StatementResult,
    period: str,
    mapping: StatementScoringMapping,
    model: CreditScoringModel,
) -> ScoringResult:
    """Compute a canonical distress score from an annual statement.

    Parameters
    ----------
    results : StatementResult
        Authoritative period plan and evaluated monetary observations.
    period : str
        Exact full-year label containing flow and closing balance-sheet observations.
    mapping : StatementScoringMapping
        Same-currency monetary node IDs required by the selected score.
    model : CreditScoringModel
        Elected model form and any required dated market capitalization.

    Returns
    -------
    ScoringResult
        Native score, classification zone and implied PD when the model defines one.

    Raises
    ------
    ValueError
        If a year, observation, currency, denominator or model input is invalid.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import score_statement
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.statements import Evaluator, ModelBuilder
    >>> from finstack_quant.statements_analytics import CreditScoringModel, StatementScoringMapping
    >>> b = ModelBuilder("borrower").periods("2024..2024", "2024").currency("USD")
    >>> values = {
    ...     "assets": 1000,
    ...     "liabilities": 600,
    ...     "current_assets": 400,
    ...     "current_liabilities": 250,
    ...     "net_income": 60,
    ... }
    >>> for name, amount in values.items():
    ...     _ = b.value_money(name, [("2024", Money(amount, "USD"))])
    >>> results = Evaluator().evaluate(b.build())
    >>> mapping = StatementScoringMapping(
    ...     total_assets="assets",
    ...     total_liabilities="liabilities",
    ...     net_income="net_income",
    ...     current_assets="current_assets",
    ...     current_liabilities="current_liabilities",
    ... )
    >>> model = CreditScoringModel.zmijewski()
    >>> score_statement(results, "2024", mapping, model).implied_pd > 0
    True
    """
    ...

def score_statement_series(
    results: StatementResult,
    rows: list[tuple[str, CreditScoringModel]],
    mapping: StatementScoringMapping,
) -> list[ScoringResult]:
    """Calculate matched credit scores for an explicitly selected annual series.

    Parameters
    ----------
    results : StatementResult
        Evaluated annual monetary observations on a common period plan.
    rows : list of pairs
        Distinct full-year periods with the model elected for each date.
    mapping : StatementScoringMapping
        Exact financial statement node names shared by the requested years.

    Returns
    -------
    list[ScoringResult]
        Native scores and zones in the submitted period order.

    Raises
    ------
    ValueError
        If a year repeats or any requested score fails validation.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import score_statement_series
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.statements import Evaluator, ModelBuilder
    >>> from finstack_quant.statements_analytics import CreditScoringModel, StatementScoringMapping
    >>> b = ModelBuilder("borrower").periods("2024..2024", "2024").currency("USD")
    >>> values = {
    ...     "assets": 1000,
    ...     "liabilities": 600,
    ...     "current_assets": 400,
    ...     "current_liabilities": 250,
    ...     "net_income": 60,
    ... }
    >>> for name, amount in values.items():
    ...     _ = b.value_money(name, [("2024", Money(amount, "USD"))])
    >>> results = Evaluator().evaluate(b.build())
    >>> mapping = StatementScoringMapping(
    ...     total_assets="assets",
    ...     total_liabilities="liabilities",
    ...     net_income="net_income",
    ...     current_assets="current_assets",
    ...     current_liabilities="current_liabilities",
    ... )
    >>> model = CreditScoringModel.zmijewski()
    >>> len(score_statement_series(results, [("2024", model)], mapping))
    1
    """
    ...
