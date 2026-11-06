"""
Academic credit scoring: Altman Z-Score family, Ohlson O-Score, Zmijewski.

Each model takes its input struct (``AltmanZScoreInput``, ...) and returns a
:class:`ScoringResult`. :func:`credit_score_report` scores one model and
grades its implied PD on the default PD master scale in one call.

Examples
--------
>>> from finstack_quant.models.credit import scoring
>>> ratios = scoring.AltmanZScoreInput(
...     working_capital_to_total_assets=0.2,
...     retained_earnings_to_total_assets=0.3,
...     ebit_to_total_assets=0.15,
...     market_equity_to_total_liabilities=1.5,
...     sales_to_total_assets=1.0,
... )
>>> round(scoring.altman_z_score(ratios).score, 3)
3.055

"""

from __future__ import annotations

from typing import Any

import pandas

from .pd import MasterScaleResult

__all__ = [
    "AltmanZDoublePrimeInput",
    "AltmanZPrimeInput",
    "AltmanZScoreInput",
    "CreditScoreReport",
    "OhlsonOScoreInput",
    "ScoringResult",
    "ZmijewskiInput",
    "altman_em_score",
    "altman_z_double_prime",
    "altman_z_prime",
    "altman_z_score",
    "credit_score_report",
    "ohlson_o_score",
    "zmijewski_score",
]

