"""Stub declarations mirroring ``finstack-quant-py/src/bindings/statements_analytics/templates_real_estate.rs``."""

from __future__ import annotations

import pandas as pd
from finstack_quant.statements import FinancialModelSpec

# Real-estate template

class RentStepSpec:
    """
    Reset a lease's base rent from one model period onward.

    Parameters
    ----------
    start : str
        First model period label at which the stepped rent applies.
    rent : float
        Replacement periodic rent in the model's currency units.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import RentStepSpec
    >>> step = RentStepSpec("2026Q1", 125.0)
    >>> (step.start, step.rent)
    ('2026Q1', 125.0)

    """

    def __init__(self, start: str, rent: float) -> None:
        """
        Reset a lease's periodic rent from a specified model period onward.

        Parameters
        ----------
        start : str
            First model-period label at which the replacement rent applies.
        rent : float
            Replacement periodic rent in the model's currency units.

        Raises
        ------
        ValueError
            If start is not a valid model period.

        """
        ...

    @property
    def start(self) -> str:
        """
        Period (inclusive) this rent level takes effect, as a period-id string.

        Returns
        -------
        str
            Effective period as a period-id string.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def rent(self) -> float:
        """
        Rent for one model period from ``start``, in model currency units.

        This replaces the prevailing rent level rather than adding to it, and restarts
        growth compounding from ``start``.

        Returns
        -------
        float
            Rent per model period from ``start``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    def to_json(self) -> str:
        """
        Serialize this object to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation of this `RentStepSpec`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    @staticmethod
    def from_json(json: str) -> RentStepSpec:
        """
        Deserialize a rent-step specification from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload containing the step start period and replacement rent.

        Returns
        -------
        RentStepSpec
            Validated `RentStepSpec` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If ``json`` is malformed or violates this type's serialized schema.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import RentStepSpec
        >>> step = RentStepSpec("2026Q1", 125.0)
        >>> RentStepSpec.from_json(step.to_json()).rent
        125.0

        """
        ...

class FreeRentWindowSpec:
    """
    Define a finite concession window that sets lease rent to zero.

    Parameters
    ----------
    start : str
        First model period label affected by the free-rent concession.
    periods : int
        Number of consecutive modeled periods with rent set to zero.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import FreeRentWindowSpec
    >>> window = FreeRentWindowSpec("2025Q1", 2)
    >>> (window.start, window.periods)
    ('2025Q1', 2)

    """

    def __init__(self, start: str, periods: int) -> None:
        """
        Define a dated window of consecutive rent-free model periods.

        Parameters
        ----------
        start : str
            First model-period label affected by the concession.
        periods : int
            Number of consecutive modeled periods with rent set to zero.

        Raises
        ------
        ValueError
            If start is not a valid model period.

        """
        ...

    @property
    def start(self) -> str:
        """
        Period (inclusive) free rent starts, as a period-id string.

        Returns
        -------
        str
            First free-rent period as a period-id string.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def periods(self) -> int:
        """
        Length of the concession as a count of model periods.

        Returns
        -------
        int
            Concession length in model periods.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    def to_json(self) -> str:
        """
        Serialize this object to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation of this `FreeRentWindowSpec`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    @staticmethod
    def from_json(json: str) -> FreeRentWindowSpec:
        """
        Deserialize a free-rent concession window from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload containing the concession start period and duration.

        Returns
        -------
        FreeRentWindowSpec
            Validated `FreeRentWindowSpec` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If ``json`` is malformed or violates this type's serialized schema.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import FreeRentWindowSpec
        >>> window = FreeRentWindowSpec("2025Q1", 2)
        >>> FreeRentWindowSpec.from_json(window.to_json()).periods
        2

        """
        ...

