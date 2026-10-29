// Generated consumer import harness. Worker entry executes only through FinstackProvider.
export const importHarnesses = {
  "finstack-base": () =>
    Promise.all([]).then((modules) => ({
      name: "finstack-base",
      modules: modules.length,
    })),
  "finstack-surface": () =>
    Promise.all([
      import("@/components/finstack/shared/primitives/surface/surface"),
    ]).then((modules) => ({
      name: "finstack-surface",
      modules: modules.length,
    })),
  "use-finstack": () =>
    Promise.all([
      import("@/hooks/shared/use-finstack/use-finstack"),
      import("@/hooks/shared/use-finstack/client"),
      import("@/hooks/shared/use-finstack/version"),
      import("@/hooks/shared/use-finstack/validation"),
      import("@/hooks/shared/use-finstack/snapshot"),
      import("@/hooks/shared/use-finstack/query"),
    ]).then((modules) => ({ name: "use-finstack", modules: modules.length })),
  "use-linked-selection": () =>
    Promise.all([
      import("@/hooks/shared/use-linked-selection/use-linked-selection"),
    ]).then((modules) => ({
      name: "use-linked-selection",
      modules: modules.length,
    })),
  "finstack-worker": () =>
    Promise.all([
      import("@/workers/finstack-service"),
      import("@/workers/services/context"),
      import("@/workers/services/core"),
      import("@/workers/services/valuations"),
      import("@/workers/services/cashflows"),
      import("@/workers/services/covenants"),
      import("@/workers/services/scenarios"),
      import("@/workers/services/attribution"),
      import("@/workers/services/analytics"),
      import("@/workers/services/portfolio"),
      import("@/workers/services/margin"),
      import("@/workers/services/calibration"),
      import("@/workers/services/models"),
      import("@/workers/services/statements"),
      import("@/workers/finstack-contract"),
    ]).then((modules) => ({
      name: "finstack-worker",
      modules: modules.length,
    })),
  "use-market-validator": () =>
    Promise.all([
      import("@/hooks/core/use-market-validator/use-market-validator"),
    ]).then((modules) => ({
      name: "use-market-validator",
      modules: modules.length,
    })),
  "use-price-instrument": () =>
    Promise.all([
      import("@/hooks/valuations/use-price-instrument/use-price-instrument"),
    ]).then((modules) => ({
      name: "use-price-instrument",
      modules: modules.length,
    })),
  "use-instrument-validator": () =>
    Promise.all([
      import("@/hooks/valuations/use-instrument-validator/use-instrument-validator"),
    ]).then((modules) => ({
      name: "use-instrument-validator",
      modules: modules.length,
    })),
  "use-cashflows": () =>
    Promise.all([
      import("@/hooks/valuations/use-cashflows/use-cashflows"),
    ]).then((modules) => ({ name: "use-cashflows", modules: modules.length })),
  "use-cashflow-ladder": () =>
    Promise.all([
      import("@/hooks/valuations/use-cashflow-ladder/use-cashflow-ladder"),
    ]).then((modules) => ({
      name: "use-cashflow-ladder",
      modules: modules.length,
    })),
  "use-scenario-table": () =>
    Promise.all([
      import("@/hooks/valuations/use-scenario-table/use-scenario-table"),
    ]).then((modules) => ({
      name: "use-scenario-table",
      modules: modules.length,
    })),
  "use-tranche-analytics": () =>
    Promise.all([
      import("@/hooks/valuations/use-tranche-analytics/use-tranche-analytics"),
    ]).then((modules) => ({
      name: "use-tranche-analytics",
      modules: modules.length,
    })),
  "use-calibrate": () =>
    Promise.all([
      import("@/hooks/calibration/use-calibrate/use-calibrate"),
    ]).then((modules) => ({ name: "use-calibrate", modules: modules.length })),
  "use-covenant-engine-validator": () =>
    Promise.all([
      import("@/hooks/covenants/use-covenant-engine-validator/use-covenant-engine-validator"),
    ]).then((modules) => ({
      name: "use-covenant-engine-validator",
      modules: modules.length,
    })),
  "use-covenant-template": () =>
    Promise.all([
      import("@/hooks/covenants/use-covenant-template/use-covenant-template"),
    ]).then((modules) => ({
      name: "use-covenant-template",
      modules: modules.length,
    })),
  "use-evaluate-covenants": () =>
    Promise.all([
      import("@/hooks/covenants/use-evaluate-covenants/use-evaluate-covenants"),
    ]).then((modules) => ({
      name: "use-evaluate-covenants",
      modules: modules.length,
    })),
  "use-market-scenario": () =>
    Promise.all([
      import("@/hooks/scenarios/use-market-scenario/use-market-scenario"),
    ]).then((modules) => ({
      name: "use-market-scenario",
      modules: modules.length,
    })),
  "use-apply-market-scenario": () =>
    Promise.all([
      import("@/hooks/scenarios/use-apply-market-scenario/use-apply-market-scenario"),
    ]).then((modules) => ({
      name: "use-apply-market-scenario",
      modules: modules.length,
    })),
  "use-horizon-return": () =>
    Promise.all([
      import("@/hooks/scenarios/use-horizon-return/use-horizon-return"),
    ]).then((modules) => ({
      name: "use-horizon-return",
      modules: modules.length,
    })),
  "use-scenario-spec-validator": () =>
    Promise.all([
      import("@/hooks/scenarios/use-scenario-spec-validator/use-scenario-spec-validator"),
    ]).then((modules) => ({
      name: "use-scenario-spec-validator",
      modules: modules.length,
    })),
  "use-list-builtin-template-metadata": () =>
    Promise.all([
      import("@/hooks/scenarios/use-list-builtin-template-metadata/use-list-builtin-template-metadata"),
    ]).then((modules) => ({
      name: "use-list-builtin-template-metadata",
      modules: modules.length,
    })),
  "use-build-from-template": () =>
    Promise.all([
      import("@/hooks/scenarios/use-build-from-template/use-build-from-template"),
    ]).then((modules) => ({
      name: "use-build-from-template",
      modules: modules.length,
    })),
  "use-build-template-component": () =>
    Promise.all([
      import("@/hooks/scenarios/use-build-template-component/use-build-template-component"),
    ]).then((modules) => ({
      name: "use-build-template-component",
      modules: modules.length,
    })),
  "use-compose-scenarios": () =>
    Promise.all([
      import("@/hooks/scenarios/use-compose-scenarios/use-compose-scenarios"),
    ]).then((modules) => ({
      name: "use-compose-scenarios",
      modules: modules.length,
    })),
  "use-attribute-pnl-many": () =>
    Promise.all([
      import("@/hooks/attribution/use-attribute-pnl-many/use-attribute-pnl-many"),
    ]).then((modules) => ({
      name: "use-attribute-pnl-many",
      modules: modules.length,
    })),
  "use-default-waterfall-order": () =>
    Promise.all([
      import("@/hooks/attribution/use-default-waterfall-order/use-default-waterfall-order"),
    ]).then((modules) => ({
      name: "use-default-waterfall-order",
      modules: modules.length,
    })),
  "use-performance-report": () =>
    Promise.all([
      import("@/hooks/analytics/use-performance-report/use-performance-report"),
    ]).then((modules) => ({
      name: "use-performance-report",
      modules: modules.length,
    })),
  "use-benchmark-report": () =>
    Promise.all([
      import("@/hooks/analytics/use-benchmark-report/use-benchmark-report"),
    ]).then((modules) => ({
      name: "use-benchmark-report",
      modules: modules.length,
    })),
  "use-value-portfolio": () =>
    Promise.all([
      import("@/hooks/portfolio/use-value-portfolio/use-value-portfolio"),
    ]).then((modules) => ({
      name: "use-value-portfolio",
      modules: modules.length,
    })),
  "use-portfolio-metrics-series": () =>
    Promise.all([
      import("@/hooks/portfolio/use-portfolio-metrics-series/use-portfolio-metrics-series"),
    ]).then((modules) => ({
      name: "use-portfolio-metrics-series",
      modules: modules.length,
    })),
  "use-portfolio-spec-validator": () =>
    Promise.all([
      import("@/hooks/portfolio/use-portfolio-spec-validator/use-portfolio-spec-validator"),
    ]).then((modules) => ({
      name: "use-portfolio-spec-validator",
      modules: modules.length,
    })),
  "use-scenario-pnl-batch": () =>
    Promise.all([
      import("@/hooks/portfolio/use-scenario-pnl-batch/use-scenario-pnl-batch"),
    ]).then((modules) => ({
      name: "use-scenario-pnl-batch",
      modules: modules.length,
    })),
  "use-apply-scenario-and-revalue": () =>
    Promise.all([
      import("@/hooks/portfolio/use-apply-scenario-and-revalue/use-apply-scenario-and-revalue"),
    ]).then((modules) => ({
      name: "use-apply-scenario-and-revalue",
      modules: modules.length,
    })),
  "use-portfolio-margin": () =>
    Promise.all([
      import("@/hooks/portfolio/use-portfolio-margin/use-portfolio-margin"),
    ]).then((modules) => ({
      name: "use-portfolio-margin",
      modules: modules.length,
    })),
  "use-simm-im": () =>
    Promise.all([import("@/hooks/margin/use-simm-im/use-simm-im")]).then(
      (modules) => ({ name: "use-simm-im", modules: modules.length }),
    ),
  "use-simm-sensitivities-validator": () =>
    Promise.all([
      import("@/hooks/margin/use-simm-sensitivities-validator/use-simm-sensitivities-validator"),
    ]).then((modules) => ({
      name: "use-simm-sensitivities-validator",
      modules: modules.length,
    })),
  "use-csa-validator": () =>
    Promise.all([
      import("@/hooks/margin/use-csa-validator/use-csa-validator"),
    ]).then((modules) => ({
      name: "use-csa-validator",
      modules: modules.length,
    })),
  "use-csa-preset": () =>
    Promise.all([import("@/hooks/margin/use-csa-preset/use-csa-preset")]).then(
      (modules) => ({ name: "use-csa-preset", modules: modules.length }),
    ),
  "use-vm": () =>
    Promise.all([import("@/hooks/margin/use-vm/use-vm")]).then((modules) => ({
      name: "use-vm",
      modules: modules.length,
    })),
  "use-margin-calls": () =>
    Promise.all([
      import("@/hooks/margin/use-margin-calls/use-margin-calls"),
    ]).then((modules) => ({
      name: "use-margin-calls",
      modules: modules.length,
    })),
  "use-margin-call-dates": () =>
    Promise.all([
      import("@/hooks/margin/use-margin-call-dates/use-margin-call-dates"),
    ]).then((modules) => ({
      name: "use-margin-call-dates",
      modules: modules.length,
    })),
  "use-fx-delta-samples": () =>
    Promise.all([
      import("@/hooks/models/use-fx-delta-samples/use-fx-delta-samples"),
    ]).then((modules) => ({
      name: "use-fx-delta-samples",
      modules: modules.length,
    })),
  "use-fx-strike-surface": () =>
    Promise.all([
      import("@/hooks/models/use-fx-strike-surface/use-fx-strike-surface"),
    ]).then((modules) => ({
      name: "use-fx-strike-surface",
      modules: modules.length,
    })),
  "use-cube-samples": () =>
    Promise.all([
      import("@/hooks/models/use-cube-samples/use-cube-samples"),
    ]).then((modules) => ({
      name: "use-cube-samples",
      modules: modules.length,
    })),
  "use-cube-slice": () =>
    Promise.all([import("@/hooks/models/use-cube-slice/use-cube-slice")]).then(
      (modules) => ({ name: "use-cube-slice", modules: modules.length }),
    ),
  "use-cube-smile": () =>
    Promise.all([import("@/hooks/models/use-cube-smile/use-cube-smile")]).then(
      (modules) => ({ name: "use-cube-smile", modules: modules.length }),
    ),
  "use-check-surface": () =>
    Promise.all([
      import("@/hooks/models/use-check-surface/use-check-surface"),
    ]).then((modules) => ({
      name: "use-check-surface",
      modules: modules.length,
    })),
  "use-migration-matrix": () =>
    Promise.all([
      import("@/hooks/models/use-migration-matrix/use-migration-matrix"),
    ]).then((modules) => ({
      name: "use-migration-matrix",
      modules: modules.length,
    })),
  "use-migration-projection": () =>
    Promise.all([
      import("@/hooks/models/use-migration-projection/use-migration-projection"),
    ]).then((modules) => ({
      name: "use-migration-projection",
      modules: modules.length,
    })),
  "use-merton-report": () =>
    Promise.all([
      import("@/hooks/models/use-merton-report/use-merton-report"),
    ]).then((modules) => ({
      name: "use-merton-report",
      modules: modules.length,
    })),
  "use-merton-model-validator": () =>
    Promise.all([
      import("@/hooks/models/use-merton-model-validator/use-merton-model-validator"),
    ]).then((modules) => ({
      name: "use-merton-model-validator",
      modules: modules.length,
    })),
  "use-credit-score": () =>
    Promise.all([
      import("@/hooks/models/use-credit-score/use-credit-score"),
    ]).then((modules) => ({
      name: "use-credit-score",
      modules: modules.length,
    })),
  "use-position-risk": () =>
    Promise.all([
      import("@/hooks/models/use-position-risk/use-position-risk"),
    ]).then((modules) => ({
      name: "use-position-risk",
      modules: modules.length,
    })),
  "use-factor-risk": () =>
    Promise.all([
      import("@/hooks/models/use-factor-risk/use-factor-risk"),
    ]).then((modules) => ({
      name: "use-factor-risk",
      modules: modules.length,
    })),
  "use-evaluate-statement": () =>
    Promise.all([
      import("@/hooks/statements/use-evaluate-statement/use-evaluate-statement"),
    ]).then((modules) => ({
      name: "use-evaluate-statement",
      modules: modules.length,
    })),
  "use-statement-model-validator": () =>
    Promise.all([
      import("@/hooks/statements/use-statement-model-validator/use-statement-model-validator"),
    ]).then((modules) => ({
      name: "use-statement-model-validator",
      modules: modules.length,
    })),
  "use-statement-formula-validator": () =>
    Promise.all([
      import("@/hooks/statements/use-statement-formula-validator/use-statement-formula-validator"),
    ]).then((modules) => ({
      name: "use-statement-formula-validator",
      modules: modules.length,
    })),
  "use-check-suite-validator": () =>
    Promise.all([
      import("@/hooks/statements/use-check-suite-validator/use-check-suite-validator"),
    ]).then((modules) => ({
      name: "use-check-suite-validator",
      modules: modules.length,
    })),
  "use-rollup-statement": () =>
    Promise.all([
      import("@/hooks/statements/use-rollup-statement/use-rollup-statement"),
    ]).then((modules) => ({
      name: "use-rollup-statement",
      modules: modules.length,
    })),
  "use-evaluate-statement-scenarios": () =>
    Promise.all([
      import("@/hooks/statements/use-evaluate-statement-scenarios/use-evaluate-statement-scenarios"),
    ]).then((modules) => ({
      name: "use-evaluate-statement-scenarios",
      modules: modules.length,
    })),
  "use-run-statement-checks": () =>
    Promise.all([
      import("@/hooks/statements/use-run-statement-checks/use-run-statement-checks"),
    ]).then((modules) => ({
      name: "use-run-statement-checks",
      modules: modules.length,
    })),
  "use-explain-statement": () =>
    Promise.all([
      import("@/hooks/statements/use-explain-statement/use-explain-statement"),
    ]).then((modules) => ({
      name: "use-explain-statement",
      modules: modules.length,
    })),
  "use-trace-statement": () =>
    Promise.all([
      import("@/hooks/statements/use-trace-statement/use-trace-statement"),
    ]).then((modules) => ({
      name: "use-trace-statement",
      modules: modules.length,
    })),
  "use-diff-statement-scenarios": () =>
    Promise.all([
      import("@/hooks/statements/use-diff-statement-scenarios/use-diff-statement-scenarios"),
    ]).then((modules) => ({
      name: "use-diff-statement-scenarios",
      modules: modules.length,
    })),
  "use-trace-statement-scenario": () =>
    Promise.all([
      import("@/hooks/statements/use-trace-statement-scenario/use-trace-statement-scenario"),
    ]).then((modules) => ({
      name: "use-trace-statement-scenario",
      modules: modules.length,
    })),
  "use-credit-assessment": () =>
    Promise.all([
      import("@/hooks/statements/use-credit-assessment/use-credit-assessment"),
    ]).then((modules) => ({
      name: "use-credit-assessment",
      modules: modules.length,
    })),
  "use-pl-summary-report": () =>
    Promise.all([
      import("@/hooks/statements/use-pl-summary-report/use-pl-summary-report"),
    ]).then((modules) => ({
      name: "use-pl-summary-report",
      modules: modules.length,
    })),
  "use-evaluate-dcf": () =>
    Promise.all([
      import("@/hooks/statements/use-evaluate-dcf/use-evaluate-dcf"),
    ]).then((modules) => ({
      name: "use-evaluate-dcf",
      modules: modules.length,
    })),
  "use-dcf-sensitivity": () =>
    Promise.all([
      import("@/hooks/statements/use-dcf-sensitivity/use-dcf-sensitivity"),
    ]).then((modules) => ({
      name: "use-dcf-sensitivity",
      modules: modules.length,
    })),
  "use-evaluate-lbo": () =>
    Promise.all([
      import("@/hooks/statements/use-evaluate-lbo/use-evaluate-lbo"),
    ]).then((modules) => ({
      name: "use-evaluate-lbo",
      modules: modules.length,
    })),
  "use-run-sensitivity": () =>
    Promise.all([
      import("@/hooks/statements/use-run-sensitivity/use-run-sensitivity"),
    ]).then((modules) => ({
      name: "use-run-sensitivity",
      modules: modules.length,
    })),
  "use-goal-seek": () =>
    Promise.all([
      import("@/hooks/statements/use-goal-seek/use-goal-seek"),
    ]).then((modules) => ({ name: "use-goal-seek", modules: modules.length })),
  "use-run-variance": () =>
    Promise.all([
      import("@/hooks/statements/use-run-variance/use-run-variance"),
    ]).then((modules) => ({
      name: "use-run-variance",
      modules: modules.length,
    })),
  "use-variance-bridge": () =>
    Promise.all([
      import("@/hooks/statements/use-variance-bridge/use-variance-bridge"),
    ]).then((modules) => ({
      name: "use-variance-bridge",
      modules: modules.length,
    })),
  "use-backtest-forecast": () =>
    Promise.all([
      import("@/hooks/statements/use-backtest-forecast/use-backtest-forecast"),
    ]).then((modules) => ({
      name: "use-backtest-forecast",
      modules: modules.length,
    })),
  "use-evaluate-monte-carlo": () =>
    Promise.all([
      import("@/hooks/statements/use-evaluate-monte-carlo/use-evaluate-monte-carlo"),
    ]).then((modules) => ({
      name: "use-evaluate-monte-carlo",
      modules: modules.length,
    })),
  "use-monte-carlo-breach-probability": () =>
    Promise.all([
      import("@/hooks/statements/use-monte-carlo-breach-probability/use-monte-carlo-breach-probability"),
    ]).then((modules) => ({
      name: "use-monte-carlo-breach-probability",
      modules: modules.length,
    })),
  "finstack-fixtures": () =>
    Promise.all([import("@/lib/finstack/fixtures/results/bond.json")]).then(
      (modules) => ({ name: "finstack-fixtures", modules: modules.length }),
    ),
  "primitive-contracts": () =>
    Promise.all([
      import("@/lib/finstack/generated/primitive-contracts.json"),
    ]).then((modules) => ({
      name: "primitive-contracts",
      modules: modules.length,
    })),
  "shared-schema-defs": () =>
    Promise.all([
      import("@/lib/finstack/generated/defs/shared.json"),
      import("@/lib/finstack/generated/types/shared"),
      import("@/lib/finstack/generated/meta/shared"),
    ]).then((modules) => ({
      name: "shared-schema-defs",
      modules: modules.length,
    })),
  "finstack-format": () =>
    Promise.all([
      import("@/lib/finstack/format/format"),
      import("@/lib/finstack/format/columns"),
      import("@/lib/finstack/format/label.mjs"),
    ]).then((modules) => ({
      name: "finstack-format",
      modules: modules.length,
    })),
  "finstack-codec": () =>
    Promise.all([
      import("@/lib/finstack/codec.mjs"),
      import("@/lib/finstack/schema.mjs"),
    ]).then((modules) => ({ name: "finstack-codec", modules: modules.length })),
  "contract-calibration": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/calibration.json"),
      import("@/lib/finstack/generated/types/calibration"),
      import("@/lib/finstack/generated/meta/calibration"),
    ]).then((modules) => ({
      name: "contract-calibration",
      modules: modules.length,
    })),
  "contract-oas-config": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/oas_config.json"),
      import("@/lib/finstack/generated/types/oas_config"),
      import("@/lib/finstack/generated/meta/oas_config"),
    ]).then((modules) => ({
      name: "contract-oas-config",
      modules: modules.length,
    })),
  "contract-scenario-grid": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/scenario_grid.json"),
      import("@/lib/finstack/generated/types/scenario_grid"),
      import("@/lib/finstack/generated/meta/scenario_grid"),
    ]).then((modules) => ({
      name: "contract-scenario-grid",
      modules: modules.length,
    })),
  "contract-covenant-engine": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/covenant_engine.json"),
      import("@/lib/finstack/generated/types/covenant_engine"),
      import("@/lib/finstack/generated/meta/covenant_engine"),
    ]).then((modules) => ({
      name: "contract-covenant-engine",
      modules: modules.length,
    })),
  "contract-covenant-report": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/covenant_report.json"),
      import("@/lib/finstack/generated/types/covenant_report"),
      import("@/lib/finstack/generated/meta/covenant_report"),
    ]).then((modules) => ({
      name: "contract-covenant-report",
      modules: modules.length,
    })),
  "contract-dated-covenant-reports": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/dated_covenant_reports.json"),
      import("@/lib/finstack/generated/types/dated_covenant_reports"),
      import("@/lib/finstack/generated/meta/dated_covenant_reports"),
    ]).then((modules) => ({
      name: "contract-dated-covenant-reports",
      modules: modules.length,
    })),
  "contract-commodity-asian-option": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/commodity_asian_option.json"),
      import("@/lib/finstack/generated/types/commodity_asian_option"),
      import("@/lib/finstack/generated/meta/commodity_asian_option"),
      import("@/lib/finstack/generated/instrument/commodity_asian_option"),
      import("@/lib/finstack/generated/examples/commodity_asian_option.json"),
    ]).then((modules) => ({
      name: "contract-commodity-asian-option",
      modules: modules.length,
    })),
  "contract-commodity-forward": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/commodity_forward.json"),
      import("@/lib/finstack/generated/types/commodity_forward"),
      import("@/lib/finstack/generated/meta/commodity_forward"),
      import("@/lib/finstack/generated/instrument/commodity_forward"),
      import("@/lib/finstack/generated/examples/commodity_forward.json"),
    ]).then((modules) => ({
      name: "contract-commodity-forward",
      modules: modules.length,
    })),
  "contract-commodity-future-option": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/commodity_future_option.json"),
      import("@/lib/finstack/generated/types/commodity_future_option"),
      import("@/lib/finstack/generated/meta/commodity_future_option"),
      import("@/lib/finstack/generated/instrument/commodity_future_option"),
      import("@/lib/finstack/generated/examples/commodity_future_option.json"),
    ]).then((modules) => ({
      name: "contract-commodity-future-option",
      modules: modules.length,
    })),
  "contract-commodity-future": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/commodity_future.json"),
      import("@/lib/finstack/generated/types/commodity_future"),
      import("@/lib/finstack/generated/meta/commodity_future"),
      import("@/lib/finstack/generated/instrument/commodity_future"),
      import("@/lib/finstack/generated/examples/commodity_future.json"),
    ]).then((modules) => ({
      name: "contract-commodity-future",
      modules: modules.length,
    })),
  "contract-commodity-spot-option": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/commodity_spot_option.json"),
      import("@/lib/finstack/generated/types/commodity_spot_option"),
      import("@/lib/finstack/generated/meta/commodity_spot_option"),
      import("@/lib/finstack/generated/instrument/commodity_spot_option"),
      import("@/lib/finstack/generated/examples/commodity_spot_option.json"),
    ]).then((modules) => ({
      name: "contract-commodity-spot-option",
      modules: modules.length,
    })),
  "contract-commodity-spread-option": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/commodity_spread_option.json"),
      import("@/lib/finstack/generated/types/commodity_spread_option"),
      import("@/lib/finstack/generated/meta/commodity_spread_option"),
      import("@/lib/finstack/generated/instrument/commodity_spread_option"),
      import("@/lib/finstack/generated/examples/commodity_spread_option.json"),
    ]).then((modules) => ({
      name: "contract-commodity-spread-option",
      modules: modules.length,
    })),
  "contract-commodity-swap": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/commodity_swap.json"),
      import("@/lib/finstack/generated/types/commodity_swap"),
      import("@/lib/finstack/generated/meta/commodity_swap"),
      import("@/lib/finstack/generated/instrument/commodity_swap"),
      import("@/lib/finstack/generated/examples/commodity_swap.json"),
    ]).then((modules) => ({
      name: "contract-commodity-swap",
      modules: modules.length,
    })),
  "contract-commodity-swaption": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/commodity_swaption.json"),
      import("@/lib/finstack/generated/types/commodity_swaption"),
      import("@/lib/finstack/generated/meta/commodity_swaption"),
      import("@/lib/finstack/generated/instrument/commodity_swaption"),
      import("@/lib/finstack/generated/examples/commodity_swaption.json"),
    ]).then((modules) => ({
      name: "contract-commodity-swaption",
      modules: modules.length,
    })),
  "contract-composite": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/composite.json"),
      import("@/lib/finstack/generated/types/composite"),
      import("@/lib/finstack/generated/meta/composite"),
      import("@/lib/finstack/generated/instrument/composite"),
      import("@/lib/finstack/generated/examples/composite.json"),
    ]).then((modules) => ({
      name: "contract-composite",
      modules: modules.length,
    })),
  "contract-cds-index": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/cds_index.json"),
      import("@/lib/finstack/generated/types/cds_index"),
      import("@/lib/finstack/generated/meta/cds_index"),
      import("@/lib/finstack/generated/instrument/cds_index"),
      import("@/lib/finstack/generated/examples/cds_index.json"),
    ]).then((modules) => ({
      name: "contract-cds-index",
      modules: modules.length,
    })),
  "contract-cds-option": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/cds_option.json"),
      import("@/lib/finstack/generated/types/cds_option"),
      import("@/lib/finstack/generated/meta/cds_option"),
      import("@/lib/finstack/generated/instrument/cds_option"),
      import("@/lib/finstack/generated/examples/cds_option.json"),
    ]).then((modules) => ({
      name: "contract-cds-option",
      modules: modules.length,
    })),
  "contract-cds-tranche": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/cds_tranche.json"),
      import("@/lib/finstack/generated/types/cds_tranche"),
      import("@/lib/finstack/generated/meta/cds_tranche"),
      import("@/lib/finstack/generated/instrument/cds_tranche"),
      import("@/lib/finstack/generated/examples/cds_tranche.json"),
    ]).then((modules) => ({
      name: "contract-cds-tranche",
      modules: modules.length,
    })),
  "contract-credit-default-swap": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/credit_default_swap.json"),
      import("@/lib/finstack/generated/types/credit_default_swap"),
      import("@/lib/finstack/generated/meta/credit_default_swap"),
      import("@/lib/finstack/generated/instrument/credit_default_swap"),
      import("@/lib/finstack/generated/examples/credit_default_swap.json"),
    ]).then((modules) => ({
      name: "contract-credit-default-swap",
      modules: modules.length,
    })),
  "contract-autocallable": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/autocallable.json"),
      import("@/lib/finstack/generated/types/autocallable"),
      import("@/lib/finstack/generated/meta/autocallable"),
      import("@/lib/finstack/generated/instrument/autocallable"),
      import("@/lib/finstack/generated/examples/autocallable.json"),
    ]).then((modules) => ({
      name: "contract-autocallable",
      modules: modules.length,
    })),
  "contract-cliquet-option": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/cliquet_option.json"),
      import("@/lib/finstack/generated/types/cliquet_option"),
      import("@/lib/finstack/generated/meta/cliquet_option"),
      import("@/lib/finstack/generated/instrument/cliquet_option"),
      import("@/lib/finstack/generated/examples/cliquet_option.json"),
    ]).then((modules) => ({
      name: "contract-cliquet-option",
      modules: modules.length,
    })),
  "contract-discounted-cash-flow": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/discounted_cash_flow.json"),
      import("@/lib/finstack/generated/types/discounted_cash_flow"),
      import("@/lib/finstack/generated/meta/discounted_cash_flow"),
      import("@/lib/finstack/generated/instrument/discounted_cash_flow"),
      import("@/lib/finstack/generated/examples/discounted_cash_flow.json"),
    ]).then((modules) => ({
      name: "contract-discounted-cash-flow",
      modules: modules.length,
    })),
  "contract-equity-future-option": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/equity_future_option.json"),
      import("@/lib/finstack/generated/types/equity_future_option"),
      import("@/lib/finstack/generated/meta/equity_future_option"),
      import("@/lib/finstack/generated/instrument/equity_future_option"),
      import("@/lib/finstack/generated/examples/equity_future_option.json"),
    ]).then((modules) => ({
      name: "contract-equity-future-option",
      modules: modules.length,
    })),
  "contract-equity-future": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/equity_future.json"),
      import("@/lib/finstack/generated/types/equity_future"),
      import("@/lib/finstack/generated/meta/equity_future"),
      import("@/lib/finstack/generated/instrument/equity_future"),
      import("@/lib/finstack/generated/examples/equity_future.json"),
    ]).then((modules) => ({
      name: "contract-equity-future",
      modules: modules.length,
    })),
  "contract-equity-option": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/equity_option.json"),
      import("@/lib/finstack/generated/types/equity_option"),
      import("@/lib/finstack/generated/meta/equity_option"),
      import("@/lib/finstack/generated/instrument/equity_option"),
      import("@/lib/finstack/generated/examples/equity_option.json"),
    ]).then((modules) => ({
      name: "contract-equity-option",
      modules: modules.length,
    })),
  "contract-equity-total-return-future": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/equity_total_return_future.json"),
      import("@/lib/finstack/generated/types/equity_total_return_future"),
      import("@/lib/finstack/generated/meta/equity_total_return_future"),
      import("@/lib/finstack/generated/instrument/equity_total_return_future"),
      import("@/lib/finstack/generated/examples/equity_total_return_future.json"),
    ]).then((modules) => ({
      name: "contract-equity-total-return-future",
      modules: modules.length,
    })),
  "contract-equity": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/equity.json"),
      import("@/lib/finstack/generated/types/equity"),
      import("@/lib/finstack/generated/meta/equity"),
      import("@/lib/finstack/generated/instrument/equity"),
      import("@/lib/finstack/generated/examples/equity.json"),
    ]).then((modules) => ({
      name: "contract-equity",
      modules: modules.length,
    })),
  "contract-levered-real-estate-equity": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/levered_real_estate_equity.json"),
      import("@/lib/finstack/generated/types/levered_real_estate_equity"),
      import("@/lib/finstack/generated/meta/levered_real_estate_equity"),
      import("@/lib/finstack/generated/instrument/levered_real_estate_equity"),
      import("@/lib/finstack/generated/examples/levered_real_estate_equity.json"),
    ]).then((modules) => ({
      name: "contract-levered-real-estate-equity",
      modules: modules.length,
    })),
  "contract-private-markets-fund": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/private_markets_fund.json"),
      import("@/lib/finstack/generated/types/private_markets_fund"),
      import("@/lib/finstack/generated/meta/private_markets_fund"),
      import("@/lib/finstack/generated/instrument/private_markets_fund"),
      import("@/lib/finstack/generated/examples/private_markets_fund.json"),
    ]).then((modules) => ({
      name: "contract-private-markets-fund",
      modules: modules.length,
    })),
  "contract-real-estate-asset": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/real_estate_asset.json"),
      import("@/lib/finstack/generated/types/real_estate_asset"),
      import("@/lib/finstack/generated/meta/real_estate_asset"),
      import("@/lib/finstack/generated/instrument/real_estate_asset"),
      import("@/lib/finstack/generated/examples/real_estate_asset.json"),
    ]).then((modules) => ({
      name: "contract-real-estate-asset",
      modules: modules.length,
    })),
  "contract-trs-equity": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/trs_equity.json"),
      import("@/lib/finstack/generated/types/trs_equity"),
      import("@/lib/finstack/generated/meta/trs_equity"),
      import("@/lib/finstack/generated/instrument/trs_equity"),
      import("@/lib/finstack/generated/examples/trs_equity.json"),
    ]).then((modules) => ({
      name: "contract-trs-equity",
      modules: modules.length,
    })),
  "contract-variance-swap": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/variance_swap.json"),
      import("@/lib/finstack/generated/types/variance_swap"),
      import("@/lib/finstack/generated/meta/variance_swap"),
      import("@/lib/finstack/generated/instrument/variance_swap"),
      import("@/lib/finstack/generated/examples/variance_swap.json"),
    ]).then((modules) => ({
      name: "contract-variance-swap",
      modules: modules.length,
    })),
  "contract-volatility-index-future-option": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/volatility_index_future_option.json"),
      import("@/lib/finstack/generated/types/volatility_index_future_option"),
      import("@/lib/finstack/generated/meta/volatility_index_future_option"),
      import("@/lib/finstack/generated/instrument/volatility_index_future_option"),
      import("@/lib/finstack/generated/examples/volatility_index_future_option.json"),
    ]).then((modules) => ({
      name: "contract-volatility-index-future-option",
      modules: modules.length,
    })),
  "contract-volatility-index-future": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/volatility_index_future.json"),
      import("@/lib/finstack/generated/types/volatility_index_future"),
      import("@/lib/finstack/generated/meta/volatility_index_future"),
      import("@/lib/finstack/generated/instrument/volatility_index_future"),
      import("@/lib/finstack/generated/examples/volatility_index_future.json"),
    ]).then((modules) => ({
      name: "contract-volatility-index-future",
      modules: modules.length,
    })),
  "contract-asian-option": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/asian_option.json"),
      import("@/lib/finstack/generated/types/asian_option"),
      import("@/lib/finstack/generated/meta/asian_option"),
      import("@/lib/finstack/generated/instrument/asian_option"),
      import("@/lib/finstack/generated/examples/asian_option.json"),
    ]).then((modules) => ({
      name: "contract-asian-option",
      modules: modules.length,
    })),
  "contract-barrier-option": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/barrier_option.json"),
      import("@/lib/finstack/generated/types/barrier_option"),
      import("@/lib/finstack/generated/meta/barrier_option"),
      import("@/lib/finstack/generated/instrument/barrier_option"),
      import("@/lib/finstack/generated/examples/barrier_option.json"),
    ]).then((modules) => ({
      name: "contract-barrier-option",
      modules: modules.length,
    })),
  "contract-basket": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/basket.json"),
      import("@/lib/finstack/generated/types/basket"),
      import("@/lib/finstack/generated/meta/basket"),
      import("@/lib/finstack/generated/instrument/basket"),
      import("@/lib/finstack/generated/examples/basket.json"),
    ]).then((modules) => ({
      name: "contract-basket",
      modules: modules.length,
    })),
  "contract-callable-range-accrual": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/callable_range_accrual.json"),
      import("@/lib/finstack/generated/types/callable_range_accrual"),
      import("@/lib/finstack/generated/meta/callable_range_accrual"),
      import("@/lib/finstack/generated/instrument/callable_range_accrual"),
      import("@/lib/finstack/generated/examples/callable_range_accrual.json"),
    ]).then((modules) => ({
      name: "contract-callable-range-accrual",
      modules: modules.length,
    })),
  "contract-lookback-option": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/lookback_option.json"),
      import("@/lib/finstack/generated/types/lookback_option"),
      import("@/lib/finstack/generated/meta/lookback_option"),
      import("@/lib/finstack/generated/instrument/lookback_option"),
      import("@/lib/finstack/generated/examples/lookback_option.json"),
    ]).then((modules) => ({
      name: "contract-lookback-option",
      modules: modules.length,
    })),
  "contract-range-accrual": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/range_accrual.json"),
      import("@/lib/finstack/generated/types/range_accrual"),
      import("@/lib/finstack/generated/meta/range_accrual"),
      import("@/lib/finstack/generated/instrument/range_accrual"),
      import("@/lib/finstack/generated/examples/range_accrual.json"),
    ]).then((modules) => ({
      name: "contract-range-accrual",
      modules: modules.length,
    })),
  "contract-snowball": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/snowball.json"),
      import("@/lib/finstack/generated/types/snowball"),
      import("@/lib/finstack/generated/meta/snowball"),
      import("@/lib/finstack/generated/instrument/snowball"),
      import("@/lib/finstack/generated/examples/snowball.json"),
    ]).then((modules) => ({
      name: "contract-snowball",
      modules: modules.length,
    })),
  "contract-tarn": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/tarn.json"),
      import("@/lib/finstack/generated/types/tarn"),
      import("@/lib/finstack/generated/meta/tarn"),
      import("@/lib/finstack/generated/instrument/tarn"),
      import("@/lib/finstack/generated/examples/tarn.json"),
    ]).then((modules) => ({ name: "contract-tarn", modules: modules.length })),
  "contract-agency-cmo": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/agency_cmo.json"),
      import("@/lib/finstack/generated/types/agency_cmo"),
      import("@/lib/finstack/generated/meta/agency_cmo"),
      import("@/lib/finstack/generated/instrument/agency_cmo"),
      import("@/lib/finstack/generated/examples/agency_cmo.json"),
    ]).then((modules) => ({
      name: "contract-agency-cmo",
      modules: modules.length,
    })),
  "contract-agency-mbs-passthrough": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/agency_mbs_passthrough.json"),
      import("@/lib/finstack/generated/types/agency_mbs_passthrough"),
      import("@/lib/finstack/generated/meta/agency_mbs_passthrough"),
      import("@/lib/finstack/generated/instrument/agency_mbs_passthrough"),
      import("@/lib/finstack/generated/examples/agency_mbs_passthrough.json"),
    ]).then((modules) => ({
      name: "contract-agency-mbs-passthrough",
      modules: modules.length,
    })),
  "contract-agency-tba": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/agency_tba.json"),
      import("@/lib/finstack/generated/types/agency_tba"),
      import("@/lib/finstack/generated/meta/agency_tba"),
      import("@/lib/finstack/generated/instrument/agency_tba"),
      import("@/lib/finstack/generated/examples/agency_tba.json"),
    ]).then((modules) => ({
      name: "contract-agency-tba",
      modules: modules.length,
    })),
  "contract-asset-backed-facility": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/asset_backed_facility.json"),
      import("@/lib/finstack/generated/types/asset_backed_facility"),
      import("@/lib/finstack/generated/meta/asset_backed_facility"),
      import("@/lib/finstack/generated/instrument/asset_backed_facility"),
      import("@/lib/finstack/generated/examples/asset_backed_facility.json"),
    ]).then((modules) => ({
      name: "contract-asset-backed-facility",
      modules: modules.length,
    })),
  "contract-bond-future": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/bond_future.json"),
      import("@/lib/finstack/generated/types/bond_future"),
      import("@/lib/finstack/generated/meta/bond_future"),
      import("@/lib/finstack/generated/instrument/bond_future"),
      import("@/lib/finstack/generated/examples/bond_future.json"),
    ]).then((modules) => ({
      name: "contract-bond-future",
      modules: modules.length,
    })),
  "contract-bond": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/bond.json"),
      import("@/lib/finstack/generated/types/bond"),
      import("@/lib/finstack/generated/meta/bond"),
      import("@/lib/finstack/generated/instrument/bond"),
      import("@/lib/finstack/generated/examples/bond.json"),
    ]).then((modules) => ({ name: "contract-bond", modules: modules.length })),
  "contract-convertible-bond": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/convertible_bond.json"),
      import("@/lib/finstack/generated/types/convertible_bond"),
      import("@/lib/finstack/generated/meta/convertible_bond"),
      import("@/lib/finstack/generated/instrument/convertible_bond"),
      import("@/lib/finstack/generated/examples/convertible_bond.json"),
    ]).then((modules) => ({
      name: "contract-convertible-bond",
      modules: modules.length,
    })),
  "contract-dollar-roll": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/dollar_roll.json"),
      import("@/lib/finstack/generated/types/dollar_roll"),
      import("@/lib/finstack/generated/meta/dollar_roll"),
      import("@/lib/finstack/generated/instrument/dollar_roll"),
      import("@/lib/finstack/generated/examples/dollar_roll.json"),
    ]).then((modules) => ({
      name: "contract-dollar-roll",
      modules: modules.length,
    })),
  "contract-inflation-linked-bond": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/inflation_linked_bond.json"),
      import("@/lib/finstack/generated/types/inflation_linked_bond"),
      import("@/lib/finstack/generated/meta/inflation_linked_bond"),
      import("@/lib/finstack/generated/instrument/inflation_linked_bond"),
      import("@/lib/finstack/generated/examples/inflation_linked_bond.json"),
    ]).then((modules) => ({
      name: "contract-inflation-linked-bond",
      modules: modules.length,
    })),
  "contract-revolving-credit": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/revolving_credit.json"),
      import("@/lib/finstack/generated/types/revolving_credit"),
      import("@/lib/finstack/generated/meta/revolving_credit"),
      import("@/lib/finstack/generated/instrument/revolving_credit"),
      import("@/lib/finstack/generated/examples/revolving_credit.json"),
    ]).then((modules) => ({
      name: "contract-revolving-credit",
      modules: modules.length,
    })),
  "contract-structured-credit": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/structured_credit.json"),
      import("@/lib/finstack/generated/types/structured_credit"),
      import("@/lib/finstack/generated/meta/structured_credit"),
      import("@/lib/finstack/generated/instrument/structured_credit"),
      import("@/lib/finstack/generated/examples/structured_credit.json"),
    ]).then((modules) => ({
      name: "contract-structured-credit",
      modules: modules.length,
    })),
  "contract-term-loan": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/term_loan.json"),
      import("@/lib/finstack/generated/types/term_loan"),
      import("@/lib/finstack/generated/meta/term_loan"),
      import("@/lib/finstack/generated/instrument/term_loan"),
      import("@/lib/finstack/generated/examples/term_loan.json"),
    ]).then((modules) => ({
      name: "contract-term-loan",
      modules: modules.length,
    })),
  "contract-trs-fixed-income-index": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/trs_fixed_income_index.json"),
      import("@/lib/finstack/generated/types/trs_fixed_income_index"),
      import("@/lib/finstack/generated/meta/trs_fixed_income_index"),
      import("@/lib/finstack/generated/instrument/trs_fixed_income_index"),
      import("@/lib/finstack/generated/examples/trs_fixed_income_index.json"),
    ]).then((modules) => ({
      name: "contract-trs-fixed-income-index",
      modules: modules.length,
    })),
  "contract-fx-barrier-option": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/fx_barrier_option.json"),
      import("@/lib/finstack/generated/types/fx_barrier_option"),
      import("@/lib/finstack/generated/meta/fx_barrier_option"),
      import("@/lib/finstack/generated/instrument/fx_barrier_option"),
      import("@/lib/finstack/generated/examples/fx_barrier_option.json"),
    ]).then((modules) => ({
      name: "contract-fx-barrier-option",
      modules: modules.length,
    })),
  "contract-fx-digital-option": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/fx_digital_option.json"),
      import("@/lib/finstack/generated/types/fx_digital_option"),
      import("@/lib/finstack/generated/meta/fx_digital_option"),
      import("@/lib/finstack/generated/instrument/fx_digital_option"),
      import("@/lib/finstack/generated/examples/fx_digital_option.json"),
    ]).then((modules) => ({
      name: "contract-fx-digital-option",
      modules: modules.length,
    })),
  "contract-fx-forward": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/fx_forward.json"),
      import("@/lib/finstack/generated/types/fx_forward"),
      import("@/lib/finstack/generated/meta/fx_forward"),
      import("@/lib/finstack/generated/instrument/fx_forward"),
      import("@/lib/finstack/generated/examples/fx_forward.json"),
    ]).then((modules) => ({
      name: "contract-fx-forward",
      modules: modules.length,
    })),
  "contract-fx-future-option": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/fx_future_option.json"),
      import("@/lib/finstack/generated/types/fx_future_option"),
      import("@/lib/finstack/generated/meta/fx_future_option"),
      import("@/lib/finstack/generated/instrument/fx_future_option"),
      import("@/lib/finstack/generated/examples/fx_future_option.json"),
    ]).then((modules) => ({
      name: "contract-fx-future-option",
      modules: modules.length,
    })),
  "contract-fx-future": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/fx_future.json"),
      import("@/lib/finstack/generated/types/fx_future"),
      import("@/lib/finstack/generated/meta/fx_future"),
      import("@/lib/finstack/generated/instrument/fx_future"),
      import("@/lib/finstack/generated/examples/fx_future.json"),
    ]).then((modules) => ({
      name: "contract-fx-future",
      modules: modules.length,
    })),
  "contract-fx-option": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/fx_option.json"),
      import("@/lib/finstack/generated/types/fx_option"),
      import("@/lib/finstack/generated/meta/fx_option"),
      import("@/lib/finstack/generated/instrument/fx_option"),
      import("@/lib/finstack/generated/examples/fx_option.json"),
    ]).then((modules) => ({
      name: "contract-fx-option",
      modules: modules.length,
    })),
  "contract-fx-spot": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/fx_spot.json"),
      import("@/lib/finstack/generated/types/fx_spot"),
      import("@/lib/finstack/generated/meta/fx_spot"),
      import("@/lib/finstack/generated/instrument/fx_spot"),
      import("@/lib/finstack/generated/examples/fx_spot.json"),
    ]).then((modules) => ({
      name: "contract-fx-spot",
      modules: modules.length,
    })),
  "contract-fx-swap": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/fx_swap.json"),
      import("@/lib/finstack/generated/types/fx_swap"),
      import("@/lib/finstack/generated/meta/fx_swap"),
      import("@/lib/finstack/generated/instrument/fx_swap"),
      import("@/lib/finstack/generated/examples/fx_swap.json"),
    ]).then((modules) => ({
      name: "contract-fx-swap",
      modules: modules.length,
    })),
  "contract-fx-touch-option": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/fx_touch_option.json"),
      import("@/lib/finstack/generated/types/fx_touch_option"),
      import("@/lib/finstack/generated/meta/fx_touch_option"),
      import("@/lib/finstack/generated/instrument/fx_touch_option"),
      import("@/lib/finstack/generated/examples/fx_touch_option.json"),
    ]).then((modules) => ({
      name: "contract-fx-touch-option",
      modules: modules.length,
    })),
  "contract-fx-variance-swap": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/fx_variance_swap.json"),
      import("@/lib/finstack/generated/types/fx_variance_swap"),
      import("@/lib/finstack/generated/meta/fx_variance_swap"),
      import("@/lib/finstack/generated/instrument/fx_variance_swap"),
      import("@/lib/finstack/generated/examples/fx_variance_swap.json"),
    ]).then((modules) => ({
      name: "contract-fx-variance-swap",
      modules: modules.length,
    })),
  "contract-ndf": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/ndf.json"),
      import("@/lib/finstack/generated/types/ndf"),
      import("@/lib/finstack/generated/meta/ndf"),
      import("@/lib/finstack/generated/instrument/ndf"),
      import("@/lib/finstack/generated/examples/ndf.json"),
    ]).then((modules) => ({ name: "contract-ndf", modules: modules.length })),
  "contract-quanto-option": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/quanto_option.json"),
      import("@/lib/finstack/generated/types/quanto_option"),
      import("@/lib/finstack/generated/meta/quanto_option"),
      import("@/lib/finstack/generated/instrument/quanto_option"),
      import("@/lib/finstack/generated/examples/quanto_option.json"),
    ]).then((modules) => ({
      name: "contract-quanto-option",
      modules: modules.length,
    })),
  "contract-basis-swap": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/basis_swap.json"),
      import("@/lib/finstack/generated/types/basis_swap"),
      import("@/lib/finstack/generated/meta/basis_swap"),
      import("@/lib/finstack/generated/instrument/basis_swap"),
      import("@/lib/finstack/generated/examples/basis_swap.json"),
    ]).then((modules) => ({
      name: "contract-basis-swap",
      modules: modules.length,
    })),
  "contract-bermudan-swaption": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/bermudan_swaption.json"),
      import("@/lib/finstack/generated/types/bermudan_swaption"),
      import("@/lib/finstack/generated/meta/bermudan_swaption"),
      import("@/lib/finstack/generated/instrument/bermudan_swaption"),
      import("@/lib/finstack/generated/examples/bermudan_swaption.json"),
    ]).then((modules) => ({
      name: "contract-bermudan-swaption",
      modules: modules.length,
    })),
  "contract-cap-floor": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/cap_floor.json"),
      import("@/lib/finstack/generated/types/cap_floor"),
      import("@/lib/finstack/generated/meta/cap_floor"),
      import("@/lib/finstack/generated/instrument/cap_floor"),
      import("@/lib/finstack/generated/examples/cap_floor.json"),
    ]).then((modules) => ({
      name: "contract-cap-floor",
      modules: modules.length,
    })),
  "contract-cms-option": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/cms_option.json"),
      import("@/lib/finstack/generated/types/cms_option"),
      import("@/lib/finstack/generated/meta/cms_option"),
      import("@/lib/finstack/generated/instrument/cms_option"),
      import("@/lib/finstack/generated/examples/cms_option.json"),
    ]).then((modules) => ({
      name: "contract-cms-option",
      modules: modules.length,
    })),
  "contract-cms-spread-option": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/cms_spread_option.json"),
      import("@/lib/finstack/generated/types/cms_spread_option"),
      import("@/lib/finstack/generated/meta/cms_spread_option"),
      import("@/lib/finstack/generated/instrument/cms_spread_option"),
      import("@/lib/finstack/generated/examples/cms_spread_option.json"),
    ]).then((modules) => ({
      name: "contract-cms-spread-option",
      modules: modules.length,
    })),
  "contract-cms-swap": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/cms_swap.json"),
      import("@/lib/finstack/generated/types/cms_swap"),
      import("@/lib/finstack/generated/meta/cms_swap"),
      import("@/lib/finstack/generated/instrument/cms_swap"),
      import("@/lib/finstack/generated/examples/cms_swap.json"),
    ]).then((modules) => ({
      name: "contract-cms-swap",
      modules: modules.length,
    })),
  "contract-deposit": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/deposit.json"),
      import("@/lib/finstack/generated/types/deposit"),
      import("@/lib/finstack/generated/meta/deposit"),
      import("@/lib/finstack/generated/instrument/deposit"),
      import("@/lib/finstack/generated/examples/deposit.json"),
    ]).then((modules) => ({
      name: "contract-deposit",
      modules: modules.length,
    })),
  "contract-forward-rate-agreement": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/forward_rate_agreement.json"),
      import("@/lib/finstack/generated/types/forward_rate_agreement"),
      import("@/lib/finstack/generated/meta/forward_rate_agreement"),
      import("@/lib/finstack/generated/instrument/forward_rate_agreement"),
      import("@/lib/finstack/generated/examples/forward_rate_agreement.json"),
    ]).then((modules) => ({
      name: "contract-forward-rate-agreement",
      modules: modules.length,
    })),
  "contract-inflation-cap-floor": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/inflation_cap_floor.json"),
      import("@/lib/finstack/generated/types/inflation_cap_floor"),
      import("@/lib/finstack/generated/meta/inflation_cap_floor"),
      import("@/lib/finstack/generated/instrument/inflation_cap_floor"),
      import("@/lib/finstack/generated/examples/inflation_cap_floor.json"),
    ]).then((modules) => ({
      name: "contract-inflation-cap-floor",
      modules: modules.length,
    })),
  "contract-inflation-swap": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/inflation_swap.json"),
      import("@/lib/finstack/generated/types/inflation_swap"),
      import("@/lib/finstack/generated/meta/inflation_swap"),
      import("@/lib/finstack/generated/instrument/inflation_swap"),
      import("@/lib/finstack/generated/examples/inflation_swap.json"),
    ]).then((modules) => ({
      name: "contract-inflation-swap",
      modules: modules.length,
    })),
  "contract-interest-rate-future-option": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/interest_rate_future_option.json"),
      import("@/lib/finstack/generated/types/interest_rate_future_option"),
      import("@/lib/finstack/generated/meta/interest_rate_future_option"),
      import("@/lib/finstack/generated/instrument/interest_rate_future_option"),
      import("@/lib/finstack/generated/examples/interest_rate_future_option.json"),
    ]).then((modules) => ({
      name: "contract-interest-rate-future-option",
      modules: modules.length,
    })),
  "contract-interest-rate-future": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/interest_rate_future.json"),
      import("@/lib/finstack/generated/types/interest_rate_future"),
      import("@/lib/finstack/generated/meta/interest_rate_future"),
      import("@/lib/finstack/generated/instrument/interest_rate_future"),
      import("@/lib/finstack/generated/examples/interest_rate_future.json"),
    ]).then((modules) => ({
      name: "contract-interest-rate-future",
      modules: modules.length,
    })),
  "contract-interest-rate-swap": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/interest_rate_swap.json"),
      import("@/lib/finstack/generated/types/interest_rate_swap"),
      import("@/lib/finstack/generated/meta/interest_rate_swap"),
      import("@/lib/finstack/generated/instrument/interest_rate_swap"),
      import("@/lib/finstack/generated/examples/interest_rate_swap.json"),
    ]).then((modules) => ({
      name: "contract-interest-rate-swap",
      modules: modules.length,
    })),
  "contract-repo": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/repo.json"),
      import("@/lib/finstack/generated/types/repo"),
      import("@/lib/finstack/generated/meta/repo"),
      import("@/lib/finstack/generated/instrument/repo"),
      import("@/lib/finstack/generated/examples/repo.json"),
    ]).then((modules) => ({ name: "contract-repo", modules: modules.length })),
  "contract-swaption": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/swaption.json"),
      import("@/lib/finstack/generated/types/swaption"),
      import("@/lib/finstack/generated/meta/swaption"),
      import("@/lib/finstack/generated/instrument/swaption"),
      import("@/lib/finstack/generated/examples/swaption.json"),
    ]).then((modules) => ({
      name: "contract-swaption",
      modules: modules.length,
    })),
  "contract-xccy-swap": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/xccy_swap.json"),
      import("@/lib/finstack/generated/types/xccy_swap"),
      import("@/lib/finstack/generated/meta/xccy_swap"),
      import("@/lib/finstack/generated/instrument/xccy_swap"),
      import("@/lib/finstack/generated/examples/xccy_swap.json"),
    ]).then((modules) => ({
      name: "contract-xccy-swap",
      modules: modules.length,
    })),
  "contract-yoy-inflation-swap": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/yoy_inflation_swap.json"),
      import("@/lib/finstack/generated/types/yoy_inflation_swap"),
      import("@/lib/finstack/generated/meta/yoy_inflation_swap"),
      import("@/lib/finstack/generated/instrument/yoy_inflation_swap"),
      import("@/lib/finstack/generated/examples/yoy_inflation_swap.json"),
    ]).then((modules) => ({
      name: "contract-yoy-inflation-swap",
      modules: modules.length,
    })),
  "contract-csa-spec": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/csa_spec.json"),
      import("@/lib/finstack/generated/types/csa_spec"),
      import("@/lib/finstack/generated/meta/csa_spec"),
    ]).then((modules) => ({
      name: "contract-csa-spec",
      modules: modules.length,
    })),
  "contract-im-result": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/im_result.json"),
      import("@/lib/finstack/generated/types/im_result"),
      import("@/lib/finstack/generated/meta/im_result"),
    ]).then((modules) => ({
      name: "contract-im-result",
      modules: modules.length,
    })),
  "contract-simm-sensitivities-json": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/simm_sensitivities_json.json"),
      import("@/lib/finstack/generated/types/simm_sensitivities_json"),
      import("@/lib/finstack/generated/meta/simm_sensitivities_json"),
    ]).then((modules) => ({
      name: "contract-simm-sensitivities-json",
      modules: modules.length,
    })),
  "contract-vm-result": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/vm_result.json"),
      import("@/lib/finstack/generated/types/vm_result"),
      import("@/lib/finstack/generated/meta/vm_result"),
    ]).then((modules) => ({
      name: "contract-vm-result",
      modules: modules.length,
    })),
  "contract-market-context-state": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/market_context_state.json"),
      import("@/lib/finstack/generated/types/market_context_state"),
      import("@/lib/finstack/generated/meta/market_context_state"),
    ]).then((modules) => ({
      name: "contract-market-context-state",
      modules: modules.length,
    })),
  "contract-altman-z-double-prime-input": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/altman_z_double_prime_input.json"),
      import("@/lib/finstack/generated/types/altman_z_double_prime_input"),
      import("@/lib/finstack/generated/meta/altman_z_double_prime_input"),
    ]).then((modules) => ({
      name: "contract-altman-z-double-prime-input",
      modules: modules.length,
    })),
  "contract-altman-z-prime-input": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/altman_z_prime_input.json"),
      import("@/lib/finstack/generated/types/altman_z_prime_input"),
      import("@/lib/finstack/generated/meta/altman_z_prime_input"),
    ]).then((modules) => ({
      name: "contract-altman-z-prime-input",
      modules: modules.length,
    })),
  "contract-altman-z-score-input": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/altman_z_score_input.json"),
      import("@/lib/finstack/generated/types/altman_z_score_input"),
      import("@/lib/finstack/generated/meta/altman_z_score_input"),
    ]).then((modules) => ({
      name: "contract-altman-z-score-input",
      modules: modules.length,
    })),
  "contract-arbitrage-report": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/arbitrage_report.json"),
      import("@/lib/finstack/generated/types/arbitrage_report"),
      import("@/lib/finstack/generated/meta/arbitrage_report"),
    ]).then((modules) => ({
      name: "contract-arbitrage-report",
      modules: modules.length,
    })),
  "contract-merton-model": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/merton_model.json"),
      import("@/lib/finstack/generated/types/merton_model"),
      import("@/lib/finstack/generated/meta/merton_model"),
    ]).then((modules) => ({
      name: "contract-merton-model",
      modules: modules.length,
    })),
  "contract-ohlson-o-score-input": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/ohlson_o_score_input.json"),
      import("@/lib/finstack/generated/types/ohlson_o_score_input"),
      import("@/lib/finstack/generated/meta/ohlson_o_score_input"),
    ]).then((modules) => ({
      name: "contract-ohlson-o-score-input",
      modules: modules.length,
    })),
  "contract-zmijewski-input": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/zmijewski_input.json"),
      import("@/lib/finstack/generated/types/zmijewski_input"),
      import("@/lib/finstack/generated/meta/zmijewski_input"),
    ]).then((modules) => ({
      name: "contract-zmijewski-input",
      modules: modules.length,
    })),
  "contract-portfolio-margin-result": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/portfolio_margin_result.json"),
      import("@/lib/finstack/generated/types/portfolio_margin_result"),
      import("@/lib/finstack/generated/meta/portfolio_margin_result"),
    ]).then((modules) => ({
      name: "contract-portfolio-margin-result",
      modules: modules.length,
    })),
  "contract-portfolio-result": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/portfolio_result.json"),
      import("@/lib/finstack/generated/types/portfolio_result"),
      import("@/lib/finstack/generated/meta/portfolio_result"),
    ]).then((modules) => ({
      name: "contract-portfolio-result",
      modules: modules.length,
    })),
  "contract-scenario-pnl-batch-item": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/scenario_pnl_batch_item.json"),
      import("@/lib/finstack/generated/types/scenario_pnl_batch_item"),
      import("@/lib/finstack/generated/meta/scenario_pnl_batch_item"),
    ]).then((modules) => ({
      name: "contract-scenario-pnl-batch-item",
      modules: modules.length,
    })),
  "contract-scenario-revalue-view": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/scenario_revalue_view.json"),
      import("@/lib/finstack/generated/types/scenario_revalue_view"),
      import("@/lib/finstack/generated/meta/scenario_revalue_view"),
    ]).then((modules) => ({
      name: "contract-scenario-revalue-view",
      modules: modules.length,
    })),
  "contract-instrument-cashflow": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/instrument_cashflow.json"),
      import("@/lib/finstack/generated/types/instrument_cashflow"),
      import("@/lib/finstack/generated/meta/instrument_cashflow"),
    ]).then((modules) => ({
      name: "contract-instrument-cashflow",
      modules: modules.length,
    })),
  "contract-valuation-result": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/valuation_result.json"),
      import("@/lib/finstack/generated/types/valuation_result"),
      import("@/lib/finstack/generated/meta/valuation_result"),
    ]).then((modules) => ({
      name: "contract-valuation-result",
      modules: modules.length,
    })),
  "contract-scenario-spec": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/scenario_spec.json"),
      import("@/lib/finstack/generated/types/scenario_spec"),
      import("@/lib/finstack/generated/meta/scenario_spec"),
    ]).then((modules) => ({
      name: "contract-scenario-spec",
      modules: modules.length,
    })),
  "contract-bridge-chart": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/bridge_chart.json"),
      import("@/lib/finstack/generated/types/bridge_chart"),
      import("@/lib/finstack/generated/meta/bridge_chart"),
    ]).then((modules) => ({
      name: "contract-bridge-chart",
      modules: modules.length,
    })),
  "contract-credit-assessment": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/credit_assessment.json"),
      import("@/lib/finstack/generated/types/credit_assessment"),
      import("@/lib/finstack/generated/meta/credit_assessment"),
    ]).then((modules) => ({
      name: "contract-credit-assessment",
      modules: modules.length,
    })),
  "contract-dcf-options": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/dcf_options.json"),
      import("@/lib/finstack/generated/types/dcf_options"),
      import("@/lib/finstack/generated/meta/dcf_options"),
    ]).then((modules) => ({
      name: "contract-dcf-options",
      modules: modules.length,
    })),
  "contract-dcf-sensitivity-result": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/dcf_sensitivity_result.json"),
      import("@/lib/finstack/generated/types/dcf_sensitivity_result"),
      import("@/lib/finstack/generated/meta/dcf_sensitivity_result"),
    ]).then((modules) => ({
      name: "contract-dcf-sensitivity-result",
      modules: modules.length,
    })),
  "contract-forecast-metrics": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/forecast_metrics.json"),
      import("@/lib/finstack/generated/types/forecast_metrics"),
      import("@/lib/finstack/generated/meta/forecast_metrics"),
    ]).then((modules) => ({
      name: "contract-forecast-metrics",
      modules: modules.length,
    })),
  "contract-lbo-config": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/lbo_config.json"),
      import("@/lib/finstack/generated/types/lbo_config"),
      import("@/lib/finstack/generated/meta/lbo_config"),
    ]).then((modules) => ({
      name: "contract-lbo-config",
      modules: modules.length,
    })),
  "contract-lbo-result": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/lbo_result.json"),
      import("@/lib/finstack/generated/types/lbo_result"),
      import("@/lib/finstack/generated/meta/lbo_result"),
    ]).then((modules) => ({
      name: "contract-lbo-result",
      modules: modules.length,
    })),
  "contract-scenario-diff": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/scenario_diff.json"),
      import("@/lib/finstack/generated/types/scenario_diff"),
      import("@/lib/finstack/generated/meta/scenario_diff"),
    ]).then((modules) => ({
      name: "contract-scenario-diff",
      modules: modules.length,
    })),
  "contract-scenario-set": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/scenario_set.json"),
      import("@/lib/finstack/generated/types/scenario_set"),
      import("@/lib/finstack/generated/meta/scenario_set"),
    ]).then((modules) => ({
      name: "contract-scenario-set",
      modules: modules.length,
    })),
  "contract-sensitivity-result": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/sensitivity_result.json"),
      import("@/lib/finstack/generated/types/sensitivity_result"),
      import("@/lib/finstack/generated/meta/sensitivity_result"),
    ]).then((modules) => ({
      name: "contract-sensitivity-result",
      modules: modules.length,
    })),
  "contract-variance-config": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/variance_config.json"),
      import("@/lib/finstack/generated/types/variance_config"),
      import("@/lib/finstack/generated/meta/variance_config"),
    ]).then((modules) => ({
      name: "contract-variance-config",
      modules: modules.length,
    })),
  "contract-check-suite-spec": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/check_suite_spec.json"),
      import("@/lib/finstack/generated/types/check_suite_spec"),
      import("@/lib/finstack/generated/meta/check_suite_spec"),
    ]).then((modules) => ({
      name: "contract-check-suite-spec",
      modules: modules.length,
    })),
  "contract-financial-model-spec": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/financial_model_spec.json"),
      import("@/lib/finstack/generated/types/financial_model_spec"),
      import("@/lib/finstack/generated/meta/financial_model_spec"),
      import("@/lib/finstack/generated/examples/financial_model_spec.json"),
    ]).then((modules) => ({
      name: "contract-financial-model-spec",
      modules: modules.length,
    })),
  "contract-monte-carlo-config": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/monte_carlo_config.json"),
      import("@/lib/finstack/generated/types/monte_carlo_config"),
      import("@/lib/finstack/generated/meta/monte_carlo_config"),
    ]).then((modules) => ({
      name: "contract-monte-carlo-config",
      modules: modules.length,
    })),
  "contract-monte-carlo-results": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/monte_carlo_results.json"),
      import("@/lib/finstack/generated/types/monte_carlo_results"),
      import("@/lib/finstack/generated/meta/monte_carlo_results"),
    ]).then((modules) => ({
      name: "contract-monte-carlo-results",
      modules: modules.length,
    })),
  "contract-rollup-spec": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/rollup_spec.json"),
      import("@/lib/finstack/generated/types/rollup_spec"),
      import("@/lib/finstack/generated/meta/rollup_spec"),
    ]).then((modules) => ({
      name: "contract-rollup-spec",
      modules: modules.length,
    })),
  "contract-statement-result": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/statement_result.json"),
      import("@/lib/finstack/generated/types/statement_result"),
      import("@/lib/finstack/generated/meta/statement_result"),
    ]).then((modules) => ({
      name: "contract-statement-result",
      modules: modules.length,
    })),
  "contract-table-envelope": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/table_envelope.json"),
      import("@/lib/finstack/generated/types/table_envelope"),
      import("@/lib/finstack/generated/meta/table_envelope"),
    ]).then((modules) => ({
      name: "contract-table-envelope",
      modules: modules.length,
    })),
  "instrument-catalogue": () =>
    Promise.all([
      import("@/lib/finstack/generated/instruments"),
      import("@/lib/finstack/generated/catalogue.json"),
    ]).then((modules) => ({
      name: "instrument-catalogue",
      modules: modules.length,
    })),
  "contract-manifest": () =>
    Promise.all([
      import("@/lib/finstack/generated/roots.json"),
      import("@/lib/finstack/generated/fixtures.json"),
      import("@/lib/finstack/contract-provenance.json"),
    ]).then((modules) => ({
      name: "contract-manifest",
      modules: modules.length,
    })),
  "finstack-views": () =>
    Promise.all([
      import("@/lib/finstack/views"),
      import("@/lib/finstack/generated/curve-views.json"),
    ]).then((modules) => ({ name: "finstack-views", modules: modules.length })),
  "finstack-host": () =>
    Promise.all([import("@/lib/finstack/host")]).then((modules) => ({
      name: "finstack-host",
      modules: modules.length,
    })),
};
