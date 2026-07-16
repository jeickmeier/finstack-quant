"""Credit risk models: academic scoring, PD calibration, and LGD / EAD.

Bindings for ``finstack_quant_core::credit``. Each submodule mirrors the Rust
module of the same name and is registered at runtime in ``sys.modules``
so that ``from finstack_quant.core.credit import scoring`` (or ``pd``, ``lgd``,
``migration``, ``recovery_waterfall``) works transparently.
"""

from __future__ import annotations

__all__ = ["scoring", "pd", "lgd", "migration", "recovery_waterfall"]

class recovery_waterfall:
    """Absolute-priority recovery allocation with estate-inclusive collateral."""

    class RecoveryClaim:
        def __init__(
            self,
            id: str,
            seniority: str,
            priority: int,
            principal: float,
            accrued: float = 0.0,
            penalties: float = 0.0,
            collateral: tuple[float, float] | None = None,
        ) -> None: ...
        @property
        def id(self) -> str: ...
        @property
        def seniority(self) -> str: ...
        @property
        def priority(self) -> int: ...
        @property
        def principal(self) -> float: ...
        @property
        def accrued(self) -> float: ...
        @property
        def penalties(self) -> float: ...
        @property
        def collateral_value(self) -> float | None: ...
        @property
        def collateral_haircut(self) -> float: ...
        @property
        def total_claim(self) -> float: ...

    class RecoveryAllocation:
        @property
        def id(self) -> str: ...
        @property
        def seniority(self) -> str: ...
        @property
        def priority(self) -> int: ...
        @property
        def total_claim(self) -> float: ...
        @property
        def collateral_recovery(self) -> float: ...
        @property
        def general_recovery(self) -> float: ...
        @property
        def total_recovery(self) -> float: ...
        @property
        def recovery_rate(self) -> float: ...
        @property
        def deficiency(self) -> float: ...

    class RecoveryWaterfallResult:
        @property
        def total_distributed(self) -> float: ...
        @property
        def undistributed_estate(self) -> float: ...
        @property
        def apr_satisfied(self) -> bool: ...
        @property
        def allocations(self) -> list[recovery_waterfall.RecoveryAllocation]: ...

    @staticmethod
    def allocate_recovery(
        estate_value: float,
        claims: list[recovery_waterfall.RecoveryClaim],
    ) -> recovery_waterfall.RecoveryWaterfallResult: ...