class RenewalSpec:
    """
    Model a lease renewal in expected-value terms after the base term.

    Parameters
    ----------
    term_periods : int
        Number of modeled periods in the renewal term when the tenant renews.
    probability : float
        Decimal probability of renewal from zero through one.
    downtime_periods : int
        Vacancy periods between the original lease and renewal; defaults to zero.
    rent_factor : float
        Multiplier applied to the ending scheduled rent for the renewal term;
        defaults to ``1.0``.
    free_rent_periods : int
        Initial renewal periods with rent set to zero; defaults to no concession.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import RenewalSpec
    >>> renewal = RenewalSpec(4, 0.75, downtime_periods=1)
    >>> (renewal.term_periods, renewal.probability, renewal.downtime_periods)
    (4, 0.75, 1)

    """

    def __init__(
        self,
        term_periods: int,
        probability: float,
        downtime_periods: int = 0,
        rent_factor: float = 1.0,
        free_rent_periods: int = 0,
    ) -> None:
        """
        Define expected-value assumptions for a lease renewal term.

        Parameters
        ----------
        term_periods : int
            Number of modeled periods in the renewal term when renewal occurs.
        probability : float
            Decimal renewal probability from zero through one.
        downtime_periods : int, default 0
            Vacancy periods between the original lease and renewal.
        rent_factor : float, default 1.0
            Multiplier applied to ending scheduled rent for the renewal term.
        free_rent_periods : int, default 0
            Initial renewal periods with rent set to zero.

        Notes
        -----
        Construction does not raise; arguments are stored as supplied.
        """
        ...

    @property
    def term_periods(self) -> int:
        """
        Renewal term length as a count of model periods.

        Returns
        -------
        int
            Renewal term length in model periods.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def probability(self) -> float:
        """
        Probability of renewal as a decimal fraction in ``[0, 1]``.

        Renewal is modelled in expected-value terms, so this weights the renewal rent
        rather than selecting a branch.

        Returns
        -------
        float
            Renewal probability in ``[0, 1]``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def downtime_periods(self) -> int:
        """
        Rent-free downtime after the initial term ends, as a count of model periods.

        Returns
        -------
        int
            Downtime length in model periods.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def rent_factor(self) -> float:
        """
        Multiplier applied to the last contractual rent of the initial term (``1.05``
        means renewal starts 5% above the prior rent level).

        Returns
        -------
        float
            Multiplier on the prior rent level at renewal.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def free_rent_periods(self) -> int:
        """
        Number of model periods of free rent at renewal start.

        Returns
        -------
        int
            Count of free-rent model periods at renewal start.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    def validate(self) -> None:
        """
        Validate this object's invariants without executing a report.

        Raises
        ------
        ValueError
            If required fields are missing, out of range, or internally inconsistent.
        """
        ...
    def to_json(self) -> str:
        """
        Serialize this object to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation of this `RenewalSpec`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    @staticmethod
    def from_json(json: str) -> RenewalSpec:
        """
        Deserialize renewal assumptions from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload containing renewal term, probability, downtime, and
            rent assumptions.

        Returns
        -------
        RenewalSpec
            Validated `RenewalSpec` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If ``json`` is malformed or violates this type's serialized schema.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import RenewalSpec
        >>> renewal = RenewalSpec(4, 0.75)
        >>> RenewalSpec.from_json(renewal.to_json()).probability
        0.75

        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the renewal spec as a single-row pandas ``DataFrame``.

        Columns: ``downtime_periods``, ``term_periods``, ``probability``,
        ``rent_factor``, ``free_rent_periods``.

        The three ``*_periods`` columns are counts of model periods;
        ``probability`` is a decimal fraction in ``[0, 1]`` and
        ``rent_factor`` is a multiplier on the prior rent level.

        Returns
        -------
        pd.DataFrame
            One row describing the renewal terms.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

class LeaseGrowthConvention:
    """
    Compounding convention for lease rent growth.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import LeaseGrowthConvention
    >>> LeaseGrowthConvention.PerPeriod.value()
    'per_period'

    """

    PerPeriod: LeaseGrowthConvention
    AnnualEscalator: LeaseGrowthConvention

    @staticmethod
    def from_str(value: str) -> LeaseGrowthConvention:
        """
        Parse a lease rent-growth compounding convention.

        Parameters
        ----------
        value : str
            Case-insensitive ``"per_period"`` or ``"annual_escalator"`` value.

        Returns
        -------
        LeaseGrowthConvention
            Enum member corresponding to the exact snake-case growth convention.

        Raises
        ------
        ValueError
            If value is not per_period or annual_escalator.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import LeaseGrowthConvention
        >>> LeaseGrowthConvention.from_str("annual_escalator").value()
        'annual_escalator'

        """
        ...
    def value(self) -> str:
        """
        Return the canonical snake-case identifier for this variant.

        Returns
        -------
        str
            Exact JSON identifier, either ``"per_period"`` or
            ``"annual_escalator"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class LeaseSpec:
    """
    Describe a rich lease schedule for rent-roll generation.

    Parameters
    ----------
    node_id : str
        Statement node ID receiving the lease's rental-revenue series.
    start : str
        First included model period label for the lease term.
    base_rent : float
        Contractual periodic rent before escalators, concessions, and occupancy
        scaling, in the model's currency units.
    end : str or None
        Optional final included model period; ``None`` extends through horizon.
    growth_rate : float
        Decimal rent-growth rate interpreted by ``growth_convention``.
    growth_convention : LeaseGrowthConvention
        Whether rent growth compounds every model period or as an annual
        step. Default ``AnnualEscalator`` (Argus/NCREIF anniversary bump).
        ``PerPeriod`` must be set explicitly.
    rent_steps : list[RentStepSpec]
        Explicit rent resets applied from each step's start period onward.
    free_rent_periods : int
        Number of initial included periods with rent set to zero.
    free_rent_windows : list[FreeRentWindowSpec]
        Additional dated rent-free concession windows within the lease term.
    occupancy : float
        Decimal occupancy multiplier applied to scheduled rent.
    renewal : RenewalSpec or None
        Optional expected-value renewal assumptions applied after the base term.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import LeaseSpec
    >>> lease = LeaseSpec("lease_a", "2025Q1", 100.0, end="2025Q4")
    >>> (lease.node_id, lease.base_rent, lease.growth_convention.value())
    ('lease_a', 100.0, 'annual_escalator')

    """

    def __init__(
        self,
        node_id: str,
        start: str,
        base_rent: float,
        end: str | None = None,
        growth_rate: float = 0.0,
        growth_convention: LeaseGrowthConvention = ...,
        rent_steps: list[RentStepSpec] = ...,
        free_rent_periods: int = 0,
        free_rent_windows: list[FreeRentWindowSpec] = ...,
        occupancy: float = 1.0,
        renewal: RenewalSpec | None = None,
    ) -> None:
        """
        Define a lease schedule with escalators, concessions, and renewal terms.

        Parameters
        ----------
        node_id : str
            Statement node receiving the lease's rental-revenue series.
        start : str
            First included model-period label for the lease term.
        base_rent : float
            Contractual periodic rent before escalators, concessions, and
            occupancy scaling, in the model's currency units.
        end : str or None, default None
            Optional final included model period; ``None`` extends through horizon.
        growth_rate : float, default 0.0
            Decimal rent-growth rate interpreted by ``growth_convention``.
        growth_convention : LeaseGrowthConvention
            Whether growth compounds every model period or as an annual
            step. Default ``AnnualEscalator``; set ``PerPeriod`` explicitly
            for per-period compounding.
        rent_steps : list[RentStepSpec]
            Explicit rent resets applied from each step's start period onward.
        free_rent_periods : int, default 0
            Number of initial included periods with rent set to zero.
        free_rent_windows : list[FreeRentWindowSpec]
            Additional dated rent-free windows within the base lease term.
        occupancy : float, default 1.0
            Decimal occupancy multiplier applied to scheduled rent.
        renewal : RenewalSpec or None, default None
            Optional expected-value renewal assumptions applied after the base term.

        Raises
        ------
        ValueError
            If start or end is not a valid model period.

        """
        ...

    @property
    def node_id(self) -> str:
        """
        Base node id; per-lease detail nodes are derived from it (``{node_id}.pgi``,
        ``.free_rent``, ``.vacancy_loss``, ``.effective_rent``).

        Returns
        -------
        str
            Base node id for the lease's derived detail nodes.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def start(self) -> str:
        """
        First period (inclusive) the lease is active, as a period-id string (e.g.
        ``"2025Q1"``).

        Returns
        -------
        str
            First active period as a period-id string.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def end(self) -> str | None:
        """
        Last period (inclusive) of the initial term, or ``None`` to run through the
        model end (which also disables renewal modelling).

        Returns
        -------
        str or None
            Last period of the initial term, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def base_rent(self) -> float:
        """
        Base rent for one model period at ``start``, in model currency units.

        A quarterly model means rent per quarter, not per year.

        Returns
        -------
        float
            Base rent per model period at ``start``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def growth_rate(self) -> float:
        """
        Growth rate applied within a rent segment as a decimal fraction (``0.03`` =
        +3%), compounded per ``growth_convention``.

        Returns
        -------
        float
            Segment growth rate as a decimal fraction.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def growth_convention(self) -> LeaseGrowthConvention:
        """
        Compounding convention for ``growth_rate``: every model period (``per_period``)
        or once per lease-start anniversary (``annual_escalator``).

        Returns
        -------
        LeaseGrowthConvention
            Compounding convention for ``growth_rate``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def free_rent_periods(self) -> int:
        """
        Number of model periods of free rent counted from ``start``, before any
        additional ``free_rent_windows``.

        Returns
        -------
        int
            Count of free-rent model periods from ``start``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def occupancy(self) -> float:
        """
        Occupancy factor in ``[0, 1]`` applied to non-free contractual rent.

        Returns
        -------
        float
            Occupancy factor in ``[0, 1]``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def renewal(self) -> RenewalSpec | None:
        """
        Renewal modelling applied after ``end``, or ``None`` for no renewal.

        Returns
        -------
        RenewalSpec or None
            Renewal specification, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    def validate(self) -> None:
        """
        Validate this object's invariants without executing a report.

        Raises
        ------
        ValueError
            If required fields are missing, out of range, or internally inconsistent.
        """
        ...
    def to_json(self) -> str:
        """
        Serialize this object to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation of this `LeaseSpec`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    @staticmethod
    def from_json(json: str) -> LeaseSpec:
        """
        Deserialize a rich lease schedule from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload containing term, rent, escalation, concession, and
            renewal assumptions.

        Returns
        -------
        LeaseSpec
            Validated `LeaseSpec` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If ``json`` is malformed or violates this type's serialized schema.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import LeaseSpec
        >>> lease = LeaseSpec("lease_a", "2025Q1", 100.0)
        >>> LeaseSpec.from_json(lease.to_json()).start
        '2025Q1'

        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the lease spec as a single-row pandas ``DataFrame``.

        Columns: ``node_id``, ``start``, ``end``, ``base_rent``,
        ``growth_rate``, ``growth_convention``, ``free_rent_periods``,
        ``occupancy``, ``rent_step_count``, ``free_rent_window_count``,
        ``has_renewal``.

        ``start`` and ``end`` are period-id strings, ``base_rent`` is per
        model period, ``growth_rate`` and ``occupancy`` are decimal fractions,
        and ``free_rent_periods`` is a count of model periods. The nested
        collections are summarised as counts here - read ``renewal`` (and its
        own ``to_dataframe``) for the renewal terms.

        Returns
        -------
        pd.DataFrame
            One row describing the lease.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

class RentRollOutputNodes:
    """
    Name the aggregate model nodes produced by a rent-roll template.

    Parameters
    ----------
    rent_pgi_node : str
        Node ID for potential gross rent before concessions and vacancy.
    free_rent_node : str
        Node ID for rent waived through free-rent concessions.
    vacancy_loss_node : str
        Node ID for the revenue reduction caused by vacancy or occupancy.
    rent_effective_node : str
        Node ID for effective rent after concessions and vacancy adjustments.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import RentRollOutputNodes
    >>> nodes = RentRollOutputNodes()
    >>> (nodes.rent_pgi_node, nodes.rent_effective_node)
    ('rent_pgi', 'rent_effective')

    """

    def __init__(
        self,
        rent_pgi_node: str = "rent_pgi",
        free_rent_node: str = "free_rent",
        vacancy_loss_node: str = "vacancy_loss",
        rent_effective_node: str = "rent_effective",
    ) -> None:
        """
        Name the four aggregate statement nodes produced by a rent roll.

        Parameters
        ----------
        rent_pgi_node : str, default "rent_pgi"
            Node ID for potential gross rent before concessions and vacancy.
        free_rent_node : str, default "free_rent"
            Node ID for rent waived through free-rent concessions.
        vacancy_loss_node : str, default "vacancy_loss"
            Node ID for the revenue reduction caused by vacancy or occupancy.
        rent_effective_node : str, default "rent_effective"
            Node ID for effective rent after concessions and vacancy adjustments.

        Notes
        -----
        Construction does not raise; arguments are stored as supplied.
        """
        ...

    @property
    def rent_pgi_node(self) -> str:
        """
        Node id holding total contractual rent (potential gross income) across all
        leases.

        Returns
        -------
        str
            Node id for total potential gross income.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def free_rent_node(self) -> str:
        """
        Node id holding total free-rent concessions.

        Returns
        -------
        str
            Node id for total free-rent concessions.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def vacancy_loss_node(self) -> str:
        """
        Node id holding total vacancy loss, including occupancy and renewal-probability
        effects.

        Returns
        -------
        str
            Node id for total vacancy loss.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def rent_effective_node(self) -> str:
        """
        Node id holding total effective rent, the EGI rent component ``rent_pgi -
        free_rent - vacancy_loss``.

        Returns
        -------
        str
            Node id for total effective rent.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    def to_json(self) -> str:
        """
        Serialize this object to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation of this `RentRollOutputNodes`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    @staticmethod
    def from_json(json: str) -> RentRollOutputNodes:
        """
        Deserialize rent-roll output-node names from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload identifying potential, concession, vacancy, and
            effective-rent output nodes.

        Returns
        -------
        RentRollOutputNodes
            Validated `RentRollOutputNodes` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If ``json`` is malformed or violates this type's serialized schema.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import RentRollOutputNodes
        >>> nodes = RentRollOutputNodes()
        >>> RentRollOutputNodes.from_json(nodes.to_json()).vacancy_loss_node
        'vacancy_loss'

        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the node-id mapping as a single-row pandas ``DataFrame``.

        Columns: ``rent_pgi_node``, ``free_rent_node``, ``vacancy_loss_node``,
        ``rent_effective_node``. Every value is a statement node id, not a
        numeric amount.

        Returns
        -------
        pd.DataFrame
            One row of rent-roll output node ids.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

class ManagementFeeBase:
    """
    Basis for management fee calculation.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import ManagementFeeBase
    >>> ManagementFeeBase.Egi.value()
    'egi'

    """

    Egi: ManagementFeeBase
    EffectiveRent: ManagementFeeBase

    @staticmethod
    def from_str(value: str) -> ManagementFeeBase:
        """
        Parse a management-fee calculation basis.

        Parameters
        ----------
        value : str
            Case-insensitive ``"egi"`` or ``"effective_rent"`` basis value.

        Returns
        -------
        ManagementFeeBase
            Enum member corresponding to the exact snake-case fee basis.

        Raises
        ------
        ValueError
            If value is not egi or effective_rent.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import ManagementFeeBase
        >>> ManagementFeeBase.from_str("effective_rent").value()
        'effective_rent'

        """
        ...
    def value(self) -> str:
        """
        Return the canonical snake-case identifier for this variant.

        Returns
        -------
        str
            Exact JSON identifier, either ``"egi"`` or ``"effective_rent"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class ManagementFeeSpec:
    """
    Set a percentage management fee and the revenue base it applies to.

    Parameters
    ----------
    rate : float
        Decimal fee rate, such as ``0.03`` for a 3% management fee.
    base : ManagementFeeBase
        Effective-rent or EGI base used to calculate the fee; defaults to the
        binding's standard basis.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import ManagementFeeBase, ManagementFeeSpec
    >>> fee = ManagementFeeSpec(0.03, ManagementFeeBase.Egi)
    >>> (fee.rate, fee.base.value())
    (0.03, 'egi')

    """

    def __init__(self, rate: float, base: ManagementFeeBase = ...) -> None:
        """
        Define a percentage management fee and its revenue base.

        Parameters
        ----------
        rate : float
            Decimal fee rate, such as ``0.03`` for a 3% management fee.
        base : ManagementFeeBase
            Effective-rent or EGI base used to calculate the fee.

        Notes
        -----
        Construction does not raise; arguments are stored as supplied.
        """
        ...

    @property
    def rate(self) -> float:
        """
        Management fee rate as a decimal fraction (``0.03`` = 3%).

        Returns
        -------
        float
            Management fee rate as a decimal fraction.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def base(self) -> ManagementFeeBase:
        """
        Basis the fee applies to: ``egi`` (effective gross income) or ``effective_rent``
        (rent only, excluding other income).

        Returns
        -------
        ManagementFeeBase
            Basis the management fee applies to.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    def to_json(self) -> str:
        """
        Serialize this object to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation of this `ManagementFeeSpec`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    @staticmethod
    def from_json(json: str) -> ManagementFeeSpec:
        """
        Deserialize management-fee assumptions from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload containing the decimal rate and revenue basis.

        Returns
        -------
        ManagementFeeSpec
            Validated `ManagementFeeSpec` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If ``json`` is malformed or violates this type's serialized schema.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import ManagementFeeSpec
        >>> fee = ManagementFeeSpec(0.03)
        >>> ManagementFeeSpec.from_json(fee.to_json()).rate
        0.03

        """
        ...

class PropertyTemplateNodes:
    """
    Name generated node IDs for a property operating-statement template.

    Parameters
    ----------
    rent_roll : RentRollOutputNodes or None
        Optional rent-roll output names; ``None`` uses the template defaults.
    other_income_total_node : str
        Node ID aggregating other-income components.
    egi_node : str
        Node ID for effective gross income after rent and other income.
    management_fee_node : str
        Node ID for the management-fee expense series.
    opex_total_node : str
        Node ID aggregating operating-expense components.
    noi_node : str
        Node ID for net operating income before capital expenditures.
    capex_total_node : str
        Node ID aggregating capital-expenditure components.
    ncf_node : str
        Node ID for net cash flow after operating items and capital expenditure.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import PropertyTemplateNodes
    >>> nodes = PropertyTemplateNodes()
    >>> (nodes.noi_node, nodes.ncf_node, nodes.rent_roll.rent_effective_node)
    ('noi', 'ncf', 'rent_effective')

    """

    def __init__(
        self,
        rent_roll: RentRollOutputNodes | None = None,
        other_income_total_node: str = "other_income_total",
        egi_node: str = "egi",
        management_fee_node: str = "management_fee",
        opex_total_node: str = "opex_total",
        noi_node: str = "noi",
        capex_total_node: str = "capex_total",
        ncf_node: str = "ncf",
    ) -> None:
        """
        Name the generated statement nodes for a property operating template.

        Parameters
        ----------
        rent_roll : RentRollOutputNodes or None, default None
            Optional rent-roll output names; ``None`` uses the template defaults.
        other_income_total_node : str, default "other_income_total"
            Node ID aggregating other-income components.
        egi_node : str, default "egi"
            Node ID for effective gross income after rent and other income.
        management_fee_node : str, default "management_fee"
            Node ID for the management-fee expense series.
        opex_total_node : str, default "opex_total"
            Node ID aggregating operating-expense components.
        noi_node : str, default "noi"
            Node ID for net operating income before capital expenditures.
        capex_total_node : str, default "capex_total"
            Node ID aggregating capital-expenditure components.
        ncf_node : str, default "ncf"
            Node ID for net cash flow after operating items and capital expenditure.

        Notes
        -----
        Construction does not raise; arguments are stored as supplied.
        """
        ...

    @property
    def rent_roll(self) -> RentRollOutputNodes:
        """
        Rent-roll output node ids (PGI, free rent, vacancy loss, effective rent).

        Returns
        -------
        RentRollOutputNodes
            Rent-roll output node ids.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def other_income_total_node(self) -> str:
        """
        Node id holding total other (non-rent) income.

        Returns
        -------
        str
            Node id for total other income.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def egi_node(self) -> str:
        """
        Node id holding effective gross income (EGI).

        Returns
        -------
        str
            Node id for effective gross income.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def management_fee_node(self) -> str:
        """
        Node id holding the management fee, when one is configured.

        Returns
        -------
        str
            Node id for the management fee.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def opex_total_node(self) -> str:
        """
        Node id holding total operating expenses, inclusive of the management fee when
        one is configured.

        Returns
        -------
        str
            Node id for total operating expenses.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def noi_node(self) -> str:
        """
        Node id holding net operating income (NOI).

        Returns
        -------
        str
            Node id for net operating income.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def capex_total_node(self) -> str:
        """
        Node id holding total capital expenditure.

        Returns
        -------
        str
            Node id for total capital expenditure.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def ncf_node(self) -> str:
        """
        Node id holding net cash flow, ``noi - capex_total``.

        Returns
        -------
        str
            Node id for net cash flow.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    def to_json(self) -> str:
        """
        Serialize this object to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation of this `PropertyTemplateNodes`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    @staticmethod
    def from_json(json: str) -> PropertyTemplateNodes:
        """
        Deserialize property-template node names from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload identifying rent, income, expense, NOI, capex, and NCF
            nodes.

        Returns
        -------
        PropertyTemplateNodes
            Validated `PropertyTemplateNodes` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If ``json`` is malformed or violates this type's serialized schema.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import PropertyTemplateNodes
        >>> nodes = PropertyTemplateNodes()
        >>> PropertyTemplateNodes.from_json(nodes.to_json()).egi_node
        'egi'

        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the node-id mapping as a single-row pandas ``DataFrame``.

        Columns: ``rent_pgi_node``, ``free_rent_node``, ``vacancy_loss_node``,
        ``rent_effective_node``, ``other_income_total_node``, ``egi_node``,
        ``management_fee_node``, ``opex_total_node``, ``noi_node``,
        ``capex_total_node``, ``ncf_node``.

        The four rent-roll node ids are flattened in rather than nested, so
        every value is a plain statement node id, not a numeric amount.

        Returns
        -------
        pd.DataFrame
            One row of property template node ids.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

def add_noi_buildup(
    model: FinancialModelSpec | str,
    total_revenue_node: str,
    revenue_nodes: list[str],
    total_expenses_node: str,
    expense_nodes: list[str],
    noi_node: str,
) -> FinancialModelSpec:
    """
    Apply the NOI buildup template and return a typed ``FinancialModelSpec``.

    Parameters
    ----------
    model : FinancialModelSpec or str
        Model specification object or JSON to augment with NOI calculations.
    total_revenue_node : str
        Output node ID that sums the selected revenue nodes.
    revenue_nodes : list[str]
        Existing node IDs included as revenue in the NOI calculation.
    total_expenses_node : str
        Output node ID that sums the selected operating-expense nodes.
    expense_nodes : list[str]
        Existing node IDs included as operating expenses in the NOI calculation.
    noi_node : str
        Output node ID for revenue less operating expenses.

    Returns
    -------
    FinancialModelSpec
        Typed model specification containing revenue, expense, and NOI aggregation nodes.

    Raises
    ------
    ValueError
        If model JSON or a revenue, expense, or NOI node identifier is invalid.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import add_noi_buildup
    >>> from finstack_quant.statements import FinancialModelSpec, ModelBuilder
    >>> builder = ModelBuilder("template")
    >>> _ = builder.periods("2025Q1..Q2")
    >>> _ = builder.value("rent", [("2025Q1", 100.0)])
    >>> _ = builder.value("opex", [("2025Q1", 30.0)])
    >>> model = builder.build()
    >>> updated = add_noi_buildup(model, "revenue", ["rent"], "expenses", ["opex"], "noi")
    >>> updated.has_node("noi")
    True

    """
    ...

def add_ncf_buildup(
    model: FinancialModelSpec | str,
    noi_node: str,
    capex_nodes: list[str],
    ncf_node: str,
) -> FinancialModelSpec:
    """
    Apply the NCF buildup template and return a typed ``FinancialModelSpec``.

    Parameters
    ----------
    model : FinancialModelSpec or str
        Model specification object or JSON to augment with NCF calculations.
    noi_node : str
        Existing node ID supplying net operating income before capital spending.
    capex_nodes : list[str]
        Existing node IDs whose values are deducted as capital expenditures.
    ncf_node : str
        Output node ID for net operating income less capital expenditures.

    Returns
    -------
    FinancialModelSpec
        Typed model specification containing the NCF node after deducting the selected capex nodes.

    Raises
    ------
    ValueError
        If model JSON or an NOI, capital-expenditure, or NCF node identifier is invalid.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import add_ncf_buildup
    >>> from finstack_quant.statements import FinancialModelSpec, ModelBuilder
    >>> builder = ModelBuilder("template")
    >>> _ = builder.periods("2025Q1..Q2")
    >>> _ = builder.value("noi", [("2025Q1", 70.0)])
    >>> _ = builder.value("capex", [("2025Q1", 10.0)])
    >>> model = builder.build()
    >>> add_ncf_buildup(model, "noi", ["capex"], "ncf").has_node("ncf")
    True

    """
    ...

def add_rent_roll(
    model: FinancialModelSpec | str,
    leases: list[LeaseSpec],
    nodes: RentRollOutputNodes | None = None,
) -> FinancialModelSpec:
    """
    Apply the rich rent-roll template and return a typed ``FinancialModelSpec``.

    Parameters
    ----------
    model : FinancialModelSpec or str
        Model specification object or JSON to augment with rental-revenue nodes.
    leases : list[LeaseSpec]
        Rich lease schedules to calculate and aggregate into the rent roll.
    nodes : RentRollOutputNodes or None
        Optional aggregate output-node names; ``None`` uses template defaults.

    Returns
    -------
    FinancialModelSpec
        Typed model specification containing per-lease schedules and aggregate rent-roll nodes.

    Raises
    ------
    ValueError
        If model JSON, a lease specification, or a rent-roll output node is invalid.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import LeaseSpec, add_rent_roll
    >>> from finstack_quant.statements import FinancialModelSpec, ModelBuilder
    >>> builder = ModelBuilder("template")
    >>> _ = builder.periods("2025Q1..Q2")
    >>> model = builder.build()
    >>> lease = LeaseSpec("lease_a", "2025Q1", 100.0)
    >>> add_rent_roll(model, [lease]).has_node("rent_effective")
    True

    """
    ...

def add_property_operating_statement(
    model: FinancialModelSpec | str,
    leases: list[LeaseSpec],
    other_income_nodes: list[str],
    opex_nodes: list[str],
    capex_nodes: list[str],
    management_fee: ManagementFeeSpec | None = None,
    nodes: PropertyTemplateNodes | None = None,
) -> FinancialModelSpec:
    """
    Apply the full property operating-statement template and return a typed model.

    Parameters
    ----------
    model : FinancialModelSpec or str
        Model specification object or JSON to augment with property statements.
    leases : list[LeaseSpec]
        Rich lease schedules used to build rental-revenue and rent-roll outputs.
    other_income_nodes : list[str]
        Existing node IDs aggregated as other income (required; pass ``[]`` for none).
    opex_nodes : list[str]
        Existing node IDs aggregated as operating expenses (required; may be ``[]``).
    capex_nodes : list[str]
        Existing node IDs aggregated as capital expenditures (required; may be ``[]``).
    management_fee : ManagementFeeSpec or None
        Optional fee assumptions; ``None`` omits management-fee calculation.
    nodes : PropertyTemplateNodes or None
        Optional generated-node names; ``None`` uses the template defaults.

    Returns
    -------
    FinancialModelSpec
        Typed model specification containing the rent roll, EGI, NOI, capex, and NCF buildup.

    Raises
    ------
    ValueError
        If model JSON, a lease or fee specification, or an operating-statement node is invalid.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import LeaseSpec, add_property_operating_statement
    >>> from finstack_quant.statements import FinancialModelSpec, ModelBuilder
    >>> builder = ModelBuilder("template")
    >>> _ = builder.periods("2025Q1..Q2")
    >>> model = builder.build()
    >>> lease = LeaseSpec("lease_a", "2025Q1", 100.0)
    >>> updated = add_property_operating_statement(model, [lease], [], [], [])
    >>> updated.has_node("ncf")
    True

    """
    ...
