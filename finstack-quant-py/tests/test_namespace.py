"""Test that domain subpackages are importable with expected exports."""

import importlib
import json
from pathlib import Path
import tomllib

import pytest

from finstack_quant.core.market_data import MarketContext
from finstack_quant.portfolio import aggregate_full_cashflows

CONTRACT_PATH = Path(__file__).parents[1] / "parity_contract.toml"
CONTRACT = tomllib.loads(CONTRACT_PATH.read_text())


class TestCoreNamespace:
    """Verify the core subpackage and its nested modules."""

    def test_core_submodules(self) -> None:
        """All core submodules should be importable from finstack_quant.core."""
        from finstack_quant.core import config, currency, dates, market_data, math, money, types  # noqa: F401

    def test_core_currency_exports(self) -> None:
        """Currency module should export Currency class."""
        from finstack_quant.core.currency import Currency

        assert callable(Currency)

    def test_core_money_exports(self) -> None:
        """Money module should export Money class."""
        from finstack_quant.core.money import Money

        assert callable(Money)

    def test_core_dates_exports(self) -> None:
        """Dates module should export day-count and period types."""
        from finstack_quant.core.dates import (  # noqa: F401
            DayCount,
            DayCountContext,
            PeriodId,
            build_periods,
        )

    def test_core_math_linalg_exports(self) -> None:
        """Math.linalg should export Cholesky functions and constants."""
        from finstack_quant.core.math.linalg import (  # noqa: F401
            DIAGONAL_TOLERANCE,
            SINGULAR_THRESHOLD,
            SYMMETRY_TOLERANCE,
            CholeskyError,
            cholesky_decomposition,
            cholesky_solve,
        )

    def test_core_market_data_exports(self) -> None:
        """Market data module should export curve and FX types."""
        from finstack_quant.core.market_data import (  # noqa: F401
            DiscountCurve,
            ForwardCurve,
            FxConversionPolicy,
            FxMatrix,
            MarketContext,
        )

    def test_core_market_data_all_matches_static_parent_exports(self) -> None:
        """Market data parent exports should match the parity contract."""
        from finstack_quant.core import market_data

        expected = CONTRACT["crates"]["core"]["market_data"]["public"]
        assert market_data.__all__ == expected
        for name in expected:
            assert hasattr(market_data, name)
        assert not hasattr(market_data, "diebold_li_fit_factors")
        assert not hasattr(market_data, "check_butterfly")

    def test_models_credit_exports_do_not_leak_binding_suffixes(self) -> None:
        """Credit scoring and PD bindings should expose canonical public names only."""
        from finstack_quant.models.credit import pd, scoring

        for module, public_names, private_names in [
            (
                scoring,
                [
                    "altman_z_score",
                    "altman_z_prime",
                    "altman_z_double_prime",
                    "ohlson_o_score",
                    "zmijewski_score",
                ],
                [
                    "altman_z_score_py",
                    "altman_z_prime_py",
                    "altman_z_double_prime_py",
                    "ohlson_o_score_py",
                    "zmijewski_score_py",
                ],
            ),
            (
                pd,
                ["pit_to_ttc", "ttc_to_pit", "central_tendency"],
                ["pit_to_ttc_py", "ttc_to_pit_py", "central_tendency_py"],
            ),
        ]:
            for name in public_names:
                assert callable(getattr(module, name))
            for name in private_names:
                assert not hasattr(module, name)


class TestAnalyticsNamespace:
    """Verify the analytics subpackage."""

    def test_analytics_exports_performance_and_value_objects(self) -> None:
        """Analytics exposes Performance plus the value-object result types."""
        from finstack_quant.analytics import (  # noqa: F401
            AnalyticsError,
            BetaResult,
            DatedSeries,
            DrawdownEpisode,
            GreeksResult,
            LookbackReturns,
            MultiFactorResult,
            Performance,
            PeriodStats,
            RollingGreeks,
        )

    def test_analytics_drops_freestanding_helpers(self) -> None:
        """Every freestanding analytic is now a method on `Performance`."""
        from finstack_quant import analytics

        for name in (
            "cagr",
            "simple_returns",
            "to_drawdown_series",
            "comp_sum",
            "comp_total",
            "value_at_risk",
            "expected_shortfall",
            "rolling_sharpe",
            "rolling_greeks",
            "multi_factor_greeks",
            "rolling_var_forecasts",
            "classify_breaches",
            "fit_garch11",
            "estimate_ruin",
            "mtd_select",
            "ytd_select",
            "fytd_select",
        ):
            assert not hasattr(analytics, name)
            assert name not in analytics.__all__

        # These four remain as freestanding convenience wrappers alongside
        # the equivalent `Performance` methods.
        for name in ("sharpe", "sortino", "volatility", "max_drawdown"):
            assert hasattr(analytics, name)
            assert name in analytics.__all__

    def test_analytics_does_not_export_statement_comps(self) -> None:
        """Comparable-company helpers belong on statements_analytics, not analytics."""
        from finstack_quant import analytics

        for name in (
            "compute_multiple",
            "peer_stats",
            "percentile_rank",
            "regression_fair_value",
            "score_relative_value",
            "z_score",
        ):
            assert not hasattr(analytics, name)
            assert name not in analytics.__all__


