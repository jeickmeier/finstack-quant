"""Reviewed exceptions and contract registries."""

from __future__ import annotations

from collections.abc import Sequence

from .models import CAPABILITIES, ExceptionEntry


def _exception(
    crate: str,
    path: str,
    type_names: Sequence[str],
    category: str,
    rationale: str,
    allowed_missing: frozenset[str] = frozenset({"Deserialize", "JsonSchema"}),
) -> tuple[ExceptionEntry, ...]:
    return tuple(
        ExceptionEntry(
            crate=crate,
            path=path,
            type_name=type_name,
            category=category,
            rationale=rationale,
            allowed_missing=allowed_missing,
        )
        for type_name in type_names
    )


ONE_WAY_EXCEPTIONS = (
    *_exception(
        "attribution",
        "src/return_contribution.rs",
        (
            "ReturnContributionResult",
            "InstrumentContribution",
            "GroupContribution",
            "FactorContribution",
            "BenchmarkRelativeContribution",
        ),
        "attribution-report",
        "Emitted attribution report rows. These gained `Deserialize` so the "
        "Python `ReturnContributionResult.from_json` round-trip works; they "
        "still carry no `JsonSchema` because the schema registry publishes the "
        "spec side, not the result side.",
        frozenset({"JsonSchema"}),
    ),
)


def _classification(
    crate: str,
    path: str,
    type_names: Sequence[str],
    category: str,
    rationale: str,
    allowed_missing: frozenset[str] = frozenset({"JsonSchema"}),
) -> tuple[ExceptionEntry, ...]:
    return _exception(
        crate,
        path,
        type_names,
        category,
        rationale,
        allowed_missing,
    )


def _computed_output(
    crate: str,
    path: str,
    type_names: Sequence[str],
) -> tuple[ExceptionEntry, ...]:
    return _classification(
        crate,
        path,
        type_names,
        "non-maintained-serde-output",
        "Computed output supports serde transport but is reproduced from canonical inputs; "
        "it is not a maintained/versioned persistence document.",
    )


def _in_process_spec(
    crate: str,
    path: str,
    type_names: Sequence[str],
) -> tuple[ExceptionEntry, ...]:
    return _classification(
        crate,
        path,
        type_names,
        "in-process-serde-spec",
        "Operation input is accepted by serde-powered adapters but has no maintained "
        "versioned-document or standalone-schema promise.",
    )


NON_MAINTAINED_SERDE_EXCEPTIONS = (
    *_in_process_spec(
        "attribution",
        "src/return_contribution.rs",
        ("ReturnContributionSpec",),
    ),
    *_classification(
        "models",
        "src/credit/registry.rs",
        ("CreditAssumptionRegistry",),
        "internal-registry-document",
        "Embedded credit-assumption registry is loaded through component-specific validation "
        "and is outside the maintained public persistence catalog.",
    ),
    *_computed_output("core", "src/expr/ast.rs", ("EvaluationResult",)),
    *_in_process_spec("core", "src/market_data/bumps.rs", ("BumpSpec",)),
    *_computed_output(
        "core",
        "src/market_data/term_structures/base_correlation.rs",
        ("ArbitrageCheckResult",),
    ),
    *_computed_output("core", "src/money/fx/types.rs", ("FxRateResult",)),
    *_classification(
        "core",
        "src/rating_scales.rs",
        ("RatingScaleRegistry",),
        "internal-registry-document",
        "Embedded rating-scale registry uses component validation and is explicitly outside "
        "the maintained public persistence catalog.",
    ),
    *_computed_output("statements", "src/adjustments/types.rs", ("NormalizationResult",)),
    *_in_process_spec(
        "statements",
        "src/checks/suite.rs",
        ("CheckSuiteSpec", "BuiltinCheckSpec", "FormulaCheckSpec"),
    ),
    *_classification(
        "statements",
        "src/registry/schema.rs",
        ("MetricRegistry",),
        "internal-registry-document",
        "Embedded statement-metric registry has registry-specific validation and is outside "
        "the maintained public persistence catalog.",
    ),
)

RESULT_ALIAS_EXCEPTIONS = tuple(
    entry
    for crate, path in (
        ("analytics", "src/correlation/mod.rs"),
        ("core", "src/error/mod.rs"),
        ("models", "src/correlation/error.rs"),
        ("portfolio", "src/error.rs"),
        ("scenarios", "src/error.rs"),
        ("statements", "src/error.rs"),
        ("valuations", "src/error.rs"),
    )
    for entry in _classification(
        crate,
        path,
        ("Result",),
        "generic-error-result-alias",
        "Generic alias to std::result::Result for crate error propagation; it is a type "
        "constructor, not a serializable DTO or persistence contract.",
        frozenset(CAPABILITIES),
    )
)