class AltmanZScoreInput:
    """
    Input ratios of the original Altman Z-Score (1968) for public manufacturers.

    Every field is a decimal ratio; Rust rejects a non-finite one when it is
    scored.

    Examples
    --------
    >>> from finstack_quant.models.credit import scoring
    >>> ratios = scoring.AltmanZScoreInput(
    ...     working_capital_to_total_assets=0.2,
    ...     retained_earnings_to_total_assets=0.3,
    ...     ebit_to_total_assets=0.15,
    ...     market_equity_to_total_liabilities=1.5,
    ...     sales_to_total_assets=1.0,
    ... )
    >>> ratios.working_capital_to_total_assets
    0.2
    """

    def __init__(
        self,
        *,
        working_capital_to_total_assets: float,
        retained_earnings_to_total_assets: float,
        ebit_to_total_assets: float,
        market_equity_to_total_liabilities: float,
        sales_to_total_assets: float,
    ) -> None:
        """
        Build the input ratios by keyword.

        Parameters
        ----------
        working_capital_to_total_assets : float
            Working capital / total assets (X1).
        retained_earnings_to_total_assets : float
            Retained earnings / total assets (X2).
        ebit_to_total_assets : float
            EBIT / total assets (X3).
        market_equity_to_total_liabilities : float
            Market value of equity / book value of total liabilities (X4).
        sales_to_total_assets : float
            Sales / total assets (X5).

        Notes
        -----
        This constructor does not raise; the scoring function rejects a
        non-finite ratio with ``ValueError``.
        """
        ...

    @property
    def working_capital_to_total_assets(self) -> float:
        """
        Working capital / total assets (X1).

        Returns
        -------
        float
            Working capital / total assets (X1).

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def retained_earnings_to_total_assets(self) -> float:
        """
        Retained earnings / total assets (X2).

        Returns
        -------
        float
            Retained earnings / total assets (X2).

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def ebit_to_total_assets(self) -> float:
        """
        EBIT / total assets (X3).

        Returns
        -------
        float
            EBIT / total assets (X3).

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def market_equity_to_total_liabilities(self) -> float:
        """
        Market value of equity / book value of total liabilities (X4).

        Returns
        -------
        float
            Market value of equity / book value of total liabilities (X4).

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def sales_to_total_assets(self) -> float:
        """
        Sales / total assets (X5).

        Returns
        -------
        float
            Sales / total assets (X5).

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @staticmethod
    def from_json(json: str) -> AltmanZScoreInput:
        """
        Deserialize a ``AltmanZScoreInput`` from its canonical JSON form.

        Parameters
        ----------
        json : str
            JSON text produced by :meth:`to_json` (strict serde; unknown or
            invalid fields are rejected).

        Returns
        -------
        AltmanZScoreInput
            Reconstructed value.

        Raises
        ------
        ValueError
            If the payload is not valid ``AltmanZScoreInput`` JSON.

        Examples
        --------
        >>> from finstack_quant.models.credit import scoring
        >>> value = scoring.AltmanZScoreInput(
        ...     working_capital_to_total_assets=0.2,
        ...     retained_earnings_to_total_assets=0.3,
        ...     ebit_to_total_assets=0.15,
        ...     market_equity_to_total_liabilities=1.5,
        ...     sales_to_total_assets=1.0,
        ... )
        >>> scoring.AltmanZScoreInput.from_json(value.to_json()) == value
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to compact canonical JSON.

        Returns
        -------
        str
            JSON text accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the canonical JSON representation.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(from_json, (json,))`` so unpickling rebuilds the value.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style representation rendered from the canonical fields.

        Returns
        -------
        str
            ``Name(field=value, ...)`` with Python literals.

        Notes
        -----
        This method does not raise.
        """
        ...

class AltmanZPrimeInput:
    """
    Input ratios of the Altman Z'-Score for private firms (book equity replaces market equity).

    Every field is a decimal ratio; Rust rejects a non-finite one when it is
    scored.

    Examples
    --------
    >>> from finstack_quant.models.credit import scoring
    >>> ratios = scoring.AltmanZPrimeInput(
    ...     working_capital_to_total_assets=0.2,
    ...     retained_earnings_to_total_assets=0.3,
    ...     ebit_to_total_assets=0.15,
    ...     book_equity_to_total_liabilities=1.5,
    ...     sales_to_total_assets=1.0,
    ... )
    >>> ratios.working_capital_to_total_assets
    0.2
    """

    def __init__(
        self,
        *,
        working_capital_to_total_assets: float,
        retained_earnings_to_total_assets: float,
        ebit_to_total_assets: float,
        book_equity_to_total_liabilities: float,
        sales_to_total_assets: float,
    ) -> None:
        """
        Build the input ratios by keyword.

        Parameters
        ----------
        working_capital_to_total_assets : float
            Working capital / total assets (X1).
        retained_earnings_to_total_assets : float
            Retained earnings / total assets (X2).
        ebit_to_total_assets : float
            EBIT / total assets (X3).
        book_equity_to_total_liabilities : float
            Book value of equity / book value of total liabilities (X4).
        sales_to_total_assets : float
            Sales / total assets (X5).

        Notes
        -----
        This constructor does not raise; the scoring function rejects a
        non-finite ratio with ``ValueError``.
        """
        ...

    @property
    def working_capital_to_total_assets(self) -> float:
        """
        Working capital / total assets (X1).

        Returns
        -------
        float
            Working capital / total assets (X1).

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def retained_earnings_to_total_assets(self) -> float:
        """
        Retained earnings / total assets (X2).

        Returns
        -------
        float
            Retained earnings / total assets (X2).

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def ebit_to_total_assets(self) -> float:
        """
        EBIT / total assets (X3).

        Returns
        -------
        float
            EBIT / total assets (X3).

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def book_equity_to_total_liabilities(self) -> float:
        """
        Book value of equity / book value of total liabilities (X4).

        Returns
        -------
        float
            Book value of equity / book value of total liabilities (X4).

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def sales_to_total_assets(self) -> float:
        """
        Sales / total assets (X5).

        Returns
        -------
        float
            Sales / total assets (X5).

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @staticmethod
    def from_json(json: str) -> AltmanZPrimeInput:
        """
        Deserialize a ``AltmanZPrimeInput`` from its canonical JSON form.

        Parameters
        ----------
        json : str
            JSON text produced by :meth:`to_json` (strict serde; unknown or
            invalid fields are rejected).

        Returns
        -------
        AltmanZPrimeInput
            Reconstructed value.

        Raises
        ------
        ValueError
            If the payload is not valid ``AltmanZPrimeInput`` JSON.

        Examples
        --------
        >>> from finstack_quant.models.credit import scoring
        >>> value = scoring.AltmanZPrimeInput(
        ...     working_capital_to_total_assets=0.2,
        ...     retained_earnings_to_total_assets=0.3,
        ...     ebit_to_total_assets=0.15,
        ...     book_equity_to_total_liabilities=1.5,
        ...     sales_to_total_assets=1.0,
        ... )
        >>> scoring.AltmanZPrimeInput.from_json(value.to_json()) == value
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to compact canonical JSON.

        Returns
        -------
        str
            JSON text accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the canonical JSON representation.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(from_json, (json,))`` so unpickling rebuilds the value.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style representation rendered from the canonical fields.

        Returns
        -------
        str
            ``Name(field=value, ...)`` with Python literals.

        Notes
        -----
        This method does not raise.
        """
        ...

