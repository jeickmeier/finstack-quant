"""
Academic credit scoring: Altman Z-Score family, Ohlson O-Score, Zmijewski.

Every model returns a :class:`ScoringResult`; feed one with an
``implied_pd`` (Ohlson, Zmijewski) to ``pd.MasterScale.map_score``.

Examples
--------
>>> from finstack_quant.models.credit import scoring
>>> round(scoring.altman_z_score(0.2, 0.3, 0.15, 1.5, 1.0).score, 3)
3.055

"""

from __future__ import annotations

from typing import Any

import pandas

__all__ = [
    "ScoringResult",
    "altman_em_score",
    "altman_z_double_prime",
    "altman_z_prime",
    "altman_z_score",
    "ohlson_o_score",
    "zmijewski_score",
]

class ScoringResult:
    """
    Outcome of one academic credit-scoring model.

    Examples
    --------
    >>> from finstack_quant.models.credit import scoring
    >>> result = scoring.zmijewski_score(0.05, 0.5, 1.5)
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
        >>> value = scoring.zmijewski_score(0.05, 0.5, 1.5)
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

def altman_z_score(
    working_capital_to_total_assets: float,
    retained_earnings_to_total_assets: float,
    ebit_to_total_assets: float,
    market_equity_to_total_liabilities: float,
    sales_to_total_assets: float,
) -> ScoringResult:
    """
    Original Altman Z-Score (1968) for publicly traded manufacturers.

    ``Z = 1.2 * X1 + 1.4 * X2 + 3.3 * X3 + 0.6 * X4 + 1.0 * X5``

    Zone cutoffs: Z > 2.99 safe, 1.81 <= Z <= 2.99 grey, Z < 1.81 distress.

    Parameters
    ----------
    working_capital_to_total_assets : float
        Working capital / total assets (X1).
    retained_earnings_to_total_assets : float
        Retained earnings / total assets (X2).
    ebit_to_total_assets : float
        EBIT / total assets (X3).
    market_equity_to_total_liabilities : float
        Market value of equity / total liabilities (X4).
    sales_to_total_assets : float
        Sales / total assets (X5).

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
    >>> scoring.altman_z_score(0.2, 0.3, 0.15, 1.5, 1.0).zone
    'safe'
    """
    ...

def altman_z_prime(
    working_capital_to_total_assets: float,
    retained_earnings_to_total_assets: float,
    ebit_to_total_assets: float,
    book_equity_to_total_liabilities: float,
    sales_to_total_assets: float,
) -> ScoringResult:
    """
    Altman Z'-Score for private firms.

    ``Z' = 0.717 * X1 + 0.847 * X2 + 3.107 * X3 + 0.420 * X4 + 0.998 * X5``

    Zone cutoffs: Z' > 2.90 safe, 1.23 <= Z' <= 2.90 grey, Z' < 1.23 distress.

    Parameters
    ----------
    working_capital_to_total_assets : float
        Working capital / total assets (X1).
    retained_earnings_to_total_assets : float
        Retained earnings / total assets (X2).
    ebit_to_total_assets : float
        EBIT / total assets (X3).
    book_equity_to_total_liabilities : float
        Book value of equity / total liabilities (X4).
    sales_to_total_assets : float
        Sales / total assets (X5).

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
    >>> scoring.altman_z_prime(0.2, 0.3, 0.15, 1.5, 1.0).zone
    'grey'
    """
    ...

def altman_z_double_prime(
    working_capital_to_total_assets: float,
    retained_earnings_to_total_assets: float,
    ebit_to_total_assets: float,
    book_equity_to_total_liabilities: float,
) -> ScoringResult:
    """
    Altman Z''-Score for non-manufacturing firms (the emerging-market variant with the +3.25 constant is ``altman_em_score``).

    ``Z'' = 6.56 * X1 + 3.26 * X2 + 6.72 * X3 + 1.05 * X4``

    Zone cutoffs: Z'' > 2.60 safe, 1.10 <= Z'' <= 2.60 grey, Z'' < 1.10 distress.

    Parameters
    ----------
    working_capital_to_total_assets : float
        Working capital / total assets (X1).
    retained_earnings_to_total_assets : float
        Retained earnings / total assets (X2).
    ebit_to_total_assets : float
        EBIT / total assets (X3).
    book_equity_to_total_liabilities : float
        Book value of equity / total liabilities (X4).

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
    >>> scoring.altman_z_double_prime(0.2, 0.3, 0.15, 1.5).zone
    'safe'
    """
    ...

def altman_em_score(
    working_capital_to_total_assets: float,
    retained_earnings_to_total_assets: float,
    ebit_to_total_assets: float,
    book_equity_to_total_liabilities: float,
) -> ScoringResult:
    """
    Altman EM-Score for emerging-market corporates (Altman, Hartzell & Peck 1995).

    ``EM = 3.25 + 6.56 * X1 + 3.26 * X2 + 6.72 * X3 + 1.05 * X4``

    Zone cutoffs: EM > 5.85 safe, 4.35 <= EM <= 5.85 grey, EM < 4.35 distress.

    Parameters
    ----------
    working_capital_to_total_assets : float
        Working capital / total assets (X1).
    retained_earnings_to_total_assets : float
        Retained earnings / total assets (X2).
    ebit_to_total_assets : float
        EBIT / total assets (X3).
    book_equity_to_total_liabilities : float
        Book value of equity / total liabilities (X4).

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
    >>> scoring.altman_em_score(0.2, 0.3, 0.15, 1.5).zone
    'safe'
    """
    ...

def ohlson_o_score(
    log_total_assets_adjusted: float,
    total_liabilities_to_total_assets: float,
    working_capital_to_total_assets: float,
    current_liabilities_to_current_assets: float,
    liabilities_exceed_assets: float,
    net_income_to_total_assets: float,
    funds_from_operations_to_total_liabilities: float,
    negative_net_income_two_years: float,
    net_income_change: float,
) -> ScoringResult:
    """
    Ohlson O-Score (1980) nine-predictor logistic bankruptcy model.

    ``O = -1.32 - 0.407 * X1 + 6.03 * X2 - 1.43 * X3 + 0.0757 * X4 - 1.72 * X5 - 2.37 * X6 - 1.83 * X7 + 0.285 * X8 - 0.521 * X9; PD = 1 / (1 + exp(-O))``

    Zone cutoffs: PD < 0.019 safe, 0.019 <= PD <= 0.038 grey, PD > 0.038 distress.

    Parameters
    ----------
    log_total_assets_adjusted : float
        log(total assets / GNP price-level index) (X1).
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
        Indicator, exactly ``1.0`` if net income was negative in each of the last two years (X8).
    net_income_change : float
        ``(NI_t - NI_t-1) / (|NI_t| + |NI_t-1|)`` (X9).

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
    >>> scoring.ohlson_o_score(8.0, 0.4, 0.2, 0.5, 0.0, 0.1, 0.3, 0.0, 0.1).zone
    'grey'
    """
    ...

def zmijewski_score(
    net_income_to_total_assets: float,
    total_liabilities_to_total_assets: float,
    current_assets_to_current_liabilities: float,
) -> ScoringResult:
    """
    Zmijewski (1984) probit bankruptcy score.

    ``Y = -4.336 - 4.513 * ROA + 5.679 * DebtRatio + 0.004 * CurrentRatio; PD = Phi(Y)``

    Zone cutoffs: PD < 0.10 safe, 0.10 <= PD <= 0.50 grey, PD > 0.50 distress.

    Parameters
    ----------
    net_income_to_total_assets : float
        Net income / total assets (ROA).
    total_liabilities_to_total_assets : float
        Total liabilities / total assets (debt ratio).
    current_assets_to_current_liabilities : float
        Current assets / current liabilities (current ratio).

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
    >>> scoring.zmijewski_score(0.05, 0.5, 1.5).zone
    'safe'
    """
    ...
