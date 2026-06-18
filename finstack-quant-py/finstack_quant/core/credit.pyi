"""Credit risk models: academic scoring, PD calibration, and LGD / EAD.

Bindings for ``finstack_quant_core::credit``. Each submodule mirrors the Rust
module of the same name and is registered at runtime in ``sys.modules``
so that ``from finstack_quant.core.credit import scoring`` (or ``pd``, ``lgd``,
``migration``) works transparently.
"""

from __future__ import annotations

__all__ = ["scoring", "pd", "lgd", "migration"]

class scoring:
    """Academic credit scoring: Altman Z-Score family, Ohlson O-Score, Zmijewski."""

    @staticmethod
    def altman_z_score(
        working_capital_to_total_assets: float,
        retained_earnings_to_total_assets: float,
        ebit_to_total_assets: float,
        market_equity_to_total_liabilities: float,
        sales_to_total_assets: float,
    ) -> tuple[float, str, float]:
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

        Returns
        -------
        tuple[float, str, float]
            ``(score, zone, implied_pd)`` where ``zone`` is one of
            ``"safe"``, ``"grey"``, or ``"distress"``.

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
    ) -> tuple[float, str, float]:
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
        tuple[float, str, float]
            ``(score, zone, implied_pd)`` where ``zone`` is ``"safe"``,
            ``"grey"``, or ``"distress"``.

        Sources
        -------
        - Altman (1968/1983): see docs/REFERENCES.md#altman-1968
        """
        ...

    @staticmethod
    def altman_z_double_prime(
        working_capital_to_total_assets: float,
        retained_earnings_to_total_assets: float,
        ebit_to_total_assets: float,
        book_equity_to_total_liabilities: float,
    ) -> tuple[float, str, float]:
        """Altman Z''-Score for non-manufacturing firms (non-EM model, no constant).

        Returns ``(score, zone, implied_pd)``.
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
        """
        ...

class pd:
    """Probability of default: PiT/TtC conversion and central-tendency calibration."""

    @staticmethod
    def pit_to_ttc(pit_pd: float, asset_correlation: float, cycle_index: float) -> float:
        r"""Convert a Point-in-Time PD to Through-the-Cycle via Merton-Vasicek.

        ``PD_TtC = Phi( Phi^{-1}(PD_PiT) * sqrt(1 - rho) + sqrt(rho) * z )``.
        """
        ...

    @staticmethod
    def ttc_to_pit(ttc_pd: float, asset_correlation: float, cycle_index: float) -> float:
        r"""Convert a Through-the-Cycle PD to Point-in-Time via Merton-Vasicek.

        ``PD_PiT = Phi( (Phi^{-1}(PD_TtC) - sqrt(rho) * z) / sqrt(1 - rho) )``.
        """
        ...

    @staticmethod
    def central_tendency(annual_default_rates: list[float]) -> float:
        """Arithmetic-mean long-run PD from annual default rates (regulatory TtC)."""
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

        Returns a dict with keys ``{"mean", "std", "alpha", "beta"}``.
        """
        ...

    @staticmethod
    def beta_recovery_sample(
        mean: float,
        std: float,
        n_samples: int,
        seed: int,
    ) -> list[float]:
        """Sample ``n_samples`` recoveries from Beta(alpha, beta) via PCG64."""
        ...

    @staticmethod
    def beta_recovery_quantile(mean: float, std: float, q: float) -> float:
        """Quantile ``q`` of a Beta recovery distribution parameterized by (mean, std)."""
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

        Returns ``(net_recovery, lgd)`` with ``lgd`` clamped to ``[0, 1]``.
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
        """
        ...

    @staticmethod
    def downturn_lgd_regulatory_floor(
        base_lgd: float,
        add_on: float,
        floor: float,
    ) -> float:
        """Regulatory-floor downturn LGD: ``max(base + add_on, floor)`` clamped to ``[0, 1]``."""
        ...

    @staticmethod
    def ead_term_loan(principal: float) -> float:
        """Exposure at default for a fully drawn term loan (equal to principal)."""
        ...

    @staticmethod
    def ead_revolver(drawn: float, undrawn: float, ccf: float) -> float:
        """Exposure at default for a revolver: ``drawn + undrawn * ccf``."""
        ...

class migration:
    """Credit migration: rating scales, transition matrices, generators, and CTMC simulation."""

    class RatingScale:
        @staticmethod
        def standard() -> migration.RatingScale: ...
        @staticmethod
        def standard_with_nr() -> migration.RatingScale: ...
        @staticmethod
        def notched() -> migration.RatingScale: ...
        @staticmethod
        def custom(labels: list[str]) -> migration.RatingScale: ...
        @staticmethod
        def custom_with_default(labels: list[str], default_label: str) -> migration.RatingScale: ...
        def n_states(self) -> int: ...
        def index_of(self, label: str) -> int | None: ...
        def default_state(self) -> int | None: ...
        def labels(self) -> list[str]: ...
        def warf(self, label: str) -> float: ...
        def rating_from_warf(self, warf: float) -> str: ...

    class TransitionMatrix:
        def __init__(self, scale: migration.RatingScale, data: list[float], horizon: float) -> None: ...
        def probability(self, from_: str, to: str) -> float: ...
        def row(self, from_: str) -> list[float]: ...
        def to_matrix(self) -> list[list[float]]: ...
        def horizon(self) -> float: ...
        def n_states(self) -> int: ...
        def default_probabilities(self) -> list[float] | None: ...

    class GeneratorMatrix:
        def __init__(self, scale: migration.RatingScale, data: list[float]) -> None: ...
        @staticmethod
        def from_transition_matrix(p: migration.TransitionMatrix) -> migration.GeneratorMatrix: ...
        def intensity(self, from_: str, to: str) -> float: ...
        def exit_rate(self, state: str) -> float: ...
        def to_matrix(self) -> list[list[float]]: ...
        def n_states(self) -> int: ...

    class RatingPath:
        def state_at(self, t: float) -> int: ...
        def label_at(self, t: float) -> str: ...
        def defaulted(self) -> bool: ...
        def default_time(self) -> float | None: ...
        def n_transitions(self) -> int: ...
        def transitions(self) -> list[tuple[float, int]]: ...
        def horizon(self) -> float: ...

    class MigrationSimulator:
        def __init__(self, generator: migration.GeneratorMatrix, horizon: float) -> None: ...
        def simulate(
            self,
            initial_state: int,
            n_paths: int,
            seed: int,
        ) -> list[migration.RatingPath]: ...
        def empirical_matrix(self, n_paths_per_state: int, seed: int) -> migration.TransitionMatrix: ...
        def horizon(self) -> float: ...

    @staticmethod
    def project(generator: migration.GeneratorMatrix, t: float) -> migration.TransitionMatrix: ...