class AltmanZDoublePrimeInput:
    """
    Input ratios of the Altman Z''-Score and EM-Score (no sales ratio).

    Every field is a decimal ratio; Rust rejects a non-finite one when it is
    scored.

    Examples
    --------
    >>> from finstack_quant.models.credit import scoring
    >>> ratios = scoring.AltmanZDoublePrimeInput(
    ...     working_capital_to_total_assets=0.2,
    ...     retained_earnings_to_total_assets=0.3,
    ...     ebit_to_total_assets=0.15,
    ...     book_equity_to_total_liabilities=1.5,
    ... )
    >>> ratios.working_capital_to_total_assets
    0.2
    """

    def __init__(
        self,
        *,
        working_capital_to_total_assets: float,
        retained_earnings_to_total_assets: float,
        ebit_to_total_assets: float,
        book_equity_to_total_liabilities: float,
    ) -> None:
        """
        Build the input ratios by keyword.

        Parameters
        ----------
        working_capital_to_total_assets : float
            Working capital / total assets (X1).
        retained_earnings_to_total_assets : float
            Retained earnings / total assets (X2).
        ebit_to_total_assets : float
            EBIT / total assets (X3).
        book_equity_to_total_liabilities : float
            Book value of equity / book value of total liabilities (X4).

        Notes
        -----
        This constructor does not raise; the scoring function rejects a
        non-finite ratio with ``ValueError``.
        """
        ...

    @property
    def working_capital_to_total_assets(self) -> float:
        """
        Working capital / total assets (X1).

        Returns
        -------
        float
            Working capital / total assets (X1).

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def retained_earnings_to_total_assets(self) -> float:
        """
        Retained earnings / total assets (X2).

        Returns
        -------
        float
            Retained earnings / total assets (X2).

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def ebit_to_total_assets(self) -> float:
        """
        EBIT / total assets (X3).

        Returns
        -------
        float
            EBIT / total assets (X3).

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def book_equity_to_total_liabilities(self) -> float:
        """
        Book value of equity / book value of total liabilities (X4).

        Returns
        -------
        float
            Book value of equity / book value of total liabilities (X4).

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @staticmethod
    def from_json(json: str) -> AltmanZDoublePrimeInput:
        """
        Deserialize a ``AltmanZDoublePrimeInput`` from its canonical JSON form.

        Parameters
        ----------
        json : str
            JSON text produced by :meth:`to_json` (strict serde; unknown or
            invalid fields are rejected).

        Returns
        -------
        AltmanZDoublePrimeInput
            Reconstructed value.

        Raises
        ------
        ValueError
            If the payload is not valid ``AltmanZDoublePrimeInput`` JSON.

        Examples
        --------
        >>> from finstack_quant.models.credit import scoring
        >>> value = scoring.AltmanZDoublePrimeInput(
        ...     working_capital_to_total_assets=0.2,
        ...     retained_earnings_to_total_assets=0.3,
        ...     ebit_to_total_assets=0.15,
        ...     book_equity_to_total_liabilities=1.5,
        ... )
        >>> scoring.AltmanZDoublePrimeInput.from_json(value.to_json()) == value
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to compact canonical JSON.

        Returns
        -------
        str
            JSON text accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the canonical JSON representation.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(from_json, (json,))`` so unpickling rebuilds the value.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style representation rendered from the canonical fields.

        Returns
        -------
        str
            ``Name(field=value, ...)`` with Python literals.

        Notes
        -----
        This method does not raise.
        """
        ...

