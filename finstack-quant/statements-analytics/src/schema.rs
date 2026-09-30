//! Schema registry for the statements-analytics crate.
//!
//! Every root serde contract the bindings exchange with a host is registered
//! here; the types it embeds are emitted as `$defs` of those roots. The
//! `gen_statements_analytics_schemas` binary writes one JSON Schema per
//! entry, and the WASM package generates its TypeScript declarations from
//! them. Render an entry with
//! [`SchemaArtifact::generate`](finstack_quant_core::schema::SchemaArtifact::generate).

use finstack_quant_core::schema::{
    externalize_schema_definitions, ExternalSchemaDefinition, SchemaArtifact,
};
use finstack_quant_core::Result;
use finstack_quant_statements::adjustments::types::NormalizationConfig;
use finstack_quant_statements::evaluator::StatementResult;
use finstack_quant_statements::FinancialModelSpec;
use finstack_quant_valuations::instruments::InstrumentJson;
use finstack_quant_valuations::results::ValuationResult;
use serde_json::Value;

/// Published contracts from other crates that statements-analytics roots embed.
const EXTERNAL_DEFINITIONS: &[ExternalSchemaDefinition] = &[
    ExternalSchemaDefinition::new::<FinancialModelSpec>(
        "FinancialModelSpec",
        "https://finstack_quant.dev/schemas/statements/1/financial_model_spec.schema.json",
    ),
    ExternalSchemaDefinition::new::<InstrumentJson>(
        "InstrumentJson",
        "https://finstack_quant.dev/schemas/common/1/instrument_json.schema.json",
    ),
    ExternalSchemaDefinition::new::<NormalizationConfig>(
        "NormalizationConfig",
        "https://finstack_quant.dev/schemas/statements/1/normalization_config.schema.json",
    ),
    ExternalSchemaDefinition::new::<StatementResult>(
        "StatementResult",
        "https://finstack_quant.dev/schemas/statements/1/statement_result.schema.json",
    ),
    ExternalSchemaDefinition::new::<ValuationResult>(
        "ValuationResult",
        "https://finstack_quant.dev/schemas/results/1/valuation_result.schema.json",
    ),
];

/// Replace embedded statement-model, statement-result, instrument and
/// valuation-result definitions with references to their published artifacts.
///
/// # Arguments
///
/// * `schema` - Generated statements-analytics schema to repoint in place.
///
/// # Errors
///
/// Returns [`finstack_quant_core::Error::Validation`] if a replaced definition
/// is not assertion-equivalent to the published artifact.
pub fn package_schema(schema: &mut Value) -> Result<()> {
    externalize_schema_definitions(schema, EXTERNAL_DEFINITIONS)
}

use crate::{
    analysis::checks::CapexReconciliation, analysis::checks::CoverageFloorCheck,
    analysis::checks::DepreciationReconciliation, analysis::checks::DividendReconciliation,
    analysis::checks::EffectiveTaxRateCheck, analysis::checks::FcfSignCheck,
    analysis::checks::GrowthRateConsistency, analysis::checks::InterestExpenseReconciliation,
    analysis::checks::LeverageRangeCheck, analysis::checks::LiquidityRunwayCheck,
    analysis::checks::TrendCheck, analysis::checks::WorkingCapitalConsistency,
    analysis::BridgeChart, analysis::CeclConfig, analysis::CeclResult, analysis::CorporateAnalysis,
    analysis::CreditAssessment, analysis::DcfOptions, analysis::DcfSensitivityResult,
    analysis::DependencyTree, analysis::EclConfig, analysis::EclRequest, analysis::EclStageRequest,
    analysis::Explanation, analysis::Exposure, analysis::ForecastMetrics, analysis::GoalSeekResult,
    analysis::LboConfig, analysis::LboResult, analysis::PeerFilter, analysis::PeerSet,
    analysis::PeerStats, analysis::PortfolioEclResult, analysis::ProvisionWaterfall,
    analysis::RatingPdMap, analysis::RegressionResult, analysis::RelativeValueResult,
    analysis::ScenarioDiff, analysis::ScenarioResults, analysis::ScenarioSet,
    analysis::ScoringDimension, analysis::SensitivityResult, analysis::VarianceConfig,
    extensions::CorkscrewConfig, extensions::CorkscrewReport, extensions::ScorecardConfig,
    extensions::ScorecardReport, templates::real_estate::LeaseSpec,
    templates::real_estate::ManagementFeeSpec, templates::real_estate::PropertyTemplateNodes,
};

