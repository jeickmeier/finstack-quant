"use client";
import { useMemo, useState } from "react";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { SchemaForm } from "@/components/finstack/core/components/schema-form/schema-form";
import { EnumField } from "@/components/finstack/shared/primitives/enum-field/enum-field";
import { FieldFrame } from "@/components/finstack/shared/primitives/field-frame/field-frame";
import { useCheckSuiteValidator } from "@/hooks/statements/use-check-suite-validator/use-check-suite-validator";
import {
  useRunStatementChecks,
  type StatementChecksRequest,
} from "@/hooks/statements/use-run-statement-checks/use-run-statement-checks";
import type { LinkedSelection } from "@/hooks/shared/use-linked-selection/use-linked-selection";
import { checkSuiteModule } from "./check-suite";
import { StatementCheckReport } from "./statement-check-report";

/**
 * Generated check-suite form or raw mapping JSON, executed against the matching native result.
 * Switching the runner keeps the suite draft and the last run's report: the suite form stays
 * mounted (hidden) while a mapping runner is selected.
 */
export function StatementChecks({
  modelJson,
  resultsJson,
  link,
  defaultConfigJson = "",
  density,
}: {
  modelJson: string;
  resultsJson: string;
  link?: LinkedSelection;
  /** Optional caller-supplied CheckSuiteSpec JSON that seeds the suite form; nothing is synthesized and mapping JSON starts empty. */
  defaultConfigJson?: string;
  /** Density of the check report; undefined inherits the surrounding surface. */
  density?: "compact" | "comfortable";
}) {
  const validateSuite = useCheckSuiteValidator();
  const [kind, setKind] = useState<StatementChecksRequest["kind"]>("suite");
  const [mappingJson, setMappingJson] = useState("");
  const [submitted, setSubmitted] = useState<StatementChecksRequest | null>(
    null,
  );
  const initial = useMemo(() => {
    if (!defaultConfigJson.trim())
      return { value: {} as Record<string, unknown> };
    try {
      const value = checkSuiteModule.codec.parse(defaultConfigJson);
      if (value === null || typeof value !== "object" || Array.isArray(value))
        throw new TypeError("Check suite must be a JSON object");
      return { value: value as Record<string, unknown> };
    } catch (error) {
      return { error: error instanceof Error ? error.message : String(error) };
    }
  }, [defaultConfigJson]);
  const request =
    submitted?.modelJson === modelJson &&
    submitted.resultsJson === resultsJson &&
    submitted.kind === kind &&
    (kind === "suite" || submitted.configJson === mappingJson)
      ? submitted
      : null;
  const query = useRunStatementChecks(request);
  return (
    <section
      aria-label="Run statement checks"
      className="space-y-3 font-sans text-sm text-foreground"
    >
      <h3 className="font-semibold">Checks</h3>
      <EnumField
        label="Check runner"
        value={kind}
        onValueChange={(value) =>
          setKind(value as StatementChecksRequest["kind"])
        }
        options={[
          { value: "suite", label: "Check suite" },
          { value: "three-statement", label: "Three statement mapping" },
          {
            value: "credit-underwriting",
            label: "Credit underwriting mapping",
          },
        ]}
      />
      {/* Hidden, not unmounted: a remount would reset the suite draft to defaultConfigJson. */}
      <div hidden={kind !== "suite"}>
        {"error" in initial ? (
          <p role="alert">{initial.error}</p>
        ) : (
          <SchemaForm
            key={defaultConfigJson}
            module={checkSuiteModule}
            defaultValues={initial.value}
            validate={validateSuite}
            onSubmit={(canonical) =>
              setSubmitted({
                modelJson,
                resultsJson,
                kind: "suite",
                configJson: canonical,
              })
            }
            onValidated={(json) => {
              // A suite edit invalidates only a suite run, never a mapping run.
              if (json === null)
                setSubmitted((current) =>
                  current?.kind === "suite" ? null : current,
                );
            }}
            submitLabel="Run native checks"
            layout="full"
          />
        )}
      </div>
      {kind !== "suite" && (
        <>
          <FieldFrame label="Canonical node mapping JSON">
            {(control) => (
              <Textarea
                {...control}
                className="min-h-32 font-mono text-xs"
                spellCheck={false}
                value={mappingJson}
                onChange={(event) => setMappingJson(event.target.value)}
              />
            )}
          </FieldFrame>
          <Button
            type="button"
            disabled={!mappingJson.trim() || query.workerStatus !== "ready"}
            onClick={() =>
              setSubmitted({
                modelJson,
                resultsJson,
                configJson: mappingJson,
                kind,
              })
            }
          >
            Run native checks
          </Button>
        </>
      )}
      {query.isPending && request && <p role="status">Running checks…</p>}
      {query.error && <p role="alert">{query.error.message}</p>}
      {request && query.data && (
        <StatementCheckReport
          report={query.data}
          link={link}
          density={density}
        />
      )}
    </section>
  );
}