class OhlsonOScoreInput:
    """
    Input predictors of the Ohlson O-Score (1980) logistic model.

    Every field is a decimal ratio; Rust rejects a non-finite one when it is
    scored.

    Examples
    --------
    >>> from finstack_quant.models.credit import scoring
    >>> ratios = scoring.OhlsonOScoreInput(
    ...     log_total_assets_adjusted=8.0,
    ...     total_liabilities_to_total_assets=0.4,
    ...     working_capital_to_total_assets=0.2,
    ...     current_liabilities_to_current_assets=0.5,
    ...     liabilities_exceed_assets=0.0,
    ...     net_income_to_total_assets=0.1,
    ...     funds_from_operations_to_total_liabilities=0.3,
    ...     negative_net_income_two_years=0.0,
    ...     net_income_change=0.1,
    ... )
    >>> ratios.log_total_assets_adjusted
    8.0
    """

    def __init__(
        self,
        *,
        log_total_assets_adjusted: float,
        total_liabilities_to_total_assets: float,
        working_capital_to_total_assets: float,
        current_liabilities_to_current_assets: float,
        liabilities_exceed_assets: float,
        net_income_to_total_assets: float,
        funds_from_operations_to_total_liabilities: float,
        negative_net_income_two_years: float,
        net_income_change: float,
    ) -> None:
        """
        Build the input ratios by keyword.

        Parameters
        ----------
        log_total_assets_adjusted : float
            ``log(total assets / GNP price-level index)`` on Ohlson's 1970s deflator scale (X1).
        total_liabilities_to_total_assets : float
            Total liabilities / total assets (X2).
        working_capital_to_total_assets : float
            Working capital / total assets (X3).
        current_liabilities_to_current_assets : float
            Current liabilities / current assets (X4).
        liabilities_exceed_assets : float
            Indicator, exactly ``1.0`` if total liabilities exceed total assets else ``0.0`` (X5).
        net_income_to_total_assets : float
            Net income / total assets (X6).
        funds_from_operations_to_total_liabilities : float
            Funds from operations / total liabilities (X7).
        negative_net_income_two_years : float
            Indicator, exactly ``1.0`` if net income was negative in each of the last two years else ``0.0`` (X8).
        net_income_change : float
            ``(NI_t - NI_t-1) / (|NI_t| + |NI_t-1|)`` (X9).

        Notes
        -----
        This constructor does not raise; the scoring function rejects a
        non-finite ratio with ``ValueError``.
        """
        ...

    @property
    def log_total_assets_adjusted(self) -> float:
        """
        ``log(total assets / GNP price-level index)`` on Ohlson's 1970s deflator scale (X1).

        Returns
        -------
        float
            ``log(total assets / GNP price-level index)`` on Ohlson's 1970s deflator scale (X1).

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def total_liabilities_to_total_assets(self) -> float:
        """
        Total liabilities / total assets (X2).

        Returns
        -------
        float
            Total liabilities / total assets (X2).

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def working_capital_to_total_assets(self) -> float:
        """
        Working capital / total assets (X3).

        Returns
        -------
        float
            Working capital / total assets (X3).

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def current_liabilities_to_current_assets(self) -> float:
        """
        Current liabilities / current assets (X4).

        Returns
        -------
        float
            Current liabilities / current assets (X4).

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def liabilities_exceed_assets(self) -> float:
        """
        Indicator, exactly ``1.0`` if total liabilities exceed total assets else ``0.0`` (X5).

        Returns
        -------
        float
            Indicator, exactly ``1.0`` if total liabilities exceed total assets else ``0.0`` (X5).

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def net_income_to_total_assets(self) -> float:
        """
        Net income / total assets (X6).

        Returns
        -------
        float
            Net income / total assets (X6).

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def funds_from_operations_to_total_liabilities(self) -> float:
        """
        Funds from operations / total liabilities (X7).

        Returns
        -------
        float
            Funds from operations / total liabilities (X7).

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def negative_net_income_two_years(self) -> float:
        """
        Indicator, exactly ``1.0`` if net income was negative in each of the last two years else ``0.0`` (X8).

        Returns
        -------
        float
            Indicator, exactly ``1.0`` if net income was negative in each of the last two years else ``0.0`` (X8).

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def net_income_change(self) -> float:
        """
        ``(NI_t - NI_t-1) / (|NI_t| + |NI_t-1|)`` (X9).

        Returns
        -------
        float
            ``(NI_t - NI_t-1) / (|NI_t| + |NI_t-1|)`` (X9).

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @staticmethod
    def from_json(json: str) -> OhlsonOScoreInput:
        """
        Deserialize a ``OhlsonOScoreInput`` from its canonical JSON form.

        Parameters
        ----------
        json : str
            JSON text produced by :meth:`to_json` (strict serde; unknown or
            invalid fields are rejected).

        Returns
        -------
        OhlsonOScoreInput
            Reconstructed value.

        Raises
        ------
        ValueError
            If the payload is not valid ``OhlsonOScoreInput`` JSON.

        Examples
        --------
        >>> from finstack_quant.models.credit import scoring
        >>> value = scoring.OhlsonOScoreInput(
        ...     log_total_assets_adjusted=8.0,
        ...     total_liabilities_to_total_assets=0.4,
        ...     working_capital_to_total_assets=0.2,
        ...     current_liabilities_to_current_assets=0.5,
        ...     liabilities_exceed_assets=0.0,
        ...     net_income_to_total_assets=0.1,
        ...     funds_from_operations_to_total_liabilities=0.3,
        ...     negative_net_income_two_years=0.0,
        ...     net_income_change=0.1,
        ... )
        >>> scoring.OhlsonOScoreInput.from_json(value.to_json()) == value
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to compact canonical JSON.

        Returns
        -------
        str
            JSON text accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the canonical JSON representation.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(from_json, (json,))`` so unpickling rebuilds the value.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style representation rendered from the canonical fields.

        Returns
        -------
        str
            ``Name(field=value, ...)`` with Python literals.

        Notes
        -----
        This method does not raise.
        """
        ...

class ZmijewskiInput:
    """
    Input ratios of the Zmijewski (1984) probit model.

    Every field is a decimal ratio; Rust rejects a non-finite one when it is
    scored.

    Examples
    --------
    >>> from finstack_quant.models.credit import scoring
    >>> ratios = scoring.ZmijewskiInput(
    ...     net_income_to_total_assets=0.05,
    ...     total_liabilities_to_total_assets=0.5,
    ...     current_assets_to_current_liabilities=1.5,
    ... )
    >>> ratios.net_income_to_total_assets
    0.05
    """

    def __init__(
        self,
        *,
        net_income_to_total_assets: float,
        total_liabilities_to_total_assets: float,
        current_assets_to_current_liabilities: float,
    ) -> None:
        """
        Build the input ratios by keyword.

        Parameters
        ----------
        net_income_to_total_assets : float
            Net income / total assets (ROA).
        total_liabilities_to_total_assets : float
            Total liabilities / total assets (debt ratio).
        current_assets_to_current_liabilities : float
            Current assets / current liabilities (current ratio).

        Notes
        -----
        This constructor does not raise; the scoring function rejects a
        non-finite ratio with ``ValueError``.
        """
        ...

    @property
    def net_income_to_total_assets(self) -> float:
        """
        Net income / total assets (ROA).

        Returns
        -------
        float
            Net income / total assets (ROA).

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def total_liabilities_to_total_assets(self) -> float:
        """
        Total liabilities / total assets (debt ratio).

        Returns
        -------
        float
            Total liabilities / total assets (debt ratio).

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def current_assets_to_current_liabilities(self) -> float:
        """
        Current assets / current liabilities (current ratio).

        Returns
        -------
        float
            Current assets / current liabilities (current ratio).

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @staticmethod
    def from_json(json: str) -> ZmijewskiInput:
        """
        Deserialize a ``ZmijewskiInput`` from its canonical JSON form.

        Parameters
        ----------
        json : str
            JSON text produced by :meth:`to_json` (strict serde; unknown or
            invalid fields are rejected).

        Returns
        -------
        ZmijewskiInput
            Reconstructed value.

        Raises
        ------
        ValueError
            If the payload is not valid ``ZmijewskiInput`` JSON.

        Examples
        --------
        >>> from finstack_quant.models.credit import scoring
        >>> value = scoring.ZmijewskiInput(
        ...     net_income_to_total_assets=0.05,
        ...     total_liabilities_to_total_assets=0.5,
        ...     current_assets_to_current_liabilities=1.5,
        ... )
        >>> scoring.ZmijewskiInput.from_json(value.to_json()) == value
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to compact canonical JSON.

        Returns
        -------
        str
            JSON text accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the canonical JSON representation.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(from_json, (json,))`` so unpickling rebuilds the value.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style representation rendered from the canonical fields.

        Returns
        -------
        str
            ``Name(field=value, ...)`` with Python literals.

        Notes
        -----
        This method does not raise.
        """
        ...

