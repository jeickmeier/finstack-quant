"""Stub declarations mirroring ``finstack-quant-py/src/bindings/margin/metrics.rs``."""

from __future__ import annotations

from typing import Any
import pandas as pd

class MarginUtilization:
    """
    Margin utilization result (ratio of posted to required margin).

    Parameters
    ----------
    posted_amount : float
        Margin amount actually posted, in the CSA currency.
    required_amount : float
        Margin amount required by the CSA calculation, in the CSA currency.
    currency : str
        ISO currency code (both amounts use this currency).

    Returns
    -------
    MarginUtilization
        Utilization metrics.

    Raises
    ------
    ValueError
        Invalid currency code.

    Examples
    --------
    >>> u = MarginUtilization(100.0, 100.0, "USD")
    >>> u.is_adequate()
    True
    """

    def __init__(
        self,
        posted_amount: float,
        required_amount: float,
        currency: str,
    ) -> None:
        """
        Compare posted and required margin in one reporting currency.

        Parameters
        ----------
        posted_amount : float
            Margin already posted, in ``currency`` units.
        required_amount : float
            Margin requirement, in the same ``currency`` units.
        currency : str
            ISO-4217 code shared by both amounts.

        Raises
        ------
        ValueError
            If ``currency`` is unrecognized, or either amount is non-finite or
            outside the representable monetary range.
        """
        ...

    @staticmethod
    def from_json(json: str) -> MarginUtilization:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            Canonical JSON for a ``MarginUtilization``.

        Returns
        -------
        MarginUtilization
            Parsed value.

        Raises
        ------
        ValueError
            If the payload is malformed.

        Examples
        --------
        >>> original = MarginUtilization(100.0, 80.0, "USD")
        >>> MarginUtilization.from_json(original.to_json()).to_json() == original.to_json()
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON accepted by ``from_json``.

        Returns
        -------
        str
            JSON string.

        Raises
        ------
        ValueError
            If serialization fails.

        Examples
        --------
        >>> isinstance(MarginUtilization(100.0, 80.0, "USD").to_json(), str)
        True
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Support ``pickle`` via the ``to_json`` / ``from_json`` round-trip."""
        ...

    @property
    def posted(self) -> float:
        """
        Margin amount actually posted, in the CSA currency.

        Returns
        -------
        float
            Posted amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> MarginUtilization(10.0, 20.0, "USD").posted
        10.0
        """
        ...

    @property
    def required(self) -> float:
        """
        Margin amount required by the CSA calculation, in the CSA currency.

        Returns
        -------
        float
            Required amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> MarginUtilization(10.0, 20.0, "USD").required
        20.0
        """
        ...

    @property
    def ratio(self) -> float:
        """
        Utilization ratio (posted / required).

        Returns
        -------
        float
            Ratio.

            Utilization ratio (posted / required).

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> MarginUtilization(50.0, 100.0, "EUR").ratio
        0.5
        """
        ...

    def is_adequate(self) -> bool:
        """
        Whether margin is adequate (ratio >= 1.0).

        Returns
        -------
        bool
            Adequacy flag.

        Notes
        -----
        This method does not raise; it returns ``True`` or ``False``.

        Examples
        --------
        >>> MarginUtilization(100.0, 100.0, "USD").is_adequate()
        True
        """
        ...

    def shortfall(self) -> float:
        """
        Shortfall amount (if any).

        Returns
        -------
        float
            Shortfall in currency units.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.

        Examples
        --------
        >>> MarginUtilization(0.0, 100.0, "USD").shortfall() >= 0
        True
        """
        ...

    def __repr__(self) -> str: ...
    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the result as a single-row pandas ``DataFrame``.

        Columns: ``posted``, ``required``, ``ratio``, ``shortfall``,
        ``is_adequate``, ``currency``.

        ``posted``, ``required`` and ``shortfall`` are floats in ``currency``.
        ``ratio`` is ``posted / required`` as a decimal fraction (``1.0`` =
        fully covered); it is ``inf`` when nothing is required but margin is
        posted, and ``1.0`` when neither is.

        Returns
        -------
        pd.DataFrame
            One row describing margin utilization.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

class ExcessCollateral:
    """
    Excess collateral result.

    Parameters
    ----------
    collateral_value : float
        Collateral mark.
    required_value : float
        Required collateral.
    currency : str
        ISO currency code.

    Returns
    -------
    ExcessCollateral
        Excess or shortfall view.

    Raises
    ------
    ValueError
        Invalid currency.

    Examples
    --------
    >>> ExcessCollateral(120.0, 100.0, "USD").has_excess()
    True
    """

    def __init__(
        self,
        collateral_value: float,
        required_value: float,
        currency: str,
    ) -> None:
        """
        Compare collateral value with the required collateral amount.

        Parameters
        ----------
        collateral_value : float
            Current collateral mark, in ``currency`` units.
        required_value : float
            Required collateral amount in the same currency.
        currency : str
            ISO-4217 code shared by both values.

        Raises
        ------
        ValueError
            If ``currency`` is unrecognized, or either amount is non-finite or
            outside the representable monetary range.
        """
        ...

    @staticmethod
    def from_json(json: str) -> ExcessCollateral:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            Canonical JSON for a ``ExcessCollateral``.

        Returns
        -------
        ExcessCollateral
            Parsed value.

        Raises
        ------
        ValueError
            If the payload is malformed.

        Examples
        --------
        >>> original = ExcessCollateral(120.0, 100.0, "USD")
        >>> ExcessCollateral.from_json(original.to_json()).to_json() == original.to_json()
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON accepted by ``from_json``.

        Returns
        -------
        str
            JSON string.

        Raises
        ------
        ValueError
            If serialization fails.

        Examples
        --------
        >>> isinstance(ExcessCollateral(120.0, 100.0, "USD").to_json(), str)
        True
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Support ``pickle`` via the ``to_json`` / ``from_json`` round-trip."""
        ...

    @property
    def collateral_value(self) -> float:
        """
        Market value of posted collateral in the CSA currency.

        Returns
        -------
        float
            Collateral mark.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> ExcessCollateral(10.0, 5.0, "USD").collateral_value
        10.0
        """
        ...

    @property
    def required_value(self) -> float:
        """
        Required collateral amount in the same currency as the mark.

        Returns
        -------
        float
            Requirement.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> ExcessCollateral(10.0, 5.0, "USD").required_value
        5.0
        """
        ...

    @property
    def excess(self) -> float:
        """
        Excess amount (positive) or shortfall (negative).

        Returns
        -------
        float
            Net excess.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> ExcessCollateral(10.0, 5.0, "USD").excess > 0
        True
        """
        ...

    def has_excess(self) -> bool:
        """
        Whether there is excess collateral.

        Returns
        -------
        bool
            True if excess > 0.

        Notes
        -----
        This method does not raise; it returns ``True`` or ``False``.

        Examples
        --------
        >>> ExcessCollateral(2.0, 1.0, "USD").has_excess()
        True
        """
        ...

    def has_shortfall(self) -> bool:
        """
        Whether there is a shortfall.

        Returns
        -------
        bool
            True if under-collateralized.

        Notes
        -----
        This method does not raise; it returns ``True`` or ``False``.

        Examples
        --------
        >>> ExcessCollateral(1.0, 2.0, "USD").has_shortfall()
        True
        """
        ...

    def excess_percentage(self) -> float:
        """
        Excess as a percentage of required.

        Returns
        -------
        float
            Fractional excess vs required.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.

        Examples
        --------
        >>> isinstance(ExcessCollateral(110.0, 100.0, "USD").excess_percentage(), float)
        True
        """
        ...

    def __repr__(self) -> str: ...
    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the result as a single-row pandas ``DataFrame``.

        Columns: ``collateral_value``, ``required_value``, ``excess``,
        ``excess_percentage``, ``has_excess``, ``has_shortfall``,
        ``currency``.

        The three amount columns are floats in ``currency``; ``excess`` is
        ``collateral_value - required_value`` and is negative on a shortfall.
        ``excess_percentage`` is a decimal fraction of ``required_value``
        (``0.1`` = 10% over-collateralised).

        Returns
        -------
        pd.DataFrame
            One row describing the collateral position.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

