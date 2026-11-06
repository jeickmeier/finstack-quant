"use client";
import { useState } from "react";
import { CovenantEngineEditor } from "@/components/finstack/covenants/components/covenant-engine-editor/covenant-engine-editor";
import { CovenantCompliance } from "@/components/finstack/covenants/components/covenant-compliance/covenant-compliance";
import { CovenantBreachForecast } from "@/components/finstack/covenants/components/covenant-breach-forecast/covenant-breach-forecast";
import { useCovenantEngineValidator } from "@/hooks/covenants/use-covenant-engine-validator/use-covenant-engine-validator";
import { useCovenantTemplate } from "@/hooks/covenants/use-covenant-template/use-covenant-template";
import { useEvaluateCovenants } from "@/hooks/covenants/use-evaluate-covenants/use-evaluate-covenants";
import { useForecastBreaches } from "@/hooks/covenants/use-forecast-breaches/use-forecast-breaches";

/**
 * The workbench Covenants tab: edits a covenant engine (from the host's engine
 * or a Rust package template) and tests it natively against the evaluated
 * statements (statements_analytics.evaluateCovenants), one column per result
 * period. A metric the model lacks returns Rust's not_found error unchanged.
 * A submitted forecast config forecasts the same engine's breaches natively
 * (statements_analytics.forecastBreaches) over the same periods.
 */
export function CovenantsPanel({
  resultsJson,
  nodeIds,
  defaultEngineJson,
  density,
}: {
  /** Host-serialized StatementResult the workbench evaluated. */
  resultsJson: string;
  /** The model's node ids, offered for every covenant metric id. */
  nodeIds: readonly string[];
  /** Host CovenantEngine JSON, evaluated on open; omitted starts empty. */
  defaultEngineJson?: string;
  density: "compact" | "comfortable";
}) {
  const validate = useCovenantEngineValidator();
  const loadTemplate = useCovenantTemplate();
  const [engineJson, setEngineJson] = useState<string | null>(
    defaultEngineJson ?? null,
  );
  const compliance = useEvaluateCovenants(
    engineJson ? { engineJson, resultsJson } : null,
  );
  const [configJson, setConfigJson] = useState<string | null>(null);
  const forecast = useForecastBreaches(
    engineJson && configJson !== null
      ? { engineJson, resultsJson, configJson }
      : null,
  );
  return (
    <div className="max-w-5xl space-y-6">
      <div className="space-y-1">
        <h3 className="text-base font-medium">Covenants</h3>
        <p className="text-sm text-muted-foreground">
          Every result period is tested natively on its last day with the
          covenants applicable on that date. Metric ids are statement node ids.
        </p>
      </div>
      <CovenantEngineEditor
        defaultJson={defaultEngineJson}
        validate={validate}
        loadTemplate={loadTemplate}
        onSubmit={setEngineJson}
        metricIds={nodeIds}
        density={density}
      />
      {engineJson === null ? (
        <p className="text-sm text-muted-foreground">
          Use covenants to test them against the statements.
        </p>
      ) : compliance.error ? (
        <p role="alert" className="text-error">
          {compliance.error.message}
        </p>
      ) : compliance.data ? (
        <CovenantCompliance series={compliance.data} density={density} />
      ) : (
        <p role="status" className="text-sm text-muted-foreground">
          Evaluating covenants…
        </p>
      )}
      {engineJson !== null && (
        <div className="space-y-2">
          <h3 className="text-base font-medium">Breach forecast</h3>
          <p className="text-sm text-muted-foreground">
            Forecasts the engine's numeric covenants over the same periods.
            Deterministic by default; a stochastic config adds a lognormal
            overlay and lists dates whose breach probability reaches its
            threshold.
          </p>
          <CovenantBreachForecast
            onSubmit={setConfigJson}
            breaches={configJson === null ? null : forecast.data}
            loading={forecast.isFetching}
            error={configJson === null ? undefined : forecast.error?.message}
            density={density}
          />
        </div>
      )}
    </div>
  );
}