/// The crate's complete schema registry.
pub const ARTIFACTS: &[SchemaArtifact] = &[
    finstack_quant_core::schema_artifact!(
        BridgeChart,
        "statements_analytics",
        "bridge_chart",
        Component,
        "Bridge chart for a single target metric and period."
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        CapexReconciliation,
        "statements_analytics",
        "capex_reconciliation",
        Component,
        "Verifies that cash-flow-statement capex equals the sum of PP&E additions and intangible additions."
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        CeclConfig,
        "statements_analytics",
        "cecl_config",
        Input,
        "Configuration for CECL (US GAAP ASC 326) calculation."
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        CeclResult,
        "statements_analytics",
        "cecl_result",
        Output,
        "CECL result for a single exposure."
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        CorkscrewConfig,
        "statements_analytics",
        "corkscrew_config",
        Input,
        "Configuration for corkscrew analysis."
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        CorkscrewReport,
        "statements_analytics",
        "corkscrew_report",
        Output,
        "Report produced by `CorkscrewExtension::execute`."
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        CorporateAnalysis,
        "statements_analytics",
        "corporate_analysis",
        Output,
        "Unified analysis result combining statement, equity, and credit perspectives."
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        CoverageFloorCheck,
        "statements_analytics",
        "coverage_floor_check",
        Component,
        "Flags periods where a coverage ratio (e.g."
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        CreditAssessment,
        "statements_analytics",
        "credit_assessment",
        Component,
        "Structured credit assessment: leverage, interest coverage, and free cash flow at a `period` plus a per-period series for trend display."
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        DcfOptions,
        "statements_analytics",
        "dcf_options",
        Input,
        "Optional configuration for DCF valuation beyond the core WACC/terminal parameters."
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        DcfSensitivityResult,
        "statements_analytics",
        "dcf_sensitivity_result",
        Output,
        "Enterprise-value tornado for the headline DCF assumptions."
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        DependencyTree,
        "statements_analytics",
        "dependency_tree",
        Component,
        "Hierarchical dependency tree structure."
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        DepreciationReconciliation,
        "statements_analytics",
        "depreciation_reconciliation",
        Component,
        "Verifies PP&E(t) = PP&E(t−1) + Capex(t) − D&A(t) − Disposals(t)."
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        DividendReconciliation,
        "statements_analytics",
        "dividend_reconciliation",
        Component,
        "Verifies that dividends on the cash flow statement equal the dividends charged against equity."
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        EclConfig,
        "statements_analytics",
        "ecl_config",
        Input,
        "Configuration for ECL calculation."
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        EclRequest,
        "statements_analytics",
        "ecl_request",
        Input,
        "Inputs for probability-weighted ECL from cumulative-PD schedules."
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        EclStageRequest,
        "statements_analytics",
        "ecl_stage_request",
        Input,
        "Inputs for the simplified IFRS 9 stage-classification workflow."
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        EffectiveTaxRateCheck,
        "statements_analytics",
        "effective_tax_rate_check",
        Component,
        "Verifies that the effective tax rate (tax expense / pretax income) falls within an expected range."
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        Explanation,
        "statements_analytics",
        "explanation",
        Component,
        "Detailed explanation of a node's calculation."
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        Exposure,
        "statements_analytics",
        "exposure",
        Component,
        "A single credit exposure for ECL computation."
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        FcfSignCheck,
        "statements_analytics",
        "fcf_sign_check",
        Component,
        "Tracks consecutive periods of negative free cash flow and flags at configurable thresholds."
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        ForecastMetrics,
        "statements_analytics",
        "forecast_metrics",
        Component,
        "Forecast accuracy metrics."
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        GoalSeekResult,
        "statements_analytics",
        "goal_seek_result",
        Output,
        "Outcome of a `goal_seek` solve."
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        GrowthRateConsistency,
        "statements_analytics",
        "growth_rate_consistency",
        Component,
        "Flags line items whose period-over-period growth exceeds configurable upper / lower bounds."
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        InterestExpenseReconciliation,
        "statements_analytics",
        "interest_expense_reconciliation",
        Component,
        "Reconciles interest expense against debt balances or the capital-structure interest schedule."
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        LboConfig,
        "statements_analytics",
        "lbo_config",
        Input,
        "Transaction assumptions for an LBO evaluation."
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        LboResult,
        "statements_analytics",
        "lbo_result",
        Output,
        "Outputs of an LBO evaluation."
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        LeaseSpec,
        "statements_analytics",
        "lease_spec",
        Input,
        "Richer lease spec for rent roll generation: - rent steps (explicit bumps) - arbitrary free-rent windows - optional renewal with downtime + probability"
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        LeverageRangeCheck,
        "statements_analytics",
        "leverage_range_check",
        Component,
        "Flags periods where Debt / TTM EBITDA falls outside configurable warning and error ranges."
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        LiquidityRunwayCheck,
        "statements_analytics",
        "liquidity_runway_check",
        Component,
        "Estimates the liquidity runway in months and flags periods that fall below configurable warning and error thresholds."
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        ManagementFeeSpec,
        "statements_analytics",
        "management_fee_spec",
        Input,
        "Management fee specification."
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        PeerFilter,
        "statements_analytics",
        "peer_filter",
        Component,
        "Criteria for filtering companies into a peer set."
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        PeerSet,
        "statements_analytics",
        "peer_set",
        Component,
        "A set of comparable companies with their metrics."
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        PeerStats,
        "statements_analytics",
        "peer_stats",
        Output,
        "Descriptive statistics for a peer set metric."
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        PortfolioEclResult,
        "statements_analytics",
        "portfolio_ecl_result",
        Output,
        "Portfolio-level ECL result with stage migration and segment breakdown."
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        PropertyTemplateNodes,
        "statements_analytics",
        "property_template_nodes",
        Output,
        "Standard node ids for a full property operating statement template."
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        ProvisionWaterfall,
        "statements_analytics",
        "provision_waterfall",
        Component,
        "Provision movement waterfall between two reporting dates."
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        RatingPdMap,
        "statements_analytics",
        "rating_pd_map",
        Component,
        "Rating-keyed map of cumulative PD curves."
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        RegressionResult,
        "statements_analytics",
        "regression_result",
        Output,
        "OLS regression result for fair-value estimation."
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        RelativeValueResult,
        "statements_analytics",
        "relative_value_result",
        Output,
        "Composite relative value result."
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        ScenarioDiff,
        "statements_analytics",
        "scenario_diff",
        Component,
        "Variance-style diff between two evaluated scenarios."
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        ScenarioResults,
        "statements_analytics",
        "scenario_results",
        Component,
        "Evaluated results for all scenarios in a `ScenarioSet`."
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        ScenarioSet,
        "statements_analytics",
        "scenario_set",
        Component,
        "Registry of named scenarios built on top of a base model."
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        ScorecardConfig,
        "statements_analytics",
        "scorecard_config",
        Input,
        "Configuration for credit scorecard analysis."
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        ScorecardReport,
        "statements_analytics",
        "scorecard_report",
        Output,
        "Report produced by `CreditScorecardExtension::execute`."
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        ScoringDimension,
        "statements_analytics",
        "scoring_dimension",
        Component,
        "Configuration for a single rich/cheap scoring dimension."
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        SensitivityResult,
        "statements_analytics",
        "sensitivity_result",
        Output,
        "Results of sensitivity analysis."
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        TrendCheck,
        "statements_analytics",
        "trend_check",
        Component,
        "Flags a metric that has been deteriorating for `lookback_periods` consecutive periods."
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        VarianceConfig,
        "statements_analytics",
        "variance_config",
        Input,
        "Configuration for variance analysis between two `StatementResult`."
    )
    .with_packager(package_schema),
    finstack_quant_core::schema_artifact!(
        WorkingCapitalConsistency,
        "statements_analytics",
        "working_capital_consistency",
        Component,
        "Verifies that the change in working capital on the cash flow statement equals the negative delta of net working capital on the balance sheet."
    )
    .with_packager(package_schema),
];
