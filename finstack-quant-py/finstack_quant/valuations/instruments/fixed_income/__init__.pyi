from __future__ import annotations

from typing import Any

from finstack_quant.core.market_data import MarketContext

class _FixedIncomeInstrument:
    """Base API for direct fixed-income valuation instrument wrappers.

    Concrete classes wrap bonds, loans, mortgage instruments, TBAs, dollar
    rolls, bond futures, structured credit, and fixed-income index TRS. Specs
    are converted to canonical Rust tagged ``InstrumentJson``.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments.fixed_income import Bond
    >>> bond = Bond.from_json(json_str)  # doctest: +SKIP
    >>> result_json = bond.price(market, "2025-06-30")  # doctest: +SKIP
    """

    def __init__(self, spec: dict[str, Any] | str | None = None, **kwargs: Any) -> None:
        """Build a fixed-income instrument from a spec dict, JSON, or fields.

        Parameters
        ----------
        spec : dict or str, optional
            Instrument specification matching the Rust serde shape, or tagged
            ``InstrumentJson`` with the concrete class' type tag.
        **kwargs
            Keyword fields for the Rust spec, such as id, notional, coupon,
            schedule dates, day-count convention, discount curve ID, projection
            curve ID, credit curve ID, collateral pool fields, or tranche terms.

        Raises
        ------
        ValueError
            If both ``spec`` and keyword fields are supplied, the type tag
            mismatches, required fields are missing, or validation fails.
        """
        ...

    @staticmethod
    def from_json(json: str) -> _FixedIncomeInstrument:
        """Deserialize and validate this fixed-income instrument from JSON.

        Parameters
        ----------
        json : str
            Tagged ``InstrumentJson`` string with a type tag matching the
            concrete class.

        Returns
        -------
        Self
            Validated direct wrapper.

        Raises
        ------
        ValueError
            If JSON is malformed, the tag mismatches, or Rust validation fails.
        """
        ...

    def to_json(self) -> str:
        """Serialize this fixed-income instrument to canonical tagged JSON.

        Returns
        -------
        str
            Pretty-printed tagged ``InstrumentJson``.
        """
        ...

    def validate(self) -> None:
        """Validate the instrument spec without pricing it.

        Raises
        ------
        ValueError
            If contractual dates, schedule terms, notionals, curves, collateral,
            or tranche definitions violate Rust instrument invariants.
        """
        ...

    def price(self, market: MarketContext | str, as_of: str, model: str = "default") -> str:
        """Price this fixed-income instrument and return ``ValuationResult`` JSON.

        Parameters
        ----------
        market : MarketContext or str
            Typed market context or serialized JSON. Fixed-income products
            typically require discount curves, forward/projection curves, credit
            or spread curves, prepayment/default assumptions, and price quotes
            referenced by the spec.
        as_of : str
            Valuation date in ISO 8601 ``YYYY-MM-DD`` form.
        model : str, optional
            Pricing model selector. ``"default"`` resolves to the instrument's
            registered Rust default model.

        Returns
        -------
        str
            JSON-encoded ``ValuationResult``.

        Raises
        ------
        ValueError
            If required market data is missing or pricing fails.

        Sources
        -------
        See ``docs/REFERENCES.md#isda-2006-definitions``,
        ``docs/REFERENCES.md#icma-rule-book``, and
        ``docs/REFERENCES.md#tuckman-serrat-fixed-income``.
        """
        ...

    def price_with_metrics(
        self,
        market: MarketContext | str,
        as_of: str,
        model: str = "default",
        metrics: list[str] = ...,
        pricing_options: str | None = None,
        market_history: str | None = None,
    ) -> str:
        """Price this fixed-income instrument and compute requested metrics.

        Parameters
        ----------
        market : MarketContext or str
            Market context object or JSON.
        as_of : str
            Valuation date in ISO 8601 form.
        model : str, optional
            Pricing model selector. Default ``"default"``.
        metrics : list[str]
            Metric IDs, such as ``"ytm"``, ``"dv01"``,
            ``"modified_duration"``, ``"oas"``, or bucketed curve risk keys
            supported by the Rust registry.
        pricing_options : str, optional
            JSON-encoded pricing overrides for bumps, models, or scenarios.
        market_history : str, optional
            JSON-encoded market history for metrics requiring time series.

        Returns
        -------
        str
            JSON-encoded ``ValuationResult`` with requested metrics populated.

        Raises
        ------
        ValueError
            If a metric is unsupported or required market data is missing.
        """
        ...

class Bond(_FixedIncomeInstrument):
    """Fixed-rate, floating-rate, amortizing, callable, or putable bond."""

class ConvertibleBond(_FixedIncomeInstrument):
    """Convertible bond with embedded equity conversion optionality."""

class InflationLinkedBond(_FixedIncomeInstrument):
    """Inflation-linked bond such as TIPS or index-linked gilts."""

class TermLoan(_FixedIncomeInstrument):
    """Bilateral or syndicated term-loan exposure wrapper."""

class RevolvingCredit(_FixedIncomeInstrument):
    """Revolving credit facility wrapper."""

class BondFuture(_FixedIncomeInstrument):
    """Bond futures contract with deliverable-basket mechanics."""

class AgencyMbsPassthrough(_FixedIncomeInstrument):
    """Agency mortgage-backed passthrough security wrapper."""

class AgencyTba(_FixedIncomeInstrument):
    """Agency TBA forward contract wrapper."""

class AgencyCmo(_FixedIncomeInstrument):
    """Agency collateralized mortgage obligation tranche wrapper."""

class DollarRoll(_FixedIncomeInstrument):
    """MBS dollar-roll financing wrapper."""

class FIIndexTotalReturnSwap(_FixedIncomeInstrument):
    """Fixed-income index total return swap wrapper."""

class StructuredCredit(_FixedIncomeInstrument):
    """Structured credit deal wrapper for ABS, RMBS, CMBS, or CLO tranches."""

__all__: list[str] = [
    "Bond",
    "ConvertibleBond",
    "InflationLinkedBond",
    "TermLoan",
    "RevolvingCredit",
    "BondFuture",
    "AgencyMbsPassthrough",
    "AgencyTba",
    "AgencyCmo",
    "DollarRoll",
    "FIIndexTotalReturnSwap",
    "StructuredCredit",
]
