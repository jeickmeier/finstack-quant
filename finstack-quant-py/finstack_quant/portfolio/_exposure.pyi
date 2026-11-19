"""Dated Philox exposure authoring and repricing over an admitted book."""

from __future__ import annotations

from collections.abc import Sequence
from datetime import date
from typing import Literal, NotRequired, TypedDict

from finstack_quant.core.config import FinstackConfig
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.money import Money
from finstack_quant.margin import CsaSpec, ExposureProfile, NettingSetId
from finstack_quant.portfolio._container import Portfolio

class _RateExposureFactor(TypedDict):
    kind: Literal["rate"]
    state: int
    curve_kind: Literal["discount", "forward"]
    curve_id: str
    discount_curve_id: NotRequired[str | None]

class _FxExposureFactor(TypedDict):
    kind: Literal["fx"]
    state: int
    base: str
    quote: str

class _EquityExposureFactor(TypedDict):
    kind: Literal["equity"]
    state: int
    price_id: str

class ExposureCashVm:
    """Cash variation-margin terms and the settled opening balance.

    Examples
    --------
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.margin import CsaSpec
    >>> from finstack_quant.portfolio import ExposureCashVm
    >>> vm = ExposureCashVm(terms=CsaSpec.usd_regulatory(), opening_settled=Money(0, "USD"))
    """

    def __init__(self, *, terms: CsaSpec, opening_settled: Money) -> None:
        """Supply cash terms before the first scheduled path observation.

        Parameters
        ----------
        terms : CsaSpec
            Admitted bilateral agreement with recurring calls, same-currency
            eligible cash, settlement lag and a collateral interest curve.
        opening_settled : Money
            Opening cash balance in the agreement currency. Positive means
            held and negative means posted. The first observation precedes
            any modeled settlement from future calls.

        Raises
        ------
        TypeError
            If an argument is not the expected native wrapper.
        """

class ExposureSimulationConfig:
    """Authored dated paths and market-factor mappings for one named netting set.

    Examples
    --------
    >>> from finstack_quant.margin import NettingSetId
    >>> from finstack_quant.portfolio import ExposureSimulationConfig
    >>> spec = ExposureSimulationConfig(
    ...     netting_set=NettingSetId.bilateral("DEALER", "AGREEMENT"),
    ...     netting_source_id="admitted-trades",
    ...     position_ids=["equity"],
    ...     dates=["2025-01-02", "2025-02-01"],
    ...     factors=[{"kind": "equity", "state": 0, "price_id": "AAPL-SPOT"}],
    ...     simulation={
    ...         "process": {"type": "gbm", "r": 0.03, "q": 0.0, "sigma": 0.2},
    ...         "initial_state": [100.0],
    ...         "time_grid": {"type": "times", "times": [0.0, 30 / 365]},
    ...         "num_paths": 4,
    ...         "seed": 22,
    ...     },
    ... )
    """

    def __init__(
        self,
        *,
        netting_set: NettingSetId,
        netting_source_id: str,
        position_ids: Sequence[str],
        dates: Sequence[date | str],
        factors: Sequence[_RateExposureFactor | _FxExposureFactor | _EquityExposureFactor],
        simulation: dict[str, object],
        cash_vm: ExposureCashVm | None = None,
    ) -> None:
        """Bind the market risk and the contractual scope of a simulation.

        Parameters
        ----------
        netting_set : NettingSetId
            Bilateral counterparty and agreement for every supplied position.
        netting_source_id : str
            Nonempty evidence identifier for the admitted scope and membership.
        position_ids : sequence of str
            Distinct position identifiers that must match the book exactly.
        dates : sequence of date or str
            Strictly increasing calendar dates starting at valuation.
        factors : sequence
            Exact market targets keyed by the process state index. Rates are
            successive moves in annual decimal rates. Equity and FX use
            positive market spot levels. FX units are quote per base currency.
        simulation : dict
            Canonical PathSimulationSpec with an explicit ACT/365 Fixed grid,
            opening state, process, stream count and exact seed.
        cash_vm : ExposureCashVm or None
            Elected cash variation-margin terms. None selects uncollateralized
            exposure. The grid must contain each scheduled call and settlement
            through the final observation when cash terms are supplied.

        Raises
        ------
        TypeError
            If a wrapper or sequence is not the expected Python type.
        ValueError
            If an observation date, structured factor or simulation cannot be decoded.
        """

