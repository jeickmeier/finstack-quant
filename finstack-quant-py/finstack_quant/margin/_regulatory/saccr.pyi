"""Stub declarations mirroring ``finstack-quant-py/src/bindings/margin/regulatory/saccr.rs``."""

from __future__ import annotations

from typing import Literal, NotRequired, TypedDict, Unpack
import datetime
from typing import Any
import pandas as pd
import polars as pl
from finstack_quant.core.table import ArrowTable
from finstack_quant.margin._types.enums import NettingSetId

class SaCcrTrade:
    """
    A derivative trade for SA-CCR EAD computation per BCBS 279.

    Build with keyword arguments, ``from_json`` or ``from_dataframe``; all
    three validate the direction / supervisory-delta / option-type coherence
    up front. ``notional`` and ``mtm`` are in the netting set's reporting
    currency; ``direction`` is ``+1.0`` (long) or ``-1.0`` (short);
    ``supervisory_delta`` is in ``[-1, 1]`` (exactly ``±1`` for linear
    trades, the signed option delta otherwise).

    Parameters
    ----------
    trade_id : str
        Unique trade identifier.
    asset_class : str
        ``"interest_rate"``, ``"foreign_exchange"``, ``"credit"``,
        ``"equity"`` or ``"commodity"``.
    notional : float
        Reporting-currency notional before supervisory duration for IR/credit;
        CRE52 adjusted notional for other classes.
    start_date : datetime.date | str
        Trade start date (forward-start trades start after the valuation
        date).
    end_date : datetime.date | str
        Underlying end date; option expiry is supplied separately.
    underlier : str
        Underlier reference (currency pair, issuer, equity, commodity).
    hedging_set : str
        Hedging-set identifier within the asset class.
    direction : float
        ``+1.0`` long or ``-1.0`` short.
    supervisory_delta : float
        ``±1`` for linear trades; the signed option delta otherwise,
        sign-consistent with ``option_type`` (BCBS 279 ¶112).
    mtm : float
        Current mark-to-market in the reporting currency.
    is_option : bool
        Whether the trade is an option (required, as in the wire form).
    option_type : str
        ``"call_long"``, ``"call_short"``, ``"put_long"`` or
        ``"put_short"``; required when ``is_option`` is ``True``.

    supervisory_category : str
        Required for credit, equity and commodity; absent for IR/FX. Labels:
        ``credit_aa``, ``credit_a``, ``credit_bbb``, ``credit_bb``, ``credit_b``,
        ``credit_ccc``, ``credit_index_ig``, ``credit_index_hy``,
        ``equity_single_name``, ``equity_index``, ``commodity_electricity``,
        ``commodity_other``. Selects the supervisory factor and correlation.
    option_maturity_date : datetime.date | str
        Required option expiry, at or before the underlying end date. Absent
        for linear trades. Separates option maturity from underlying duration.

    Examples
    --------
    >>> from finstack_quant.margin import SaCcrTrade
    >>> trade = SaCcrTrade(
    ...     trade_id="t1",
    ...     asset_class="interest_rate",
    ...     notional=1_000_000,
    ...     start_date="2025-01-01",
    ...     end_date="2030-01-01",
    ...     underlier="USD-SOFR",
    ...     hedging_set="rates",
    ...     direction=1.0,
    ...     supervisory_delta=1.0,
    ...     mtm=0.0,
    ...     is_option=False,
    ... )
    >>> (trade.trade_id, trade.asset_class, trade.is_option)
    ('t1', 'interest_rate', False)
    """

    def __init__(
        self,
        *,
        trade_id: str,
        asset_class: str,
        notional: float,
        start_date: datetime.date | str,
        end_date: datetime.date | str,
        underlier: str,
        hedging_set: str,
        direction: float,
        supervisory_delta: float,
        mtm: float,
        is_option: bool,
        option_type: str | None = None,
        supervisory_category: str | None = None,
        option_maturity_date: datetime.date | str | None = None,
    ) -> None:
        """
        Create and validate a trade from keyword-only fields (see the class
        docstring for each field).

        Rust reads the keywords as the ``SaCcrTrade`` wire struct and rejects
        an unknown keyword.

        Parameters
        ----------
        trade_id : str
            Unique trade identifier.
        asset_class : str
            Lower-case SA-CCR asset class label.
        notional : float
            Reporting-currency notional before supervisory duration for IR/credit;
        CRE52 adjusted notional for other classes.
        start_date : datetime.date | str
            Trade start date (date-like or ISO ``YYYY-MM-DD``).
        end_date : datetime.date | str
            Trade end date / maturity (date-like or ISO ``YYYY-MM-DD``).
        underlier : str
            Underlier reference.
        hedging_set : str
            Hedging-set identifier within the asset class.
        direction : float
            ``+1.0`` long or ``-1.0`` short.
        supervisory_delta : float
            Supervisory delta in ``[-1, 1]``.
        mtm : float
            Current mark-to-market in the reporting currency.
        is_option : bool
            Whether the trade is an option.
        option_type : str | None, optional
            Option type label, required when ``is_option`` is ``True``.

        supervisory_category : str | None, optional
            Required for credit, equity and commodity; absent for IR/FX. Labels:
            ``credit_aa``, ``credit_a``, ``credit_bbb``, ``credit_bb``, ``credit_b``,
            ``credit_ccc``, ``credit_index_ig``, ``credit_index_hy``,
            ``equity_single_name``, ``equity_index``, ``commodity_electricity``,
            ``commodity_other``. Selects the supervisory factor and correlation.
        option_maturity_date : datetime.date | str
            Required option expiry, at or before the underlying end date. Absent
            for linear trades. Separates option maturity from underlying duration.

        Raises
        ------
        ValueError
            If a required field is missing, a keyword or label is unknown, a
            date string is not ISO 8601, or the trade fails the BCBS 279
            coherence checks (zero direction, delta outside ``[-1, 1]``,
            linear delta not ``±1`` or disagreeing with direction, option
            delta sign inconsistent with ``option_type``).
        TypeError
            If a positional argument is passed, or a value is not
            JSON-encodable.

        Examples
        --------
        >>> from finstack_quant.margin import SaCcrTrade
        >>> trade = SaCcrTrade(
        ...     trade_id="t1",
        ...     asset_class="interest_rate",
        ...     notional=1_000_000.0,
        ...     start_date="2025-01-01",
        ...     end_date="2030-01-01",
        ...     underlier="USD-SOFR",
        ...     hedging_set="rates",
        ...     direction=1.0,
        ...     supervisory_delta=1.0,
        ...     mtm=0.0,
        ...     is_option=False,
        ... )
        >>> trade.option_type is None
        True
        """
        ...

    @property
    def supervisory_category(self) -> str | None:
        """Supervisory factor and correlation category, absent for IR and FX.

        Returns
        -------
        str | None
            Supervisory factor and correlation category, absent for IR and FX.

        Notes
        -----
        This accessor does not raise; it returns the validated stored value.
        """
        ...

    @property
    def option_maturity_date(self) -> datetime.date | None:
        """Option expiry used for the maturity factor, absent for linear trades.

        Returns
        -------
        datetime.date | None
            Option expiry used for the maturity factor, absent for linear trades.

        Notes
        -----
        This accessor does not raise; it returns the validated stored value.
        """
        ...

    @staticmethod
    def from_dataframe(frame: pd.DataFrame | pl.DataFrame | Any) -> list[SaCcrTrade]:
        """
        Build one validated trade per row of a trade tape.

        Parameters
        ----------
        frame : pd.DataFrame | pl.DataFrame | Any
            Pandas ``DataFrame``, Polars ``DataFrame``, or Arrow table with
            columns ``trade_id``, ``asset_class``, ``notional``,
            ``start_date``, ``end_date``, ``underlier``, ``hedging_set``,
            ``direction``, ``supervisory_delta``, ``mtm``, ``is_option``;
            ``option_type``, ``supervisory_category`` and
            ``option_maturity_date`` may be null (``None``, ``NaN``, or
            ``NaT``). Dates may be ISO strings or date-like values.

        Returns
        -------
        list[SaCcrTrade]
            One trade per row, in row order.

        Raises
        ------
        ValueError
            Naming the first row that is missing a column, has an unknown
            label, or fails the BCBS 279 coherence checks.
        TypeError
            If ``frame`` is not a pandas/Polars DataFrame or Arrow table.

        Examples
        --------
        >>> trade = SaCcrTrade(
        ...     trade_id="t1",
        ...     asset_class="interest_rate",
        ...     notional=1_000_000,
        ...     start_date="2025-01-01",
        ...     end_date="2030-01-01",
        ...     underlier="USD-SOFR",
        ...     hedging_set="rates",
        ...     direction=1.0,
        ...     supervisory_delta=1.0,
        ...     mtm=0.0,
        ...     is_option=False,
        ... )
        >>> [t.trade_id for t in SaCcrTrade.from_dataframe(trade.to_arrow())]
        ['t1']
        """
        ...

    def to_arrow(self) -> ArrowTable:
        """
        Export the trade as a single-row :class:`~finstack_quant.core.table.ArrowTable`.

        Columns are the constructor fields in order (``trade_id`` ..
        ``option_type``); dates are ISO 8601 strings and ``option_type`` is
        null for linear trades. ``from_dataframe`` accepts this table back.

        Returns
        -------
        ArrowTable
            One row describing the trade.

        Raises
        ------
        ValueError
            If the trade cannot be serialized into an Arrow table.

        Examples
        --------
        >>> trade = SaCcrTrade(
        ...     trade_id="t1",
        ...     asset_class="interest_rate",
        ...     notional=1_000_000,
        ...     start_date="2025-01-01",
        ...     end_date="2030-01-01",
        ...     underlier="USD-SOFR",
        ...     hedging_set="rates",
        ...     direction=1.0,
        ...     supervisory_delta=1.0,
        ...     mtm=0.0,
        ...     is_option=False,
        ... )
        >>> trade.to_arrow().to_dict()["end_date"][0]
        '2030-01-01'
        """
        ...

    def to_pandas(self) -> pd.DataFrame:
        """
        Export the trade as a single-row pandas ``DataFrame``.

        Returns
        -------
        pd.DataFrame
            One row describing the trade.

        Raises
        ------
        ImportError
            If ``pandas`` is not installed.

        Examples
        --------
        >>> trade = SaCcrTrade(
        ...     trade_id="t1",
        ...     asset_class="interest_rate",
        ...     notional=1_000_000,
        ...     start_date="2025-01-01",
        ...     end_date="2030-01-01",
        ...     underlier="USD-SOFR",
        ...     hedging_set="rates",
        ...     direction=1.0,
        ...     supervisory_delta=1.0,
        ...     mtm=0.0,
        ...     is_option=False,
        ... )
        >>> trade.to_pandas().iloc[0]["end_date"]
        '2030-01-01'
        """
        ...

    def to_polars(self) -> pl.DataFrame:
        """
        Export the trade as a single-row Polars ``DataFrame``.

        Returns
        -------
        pl.DataFrame
            One row describing the trade.

        Raises
        ------
        ImportError
            If ``polars`` is not installed.
        """
        ...

    def to_pyarrow(self) -> Any:
        """
        Export the trade as a single-row ``pyarrow.Table``.

        Returns
        -------
        pyarrow.Table
            One row describing the trade.

        Raises
        ------
        ImportError
            If ``pyarrow`` is not installed.
        """
        ...

    def __arrow_c_stream__(self, requested_schema: object | None = None) -> object: ...
    def __arrow_c_schema__(self) -> object: ...
    def _repr_html_(self) -> str | None: ...
    @staticmethod
    def from_json(json: str) -> SaCcrTrade:
        """
        Construct from a JSON serialization.

        Parameters
        ----------
        json : str
            JSON string produced by ``to_json``.

        Returns
        -------
        SaCcrTrade
            Parsed trade.

        Raises
        ------
        ValueError
            If ``json`` is malformed, does not match the canonical schema, or
            violates direction, supervisory-delta, or option-type invariants.

        Examples
        --------
        >>> import json
        >>> from finstack_quant.margin import SaCcrTrade
        >>> payload = {
        ...     "trade_id": "t1",
        ...     "asset_class": "interest_rate",
        ...     "notional": 1_000_000,
        ...     "start_date": "2025-01-01",
        ...     "end_date": "2030-01-01",
        ...     "underlier": "USD-SOFR",
        ...     "hedging_set": "rates",
        ...     "direction": 1.0,
        ...     "supervisory_delta": 1.0,
        ...     "mtm": 0.0,
        ...     "is_option": False,
        ...     "option_type": None,
        ... }
        >>> SaCcrTrade.from_json(json.dumps(payload)).trade_id
        't1'
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to a JSON string.

        Returns
        -------
        str
            JSON serialization of the trade.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def trade_id(self) -> str:
        """
        Unique trade identifier.

        Returns
        -------
        str
            Trade id string.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def asset_class(self) -> str:
        """
        SA-CCR asset-class label used to select supervisory factors.

        Returns
        -------
        str
            One of ``"interest_rate"``, ``"foreign_exchange"``,
            ``"credit"``, ``"equity"``, ``"commodity"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def notional(self) -> float:
        """
        Adjusted notional in reporting currency.

        Returns
        -------
        float
            Notional amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def mtm(self) -> float:
        """
        Current mark-to-market value.

        Returns
        -------
        float
            MtM value in the reporting currency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def start_date(self) -> datetime.date:
        """
        Trade start date.

        Returns
        -------
        datetime.date
            Start date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> trade = SaCcrTrade(
        ...     trade_id="t1",
        ...     asset_class="interest_rate",
        ...     notional=1_000_000,
        ...     start_date="2025-01-01",
        ...     end_date="2030-01-01",
        ...     underlier="USD-SOFR",
        ...     hedging_set="rates",
        ...     direction=1.0,
        ...     supervisory_delta=1.0,
        ...     mtm=0.0,
        ...     is_option=False,
        ... )
        >>> trade.start_date
        datetime.date(2025, 1, 1)
        """
        ...

    @property
    def end_date(self) -> datetime.date:
        """
        Underlying end date; option expiry is supplied separately.

        Returns
        -------
        datetime.date
            End date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> trade = SaCcrTrade(
        ...     trade_id="t1",
        ...     asset_class="interest_rate",
        ...     notional=1_000_000,
        ...     start_date="2025-01-01",
        ...     end_date="2030-01-01",
        ...     underlier="USD-SOFR",
        ...     hedging_set="rates",
        ...     direction=1.0,
        ...     supervisory_delta=1.0,
        ...     mtm=0.0,
        ...     is_option=False,
        ... )
        >>> trade.end_date
        datetime.date(2030, 1, 1)
        """
        ...

    @property
    def underlier(self) -> str:
        """
        Underlier reference (currency pair, issuer, equity, commodity).

        Returns
        -------
        str
            Underlier identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> trade = SaCcrTrade(
        ...     trade_id="t1",
        ...     asset_class="interest_rate",
        ...     notional=1_000_000,
        ...     start_date="2025-01-01",
        ...     end_date="2030-01-01",
        ...     underlier="USD-SOFR",
        ...     hedging_set="rates",
        ...     direction=1.0,
        ...     supervisory_delta=1.0,
        ...     mtm=0.0,
        ...     is_option=False,
        ... )
        >>> trade.underlier
        'USD-SOFR'
        """
        ...

    @property
    def hedging_set(self) -> str:
        """
        Hedging-set identifier within the asset class.

        Returns
        -------
        str
            Hedging-set identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> trade = SaCcrTrade(
        ...     trade_id="t1",
        ...     asset_class="interest_rate",
        ...     notional=1_000_000,
        ...     start_date="2025-01-01",
        ...     end_date="2030-01-01",
        ...     underlier="USD-SOFR",
        ...     hedging_set="rates",
        ...     direction=1.0,
        ...     supervisory_delta=1.0,
        ...     mtm=0.0,
        ...     is_option=False,
        ... )
        >>> trade.hedging_set
        'rates'
        """
        ...

    @property
    def direction(self) -> float:
        """
        ``+1.0`` for long, ``-1.0`` for short.

        Returns
        -------
        float
            Signed direction.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> trade = SaCcrTrade(
        ...     trade_id="t1",
        ...     asset_class="interest_rate",
        ...     notional=1_000_000,
        ...     start_date="2025-01-01",
        ...     end_date="2030-01-01",
        ...     underlier="USD-SOFR",
        ...     hedging_set="rates",
        ...     direction=1.0,
        ...     supervisory_delta=1.0,
        ...     mtm=0.0,
        ...     is_option=False,
        ... )
        >>> trade.direction
        1.0
        """
        ...

    @property
    def supervisory_delta(self) -> float:
        """
        Supervisory delta in ``[-1, 1]``.

        Returns
        -------
        float
            Signed supervisory delta.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> trade = SaCcrTrade(
        ...     trade_id="t1",
        ...     asset_class="interest_rate",
        ...     notional=1_000_000,
        ...     start_date="2025-01-01",
        ...     end_date="2030-01-01",
        ...     underlier="USD-SOFR",
        ...     hedging_set="rates",
        ...     direction=1.0,
        ...     supervisory_delta=1.0,
        ...     mtm=0.0,
        ...     is_option=False,
        ... )
        >>> trade.supervisory_delta
        1.0
        """
        ...

    @property
    def is_option(self) -> bool:
        """
        Whether the trade is an option.

        Returns
        -------
        bool
            ``True`` for options.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> trade = SaCcrTrade(
        ...     trade_id="t1",
        ...     asset_class="interest_rate",
        ...     notional=1_000_000,
        ...     start_date="2025-01-01",
        ...     end_date="2030-01-01",
        ...     underlier="USD-SOFR",
        ...     hedging_set="rates",
        ...     direction=1.0,
        ...     supervisory_delta=1.0,
        ...     mtm=0.0,
        ...     is_option=False,
        ... )
        >>> trade.is_option
        False
        """
        ...

    @property
    def option_type(self) -> str | None:
        """
        Option type label, or ``None`` for a linear trade.

        Returns
        -------
        str | None
            ``"call_long"``, ``"call_short"``, ``"put_long"``, ``"put_short"`` or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> trade = SaCcrTrade(
        ...     trade_id="t1",
        ...     asset_class="interest_rate",
        ...     notional=1_000_000,
        ...     start_date="2025-01-01",
        ...     end_date="2030-01-01",
        ...     underlier="USD-SOFR",
        ...     hedging_set="rates",
        ...     direction=1.0,
        ...     supervisory_delta=1.0,
        ...     mtm=0.0,
        ...     is_option=False,
        ... )
        >>> trade.option_type is None
        True
        """
        ...

    def __repr__(self) -> str: ...

class SaCcrNettingSetConfig:
    """
    SA-CCR netting-set configuration with explicit valuation date.

    Collateral terms that select the margined or unmargined RC/PFE formulas.
    All amounts are in the netting set's reporting currency; ``mpor_days`` is
    the margin period of risk in business days. Keyed by a ``NettingSetId``
    (bilateral or cleared).

    Parameters
    ----------
    (Use the ``unmargined`` or ``margined`` factories.)

    Examples
    --------
    >>> from finstack_quant.margin import NettingSetId, SaCcrNettingSetConfig
    >>> config = SaCcrNettingSetConfig.unmargined(NettingSetId.bilateral("CPTY", "CSA"), 0.0, "2025-01-15")
    >>> config.is_margined
    False
    """

    @staticmethod
    def unmargined(
        netting_set_id: NettingSetId,
        collateral: float,
        as_of: datetime.date | str,
    ) -> SaCcrNettingSetConfig:
        """
        Create an unmargined netting-set configuration.

        Threshold, MTA and NICA are zero and ``mpor_days`` is the
        10-business-day bilateral default (only used for the reporting
        maturity factor).

        Parameters
        ----------
        netting_set_id : NettingSetId
            Bilateral (``NettingSetId.bilateral``) or cleared
            (``NettingSetId.cleared``) netting-set key.
        collateral : float
            Net variation margin held, excluding NICA (positive = bank holds
            collateral).
        as_of : datetime.date | str
            Valuation date for forward-start and remaining-maturity
            calculations (date-like or ISO ``YYYY-MM-DD``).

        Returns
        -------
        SaCcrNettingSetConfig
            Unmargined netting-set config.

        Raises
        ------
        ValueError
            If ``collateral`` is non-finite or a date string is not ISO 8601.
        TypeError
            If ``netting_set_id`` is not a ``NettingSetId`` or ``as_of`` is
            neither a string nor date-like.

        Examples
        --------
        >>> from finstack_quant.margin import NettingSetId, SaCcrNettingSetConfig
        >>> config = SaCcrNettingSetConfig.unmargined(NettingSetId.cleared("LCH"), 0.0, "2025-01-15")
        >>> (config.is_margined, config.netting_set_id.is_cleared)
        (False, True)
        """
        ...

    @staticmethod
    def margined(
        netting_set_id: NettingSetId,
        collateral: float,
        threshold: float,
        mta: float,
        nica: float,
        mpor_days: int,
        as_of: datetime.date | str,
    ) -> SaCcrNettingSetConfig:
        """
        Create a margined netting-set configuration.

        Parameters
        ----------
        netting_set_id : NettingSetId
            Bilateral or cleared netting-set key.
        collateral : float
            Net variation margin held, excluding NICA (positive = bank holds
            collateral).
        threshold : float
            CSA threshold (TH), non-negative.
        mta : float
            Minimum transfer amount, non-negative.
        nica : float
            Net independent collateral amount, signed.
        mpor_days : int
            Margin period of risk in business days; at least 10 bilateral or
            5 cleared. Supply any longer period required by liquidity,
            disputes, trade count or remargining frequency.
        as_of : datetime.date | str
            Valuation date for forward-start and remaining-maturity
            calculations.

        Returns
        -------
        SaCcrNettingSetConfig
            Margined netting-set config.

        Raises
        ------
        ValueError
            If an amount is non-finite, threshold or MTA is negative,
            ``mpor_days`` is below the applicable floor, or a date string is not ISO 8601.
        TypeError
            If ``netting_set_id`` is not a ``NettingSetId`` or ``as_of`` is
            neither a string nor date-like.

        Examples
        --------
        >>> from finstack_quant.margin import NettingSetId, SaCcrNettingSetConfig
        >>> config = SaCcrNettingSetConfig.margined(
        ...     NettingSetId.bilateral("CPTY", "CSA"),
        ...     collateral=0.0,
        ...     threshold=0.0,
        ...     mta=0.0,
        ...     nica=0.0,
        ...     mpor_days=10,
        ...     as_of="2025-01-15",
        ... )
        >>> (config.is_margined, config.mpor_days)
        (True, 10)
        """
        ...

    @staticmethod
    def from_json(json: str) -> SaCcrNettingSetConfig:
        """
        Construct from a JSON serialization.

        Parameters
        ----------
        json : str
            JSON string produced by ``to_json``.

        Returns
        -------
        SaCcrNettingSetConfig
            Parsed netting-set config.

        Raises
        ------
        ValueError
            If ``json`` is malformed or does not match the serialized
            ``SaCcrNettingSetConfig`` schema.

        Examples
        --------
        >>> from finstack_quant.margin import NettingSetId, SaCcrNettingSetConfig
        >>> original = SaCcrNettingSetConfig.unmargined(NettingSetId.bilateral("CPTY", "CSA"), 0.0, "2025-01-15")
        >>> SaCcrNettingSetConfig.from_json(original.to_json()).is_margined
        False
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to a JSON string.

        Returns
        -------
        str
            JSON serialization of the netting-set config.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def is_margined(self) -> bool:
        """
        Whether the netting set is margined.

        Returns
        -------
        bool
            True if subject to daily margin agreement.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def collateral(self) -> float:
        """
        Net variation margin currently held, excluding NICA.

        Returns
        -------
        float
            Collateral amount in the reporting currency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def as_of(self) -> datetime.date:
        """
        Valuation date used for forward-start and remaining-maturity calculations.

        Returns
        -------
        datetime.date
            Valuation date the netting set is priced at.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> config = SaCcrNettingSetConfig.margined(
        ...     NettingSetId.cleared("LCH"), 250_000.0, 100_000.0, 50_000.0, 0.0, 5, "2025-01-15"
        ... )
        >>> config.as_of
        datetime.date(2025, 1, 15)
        """
        ...

    @property
    def netting_set_id(self) -> NettingSetId:
        """
        Netting-set key.

        Returns
        -------
        NettingSetId
            Bilateral or cleared id.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> config = SaCcrNettingSetConfig.margined(
        ...     NettingSetId.cleared("LCH"), 250_000.0, 100_000.0, 50_000.0, 0.0, 5, "2025-01-15"
        ... )
        >>> config.netting_set_id.ccp_id
        'LCH'
        """
        ...

    @property
    def threshold(self) -> float:
        """
        CSA threshold (TH) in the reporting currency.

        Returns
        -------
        float
            Threshold amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> config = SaCcrNettingSetConfig.margined(
        ...     NettingSetId.cleared("LCH"), 250_000.0, 100_000.0, 50_000.0, 0.0, 5, "2025-01-15"
        ... )
        >>> config.threshold
        100000.0
        """
        ...

    @property
    def mta(self) -> float:
        """
        Minimum transfer amount in the reporting currency.

        Returns
        -------
        float
            MTA amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> config = SaCcrNettingSetConfig.margined(
        ...     NettingSetId.cleared("LCH"), 250_000.0, 100_000.0, 50_000.0, 0.0, 5, "2025-01-15"
        ... )
        >>> config.mta
        50000.0
        """
        ...

    @property
    def nica(self) -> float:
        """
        Net independent collateral amount in the reporting currency, held
        separately from ``collateral`` and added to it when forming ``C``.

        Returns
        -------
        float
            NICA amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> config = SaCcrNettingSetConfig.margined(
        ...     NettingSetId.cleared("LCH"), 250_000.0, 100_000.0, 50_000.0, 0.0, 5, "2025-01-15"
        ... )
        >>> config.nica
        0.0
        """
        ...

    @property
    def mpor_days(self) -> int:
        """
        Margin period of risk in business days.

        Returns
        -------
        int
            MPOR in business days.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> config = SaCcrNettingSetConfig.margined(
        ...     NettingSetId.cleared("LCH"), 250_000.0, 100_000.0, 50_000.0, 0.0, 5, "2025-01-15"
        ... )
        >>> config.mpor_days
        5
        """
        ...

    def validate(self) -> None:
        """
        Validate the collateral and margin-agreement terms.

        Returns
        -------
        None
            Returns ``None`` when the terms are valid.

        Raises
        ------
        ValueError
            If an amount is non-finite, threshold or MTA is negative, or a
            margined set has zero MPOR.

        Examples
        --------
        >>> config = SaCcrNettingSetConfig.margined(
        ...     NettingSetId.cleared("LCH"), 250_000.0, 100_000.0, 50_000.0, 0.0, 5, "2025-01-15"
        ... )
        >>> config.validate()
        """
        ...

    def __repr__(self) -> str: ...

