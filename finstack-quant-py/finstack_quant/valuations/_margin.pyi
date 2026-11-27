"""SIMM for one selected set of instruments."""

from collections.abc import Sequence
from datetime import date

from finstack_quant.core.currency import Currency
from finstack_quant.core.market_data import MarketContext
from finstack_quant.margin import ImResult
from finstack_quant.valuations.instruments._pricing import _InstrumentInput

def calculate_simm(
    instruments: Sequence[_InstrumentInput],
    market: MarketContext,
    as_of: date | str,
    *,
    version: str | None = None,
    mpor_days: int | None = None,
    currency: Currency | str | None = None,
) -> ImResult:
    """Compute model SIMM for a set of trades already selected for netting.

    Each trade must price successfully at the valuation date. The result
    carries the approximation classification of the product estimators.

    Parameters
    ----------
    instruments : Sequence[_InstrumentInput]
        Typed instruments or canonical instrument envelopes from the same
        netting set. Each trade contributes at its own notional and direction.
    market : MarketContext
        Market snapshot containing each trade's risk
        factors, including any referenced curves and fixings.
    as_of : date | str
        Valuation date supplied as a Python date or ISO-8601 date text.
    version : str | None
        Registered SIMM version label such as ``"v2_6"``; None uses the
        registered default.
    mpor_days : int | None
        Positive margin period of risk in business days; None uses the rule
        set's default.
    currency : Currency | str | None
        USD calculation currency as a Currency or ISO code; None uses USD.
        Trade sensitivities must already be expressed in this currency.

    Returns
    -------
    ImResult
        Initial margin in USD with its risk breakdown, date, methodology and
        approximation flag. Thresholds and posted collateral are handled by the CSA.

    Raises
    ------
    TypeError
        If an input has the wrong Python type.
    ValueError
        If the netting set is empty, a contract does not support SIMM, or a
        date, contract, version, currency or margin period is invalid.
    KeyError
        If a required market factor is missing.
    RuntimeError
        If trade sensitivities or margin cannot be computed.
    OverflowError
        If mpor_days cannot be represented as an unsigned 32-bit integer.

    Examples
    --------
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.valuations import calculate_simm
    >>> try:
    ...     calculate_simm([], MarketContext(), "2025-01-15")
    ... except ValueError as error:
    ...     print("empty" in str(error))
    True
    """
    ...