class TestCashflowsNamespace:
    """Verify the cashflows subpackage."""

    def test_cashflows_exports(self) -> None:
        """Cashflows should expose the JSON bridge functions."""
        from finstack_quant.cashflows import (  # noqa: F401
            accrued_interest,
            build_cashflow_schedule_json,
            dated_flows_json,
            validate_cashflow_schedule_json,
        )

    def test_bond_conversion_belongs_to_valuations(self) -> None:
        """Bond construction should live with valuation instruments."""
        from finstack_quant import cashflows
        from finstack_quant.valuations.instruments import bond_from_cashflows_json  # noqa: F401

        assert not hasattr(cashflows, "bond_from_cashflows_json")
        assert "bond_from_cashflows_json" not in cashflows.__all__


class TestCorrelationNamespace:
    """Verify the correlation subpackage nested under models."""

    def test_correlation_exports(self) -> None:
        """Correlation should export copula, recovery, factor, and Bernoulli types."""
        from finstack_quant.models.correlation import (  # noqa: F401
            Copula,
            CopulaSpec,
            CorrelatedBernoulli,
            LatentFactorKind,
            LatentFactorSpec,
            LatentMultiFactor,
            LatentSingleFactor,
            LatentTwoFactor,
            RecoveryModel,
            RecoverySpec,
            cholesky_decompose,
            correlation_bounds,
            joint_probabilities,
            validate_correlation_matrix,
        )

    def test_correlation_accessible_via_models(self) -> None:
        """``finstack_quant.models.correlation`` is importable as a submodule attribute."""
        from finstack_quant import models

        assert models.correlation.CopulaSpec is not None


class TestFactorModelNamespace:
    """Verify the models.factor subpackage mirrors the Rust module boundary."""

    def test_factor_model_credit_exports(self) -> None:
        """Credit factor APIs should be available under finstack_quant.models.factor.credit."""
        from finstack_quant.models.factor.credit import (  # noqa: F401
            CreditCalibrator,
            CreditFactorModel,
            FactorCovarianceForecast,
            LevelsAtDate,
            PeriodDecomposition,
            decompose_levels,
            decompose_period,
        )

    def test_removed_top_level_factor_model_namespace_is_not_importable(self) -> None:
        """The deleted compatibility-free top-level path must stay absent."""
        import finstack_quant

        assert not hasattr(finstack_quant, "factor_model")
        with pytest.raises(ModuleNotFoundError):
            importlib.import_module("finstack_quant.factor_model")

    def test_valuations_credit_factor_aliases_are_removed(self) -> None:
        """Credit factor APIs should live only under models.factor."""
        from finstack_quant import models, valuations

        assert not hasattr(models, "CreditFactorModel")
        assert not hasattr(models, "CreditCalibrator")
        assert not hasattr(valuations, "CreditFactorModel")
        assert not hasattr(valuations, "CreditCalibrator")


class TestMonteCarloNamespace:
    """Verify the models.monte_carlo subpackage."""

    def test_monte_carlo_exports(self) -> None:
        """Monte Carlo should export canonical pricer and result types."""
        from finstack_quant import models
        from finstack_quant.models.monte_carlo import (  # noqa: F401
            EuropeanPricer,
            LsmcPricer,
            MoneyEstimate,
            PathDependentPricer,
        )

        monte_carlo = models.monte_carlo
        assert "price_european_call" not in monte_carlo.__all__
        assert "price_european_put" not in monte_carlo.__all__
        assert not hasattr(monte_carlo, "price_european_call")
        assert not hasattr(monte_carlo, "price_european_put")

    def test_removed_root_namespace_is_absent(self) -> None:
        import finstack_quant

        assert "monte_carlo" not in finstack_quant.__all__
        assert not hasattr(finstack_quant, "monte_carlo")


class TestMarginNamespace:
    """Verify the margin subpackage."""

    def test_margin_exports(self) -> None:
        """Margin should export IM/VM types and CSA spec."""
        from finstack_quant.margin import (  # noqa: F401
            CsaSpec,
            HaircutImCalculator,
            ImMethodology,
            ImResult,
            NettingSetId,
            ScheduleImCalculator,
            SimmCalculator,
            SimmSensitivities,
            VmCalculator,
            VmResult,
        )