def portfolio_exposure_profile(
    portfolio: Portfolio,
    market: MarketContext,
    pricing: FinstackConfig,
    spec: ExposureSimulationConfig,
) -> ExposureProfile:
    """Estimate expected exposure from signed book repricing at each path date.

    Parameters
    ----------
    portfolio : Portfolio
        Admitted book with signed quantities, valuation date and reporting currency.
    market : MarketContext
        Immutable opening prices, curves, fixings and FX for the whole book.
    pricing : FinstackConfig
        Valuation and conversion policy used for every trial market.
    spec : ExposureSimulationConfig
        Calendar observation grid, exact risk factor mappings, and eligible netting
        and optional cash collateral terms.

    Returns
    -------
    ExposureProfile
        Gross MtM, expected positive and negative exposure and complete path
        counts, source fingerprint and reporting currency. Expected exposures
        reflect settled collateral if cash VM is selected.

    Raises
    ------
    PortfolioError
        If a path, scenario, quote identity or netting selection fails.
    KeyError
        If a required valuation observation is missing.
    ValueError
        If the factor basis or calendar grid cannot be admitted.
    RuntimeError
        If instrumental pricing or currency conversion fails.

    Examples
    --------
    >>> from finstack_quant.core.config import FinstackConfig
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.margin import NettingSetId
    >>> from finstack_quant.portfolio import ExposureSimulationConfig, Portfolio, portfolio_exposure_profile
    >>> from finstack_quant.valuations.instruments import Equity
    >>> market = MarketContext()
    >>> _ = market.insert_price("AAPL-SPOT", 100.0, "USD")
    >>> book = Portfolio.builder("short", "USD", "2025-01-02").position("short", Equity.example(), -1).build()
    >>> spec = ExposureSimulationConfig(
    ...     netting_set=NettingSetId.bilateral("DEALER", "AGREEMENT"),
    ...     netting_source_id="approval",
    ...     position_ids=["short"],
    ...     dates=["2025-01-02", "2025-02-01"],
    ...     factors=[{"kind": "equity", "state": 0, "price_id": "AAPL-SPOT"}],
    ...     simulation={
    ...         "process": {"type": "gbm", "r": 0.0, "q": 0.0, "sigma": 0.0},
    ...         "initial_state": [100.0],
    ...         "time_grid": {"type": "times", "times": [0.0, 30 / 365]},
    ...         "num_paths": 4,
    ...         "seed": 22,
    ...     },
    ... )
    >>> portfolio_exposure_profile(book, market, FinstackConfig(), spec).epe
    [0.0, 0.0]
    """

def instrument_exposure_profile(
    instrument: object,
    currency: str,
    market: MarketContext,
    pricing: FinstackConfig,
    spec: ExposureSimulationConfig,
) -> ExposureProfile:
    """Estimate dated expected exposure at one instrument's contractual size.

    Parameters
    ----------
    instrument : object
        Typed Finstack instrument or validated envelope to reprice as one position.
    currency : str
        ISO 4217 code of the selected reporting currency.
    market : MarketContext
        Immutable opening market and pricing observations.
    pricing : FinstackConfig
        Valuation and conversion policy used on every path.
    spec : ExposureSimulationConfig
        Complete dated path request. Its single position ID must equal the instrument ID.

    Returns
    -------
    ExposureProfile
        Complete dated means and exposure moments in the reporting currency.

    Raises
    ------
    TypeError
        If the host instrument does not expose the required representation.
    PortfolioError
        If valuation of the admitted instrument or its netting scope fails.
    KeyError
        If a pricing observation is missing.
    ValueError
        If the supplied instrument, currency or simulation cannot be admitted.
    RuntimeError
        If pricing or model calculations fail.

    Examples
    --------
    >>> from finstack_quant.core.config import FinstackConfig
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.margin import NettingSetId
    >>> from finstack_quant.portfolio import ExposureSimulationConfig, instrument_exposure_profile
    >>> from finstack_quant.valuations.instruments import Equity
    >>> instrument = Equity.example()
    >>> market = MarketContext()
    >>> _ = market.insert_price("AAPL-SPOT", 100.0, "USD")
    >>> spec = ExposureSimulationConfig(
    ...     netting_set=NettingSetId.bilateral("DEALER", "AGREEMENT"),
    ...     netting_source_id="approval",
    ...     position_ids=[instrument.id],
    ...     dates=["2025-01-02", "2025-02-01"],
    ...     factors=[{"kind": "equity", "state": 0, "price_id": "AAPL-SPOT"}],
    ...     simulation={
    ...         "process": {"type": "gbm", "r": 0.0, "q": 0.0, "sigma": 0.0},
    ...         "initial_state": [100.0],
    ...         "time_grid": {"type": "times", "times": [0.0, 30 / 365]},
    ...         "num_paths": 4,
    ...         "seed": 22,
    ...     },
    ... )
    >>> instrument_exposure_profile(instrument, "USD", market, FinstackConfig(), spec).ene
    [0.0, 0.0]
    """