class scoring:
    """Academic credit scoring: Altman Z-Score family, Ohlson O-Score, Zmijewski."""

    class AltmanPdCalibration:
        """Explicit versioned Altman score-to-PD heuristics."""

        HEURISTIC_V1: scoring.AltmanPdCalibration

    @staticmethod
    def altman_z_score(
        working_capital_to_total_assets: float,
        retained_earnings_to_total_assets: float,
        ebit_to_total_assets: float,
        market_equity_to_total_liabilities: float,
        sales_to_total_assets: float,
        pd_calibration: AltmanPdCalibration | None = None,
    ) -> tuple[float, str, float | None]:
        """Original Altman Z-Score (1968) for publicly traded manufacturers.

        Parameters
        ----------
        working_capital_to_total_assets : float
            Working capital / total assets (X1).
        retained_earnings_to_total_assets : float
            Retained earnings / total assets (X2).
        ebit_to_total_assets : float
            EBIT / total assets (X3).
        market_equity_to_total_liabilities : float
            Market equity / total liabilities (X4).
        sales_to_total_assets : float
            Sales / total assets (X5).
        pd_calibration : AltmanPdCalibration | None
            Explicit score-to-PD mapping. ``HEURISTIC_V1`` is an uncalibrated
            house heuristic, not an empirical Altman calibration.

        Returns
        -------
        tuple[float, str, float | None]
            ``(score, zone, implied_pd)`` where ``zone`` is one of
            ``"safe"``, ``"grey"``, or ``"distress"``. ``implied_pd`` is
            ``None`` unless ``pd_calibration`` is supplied.

        Raises
        ------
        ValueError
            If any ratio is non-finite.

        Sources
        -------
        See ``docs/REFERENCES.md#altman-1968``.

        Examples
        --------
        >>> from finstack_quant.core.credit import scoring
        >>> score, zone, pd = scoring.altman_z_score(0.2, 0.3, 0.15, 1.5, 1.0)
        >>> zone
        'safe'
        """
        ...

    @staticmethod
    def altman_z_prime(
        working_capital_to_total_assets: float,
        retained_earnings_to_total_assets: float,
        ebit_to_total_assets: float,
        book_equity_to_total_liabilities: float,
        sales_to_total_assets: float,
        pd_calibration: AltmanPdCalibration | None = None,
    ) -> tuple[float, str, float | None]:
        """Altman Z'-Score (1983) for private firms.

        Parameters
        ----------
        working_capital_to_total_assets, retained_earnings_to_total_assets,
        ebit_to_total_assets, book_equity_to_total_liabilities,
        sales_to_total_assets : float
            Balance-sheet ratios as in Altman (1983); book equity replaces
            market equity versus the original Z-Score.

        Returns
        -------
        tuple[float, str, float | None]
            ``(score, zone, implied_pd)`` where ``zone`` is ``"safe"``,
            ``"grey"``, or ``"distress"``. PD is absent unless an explicit
            versioned heuristic is supplied.

        Sources
        -------
        - Altman (1968/1983): see docs/REFERENCES.md#altman-1968

        Examples
        --------
        >>> from finstack_quant.core.credit import scoring
        >>> score, zone, pd = scoring.altman_z_prime(0.2, 0.3, 0.15, 1.5, 1.0)
        >>> zone in ("safe", "grey", "distress")
        True
        """
        ...

    @staticmethod
    def altman_z_double_prime(
        working_capital_to_total_assets: float,
        retained_earnings_to_total_assets: float,
        ebit_to_total_assets: float,
        book_equity_to_total_liabilities: float,
        pd_calibration: AltmanPdCalibration | None = None,
    ) -> tuple[float, str, float | None]:
        """Altman Z''-Score for non-manufacturing firms (non-EM model, no constant).

        Returns ``(score, zone, implied_pd)``; PD is ``None`` unless an
        explicit versioned heuristic is supplied.

        Examples
        --------
        >>> from finstack_quant.core.credit import scoring
        >>> score, zone, pd = scoring.altman_z_double_prime(0.2, 0.3, 0.15, 1.5)
        >>> zone in ("safe", "grey", "distress")
        True
        """
        ...

    @staticmethod
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
    ) -> tuple[float, str, float]:
        """Ohlson O-Score (1980) logistic bankruptcy model.

        Returns ``(score, zone, implied_pd)``.

        Examples
        --------
        >>> from finstack_quant.core.credit import scoring
        >>> score, zone, pd = scoring.ohlson_o_score(-0.5, 0.5, 0.1, 0.8, 0.0, 0.05, 0.2, 1.0, -0.01)
        >>> zone in ("safe", "grey", "distress")
        True
        """
        ...

    @staticmethod
    def zmijewski_score(
        net_income_to_total_assets: float,
        total_liabilities_to_total_assets: float,
        current_assets_to_current_liabilities: float,
    ) -> tuple[float, str, float]:
        """Zmijewski (1984) probit bankruptcy score.

        Returns ``(score, zone, implied_pd)``.

        Examples
        --------
        >>> from finstack_quant.core.credit import scoring
        >>> score, zone, pd = scoring.zmijewski_score(0.05, 0.4, 1.5)
        >>> zone in ("safe", "grey", "distress")
        True
        """
        ...