class SaCcrEngine:
    """
    SA-CCR EAD engine matching the canonical Rust API.

    Parameters
    ----------
    alpha : float or None, optional
        Supervisory alpha factor; defaults to 1.4 when ``None``.

    Monetary values in a calculation must already use one consistent currency;
    the engine does not perform currency conversion.

    Examples
    --------
    >>> from finstack_quant.margin import NettingSetId, SaCcrEngine, SaCcrNettingSetConfig
    >>> config = SaCcrNettingSetConfig.unmargined(NettingSetId.bilateral("CPTY", "CSA"), 0.0, "2025-01-15")
    >>> SaCcrEngine().calculate_ead([], config).ead
    0.0
    """

    def __init__(self, alpha: float | None = None) -> None:
        """
        Configure the supervisory multiplier for SA-CCR.

        Parameters
        ----------
        alpha : float or None, default None
            Supervisory alpha multiplier; ``None`` uses the regulatory default
            of ``1.4`` and explicit values must be at least ``1.0``.

        Raises
        ------
        ValueError
            If a supplied ``alpha`` is non-finite or less than ``1.0``.
        """
        ...

    @property
    def alpha(self) -> float:
        """
        Alpha multiplier applied to ``RC + PFE``.

        Returns
        -------
        float
            1.4 unless overridden at construction.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> SaCcrEngine(alpha=1.5).alpha
        1.5
        """
        ...

    def calculate_ead(self, trades: list[SaCcrTrade], config: SaCcrNettingSetConfig) -> EadResult:
        """
        Calculate SA-CCR EAD for a netting set and trade list.

        Parameters
        ----------
        config : SaCcrNettingSetConfig
            Netting-set configuration with valuation date and collateral.
        trades : list[SaCcrTrade]
            Derivative trades in the netting set.

        Returns
        -------
        EadResult
            Exposure at default with replacement cost, PFE, the multiplier,
            the aggregate and per-asset-class add-ons, alpha, and the
            maturity factor.

        Raises
        ------
        ValueError
            If netting-set amounts or trade numeric fields are non-finite, a
            threshold or MTA is negative, a margined MPOR is zero, or a trade's
            direction and supervisory-delta fields are inconsistent.

        Examples
        --------
        >>> config = SaCcrNettingSetConfig.unmargined(NettingSetId.bilateral("CPTY", "CSA"), 0.0, "2025-01-01")
        >>> trade = SaCcrTrade(
        ...     trade_id="t1",
        ...     asset_class="interest_rate",
        ...     notional=1_000_000,
        ...     start_date="2025-01-01",
        ...     end_date="2030-01-01",
        ...     underlier="USD-SOFR",
        ...     hedging_set="rates",
        ...     direction=1.0,
        ...     supervisory_delta=1.0,
        ...     mtm=0.0,
        ...     is_option=False,
        ... )
        >>> round(SaCcrEngine().calculate_ead([trade], config).ead, 2)
        30982.83
        """
        ...

    def __repr__(self) -> str: ...