class ScoringResult:
    """
    Outcome of one academic credit-scoring model.

    Examples
    --------
    >>> from finstack_quant.models.credit import scoring
    >>> ratios = scoring.ZmijewskiInput(
    ...     net_income_to_total_assets=0.05,
    ...     total_liabilities_to_total_assets=0.5,
    ...     current_assets_to_current_liabilities=1.5,
    ... )
    >>> result = scoring.zmijewski_score(ratios)
    >>> (result.zone, result.implied_pd is not None, result.model)
    ('safe', True, 'Zmijewski Probit (1984)')
    """

    @property
    def score(self) -> float:
        """
        Raw score value (Z, Z', Z'', EM, O, or Zmijewski Y).

        Returns
        -------
        float
            Raw score value (Z, Z', Z'', EM, O, or Zmijewski Y).

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def zone(self) -> str:
        """
        Risk zone: ``"safe"``, ``"grey"`` or ``"distress"``.

        Returns
        -------
        str
            Risk zone: ``"safe"``, ``"grey"`` or ``"distress"``.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def implied_pd(self) -> float | None:
        """
        Native implied probability of default as a decimal, or ``None`` for the Altman family.

        Returns
        -------
        float | None
            Native implied probability of default as a decimal, or ``None`` for the Altman family.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def model(self) -> str:
        """
        Name of the model that produced this result.

        Returns
        -------
        str
            Name of the model that produced this result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @staticmethod
    def from_json(json: str) -> ScoringResult:
        """
        Deserialize a ``ScoringResult`` from its canonical JSON form.

        Parameters
        ----------
        json : str
            JSON text produced by :meth:`to_json` (strict serde; unknown or
            invalid fields are rejected).

        Returns
        -------
        ScoringResult
            Reconstructed value.

        Raises
        ------
        ValueError
            If the payload is not valid ``ScoringResult`` JSON.

        Examples
        --------
        >>> from finstack_quant.models.credit import scoring
        >>> ratios = scoring.ZmijewskiInput(
        ...     net_income_to_total_assets=0.05,
        ...     total_liabilities_to_total_assets=0.5,
        ...     current_assets_to_current_liabilities=1.5,
        ... )
        >>> value = scoring.zmijewski_score(ratios)
        >>> scoring.ScoringResult.from_json(value.to_json()) == value
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to compact canonical JSON.

        Returns
        -------
        str
            JSON text accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the canonical JSON representation.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(from_json, (json,))`` so unpickling rebuilds the value.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...
    def to_dataframe(self) -> pandas.DataFrame:
        """
        Single-row frame with ``model``, ``score``, ``zone``, ``implied_pd``.

        Returns
        -------
        pandas.DataFrame
            Single-row frame with ``model``, ``score``, ``zone``, ``implied_pd``.

        Raises
        ------
        ValueError
            If the value cannot be serialized into a pandas object.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style representation rendered from the canonical fields.

        Returns
        -------
        str
            ``Name(field=value, ...)`` with Python literals.

        Notes
        -----
        This method does not raise.
        """
        ...

class CreditScoreReport:
    """
    A scoring model's result and the default master-scale grade of its implied PD.

    Examples
    --------
    >>> from finstack_quant.models.credit import scoring
    >>> ratios = {"net_income_to_total_assets": 0.05, "total_liabilities_to_total_assets": 0.5}
    >>> ratios["current_assets_to_current_liabilities"] = 1.5
    >>> report = scoring.credit_score_report({"model": "zmijewski_score", "input": ratios})
    >>> (report.result.zone, report.grade is not None)
    ('safe', True)
    """

    @property
    def result(self) -> ScoringResult:
        """
        The scoring model's result.

        Returns
        -------
        ScoringResult
            The scoring model's result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @property
    def grade(self) -> MasterScaleResult | None:
        """
        Default master-scale grade of the implied PD, or ``None`` for the Altman family.

        Returns
        -------
        MasterScaleResult | None
            Default master-scale grade of the implied PD, or ``None`` when the
            model has no native implied PD (the Altman family).

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
    @staticmethod
    def from_json(json: str) -> CreditScoreReport:
        """
        Deserialize a ``CreditScoreReport`` from its canonical JSON form.

        Parameters
        ----------
        json : str
            JSON text produced by :meth:`to_json` (strict serde; unknown or
            invalid fields are rejected).

        Returns
        -------
        CreditScoreReport
            Reconstructed value.

        Raises
        ------
        ValueError
            If the payload is not valid ``CreditScoreReport`` JSON.

        Examples
        --------
        >>> from finstack_quant.models.credit import scoring
        >>> ratios = {"net_income_to_total_assets": 0.05, "total_liabilities_to_total_assets": 0.5}
        >>> ratios["current_assets_to_current_liabilities"] = 1.5
        >>> value = scoring.credit_score_report({"model": "zmijewski_score", "input": ratios})
        >>> scoring.CreditScoreReport.from_json(value.to_json()) == value
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to compact canonical JSON.

        Returns
        -------
        str
            JSON text accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the canonical JSON representation.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(from_json, (json,))`` so unpickling rebuilds the value.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...
    def __repr__(self) -> str:
        """
        Python-style representation rendered from the canonical fields.

        Returns
        -------
        str
            ``Name(field=value, ...)`` with Python literals.

        Notes
        -----
        This method does not raise.
        """
        ...