class pd:
    """Probability of default: PiT/TtC conversion and central-tendency calibration."""

    @staticmethod
    def pit_to_ttc(pit_pd: float, asset_correlation: float, cycle_index: float) -> float:
        r"""Convert a Point-in-Time PD to Through-the-Cycle via Merton-Vasicek.

        ``PD_TtC = Phi( Phi^{-1}(PD_PiT) * sqrt(1 - rho) + sqrt(rho) * z )``.

        Parameters
        ----------
        pit_pd:
            Point-in-time probability of default in ``(0, 1)``.
        asset_correlation:
            Asset correlation ``rho`` in ``[0, 1)``.
        cycle_index:
            Standardized credit cycle index ``z`` (negative = downturn,
            positive = benign).

        Returns
        -------
        float
            Through-the-cycle PD in ``(0, 1)``.

        Examples
        --------
        >>> from finstack_quant.core.credit import pd
        >>> ttc = pd.pit_to_ttc(0.02, 0.12, 0.0)  # doctest: +SKIP
        """
        ...

    @staticmethod
    def ttc_to_pit(ttc_pd: float, asset_correlation: float, cycle_index: float) -> float:
        r"""Convert a Through-the-Cycle PD to Point-in-Time via Merton-Vasicek.

        ``PD_PiT = Phi( (Phi^{-1}(PD_TtC) - sqrt(rho) * z) / sqrt(1 - rho) )``.

        Parameters
        ----------
        ttc_pd:
            Through-the-cycle probability of default in ``(0, 1)``.
        asset_correlation:
            Asset correlation ``rho`` in ``[0, 1)``.
        cycle_index:
            Standardized credit cycle index ``z`` (negative = downturn,
            positive = benign).

        Returns
        -------
        float
            Point-in-time PD in ``(0, 1)``.

        Examples
        --------
        >>> from finstack_quant.core.credit import pd
        >>> pit = pd.ttc_to_pit(0.02, 0.12, 1.0)  # doctest: +SKIP
        """
        ...

    @staticmethod
    def central_tendency(annual_default_rates: list[float]) -> float:
        """Arithmetic-mean long-run PD from annual default rates (regulatory TtC).

        Parameters
        ----------
        annual_default_rates:
            Observed annual default rates as decimals.

        Returns
        -------
        float
            Long-run average PD.

        Examples
        --------
        >>> from finstack_quant.core.credit import pd
        >>> pd.central_tendency([0.01, 0.02, 0.015])  # doctest: +SKIP
        0.015
        """
        ...

