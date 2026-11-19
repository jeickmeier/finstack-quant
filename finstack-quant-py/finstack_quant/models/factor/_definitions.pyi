"""Canonical factor definitions and declarative market mappings."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from typing import Any
from finstack_quant.core.currency import Currency

class MarketMapping:
    """Describe the market data moved by one factor coordinate.

    Examples
    --------
    >>> from finstack_quant.models.factor import MarketMapping
    >>> isinstance(MarketMapping.curve_parallel(["USD-OIS"], "rate_bp"), MarketMapping)
    True
    """

    @staticmethod
    def curve_parallel(curve_ids: Sequence[str], units: str) -> MarketMapping:
        """Move a set of curves by one parallel shift.

        Parameters
        ----------
        curve_ids : Sequence[str]
            Market curve IDs to move in parallel.
        units : str
            Native market shock unit such as rate_bp.

        Returns
        -------
        MarketMapping
            Factor-to-market binding embedded in a FactorDefinition.

        Raises
        ------
        TypeError
            If any argument has an incompatible Python type.
        ValueError
            If a currency or units label cannot be decoded where required.

        Examples
        --------
        >>> from finstack_quant.models.factor import MarketMapping
        >>> isinstance(MarketMapping.curve_parallel(["USD-OIS"], "rate_bp"), MarketMapping)
        True
        """
        ...

    @staticmethod
    def curve_bucketed(curve_id: str, tenor_weights: Sequence[tuple[float, float]]) -> MarketMapping:
        """Compose key-rate moves along a single curve.

        Parameters
        ----------
        curve_id : str
            Market curve ID targeted by the factor.
        tenor_weights : Sequence[tuple[float, float]]
            Year-fraction vertices and their signed bump weights.

        Returns
        -------
        MarketMapping
            Factor-to-market binding embedded in a FactorDefinition.

        Raises
        ------
        TypeError
            If any argument has an incompatible Python type.
        ValueError
            If a currency or units label cannot be decoded where required.

        Examples
        --------
        >>> from finstack_quant.models.factor import MarketMapping
        >>> isinstance(MarketMapping.curve_bucketed("USD-OIS", [(5.0, 1.0)]), MarketMapping)
        True
        """
        ...

    @staticmethod
    def equity_spot(tickers: Sequence[str]) -> MarketMapping:
        """Map a factor to equity spot moves.

        Parameters
        ----------
        tickers : Sequence[str]
            Market spot IDs receiving the same relative move.

        Returns
        -------
        MarketMapping
            Factor-to-market binding embedded in a FactorDefinition.

        Raises
        ------
        TypeError
            If any argument has an incompatible Python type.
        ValueError
            If a currency or units label cannot be decoded where required.

        Examples
        --------
        >>> from finstack_quant.models.factor import MarketMapping
        >>> isinstance(MarketMapping.equity_spot(["SPX"]), MarketMapping)
        True
        """
        ...

    @staticmethod
    def fx_rate(base: Currency | str, quote: Currency | str) -> MarketMapping:
        """Map a factor to one ordered FX pair.

        Parameters
        ----------
        base : Currency | str
            Base currency quoted in units of quote.
        quote : Currency | str
            Currency per one unit of base.

        Returns
        -------
        MarketMapping
            Factor-to-market binding embedded in a FactorDefinition.

        Raises
        ------
        TypeError
            If any argument has an incompatible Python type.
        ValueError
            If a currency or units label cannot be decoded where required.

        Examples
        --------
        >>> from finstack_quant.models.factor import MarketMapping
        >>> isinstance(MarketMapping.fx_rate("EUR", "USD"), MarketMapping)
        True
        """
        ...

    @staticmethod
    def vol_shift(vol_surface_ids: Sequence[str], units: str) -> MarketMapping:
        """Map a factor to volatility surface shifts.

        Parameters
        ----------
        vol_surface_ids : Sequence[str]
            Surface identifiers receiving the volatility shock.
        units : str
            Native volatility bump unit such as percent.

        Returns
        -------
        MarketMapping
            Factor-to-market binding embedded in a FactorDefinition.

        Raises
        ------
        TypeError
            If any argument has an incompatible Python type.
        ValueError
            If a currency or units label cannot be decoded where required.

        Examples
        --------
        >>> from finstack_quant.models.factor import MarketMapping
        >>> isinstance(MarketMapping.vol_shift(["SPX-VOL"], "percent"), MarketMapping)
        True
        """
        ...

    @staticmethod
    def from_json(json: str) -> MarketMapping:
        """Load a canonical MarketMapping from JSON.

        Parameters
        ----------
        json : str
            Serde JSON matching the Rust MarketMapping shape.

        Returns
        -------
        MarketMapping
            Decoded factor data.

        Raises
        ------
        ValueError
            If JSON cannot be decoded as the corresponding Rust type.

        Examples
        --------
        >>> from finstack_quant.models.factor import MarketMapping
        >>> isinstance(MarketMapping.from_json('{"equity_spot":{"tickers":["SPX"]}}'), MarketMapping)
        True
        """
        ...

    def to_json(self) -> str:
        """Serialize the canonical factor data for wire interchange.

        Returns
        -------
        str
            Compact JSON preserving the native factor shape.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Return the native JSON reconstruction tuple for pickle.

        Returns
        -------
        tuple
            Loader and one JSON argument for this immutable factor value.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

class FactorDefinition:
    """Named risk factor with its classical mapping to market perturbations.

    Examples
    --------
    >>> from finstack_quant.models.factor import FactorDefinition, MarketMapping
    >>> factor = FactorDefinition("usd", "rates", MarketMapping.curve_parallel(["USD-OIS"], "rate_bp"))
    >>> isinstance(factor, FactorDefinition)
    True
    """

    def __init__(
        self,
        id: str,
        factor_type: str | Mapping[str, Any],
        market_mapping: MarketMapping,
        description: str | None = None,
    ) -> None:
        """Capture a factor identifier and its native market bump convention.

        Parameters
        ----------
        id : str
            Case-sensitive identifier used in sensitivity and covariance axes.
        factor_type : str or Mapping[str, Any]
            Broad class such as rates, credit, equity, fx or volatility, or a
            canonical tagged custom classification.
        market_mapping : MarketMapping
            Curves, prices, currency pair or volatility surfaces to shock.
        description : str or None
            Optional literal description for presentation.

        Raises
        ------
        TypeError
            If an argument has an incompatible Python type.
        ValueError
            If factor_type fails decoding. The engine checks market bindings.
        """
        ...

    @staticmethod
    def from_json(json: str) -> FactorDefinition:
        """Load a canonical FactorDefinition from JSON.

        Parameters
        ----------
        json : str
            Serde JSON matching the Rust FactorDefinition shape.

        Returns
        -------
        FactorDefinition
            Decoded factor data.

        Raises
        ------
        ValueError
            If JSON cannot be decoded as the corresponding Rust type.

        Examples
        --------
        >>> from finstack_quant.models.factor import FactorDefinition
        >>> isinstance(
        ...     FactorDefinition.from_json(
        ...         '{"id":"A","factor_type":"equity","market_mapping":{"equity_spot":{"tickers":["SPX"]}}}'
        ...     ),
        ...     FactorDefinition,
        ... )
        True
        """
        ...

    def to_json(self) -> str:
        """Serialize the canonical factor data for wire interchange.

        Returns
        -------
        str
            Compact JSON preserving the native factor shape.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Return the native JSON reconstruction tuple for pickle.

        Returns
        -------
        tuple
            Loader and one JSON argument for this immutable factor value.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...