class TestPortfolioNamespace:
    """Verify the portfolio subpackage."""

    def test_portfolio_exports(self) -> None:
        """Portfolio should export parsing, building, metric functions, and typed wrappers."""
        from finstack_quant.portfolio import (  # noqa: F401
            FactorPnlProfile,
            FactorRiskDecomposition,
            Portfolio,
            PortfolioError,
            PortfolioResult,
            PortfolioValuation,
            SensitivityMatrix,
            aggregate_full_cashflows,
            aggregate_metrics,
            build_credit_vol_report,
            build_portfolio_from_spec_json,
            compute_factor_sensitivities,
            compute_pnl_profiles,
            decompose_factor_risk,
            factor_stress,
            parse_portfolio_spec_json,
            position_what_if,
        )

    def test_factor_risk_exports(self) -> None:
        """Pure factor-risk kernels should live under models.factor.risk."""
        from finstack_quant import portfolio
        from finstack_quant.models.factor.risk import (  # noqa: F401
            DecompositionConfig,
            RiskDecomposition,
            StressAttribution,
            build_stress_attribution,
            evaluate_risk_budget,
            parametric_var_decomposition,
        )

        assert not hasattr(portfolio, "RiskDecomposition")
        assert not hasattr(portfolio, "build_stress_attribution")

    def test_m18_position_filter_exports_python_keyword_safe_not(self) -> None:
        """PositionFilter exposes not_ rather than unusable Python keyword spelling."""
        from finstack_quant.portfolio import PositionFilter

        assert callable(PositionFilter.not_)
        assert not hasattr(PositionFilter, "not")

    def test_portfolio_domain_errors_are_typed(self) -> None:
        """Portfolio domain failures should expose a portfolio-specific exception."""
        from finstack_quant.portfolio import PortfolioError, build_portfolio_from_spec_json

        spec_json = json.dumps({
            "id": "bad_portfolio",
            "name": "Bad",
            "base_currency": "USD",
            "as_of": "2024-01-15",
            "entities": {},
            "positions": [
                {
                    "position_id": "P1",
                    "entity_id": "MISSING",
                    "instrument_id": "D1",
                    "instrument_spec": None,
                    "quantity": 1.0,
                    "unit": "units",
                }
            ],
        })

        with pytest.raises(PortfolioError):
            build_portfolio_from_spec_json(spec_json)

    def test_portfolio_full_cashflows_empty_portfolio(self) -> None:
        """Full cashflow ladder should be exposed and preserve the rich empty shape."""
        spec_json = json.dumps({
            "id": "test_portfolio",
            "name": "Test",
            "base_currency": "USD",
            "as_of": "2024-01-15",
            "entities": {},
            "positions": [],
        })
        cashflows = aggregate_full_cashflows(spec_json, MarketContext())
        assert len(cashflows) == 0
        assert cashflows.num_positions == 0
        assert cashflows.num_issues == 0

        result = json.loads(cashflows.to_json())
        assert result["events"] == []
        assert result["by_position"] == {}
        assert result["by_date"] == {}
        assert result["position_summaries"] == {}
        assert result["issues"] == []


class TestScenariosNamespace:
    """Verify the scenarios subpackage."""

    def test_scenarios_exports(self) -> None:
        """Scenarios should export spec builders and template functions."""
        from finstack_quant.scenarios import (  # noqa: F401
            HierarchyTarget,
            ScenarioSpec,
            TemplateMetadata,
            build_from_template,
            build_template_component,
            compose_scenarios,
            list_builtin_template_metadata,
            list_builtin_templates,
            list_template_components,
            validate_scenario_spec,
        )


class TestStatementsNamespace:
    """Verify the statements subpackage."""

    def test_statements_exports(self) -> None:
        """Statements should export model spec and enum types."""
        from finstack_quant.statements import (  # noqa: F401
            FinancialModelSpec,
            ForecastMethod,
            NodeId,
            NodeType,
            NumericMode,
        )

    def test_statements_evaluator_exposes_market_aware_evaluation(self) -> None:
        """Statement evaluator exposes the Rust market/as-of path."""
        from finstack_quant.statements import Evaluator

        assert hasattr(Evaluator(), "evaluate_with_market")


class TestStatementsAnalyticsNamespace:
    """Verify the statements_analytics subpackage."""

    def test_statements_analytics_exports(self) -> None:
        """Statements analytics should export sensitivity and variance functions."""
        from finstack_quant.statements_analytics import (  # noqa: F401
            backtest_forecast,
            compute_multiple,
            evaluate_scenario_set,
            peer_stats,
            percentile_rank,
            regression_fair_value,
            run_sensitivity,
            run_variance,
            score_relative_value,
            z_score,
        )