class lgd:
    """Loss-given-default: seniority recovery, workout LGD, downturn adjustments, EAD."""

    @staticmethod
    def seniority_recovery_stats(
        seniority: str,
        rating_agency: str | None = None,
    ) -> dict[str, float]:
        """Historical recovery moments for a seniority class.

        If ``rating_agency`` is omitted, the Rust credit-assumptions registry
        default seniority calibration is used.

        Parameters
        ----------
        seniority:
            Seniority label (e.g. ``"senior_secured"``, ``"senior_unsecured"``,
            ``"subordinated"``).
        rating_agency:
            Optional agency source (e.g. ``"Moody"``, ``"S&P"``).

        Returns
        -------
        dict[str, float]
            Dict with keys ``{"mean", "std", "alpha", "beta"}``.

        Examples
        --------
        >>> from finstack_quant.core.credit import lgd
        >>> stats = lgd.seniority_recovery_stats("senior_secured")  # doctest: +SKIP
        >>> "mean" in stats
        True
        """
        ...

    @staticmethod
    def beta_recovery_sample(
        mean: float,
        std: float,
        n_samples: int,
        seed: int,
    ) -> list[float]:
        """Sample ``n_samples`` recoveries from Beta(alpha, beta) via PCG64.

        Parameters
        ----------
        mean:
            Target mean recovery rate in ``(0, 1)``.
        std:
            Target standard deviation of recovery rate.
        n_samples:
            Number of samples to draw.
        seed:
            Random seed for reproducibility.

        Returns
        -------
        list[float]
            Sampled recovery rates in ``(0, 1)``.

        Examples
        --------
        >>> from finstack_quant.core.credit import lgd
        >>> samples = lgd.beta_recovery_sample(0.4, 0.2, 100, 42)  # doctest: +SKIP
        """
        ...

    @staticmethod
    def beta_recovery_quantile(mean: float, std: float, q: float) -> float:
        """Quantile ``q`` of a Beta recovery distribution parameterized by (mean, std).

        Parameters
        ----------
        mean:
            Mean recovery rate in ``(0, 1)``.
        std:
            Standard deviation of recovery rate.
        q:
            Quantile in ``[0, 1]``.

        Returns
        -------
        float
            Recovery rate at the given quantile.

        Examples
        --------
        >>> from finstack_quant.core.credit import lgd
        >>> lgd.beta_recovery_quantile(0.4, 0.2, 0.95)  # doctest: +SKIP
        """
        ...

    @staticmethod
    def workout_lgd(
        ead: float,
        collateral: list[tuple[str, float, float]],
        direct_cost_pct: float,
        indirect_cost_pct: float,
        time_to_resolution_years: float,
        discount_rate: float,
    ) -> tuple[float, float]:
        """Workout LGD from collateral waterfall, costs, and discounting.

        Parameters
        ----------
        ead:
            Exposure at default.
        collateral:
            List of ``(collateral_id, recovery_value, recovery_rate)`` tuples
            in priority order.
        direct_cost_pct:
            Direct workout costs as a fraction of EAD.
        indirect_cost_pct:
            Indirect workout costs as a fraction of EAD.
        time_to_resolution_years:
            Time from default to workout resolution.
        discount_rate:
            Annual discount rate for time-value adjustment.

        Returns
        -------
        tuple[float, float]
            ``(net_recovery, lgd)`` with ``lgd`` clamped to ``[0, 1]``.

        Examples
        --------
        >>> from finstack_quant.core.credit import lgd
        >>> net_rec, loss = lgd.workout_lgd(100.0, [("cash", 40.0, 1.0)], 0.02, 0.01, 1.5, 0.05)  # doctest: +SKIP
        """
        ...

    @staticmethod
    def downturn_lgd_stressed(
        base_lgd: float,
        asset_correlation: float,
        lgd_sensitivity: float,
        stress_quantile: float,
    ) -> float:
        """Stressed downturn LGD adjustment, clamped to ``[0, 1]``.

        Proprietary mean-plus-multiple-of-Bernoulli-stdev approximation
        (not the Frye-Jacobs 2012 model). Typical ``lgd_sensitivity``:
        0.3-0.5.

        Parameters
        ----------
        base_lgd:
            Baseline LGD in ``[0, 1]``.
        asset_correlation:
            Asset correlation ``rho`` in ``[0, 1)``.
        lgd_sensitivity:
            Sensitivity of LGD to systematic risk (typical: 0.3-0.5).
        stress_quantile:
            Quantile of the systematic factor for stress (e.g. ``0.999``).

        Returns
        -------
        float
            Stressed LGD in ``[0, 1]``.

        Examples
        --------
        >>> from finstack_quant.core.credit import lgd
        >>> lgd.downturn_lgd_stressed(0.4, 0.12, 0.3, 0.999)  # doctest: +SKIP
        """
        ...

    @staticmethod
    def downturn_lgd_regulatory_floor(
        base_lgd: float,
        add_on: float,
        floor: float,
    ) -> float:
        """Regulatory-floor downturn LGD: ``max(base + add_on, floor)`` clamped to ``[0, 1]``.

        Parameters
        ----------
        base_lgd:
            Baseline LGD in ``[0, 1]``.
        add_on:
            Downturn add-on.
        floor:
            Regulatory floor LGD.

        Returns
        -------
        float
            Floored downturn LGD in ``[0, 1]``.

        Examples
        --------
        >>> from finstack_quant.core.credit import lgd
        >>> lgd.downturn_lgd_regulatory_floor(0.4, 0.05, 0.45)  # doctest: +SKIP
        """
        ...

    @staticmethod
    def ead_term_loan(principal: float) -> float:
        """Exposure at default for a fully drawn term loan (equal to principal).

        Parameters
        ----------
        principal:
            Outstanding principal amount.

        Returns
        -------
        float
            EAD equal to ``principal``.

        Examples
        --------
        >>> from finstack_quant.core.credit import lgd
        >>> lgd.ead_term_loan(1_000_000.0)  # doctest: +SKIP
        1000000.0
        """
        ...

    @staticmethod
    def ead_revolver(drawn: float, undrawn: float, ccf: float) -> float:
        """Exposure at default for a revolver: ``drawn + undrawn * ccf``.

        Parameters
        ----------
        drawn:
            Drawn amount.
        undrawn:
            Undrawn commitment.
        ccf:
            Credit conversion factor in ``[0, 1]``.

        Returns
        -------
        float
            EAD for the revolver facility.

        Examples
        --------
        >>> from finstack_quant.core.credit import lgd
        >>> lgd.ead_revolver(50.0, 50.0, 0.5)  # doctest: +SKIP
        75.0
        """
        ...

