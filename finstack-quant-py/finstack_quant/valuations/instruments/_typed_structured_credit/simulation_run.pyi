"""Stub declarations mirroring ``finstack-quant-py/src/bindings/valuations/typed_structured_credit/simulation_run.rs``."""

from __future__ import annotations

from typing import Any
from finstack_quant.valuations.instruments._typed_structured_credit.diagnostics import SimulationDiagnostics
from finstack_quant.valuations.instruments._typed_structured_credit.tranche_cashflows import TrancheCashflows

class SimulationRun:
    """Complete result of one deterministic structured-credit simulation.

    Contains tranche flows and accounting from the same validated run.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.valuations.instruments import StructuredCredit
    >>> from finstack_quant.core.market_data import DiscountCurve, MarketContext
    >>> as_of = datetime.date(2024, 1, 1)
    >>> market = MarketContext().insert(DiscountCurve.flat("USD-OIS", as_of, 0.03))
    >>> run = StructuredCredit.example().run_simulation(market, as_of)
    >>> bool(run.tranches)
    True
    """
    @property
    def tranches(self) -> dict[str, TrancheCashflows]:
        """Detailed flows indexed by stable tranche identifier.

        Returns
        -------
        dict[str, TrancheCashflows]
            Owned wrappers for interest, principal and other flow components.

        Notes
        -----
        This accessor does not raise; it returns an owned view of the simulation result.
        """
        ...
    @property
    def diagnostics(self) -> SimulationDiagnostics:
        """Complete deal accounting produced by the same simulation.

        Returns
        -------
        SimulationDiagnostics
            Period, reserve, draw-funding and coverage-test records.

        Notes
        -----
        This accessor does not raise; it returns an owned view of the simulation result.
        """
        ...
    def to_dict(self) -> dict[str, Any]:
        """Serialize tranche flows and diagnostics to their canonical mapping.

        Returns
        -------
        dict[str, Any]
            Rust wire fields ``tranches`` and ``diagnostics``.

        Raises
        ------
        ValueError
            If the simulation result cannot be serialized to a Python mapping.
        """
        ...
    def to_json(self) -> str:
        """Serialize the complete run to canonical JSON.

        Returns
        -------
        str
            Rust wire record containing all result components.

        Raises
        ------
        ValueError
            If the simulation result cannot be serialized to JSON.
        """
        ...
    @staticmethod
    def from_json(json: str) -> SimulationRun:
        """Deserialize a complete run without executing simulation.

        Parameters
        ----------
        json : str
            Canonical JSON with tranche results and diagnostics.

        Returns
        -------
        SimulationRun
            Restored complete simulation result.

        Raises
        ------
        ValueError
            If the JSON cannot be decoded as a complete run.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import SimulationRun
        >>> try:
        ...     SimulationRun.from_json("{}")
        ... except ValueError:
        ...     print("invalid run")
        invalid run
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            Reconstruction callable and canonical JSON arguments.

        Raises
        ------
        ValueError
            If serialization of the stored value fails.
        """
        ...
