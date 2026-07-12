"""Test that domain subpackages are importable with expected exports."""

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

    def test_core_credit_exports_do_not_leak_binding_suffixes(self) -> None:
        """Credit scoring and PD bindings should expose canonical public names only."""
        from finstack_quant.core.credit import pd, scoring

        for module, public_names, private_names in [
            (
                scoring,
                [
                    "AltmanPdCalibration",
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
            "sharpe",
            "sortino",
            "volatility",
            "simple_returns",
            "max_drawdown",
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
            accrued_interest_json,
            bond_from_cashflows_json,
            build_cashflow_schedule_json,
            dated_flows_json,
            validate_cashflow_schedule_json,
        )


class TestCorrelationNamespace:
    """Verify the correlation subpackage nested under valuations."""

    def test_correlation_exports(self) -> None:
        """Correlation should export copula, recovery, factor, and Bernoulli types."""
        from finstack_quant.valuations.correlation import (  # noqa: F401
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

    def test_correlation_accessible_via_valuations(self) -> None:
        """``finstack_quant.valuations.correlation`` is importable as a submodule attribute."""
        from finstack_quant import valuations

        assert valuations.correlation.CopulaSpec is not None


class TestFactorModelNamespace:
    """Verify the factor_model subpackage mirrors the Rust crate boundary."""

    def test_factor_model_credit_exports(self) -> None:
        """Credit factor APIs should be available under finstack_quant.factor_model.credit."""
        from finstack_quant.factor_model.credit import (  # noqa: F401
            CreditCalibrator,
            CreditFactorModel,
            FactorCovarianceForecast,
            LevelsAtDate,
            PeriodDecomposition,
            decompose_levels,
            decompose_period,
        )

    def test_valuations_credit_factor_aliases_are_removed(self) -> None:
        """Credit factor APIs should live only under the factor_model namespace."""
        from finstack_quant import factor_model, valuations

        assert not hasattr(factor_model, "CreditFactorModel")
        assert not hasattr(factor_model, "CreditCalibrator")
        assert not hasattr(valuations, "CreditFactorModel")
        assert not hasattr(valuations, "CreditCalibrator")


class TestMonteCarloNamespace:
    """Verify the monte_carlo subpackage."""

    def test_monte_carlo_exports(self) -> None:
        """Monte Carlo should export engine, pricer, and result types."""
        from finstack_quant.monte_carlo import (  # noqa: F401
            EuropeanPricer,
            LsmcPricer,
            McEngine,
            MoneyEstimate,
            PathDependentPricer,
            price_european_call,
            price_european_put,
        )


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
            FinstackFxError,
            FinstackOptimizationError,
            FinstackValuationError,
            Portfolio,
            PortfolioError,
            PortfolioResult,
            PortfolioValuation,
            SensitivityMatrix,
            aggregate_full_cashflows,
            aggregate_metrics,
            build_credit_vol_report,
            build_portfolio_from_spec,
            build_stress_attribution,
            compute_factor_sensitivities,
            compute_pnl_profiles,
            decompose_factor_risk,
            factor_stress,
            parse_portfolio_spec,
            portfolio_result_get_metric,
            portfolio_result_total_value,
            position_what_if,
        )

    def test_m18_position_filter_exports_python_keyword_safe_not(self) -> None:
        """PositionFilter exposes not_ rather than unusable Python keyword spelling."""
        from finstack_quant.portfolio import PositionFilter

        assert callable(PositionFilter.not_)
        assert not hasattr(PositionFilter, "not")

    def test_portfolio_domain_errors_are_typed(self) -> None:
        """Portfolio domain failures should expose a portfolio-specific exception."""
        from finstack_quant.portfolio import PortfolioError, build_portfolio_from_spec

        spec_json = json.dumps({
            "id": "bad_portfolio",
            "name": "Bad",
            "base_ccy": "USD",
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
            build_portfolio_from_spec(spec_json)

    def test_portfolio_full_cashflows_empty_portfolio(self) -> None:
        """Full cashflow ladder should be exposed and preserve the rich empty shape."""
        spec_json = json.dumps({
            "id": "test_portfolio",
            "name": "Test",
            "base_ccy": "USD",
            "as_of": "2024-01-15",
            "entities": {},
            "positions": [],
        })
        cashflows = aggregate_full_cashflows(spec_json, MarketContext())
        assert len(cashflows) == 0
        assert cashflows.num_positions() == 0
        assert cashflows.num_issues() == 0

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
            build_from_template,
            build_scenario_spec,
            build_template_component,
            compose_scenarios,
            list_builtin_template_metadata,
            list_builtin_templates,
            list_template_components,
            parse_scenario_spec,
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
        from finstack_quant.valuations import (  # noqa: F401
            ValuationResult,
            bs_cos_price,
            merton_jump_cos_price,
            vg_cos_price,
        )

    def test_valuations_stub_exports_fourier_pricers(self) -> None:
        """Valuations stubs should declare the runtime Fourier pricing exports."""
        stub_path = Path(__file__).parents[1] / "finstack_quant" / "valuations" / "__init__.pyi"
        stub = stub_path.read_text()
        for name in ("bs_cos_price", "vg_cos_price", "merton_jump_cos_price"):
            assert f'"{name}"' in stub
            assert f"def {name}(" in stub

    def test_valuations_instruments_namespace_exports(self) -> None:
        """Instrument helpers should be available from valuations.instruments."""
        from finstack_quant.valuations import instruments

        assert hasattr(instruments, "commodity")
        assert hasattr(instruments, "credit_derivatives")
        assert hasattr(instruments, "equity")
        assert hasattr(instruments, "exotics")
        assert hasattr(instruments, "fixed_income")
        assert hasattr(instruments, "fx")
        assert hasattr(instruments, "rates")
        assert hasattr(instruments, "validate_instrument_json")
        assert hasattr(instruments, "price_instrument")
        assert hasattr(instruments, "price_instrument_with_metrics")
        assert hasattr(instruments, "list_standard_metrics")

    def test_valuations_fx_namespace_exports(self) -> None:
        """Direct FX instruments should be available from valuations.instruments.fx."""
        from finstack_quant.valuations.instruments import fx

        for name in (
            "FxSpot",
            "FxForward",
            "FxSwap",
            "Ndf",
            "FxOption",
            "FxDigitalOption",
            "FxTouchOption",
            "FxBarrierOption",
            "FxVarianceSwap",
            "QuantoOption",
        ):
            assert hasattr(fx, name)

    def test_valuations_commodity_namespace_exports(self) -> None:
        """Direct commodity instruments should be available from valuations.instruments.commodity."""
        from finstack_quant.valuations.instruments import commodity

        for name in (
            "CommodityOption",
            "CommodityAsianOption",
            "CommodityForward",
            "CommoditySwap",
            "CommoditySwaption",
            "CommoditySpreadOption",
        ):
            cls = getattr(commodity, name)
            assert cls.__module__ == "finstack_quant.valuations.instruments.commodity"

    def test_valuations_equity_namespace_exports(self) -> None:
        """Direct equity instruments should be available from valuations.instruments.equity."""
        from finstack_quant.valuations.instruments import equity

        for name in (
            "Equity",
            "EquityOption",
            "VarianceSwap",
            "EquityIndexFuture",
            "VolatilityIndexFuture",
            "VolatilityIndexOption",
            "Autocallable",
            "CliquetOption",
            "EquityTotalReturnSwap",
            "PrivateMarketsFund",
            "RealEstateAsset",
            "LeveredRealEstateEquity",
            "DiscountedCashFlow",
        ):
            cls = getattr(equity, name)
            assert cls.__module__ == "finstack_quant.valuations.instruments.equity"

    def test_valuations_exotics_namespace_exports(self) -> None:
        """Direct exotic instruments should be available from valuations.instruments.exotics."""
        from finstack_quant.valuations.instruments import exotics

        for name in ("AsianOption", "BarrierOption", "LookbackOption", "Basket"):
            assert hasattr(exotics, name)

    def test_valuations_fixed_income_namespace_exports(self) -> None:
        """Direct fixed-income instruments should be available from valuations.instruments.fixed_income."""
        from finstack_quant.valuations.instruments import fixed_income

        for name in (
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
        ):
            cls = getattr(fixed_income, name)
            assert cls.__module__ == "finstack_quant.valuations.instruments.fixed_income"

    def test_valuations_rates_namespace_exports(self) -> None:
        """Direct rates instruments should be available from valuations.instruments.rates."""
        from finstack_quant.valuations.instruments import rates

        for name in (
            "InterestRateSwap",
            "BasisSwap",
            "XccySwap",
            "InflationSwap",
            "YoYInflationSwap",
            "InflationCapFloor",
            "ForwardRateAgreement",
            "Swaption",
            "BermudanSwaption",
            "InterestRateFuture",
            "CapFloor",
            "CmsSwap",
            "CmsOption",
            "IrFutureOption",
            "Deposit",
            "Repo",
            "RangeAccrual",
            "Tarn",
            "Snowball",
            "CmsSpreadOption",
            "CallableRangeAccrual",
        ):
            cls = getattr(rates, name)
            assert cls.__module__ == "finstack_quant.valuations.instruments.rates"

    def test_valuations_models_credit_namespace_exports(self) -> None:
        """Structural credit models should mirror valuations.models.credit."""
        from finstack_quant.valuations.models import credit

        for name in (
            "CreditState",
            "DynamicRecoverySpec",
            "EndogenousHazardSpec",
            "MertonModel",
            "ToggleExerciseModel",
        ):
            assert hasattr(credit, name)

    def test_valuations_extension_submodules_are_registered(self) -> None:
        """PyO3 valuation submodules should have stable extension-qualified names."""
        import sys

        from finstack_quant.finstack_quant import valuations as ext_valuations

        root_package = ext_valuations.__package__
        assert root_package == "finstack_quant.finstack_quant.valuations"
        for name in ("correlation", "instruments", "models"):
            module = getattr(ext_valuations, name)
            qualified = f"{root_package}.{name}"
            assert module.__package__ == qualified
            assert sys.modules[qualified] is module

        instruments_package = f"{root_package}.instruments"
        for name in (
            "commodity",
            "credit_derivatives",
            "equity",
            "exotics",
            "fixed_income",
            "fx",
            "rates",
        ):
            module = getattr(ext_valuations.instruments, name)
            qualified = f"{instruments_package}.{name}"
            assert module.__package__ == qualified
            assert sys.modules[qualified] is module

        credit = ext_valuations.models.credit
        qualified = f"{root_package}.models.credit"
        assert credit.__package__ == qualified
        assert sys.modules[qualified] is credit