class EadResult:
    """
    SA-CCR Exposure at Default result (BCBS 279).

    Returned by :meth:`SaCcrEngine.calculate_ead`.
    ``pfe == multiplier * add_on_aggregate``; ``ead`` is ``alpha * (rc + pfe)``
    capped at the unmargined EAD for margined sets. RC and PFE remain the
    components before the cap;
    amounts are floats in the netting set's reporting currency.

    Examples
    --------
    >>> from finstack_quant.margin import NettingSetId, SaCcrEngine, SaCcrNettingSetConfig
    >>> config = SaCcrNettingSetConfig.unmargined(NettingSetId.bilateral("CPTY", "CSA"), 0.0, "2025-01-15")
    >>> SaCcrEngine().calculate_ead([], config).ead
    0.0
    """

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Support ``pickle`` via the ``to_json`` / ``from_json`` round-trip."""
        ...

    @staticmethod
    def from_json(json: str) -> EadResult:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``EadResult``.

        Returns
        -------
        EadResult
            The decoded result.

        Raises
        ------
        ValueError
            If ``json`` is not valid JSON for the ``EadResult`` shape.

        Examples
        --------
        >>> from finstack_quant.margin import EadResult
        >>> restored = EadResult.from_json(result.to_json())  # doctest: +SKIP
        >>> restored.to_json() == result.to_json()  # doctest: +SKIP
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize back to the same JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            JSON-encoded ``EadResult``.

        Notes
        -----
        This method does not raise; it derives the value from stored state.
        """
        ...

    @property
    def ead(self) -> float:
        """
        Exposure at Default after the unmargined cap for margined sets.

        Returns
        -------
        float
            Exposure at Default after the unmargined cap for margined sets.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def rc(self) -> float:
        """
        Replacement cost component.

        Returns
        -------
        float
            Replacement cost component.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def pfe(self) -> float:
        """
        Potential future exposure: ``multiplier * add_on_aggregate``.

        Returns
        -------
        float
            Potential future exposure: ``multiplier * add_on_aggregate``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def multiplier(self) -> float:
        """
        PFE multiplier recognising over-collateralization (floored at 0.05).

        Returns
        -------
        float
            PFE multiplier recognising over-collateralization (floored at 0.05).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def add_on_aggregate(self) -> float:
        """
        Aggregate add-on across asset classes, before the multiplier.

        Returns
        -------
        float
            Aggregate add-on across asset classes, before the multiplier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def alpha(self) -> float:
        """
        Alpha multiplier (1.4 per BCBS 279 unless overridden on the engine).

        Returns
        -------
        float
            Alpha multiplier (1.4 per BCBS 279 unless overridden on the engine).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def maturity_factor(self) -> float:
        """
        Maturity factor applied to the netting set.

        Returns
        -------
        float
            Maturity factor applied to the netting set.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def add_on_by_asset_class(self) -> dict[str, float]:
        """
        Add-on keyed by asset-class wire label (e.g. ``"interest_rate"``).

        Returns
        -------
        dict[str, float]
            Add-on keyed by asset-class wire label (e.g. ``"interest_rate"``).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def meta(self) -> dict[str, Any]:
        """
        Policy metadata stamped by the computing layer.

        Returns
        -------
        dict[str, Any]
            ``numeric_mode``, ``rounding`` (the active rounding context),
            ``fx_policy_applied`` (or ``None``), ``parallel`` and
            ``timestamp``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> config = SaCcrNettingSetConfig.unmargined(NettingSetId.bilateral("CPTY", "CSA"), 0.0, "2025-01-15")
        >>> result = SaCcrEngine().calculate_ead([], config)
        >>> "numeric_mode" in result.meta
        True
        """
        ...

    def to_arrow(self) -> ArrowTable:
        """
        Export the headline exposure as a single-row :class:`~finstack_quant.core.table.ArrowTable`.

        Returns
        -------
        ArrowTable
            Columns ``ead``, ``rc``, ``pfe``, ``multiplier``,
            ``add_on_aggregate``, ``alpha``, ``maturity_factor``. One row.

        Notes
        -----
        This method does not raise; it derives the value from stored state.
        """
        ...

    def to_pandas(self) -> pd.DataFrame:
        """
        Export the headline exposure as a single-row pandas ``DataFrame``.

        Returns
        -------
        pandas.DataFrame
            Columns ``ead``, ``rc``, ``pfe``, ``multiplier``,
            ``add_on_aggregate``, ``alpha``, ``maturity_factor``. One row.

        Raises
        ------
        ImportError
            If ``pandas`` is not installed.
        """
        ...

    def to_polars(self) -> pl.DataFrame:
        """
        Export the headline exposure as a single-row Polars ``DataFrame``.

        Returns
        -------
        polars.DataFrame
            One row describing the headline SA-CCR exposure.

        Raises
        ------
        ImportError
            If ``polars`` is not installed.
        """
        ...

    def to_pyarrow(self) -> Any:
        """
        Export the headline exposure as a single-row ``pyarrow.Table``.

        Returns
        -------
        pyarrow.Table
            One row describing the headline SA-CCR exposure.

        Raises
        ------
        ImportError
            If ``pyarrow`` is not installed.
        """
        ...

    def __arrow_c_stream__(self, requested_schema: object | None = None) -> object: ...
    def __arrow_c_schema__(self) -> object: ...
    def _repr_html_(self) -> str | None: ...
    def to_add_on_table(self) -> ArrowTable:
        """
        Export the per-asset-class add-on as an :class:`~finstack_quant.core.table.ArrowTable`.

        Returns
        -------
        ArrowTable
            Columns ``asset_class`` and ``add_on``, one row per asset class
            present. A netting set with no trades yields a zero-row table that
            still carries both columns with their real dtypes.

        Notes
        -----
        This method does not raise; it derives the value from stored state.
        """
        ...

    def __repr__(self) -> str:
        """
        Return ``repr(self)``.

        Returns
        -------
        str
            Return ``repr(self)``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