class TestValuationsNamespace:
    """Verify the valuations subpackage."""

    def test_valuations_exports(self) -> None:
        """Valuations should export ValuationResult and validation function."""
        from finstack_quant.valuations import ValuationResult  # noqa: F401

    def test_valuations_do_not_export_model_engines(self) -> None:
        from finstack_quant import valuations

        for name in ("bs_price", "bs_cos_price", "SabrModel", "correlation", "models"):
            assert name not in valuations.__all__
            assert not hasattr(valuations, name)

    def test_valuations_do_not_export_calibration(self) -> None:
        """Calibration has no compatibility aliases under valuations."""
        from finstack_quant import valuations

        for name in (
            "CalibrationEnvelope",
            "CalibrationEnvelopeError",
            "CalibrationResult",
            "calibrate",
            "calibrate_bermudan_lmm_base_vol",
            "dry_run",
            "validate_calibration_json",
        ):
            assert name not in valuations.__all__
            assert not hasattr(valuations, name)


class TestCalibrationNamespace:
    """Verify the calibration crate's top-level Python namespace."""

    def test_calibration_owns_compiled_surface(self) -> None:
        """Calibration symbols should resolve only from the calibration package."""
        from finstack_quant import calibration

        expected = {
            "CalibrationConfig",
            "CalibrationDiagnostics",
            "CalibrationEnvelope",
            "CalibrationEnvelopeError",
            "CalibrationPlan",
            "CalibrationReport",
            "CalibrationResult",
            "CalibrationStep",
            "CalibrationValidationReport",
            "CdsQuote",
            "QuoteQuality",
            "RateBounds",
            "RateQuote",
            "SolverConfig",
            "ValidationConfig",
            "VolQuote",
            "calibrate",
            "calibrate_bermudan_lmm_base_vol",
            "dry_run",
            "dry_run_json",
            "hull_white",
            "schema",
            "validate_calibration",
            "validate_calibration_json",
        }
        assert set(calibration.__all__) == expected
        assert calibration.CalibrationResult.__module__ == "finstack_quant.calibration"
        assert calibration.CalibrationEnvelopeError.__module__ == "finstack_quant.calibration"

    def test_models_stub_exports_fourier_pricers(self) -> None:
        """Models stubs should declare the runtime Fourier pricing exports."""
        stub_path = Path(__file__).parents[1] / "finstack_quant" / "models" / "__init__.pyi"
        stub = stub_path.read_text()
        for name in ("bs_cos_price", "vg_cos_price", "merton_jump_cos_price"):
            assert f'"{name}"' in stub
            assert f"def {name}(" in stub

    def test_valuations_instruments_namespace_exports(self) -> None:
        """Instrument helpers should be available from valuations.instruments."""
        from finstack_quant.valuations import instruments

        assert hasattr(instruments, "validate_instrument_json")
        assert hasattr(instruments, "price_instrument")
        assert hasattr(instruments, "price_instrument")
        assert hasattr(instruments, "list_standard_metrics")

    def test_models_credit_namespace_exports(self) -> None:
        """Structural credit models should live under models.credit."""
        from finstack_quant.models import credit

        for name in (
            "AssetDynamics",
            "MertonBarrierType",
            "CreditState",
            "DynamicRecoverySpec",
            "EndogenousHazardSpec",
            "lgd",
            "liability_management",
            "MertonModel",
            "migration",
            "moodys_warf_factor",
            "pd",
            "recovery_waterfall",
            "scoring",
            "SimulatedPaths",
            "ToggleExerciseModel",
        ):
            assert hasattr(credit, name)

        from finstack_quant import core

        assert not hasattr(core, "credit")

        from finstack_quant.valuations import instruments

        for name in (
            "BarrierCrossing",
            "MertonMcConfig",
            "MertonMcResult",
            "PathStatistics",
            "PikMode",
            "PikSchedule",
        ):
            assert hasattr(instruments, name)

    def test_models_extension_submodules_are_registered(self) -> None:
        """PyO3 model submodules should have stable extension-qualified names."""
        import sys

        from finstack_quant.finstack_quant import models as ext_models

        root_package = ext_models.__package__
        assert root_package == "finstack_quant.finstack_quant.models"
        for name in ("correlation", "credit", "monte_carlo"):
            module = getattr(ext_models, name)
            qualified = f"{root_package}.{name}"
            assert module.__package__ == qualified
            assert sys.modules[qualified] is module