def altman_z_score(input: AltmanZScoreInput) -> ScoringResult:
    """
    Original Altman Z-Score (1968) for publicly traded manufacturers.

    ``Z = 1.2 * X1 + 1.4 * X2 + 3.3 * X3 + 0.6 * X4 + 1.0 * X5``

    Zone cutoffs: Z > 2.99 safe, 1.81 <= Z <= 2.99 grey, Z < 1.81 distress.

    Parameters
    ----------
    input : AltmanZScoreInput
        The model's input ratios.

    Returns
    -------
    ScoringResult
        Raw score, zone (``"safe"`` / ``"grey"`` / ``"distress"``) and
        ``implied_pd``: ``None`` (calibrate score-to-PD separately).

    Raises
    ------
    ValueError
        If any ratio is non-finite.

    Examples
    --------
    >>> from finstack_quant.models.credit import scoring
    >>> ratios = scoring.AltmanZScoreInput(
    ...     working_capital_to_total_assets=0.2,
    ...     retained_earnings_to_total_assets=0.3,
    ...     ebit_to_total_assets=0.15,
    ...     market_equity_to_total_liabilities=1.5,
    ...     sales_to_total_assets=1.0,
    ... )
    >>> scoring.altman_z_score(ratios).zone
    'safe'
    """
    ...