class MarginFundingCost:
    """
    Margin funding cost result.

    Parameters
    ----------
    margin_posted : float
        Margin amount actually posted, in the CSA currency.
    funding_rate : float
        Funding rate (annualized).
    collateral_rate : float
        Rate earned on posted collateral, as a decimal.
    currency : str
        ISO currency code.

    Returns
    -------
    MarginFundingCost
        Annual and periodic funding cost view.

    Raises
    ------
    ValueError
        Invalid currency.

    Examples
    --------
    >>> m = MarginFundingCost(1e6, 0.05, 0.01, "USD")
    >>> m.spread() == 0.04
    True
    """

    def __init__(
        self,
        margin_posted: float,
        funding_rate: float,
        collateral_rate: float,
        currency: str,
    ) -> None:
        """
        Describe the annual funding spread and cost of posted margin.

        Parameters
        ----------
        margin_posted : float
            Posted margin principal, in ``currency`` units.
        funding_rate : float
            Annual funding rate as a decimal, such as ``0.05`` for 5%.
        collateral_rate : float
            Annual collateral remuneration rate as a decimal.
        currency : str
            ISO-4217 code for the margin and calculated costs.

        Raises
        ------
        ValueError
            If ``currency`` is unrecognized, ``margin_posted`` or the funding
            spread is non-finite, or the annual cost is outside the
            representable monetary range.
        """
        ...

    @staticmethod
    def from_json(json: str) -> MarginFundingCost:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            Canonical JSON for a ``MarginFundingCost``.

        Returns
        -------
        MarginFundingCost
            Parsed value.

        Raises
        ------
        ValueError
            If the payload is malformed.

        Examples
        --------
        >>> original = MarginFundingCost(1e6, 0.05, 0.02, "USD")
        >>> MarginFundingCost.from_json(original.to_json()).to_json() == original.to_json()
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON accepted by ``from_json``.

        Returns
        -------
        str
            JSON string.

        Raises
        ------
        ValueError
            If serialization fails.

        Examples
        --------
        >>> isinstance(MarginFundingCost(1e6, 0.05, 0.02, "USD").to_json(), str)
        True
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Support ``pickle`` via the ``to_json`` / ``from_json`` round-trip."""
        ...

    @property
    def margin_posted(self) -> float:
        """
        Margin amount actually posted, in the CSA currency.

        Returns
        -------
        float
            Margin posted.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> MarginFundingCost(1.0, 0.1, 0.0, "USD").margin_posted
        1.0
        """
        ...

    @property
    def funding_rate(self) -> float:
        """
        Funding rate (annualized).

        Returns
        -------
        float
            Funding rate.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> MarginFundingCost(1.0, 0.06, 0.02, "USD").funding_rate
        0.06
        """
        ...

    @property
    def collateral_rate(self) -> float:
        """
        Rate earned on posted collateral, as a decimal.

        Returns
        -------
        float
            Collateral rate.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> MarginFundingCost(1.0, 0.06, 0.02, "USD").collateral_rate
        0.02
        """
        ...

    @property
    def annual_cost(self) -> float:
        """
        Annualized funding cost.

        Returns
        -------
        float
            Annual cost amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> MarginFundingCost(1e6, 0.05, 0.0, "USD").annual_cost > 0
        True
        """
        ...

    def spread(self) -> float:
        """
        Funding spread (funding rate − collateral rate).

        Returns
        -------
        float
            Net spread.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.

        Examples
        --------
        >>> round(MarginFundingCost(0.0, 0.05, 0.02, "USD").spread(), 2)
        0.03
        """
        ...

    def cost_for_period(self, year_fraction: float) -> float:
        """
        Cost for a specific period.

        Parameters
        ----------
        year_fraction : float
            Length of period in years.

        Returns
        -------
        float
            Cost over the period.

        Raises
        ------
        ValueError
            If ``year_fraction`` is non-finite or the period cost is outside
            the representable monetary range.

        Examples
        --------
        >>> MarginFundingCost(1e6, 0.04, 0.0, "USD").cost_for_period(0.5) >= 0
        True
        """
        ...

    def __repr__(self) -> str: ...
    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the result as a single-row pandas ``DataFrame``.

        Columns: ``margin_posted``, ``funding_rate``, ``collateral_rate``,
        ``spread``, ``annual_cost``, ``currency``.

        ``margin_posted`` and ``annual_cost`` are floats in ``currency``; the
        three rate columns are annualized decimal fractions (``0.03`` = 3%).
        ``annual_cost`` is ``margin_posted * spread``, so a collateral rate
        above the funding rate makes it negative (a funding benefit).

        Returns
        -------
        pd.DataFrame
            One row describing the funding cost.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

class Haircut01:
    """
    Haircut sensitivity: PV change for +1bp haircut change.

    Parameters
    ----------
    collateral_value : float
        Collateral mark.
    current_haircut : float
        Current haircut as decimal.
    currency : str
        ISO currency code.

    Returns
    -------
    Haircut01
        Sensitivity metrics.

    Raises
    ------
    ValueError
        Invalid currency.

    Examples
    --------
    >>> h = Haircut01(1e6, 0.05, "USD")
    >>> isinstance(h.pv_change, float)
    True
    """

    def __init__(
        self,
        collateral_value: float,
        current_haircut: float,
        currency: str,
    ) -> None:
        """
        Measure collateral-value sensitivity to a one-basis-point haircut increase.

        Parameters
        ----------
        collateral_value : float
            Pre-haircut collateral mark, in ``currency`` units.
        current_haircut : float
            Current haircut as a decimal fraction, such as ``0.05`` for 5%.
        currency : str
            ISO-4217 code for the collateral value and PV change.

        Raises
        ------
        ValueError
            If ``currency`` is unrecognized, or ``collateral_value`` is
            non-finite or outside the representable monetary range.
        """
        ...

    @staticmethod
    def from_json(json: str) -> Haircut01:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            Canonical JSON for a ``Haircut01``.

        Returns
        -------
        Haircut01
            Parsed value.

        Raises
        ------
        ValueError
            If the payload is malformed.

        Examples
        --------
        >>> original = Haircut01(1e6, 0.05, "USD")
        >>> Haircut01.from_json(original.to_json()).to_json() == original.to_json()
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON accepted by ``from_json``.

        Returns
        -------
        str
            JSON string.

        Raises
        ------
        ValueError
            If serialization fails.

        Examples
        --------
        >>> isinstance(Haircut01(1e6, 0.05, "USD").to_json(), str)
        True
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Support ``pickle`` via the ``to_json`` / ``from_json`` round-trip."""
        ...

    @property
    def collateral_value(self) -> float:
        """
        Market value of posted collateral in the CSA currency.

        Returns
        -------
        float
            Collateral mark.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> Haircut01(100.0, 0.1, "USD").collateral_value
        100.0
        """
        ...

    @property
    def current_haircut(self) -> float:
        """
        Current haircut (decimal).

        Returns
        -------
        float
            Haircut.

            Current haircut (decimal).

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> Haircut01(100.0, 0.1, "USD").current_haircut
        0.1
        """
        ...

    @property
    def pv_change(self) -> float:
        """
        PV change for +1bp haircut.

        Returns
        -------
        float
            Sensitivity amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.

        Examples
        --------
        >>> isinstance(Haircut01(1e6, 0.05, "USD").pv_change, float)
        True
        """
        ...

    def haircut_bp(self) -> float:
        """
        Current haircut in basis points.

        Returns
        -------
        float
            Haircut in bp.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.

        Examples
        --------
        >>> Haircut01(1.0, 0.01, "USD").haircut_bp()
        100.0
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the result as a single-row pandas ``DataFrame``.

        Columns: ``collateral_value``, ``current_haircut``, ``haircut_bp``,
        ``pv_change``, ``currency``. ``collateral_value`` and ``pv_change``
        are floats in ``currency``; ``current_haircut`` is a decimal fraction
        and ``haircut_bp`` the same haircut in basis points.

        Returns
        -------
        pd.DataFrame
            One row describing the haircut sensitivity.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.

        Examples
        --------
        >>> float(Haircut01(1.0, 0.01, "USD").to_dataframe()["haircut_bp"].iloc[0])
        100.0
        """
        ...

    def __repr__(self) -> str: ...