class migration:
    """Credit migration: rating scales, transition matrices, generators, and CTMC simulation.

    Example
    -------
    >>> from finstack_quant.core.credit import migration
    >>> scale = migration.RatingScale.standard()  # doctest: +SKIP
    >>> scale.n_states()  # doctest: +SKIP
    8
    """

    class RatingScale:
        """Ordinal rating scale for credit migration modelling.

        Provides standard agency scales (S&P/Moody's/Fitch) or custom scales
        with an optional default absorbing state.

        Example
        -------
        >>> from finstack_quant.core.credit import migration
        >>> scale = migration.RatingScale.standard()  # doctest: +SKIP
        >>> scale.labels()  # doctest: +SKIP
        ['AAA', 'AA', 'A', 'BBB', 'BB', 'B', 'CCC', 'D']
        """

        @staticmethod
        def standard() -> migration.RatingScale:
            """Standard 8-state agency scale (AAA through D).

            Returns
            -------
            migration.RatingScale
                Scale with labels ``AAA, AA, A, BBB, BB, B, CCC, D``.

            Example
            -------
            >>> scale = migration.RatingScale.standard()  # doctest: +SKIP
            """
            ...

        @staticmethod
        def standard_with_nr() -> migration.RatingScale:
            """Standard scale with an explicit ``NR`` (not rated) state.

            Returns
            -------
            migration.RatingScale
                Scale with labels ``AAA, AA, A, BBB, BB, B, CCC, D, NR``.

            Example
            -------
            >>> scale = migration.RatingScale.standard_with_nr()  # doctest: +SKIP
            """
            ...

        @staticmethod
        def notched() -> migration.RatingScale:
            """Notched 18-state scale (AAA through CCC- and D).

            Returns
            -------
            migration.RatingScale
                Scale with notched sub-grades.

            Example
            -------
            >>> scale = migration.RatingScale.notched()  # doctest: +SKIP
            """
            ...

        @staticmethod
        def custom(labels: list[str]) -> migration.RatingScale:
            """Build a custom rating scale from an ordered label list.

            Parameters
            ----------
            labels:
                Rating labels from best to worst.  The last label is the
                default absorbing state.

            Returns
            -------
            migration.RatingScale
                Custom scale.

            Example
            -------
            >>> scale = migration.RatingScale.custom(["A", "B", "C", "D"])  # doctest: +SKIP
            """
            ...

        @staticmethod
        def custom_with_default(labels: list[str], default_label: str) -> migration.RatingScale:
            """Build a custom rating scale with an explicit default label.

            Parameters
            ----------
            labels:
                Non-default rating labels from best to worst.
            default_label:
                Label for the default absorbing state.

            Returns
            -------
            migration.RatingScale
                Custom scale with the default state appended.

            Example
            -------
            >>> scale = migration.RatingScale.custom_with_default(["A", "B", "C"], "DEFAULT")  # doctest: +SKIP
            """
            ...

        def n_states(self) -> int:
            """Number of states on this scale (including default if present).

            Returns
            -------
            int
                State count.

            Example
            -------
            >>> scale = migration.RatingScale.standard()  # doctest: +SKIP
            >>> scale.n_states()  # doctest: +SKIP
            8
            """
            ...

        def index_of(self, label: str) -> int | None:
            """Return the 0-based index of ``label``, or ``None`` if not found.

            Parameters
            ----------
            label:
                Rating label to look up.

            Returns
            -------
            int or None
                State index, or ``None`` when the label is not on this scale.

            Example
            -------
            >>> scale = migration.RatingScale.standard()  # doctest: +SKIP
            >>> scale.index_of("BBB")  # doctest: +SKIP
            3
            """
            ...

        def default_state(self) -> int | None:
            """Return the index of the default absorbing state, or ``None``.

            Returns
            -------
            int or None
                Default state index, or ``None`` if no default state exists.

            Example
            -------
            >>> scale = migration.RatingScale.standard()  # doctest: +SKIP
            >>> scale.default_state()  # doctest: +SKIP
            7
            """
            ...

        def labels(self) -> list[str]:
            """Return all rating labels in ordinal order.

            Returns
            -------
            list[str]
                Ordered label list.

            Example
            -------
            >>> scale = migration.RatingScale.standard()  # doctest: +SKIP
            >>> scale.labels()  # doctest: +SKIP
            ['AAA', 'AA', 'A', 'BBB', 'BB', 'B', 'CCC', 'D']
            """
            ...

        def warf(self, label: str) -> float:
            """Weighted average rating factor (WARF) for a rating label.

            Parameters
            ----------
            label:
                Rating label on this scale.

            Returns
            -------
            float
                WARF value (higher = riskier).

            Raises
            ------
            ValueError
                If ``label`` is not on this scale.

            Example
            -------
            >>> scale = migration.RatingScale.standard()  # doctest: +SKIP
            >>> scale.warf("BBB")  # doctest: +SKIP
            250.0
            """
            ...

        def rating_from_warf(self, warf: float) -> str:
            """Map a WARF value back to the closest rating label.

            Parameters
            ----------
            warf:
                Weighted average rating factor.

            Returns
            -------
            str
                Rating label whose WARF bucket contains the given value.

            Example
            -------
            >>> scale = migration.RatingScale.standard()  # doctest: +SKIP
            >>> scale.rating_from_warf(250.0)  # doctest: +SKIP
            'BBB'
            """
            ...

    class TransitionMatrix:
        """Discrete-horizon rating transition probability matrix.

        Parameters
        ----------
        scale:
            Rating scale defining the row/column ordering.
        data:
            Row-major transition probabilities (length ``n_states * n_states``).
        horizon:
            Time horizon in years (e.g. ``1.0`` for a 1-year matrix).

        Raises
        ------
        ValueError
            If ``data`` length does not match ``scale.n_states() ** 2`` or rows
            do not sum to 1.

        Example
        -------
        >>> from finstack_quant.core.credit import migration
        >>> scale = migration.RatingScale.standard()  # doctest: +SKIP
        >>> tm = migration.TransitionMatrix(scale, [...], 1.0)  # doctest: +SKIP
        """

        def __init__(self, scale: migration.RatingScale, data: list[float], horizon: float) -> None: ...
        def probability(self, from_: str, to: str) -> float:
            """Transition probability from one rating to another.

            Parameters
            ----------
            from_:
                Origin rating label.
            to:
                Destination rating label.

            Returns
            -------
            float
                Transition probability in ``[0, 1]``.

            Example
            -------
            >>> tm.probability("BBB", "BB")  # doctest: +SKIP
            0.04
            """
            ...

        def row(self, from_: str) -> list[float]:
            """Return the full transition row for a given origin rating.

            Parameters
            ----------
            from_:
                Origin rating label.

            Returns
            -------
            list[float]
                Transition probabilities to every state on the scale.

            Example
            -------
            >>> tm.row("BBB")  # doctest: +SKIP
            [0.9, 0.05, 0.04, ...]
            """
            ...

        def to_matrix(self) -> list[list[float]]:
            """Return the full transition matrix as nested lists.

            Returns
            -------
            list[list[float]]
                Row-major matrix of transition probabilities.

            Example
            -------
            >>> tm.to_matrix()  # doctest: +SKIP
            [[0.95, 0.04, ...], ...]
            """
            ...

        def horizon(self) -> float:
            """Return the time horizon in years.

            Returns
            -------
            float
                Horizon (e.g. ``1.0``).

            Example
            -------
            >>> tm.horizon()  # doctest: +SKIP
            1.0
            """
            ...

        def n_states(self) -> int:
            """Return the number of states on the underlying scale.

            Returns
            -------
            int
                State count.

            Example
            -------
            >>> tm.n_states()  # doctest: +SKIP
            8
            """
            ...

        def default_probabilities(self) -> list[float] | None:
            """Return per-state default probabilities, or ``None`` if no default state.

            Returns
            -------
            list[float] or None
                Probability of default from each state, or ``None`` when the
                scale has no default absorbing state.

            Example
            -------
            >>> tm.default_probabilities()  # doctest: +SKIP
            [0.0, 0.001, 0.005, ...]
            """
            ...

    class GeneratorMatrix:
        """Continuous-time generator matrix (Q) for CTMC credit migration.

        Parameters
        ----------
        scale:
            Rating scale defining the row/column ordering.
        data:
            Row-major generator intensities (length ``n_states * n_states``).

        Raises
        ------
        ValueError
            If ``data`` length does not match ``scale.n_states() ** 2`` or rows
            do not sum to zero.

        Example
        -------
        >>> from finstack_quant.core.credit import migration
        >>> scale = migration.RatingScale.standard()  # doctest: +SKIP
        >>> gm = migration.GeneratorMatrix(scale, [...])  # doctest: +SKIP
        """

        def __init__(self, scale: migration.RatingScale, data: list[float]) -> None: ...
        @staticmethod
        def from_transition_matrix(p: migration.TransitionMatrix) -> migration.GeneratorMatrix:
            """Estimate a generator matrix from a discrete transition matrix.

            Uses the eigendecomposition method (Israel, Rosenthal, Wei 2001).

            Parameters
            ----------
            p:
                A :class:`migration.TransitionMatrix` to invert.

            Returns
            -------
            migration.GeneratorMatrix
                Estimated generator matrix.

            Example
            -------
            >>> gm = migration.GeneratorMatrix.from_transition_matrix(tm)  # doctest: +SKIP
            """
            ...

        def intensity(self, from_: str, to: str) -> float:
            """Generator intensity (migration rate) from one state to another.

            Parameters
            ----------
            from_:
                Origin rating label.
            to:
                Destination rating label.

            Returns
            -------
            float
                Generator intensity.  Diagonal entries are negative (exit rates).

            Example
            -------
            >>> gm.intensity("BBB", "BB")  # doctest: +SKIP
            0.04
            """
            ...

        def exit_rate(self, state: str) -> float:
            """Total exit rate (sum of off-diagonal intensities) for a state.

            Parameters
            ----------
            state:
                Rating label.

            Returns
            -------
            float
                Non-negative exit rate.  The default absorbing state has rate 0.

            Example
            -------
            >>> gm.exit_rate("BBB")  # doctest: +SKIP
            0.06
            """
            ...

        def to_matrix(self) -> list[list[float]]:
            """Return the full generator matrix as nested lists.

            Returns
            -------
            list[list[float]]
                Row-major generator matrix.

            Example
            -------
            >>> gm.to_matrix()  # doctest: +SKIP
            [[-0.06, 0.01, ...], ...]
            """
            ...

        def n_states(self) -> int:
            """Return the number of states on the underlying scale.

            Returns
            -------
            int
                State count.

            Example
            -------
            >>> gm.n_states()  # doctest: +SKIP
            8
            """
            ...

        @property
        def regularization_l1(self) -> float:
            """L1 mass clamped by Kreinin-Sidenius regularization.

            Returns ``0.0`` for directly constructed generators.
            """
            ...

        @property
        def round_trip_error(self) -> float:
            """Infinity-norm reconstruction error against the source matrix.

            Returns ``0.0`` for directly constructed generators.
            """
            ...

    class RatingPath:
        """Simulated rating migration path over a time horizon.

        Produced by :meth:`migration.MigrationSimulator.simulate`.

        Example
        -------
        >>> path = simulator.simulate(3, 1, 42)[0]  # doctest: +SKIP
        >>> path.label_at(0.5)  # doctest: +SKIP
        'BBB'
        """

        def state_at(self, t: float) -> int:
            """Return the state index occupied at time ``t``.

            Parameters
            ----------
            t:
                Time in years within ``[0, horizon]``.

            Returns
            -------
            int
                State index at time ``t``.

            Example
            -------
            >>> path.state_at(0.5)  # doctest: +SKIP
            3
            """
            ...

        def label_at(self, t: float) -> str:
            """Return the rating label occupied at time ``t``.

            Parameters
            ----------
            t:
                Time in years within ``[0, horizon]``.

            Returns
            -------
            str
                Rating label at time ``t``.

            Example
            -------
            >>> path.label_at(0.5)  # doctest: +SKIP
            'BBB'
            """
            ...

        def defaulted(self) -> bool:
            """Return whether this path entered the default state.

            Returns
            -------
            bool
                ``True`` if the path defaulted at any point.

            Example
            -------
            >>> path.defaulted()  # doctest: +SKIP
            False
            """
            ...

        def default_time(self) -> float | None:
            """Return the time of default, or ``None`` if not defaulted.

            Returns
            -------
            float or None
                Default time in years, or ``None``.

            Example
            -------
            >>> path.default_time()  # doctest: +SKIP
            None
            """
            ...

        def n_transitions(self) -> int:
            """Return the number of rating transitions in this path.

            Returns
            -------
            int
                Transition count (excluding the initial state).

            Example
            -------
            >>> path.n_transitions()  # doctest: +SKIP
            2
            """
            ...

        def transitions(self) -> list[tuple[float, int]]:
            """Return all transitions as ``(time, new_state)`` pairs.

            Returns
            -------
            list[tuple[float, int]]
                Ordered list of transition events.

            Example
            -------
            >>> path.transitions()  # doctest: +SKIP
            [(0.3, 4), (0.7, 3)]
            """
            ...

        def horizon(self) -> float:
            """Return the simulation horizon in years.

            Returns
            -------
            float
                Horizon.

            Example
            -------
            >>> path.horizon()  # doctest: +SKIP
            1.0
            """
            ...

    class MigrationSimulator:
        """CTMC simulator for credit rating migration paths.

        Parameters
        ----------
        generator:
            Generator matrix defining migration intensities.
        horizon:
            Simulation horizon in years.

        Example
        -------
        >>> from finstack_quant.core.credit import migration
        >>> gm = migration.GeneratorMatrix(scale, [...])  # doctest: +SKIP
        >>> sim = migration.MigrationSimulator(gm, 1.0)  # doctest: +SKIP
        """

        def __init__(self, generator: migration.GeneratorMatrix, horizon: float) -> None: ...
        def simulate(
            self,
            initial_state: int,
            n_paths: int,
            seed: int,
        ) -> list[migration.RatingPath]:
            """Simulate rating migration paths from a single starting state.

            Parameters
            ----------
            initial_state:
                0-based index of the starting rating.
            n_paths:
                Number of independent paths to simulate.
            seed:
                Random seed for reproducibility.

            Returns
            -------
            list[migration.RatingPath]
                Simulated paths.

            Example
            -------
            >>> paths = sim.simulate(3, 1000, 42)  # doctest: +SKIP
            >>> len(paths)  # doctest: +SKIP
            1000
            """
            ...

        def empirical_matrix(self, n_paths_per_state: int, seed: int) -> migration.TransitionMatrix:
            """Estimate a transition matrix by Monte Carlo simulation.

            Simulates ``n_paths_per_state`` paths from every non-default state
            and computes the empirical transition probabilities.

            Parameters
            ----------
            n_paths_per_state:
                Number of paths to simulate per starting state.
            seed:
                Random seed for reproducibility.

            Returns
            -------
            migration.TransitionMatrix
                Empirically estimated transition matrix.

            Example
            -------
            >>> tm = sim.empirical_matrix(5000, 42)  # doctest: +SKIP
            """
            ...

        def horizon(self) -> float:
            """Return the simulation horizon in years.

            Returns
            -------
            float
                Horizon.

            Example
            -------
            >>> sim.horizon()  # doctest: +SKIP
            1.0
            """
            ...

    @staticmethod
    def project(generator: migration.GeneratorMatrix, t: float) -> migration.TransitionMatrix:
        """Compute the transition matrix at time ``t`` via matrix exponential.

        Computes ``P(t) = exp(Q * t)`` where ``Q`` is the generator matrix.

        Parameters
        ----------
        generator:
            Generator matrix to project.
        t:
            Time horizon in years.

        Returns
        -------
        migration.TransitionMatrix
            Transition matrix at time ``t``.

        Example
        -------
        >>> tm = migration.project(gm, 5.0)  # doctest: +SKIP
        """
        ...