def altman_z_prime(input: AltmanZPrimeInput) -> ScoringResult:
    """
    Altman Z'-Score for private firms.

    ``Z' = 0.717 * X1 + 0.847 * X2 + 3.107 * X3 + 0.420 * X4 + 0.998 * X5``

    Zone cutoffs: Z' > 2.90 safe, 1.23 <= Z' <= 2.90 grey, Z' < 1.23 distress.

    Parameters
    ----------
    input : AltmanZPrimeInput
        The model's input ratios.

    Returns
    -------
    ScoringResult
        Raw score, zone (``"safe"`` / ``"grey"`` / ``"distress"``) and
        ``implied_pd``: ``None``.

    Raises
    ------
    ValueError
        If any ratio is non-finite.

    Examples
    --------
    >>> from finstack_quant.models.credit import scoring
    >>> ratios = scoring.AltmanZPrimeInput(
    ...     working_capital_to_total_assets=0.2,
    ...     retained_earnings_to_total_assets=0.3,
    ...     ebit_to_total_assets=0.15,
    ...     book_equity_to_total_liabilities=1.5,
    ...     sales_to_total_assets=1.0,
    ... )
    >>> scoring.altman_z_prime(ratios).zone
    'grey'
    """
    ...

def altman_z_double_prime(input: AltmanZDoublePrimeInput) -> ScoringResult:
    """
    Altman Z''-Score for non-manufacturing firms (the emerging-market variant with the +3.25 constant is ``altman_em_score``).

    ``Z'' = 6.56 * X1 + 3.26 * X2 + 6.72 * X3 + 1.05 * X4``

    Zone cutoffs: Z'' > 2.60 safe, 1.10 <= Z'' <= 2.60 grey, Z'' < 1.10 distress.

    Parameters
    ----------
    input : AltmanZDoublePrimeInput
        The model's input ratios.

    Returns
    -------
    ScoringResult
        Raw score, zone (``"safe"`` / ``"grey"`` / ``"distress"``) and
        ``implied_pd``: ``None``.

    Raises
    ------
    ValueError
        If any ratio is non-finite.

    Examples
    --------
    >>> from finstack_quant.models.credit import scoring
    >>> ratios = scoring.AltmanZDoublePrimeInput(
    ...     working_capital_to_total_assets=0.2,
    ...     retained_earnings_to_total_assets=0.3,
    ...     ebit_to_total_assets=0.15,
    ...     book_equity_to_total_liabilities=1.5,
    ... )
    >>> scoring.altman_z_double_prime(ratios).zone
    'safe'
    """
    ...

def altman_em_score(input: AltmanZDoublePrimeInput) -> ScoringResult:
    """
    Altman EM-Score for emerging-market corporates (Altman, Hartzell & Peck 1995).

    ``EM = 3.25 + 6.56 * X1 + 3.26 * X2 + 6.72 * X3 + 1.05 * X4``

    Zone cutoffs: EM > 5.85 safe, 4.35 <= EM <= 5.85 grey, EM < 4.35 distress.

    Parameters
    ----------
    input : AltmanZDoublePrimeInput
        The model's input ratios.

    Returns
    -------
    ScoringResult
        Raw score, zone (``"safe"`` / ``"grey"`` / ``"distress"``) and
        ``implied_pd``: ``None``.

    Raises
    ------
    ValueError
        If any ratio is non-finite.

    Examples
    --------
    >>> from finstack_quant.models.credit import scoring
    >>> ratios = scoring.AltmanZDoublePrimeInput(
    ...     working_capital_to_total_assets=0.2,
    ...     retained_earnings_to_total_assets=0.3,
    ...     ebit_to_total_assets=0.15,
    ...     book_equity_to_total_liabilities=1.5,
    ... )
    >>> scoring.altman_em_score(ratios).zone
    'safe'
    """
    ...