def _runtime_exception(
    crate: str,
    path: str,
    type_names: Sequence[str],
    category: str = "runtime-result",
) -> tuple[ExceptionEntry, ...]:
    return _exception(
        crate,
        path,
        type_names,
        category,
        "In-memory algorithm state or output; never accepted as a persisted wire contract.",
        frozenset(CAPABILITIES),
    )


# Runtime Result names are individually reviewed because their suffix alone cannot
# distinguish persisted DTOs from in-memory solver or algorithm state.
RUNTIME_RESULT_EXCEPTIONS = (
    *_runtime_exception(
        "models",
        "src/monte_carlo/greeks/gbm_european.rs",
        ("GbmEuropeanFdSpec",),
        "runtime-spec",
    ),
    *_runtime_exception("core", "src/math/linalg.rs", ("LedoitWolfResult",)),
    *_runtime_exception(
        "statements",
        "src/capital_structure/waterfall/mod.rs",
        ("WaterfallPeriodResult",),
    ),
    *_runtime_exception(
        "valuations",
        "src/instruments/credit_derivatives/cds_index/types.rs",
        ("ConstituentResult", "IndexResult", "IndexParSpreadResult"),
    ),
    *_runtime_exception(
        "valuations",
        "src/instruments/fixed_income/bond/pricing/ytm_solver.rs",
        ("YtmPricingSpec",),
        "runtime-spec",
    ),
    *_runtime_exception(
        "valuations",
        "src/instruments/fixed_income/cmo/waterfall.rs",
        ("CmoWaterfallPeriodResult",),
    ),
    *_runtime_exception(
        "valuations",
        "src/instruments/fixed_income/dollar_roll/carry.rs",
        ("CarryResult",),
    ),
    *_runtime_exception(
        "valuations",
        "src/instruments/rates/cms_swap/types.rs",
        ("FundingLegSpec",),
        "runtime-spec",
    ),
    *_runtime_exception("valuations", "src/metrics/risk/var_calculator.rs", ("VarResult",)),
    *_runtime_exception(
        "models",
        "src/trees/short_rate_tree/tree.rs",
        ("TreeCalibrationResult",),
    ),
)

REVIEWED_EXCEPTIONS = (
    *ONE_WAY_EXCEPTIONS,
    *NON_MAINTAINED_SERDE_EXCEPTIONS,
    *RESULT_ALIAS_EXCEPTIONS,
    *RUNTIME_RESULT_EXCEPTIONS,
)
MAINTAINED_CONTRACTS = frozenset({
    ("valuations", "src/instruments/json_loader.rs", "InstrumentEnvelope"),
    ("calibration", "src/api/schema.rs", "CalibrationEnvelope"),
    ("calibration", "src/api/schema.rs", "CalibrationResultEnvelope"),
    ("core", "src/market_data/context/state_serde.rs", "MarketContextState"),
    ("statements", "src/types/model.rs", "FinancialModelSpec"),
    ("scenarios", "src/envelope.rs", "ScenarioEnvelope"),
    ("factor-model", "src/envelope.rs", "FactorModelConfigEnvelope"),
    ("factor-model", "src/credit/hierarchy.rs", "CreditFactorModel"),
    ("portfolio", "src/materialization/envelope.rs", "PortfolioMaterializationEnvelope"),
    ("valuations", "src/results/valuation_result.rs", "ValuationResult"),
    ("statements", "src/evaluator/results.rs", "StatementResult"),
    ("portfolio", "src/results.rs", "PortfolioResult"),
    ("portfolio", "src/optimization/result.rs", "PortfolioOptimizationResult"),
})

# Required public binding outputs are not maintained persistence contracts:
# they need no version marker, strict loader, or contract-matrix entry. This
# registry keeps their public reachability and effective output schema
# fail-closed when module/re-export resolution changes.
REQUIRED_PUBLIC_TYPES = {
    (
        "valuations",
        "src/instruments/common_impl/cashflow_export.rs",
        "InstrumentCashflowEnvelope",
    ): CAPABILITIES,
}

MAINTAINED_REQUIRED_CAPABILITIES = {
    identity: (
        frozenset({"Serialize", "JsonSchema"})
        if identity
        == (
            "portfolio",
            "src/optimization/result.rs",
            "PortfolioOptimizationResult",
        )
        else CAPABILITIES
    )
    for identity in MAINTAINED_CONTRACTS
}

MAINTAINED_ONE_WAY_OUTPUTS = frozenset({
    (
        "portfolio",
        "src/optimization/result.rs",
        "PortfolioOptimizationResult",
    ),
})

ONE_WAY_OUTPUT_IDENTITIES = frozenset({
    *((entry.crate, entry.path, entry.type_name) for entry in ONE_WAY_EXCEPTIONS),
    *MAINTAINED_ONE_WAY_OUTPUTS,
})
