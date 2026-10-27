"""Stub declarations mirroring ``finstack-quant-py/src/bindings/scenarios/operation_spec/rate_binding.rs``."""

from __future__ import annotations

from finstack_quant.core.dates import DayCount
from finstack_quant.scenarios._operation_spec.kinds import Compounding

class RateBindingSpec:
    """
    Configuration linking a statement rate node to a market curve.

    Examples
    --------
    >>> from finstack_quant.scenarios import RateBindingSpec, Compounding
    >>> spec = RateBindingSpec("node_1", "USD-OIS", "5Y", Compounding.continuous())
    >>> (spec.curve_id, spec.tenor, spec.compounding.value)
    ('USD-OIS', '5Y', 'continuous')
    >>> spec == RateBindingSpec("node_1", "USD-OIS", "5Y", "continuous")
    True
    """

    def __init__(
        self,
        node_id: str,
        curve_id: str,
        tenor: str,
        compounding: Compounding | str | None = None,
        day_count: DayCount | None = None,
    ) -> None:
        """
        Create a rate binding specification.

        Parameters
        ----------
        node_id : str
            Dimensionless scalar rate node identifier. Applying a binding to a
            monetary node records a rate-binding failure without changing values.
        curve_id : str
            Market curve identifier (e.g. ``"USD-OIS"``).
        tenor : str
            Tenor string (e.g. ``"5Y"``); parsed eagerly by :meth:`validate`.
        compounding : Compounding | str, optional
            Output compounding convention, typed or as its wire label
            (``"continuous"``, ``"periodic(1)"``, ...). Defaults to continuous.
            The extracted rate stays a decimal annualized rate.
        day_count : DayCount, optional
            Output quote day-count convention; ``None`` uses the curve default.
            Maturity dates and curve-implied accumulation factors are preserved.

        Raises
        ------
        ValueError
            If ``compounding`` is not an accepted label.
        """
        ...

    def validate(self) -> None:
        """
        Validate identifiers and eagerly parse the tenor.

        Raises
        ------
        ValueError
            If ``node_id`` or ``curve_id`` is blank, or ``tenor`` is not a
            valid tenor string.

        Examples
        --------
        >>> from finstack_quant.scenarios import RateBindingSpec
        >>> RateBindingSpec("node_1", "USD-OIS", "5Y").validate() is None
        True
        """
        ...

    def __eq__(self, other: object) -> bool:
        """
        Structural equality on every field.

        Parameters
        ----------
        other : object
            Value to compare; non-``RateBindingSpec`` values compare unequal.

        Returns
        -------
        bool
            ``True`` when both bindings serialize identically.

        Raises
        ------
        None
            This method does not raise.
        """
        ...

    @property
    def node_id(self) -> str:
        """
        Statement rate node identifier.

        Returns
        -------
        str
            Node ID string.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def curve_id(self) -> str:
        """
        Market curve identifier.

        Returns
        -------
        str
            Curve ID string (e.g. ``"USD-OIS"``).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def tenor(self) -> str:
        """
        Tenor label used when binding the rate (for example ``"5Y"``).

        Returns
        -------
        str
            Tenor label (e.g. ``"5Y"``).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def compounding(self) -> Compounding:
        """
        Compounding convention used when converting the bound rate.

        Returns
        -------
        Compounding
            Compounding enum value, or the curve default when not specified.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def day_count(self) -> DayCount | None:
        """
        Day-count convention used when converting the bound rate.

        Returns
        -------
        DayCount or None
            Typed day-count convention, or ``None`` when not specified.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this rate binding to a JSON-compatible dict.

        Returns
        -------
        str
            JSON-serialized ``RateBindingSpec``.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @staticmethod
    def from_json(json: str) -> RateBindingSpec:
        """
        Deserialize a ``RateBindingSpec`` from JSON.

        Parameters
        ----------
        json : str
            JSON string produced by ``to_json``.

        Returns
        -------
        RateBindingSpec
            Parsed rate binding specification.

        Raises
        ------
        ValueError
            If the JSON is malformed or fields are invalid.

        Examples
        --------
        >>> from finstack_quant.scenarios import RateBindingSpec
        >>> spec = RateBindingSpec.from_json(
        ...     '{"node_id":"node_1","curve_id":"USD-OIS","tenor":"5Y","compounding":"continuous","day_count":null}'
        ... )
        >>> spec.tenor
        '5Y'
        """
        ...