def ohlson_o_score(input: OhlsonOScoreInput) -> ScoringResult:
    """
    Ohlson O-Score (1980) nine-predictor logistic bankruptcy model.

    ``O = -1.32 - 0.407 * X1 + 6.03 * X2 - 1.43 * X3 + 0.0757 * X4 - 1.72 * X5 - 2.37 * X6 - 1.83 * X7 + 0.285 * X8 - 0.521 * X9; PD = 1 / (1 + exp(-O))``

    Zone cutoffs: PD < 0.019 safe, 0.019 <= PD <= 0.038 grey, PD > 0.038 distress.

    Parameters
    ----------
    input : OhlsonOScoreInput
        The model's input ratios.

    Returns
    -------
    ScoringResult
        Raw score, zone (``"safe"`` / ``"grey"`` / ``"distress"``) and
        ``implied_pd``: the logistic probability.

    Raises
    ------
    ValueError
        If any ratio is non-finite or an indicator is not exactly 0 or 1.

    Examples
    --------
    >>> from finstack_quant.models.credit import scoring
    >>> ratios = scoring.OhlsonOScoreInput(
    ...     log_total_assets_adjusted=8.0,
    ...     total_liabilities_to_total_assets=0.4,
    ...     working_capital_to_total_assets=0.2,
    ...     current_liabilities_to_current_assets=0.5,
    ...     liabilities_exceed_assets=0.0,
    ...     net_income_to_total_assets=0.1,
    ...     funds_from_operations_to_total_liabilities=0.3,
    ...     negative_net_income_two_years=0.0,
    ...     net_income_change=0.1,
    ... )
    >>> scoring.ohlson_o_score(ratios).zone
    'grey'
    """
    ...

def zmijewski_score(input: ZmijewskiInput) -> ScoringResult:
    """
    Zmijewski (1984) probit bankruptcy score.

    ``Y = -4.336 - 4.513 * ROA + 5.679 * DebtRatio + 0.004 * CurrentRatio; PD = Phi(Y)``

    Zone cutoffs: PD < 0.10 safe, 0.10 <= PD <= 0.50 grey, PD > 0.50 distress.

    Parameters
    ----------
    input : ZmijewskiInput
        The model's input ratios.

    Returns
    -------
    ScoringResult
        Raw score, zone (``"safe"`` / ``"grey"`` / ``"distress"``) and
        ``implied_pd``: the probit probability.

    Raises
    ------
    ValueError
        If any ratio is non-finite.

    Examples
    --------
    >>> from finstack_quant.models.credit import scoring
    >>> ratios = scoring.ZmijewskiInput(
    ...     net_income_to_total_assets=0.05,
    ...     total_liabilities_to_total_assets=0.5,
    ...     current_assets_to_current_liabilities=1.5,
    ... )
    >>> scoring.zmijewski_score(ratios).zone
    'safe'
    """
    ...

def credit_score_report(request: str | dict[str, Any]) -> CreditScoreReport:
    """
    Score one model and grade its implied PD on the default PD master scale.

    The implied PD of a model that has one (Ohlson, Zmijewski) is graded on the
    default PD master scale, so every host gets the same grade; the Altman
    family has no implied PD and no grade.

    Parameters
    ----------
    request : str | dict[str, Any]
        ``CreditScoreRequest`` as a dict or JSON text: ``model`` (one of
        ``"altman_z_score"``, ``"altman_z_prime"``, ``"altman_z_double_prime"``,
        ``"altman_em_score"``, ``"ohlson_o_score"``, ``"zmijewski_score"``) and
        ``input`` (that model's input ratios by Rust field name, decimals;
        ``"altman_em_score"`` takes the Z''-Score ratios).

    Returns
    -------
    CreditScoreReport
        The model's result and the default master-scale grade of its implied PD.

    Raises
    ------
    ValueError
        If the request is malformed or has an unknown model or field, a ratio is
        non-finite, or an Ohlson indicator is not exactly 0 or 1.

    Examples
    --------
    >>> from finstack_quant.models.credit import scoring
    >>> ratios = {"net_income_to_total_assets": 0.05, "total_liabilities_to_total_assets": 0.5}
    >>> ratios["current_assets_to_current_liabilities"] = 1.5
    >>> report = scoring.credit_score_report({"model": "zmijewski_score", "input": ratios})
    >>> report.grade is not None
    True
    """
    ...
