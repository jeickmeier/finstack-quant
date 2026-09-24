"use client";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { StatementChecksRequest } from "@/workers/finstack-contract";
import type { LinkedSelection } from "@/hooks/shared/use-linked-selection/use-linked-selection";
import { StatementCheckReport } from "./statement-check-report";

/** Raw check-suite or mapping input, executed against the matching native result. */
export function StatementChecks({
  modelJson,
  resultsJson,
  link,
  defaultConfigJson = "",
}: {
  modelJson: string;
  resultsJson: string;
  link?: LinkedSelection;
  /** Optional caller-supplied initial JSON. No suite or mapping is synthesized. */
  defaultConfigJson?: string;
}) {
  const [kind, setKind] = useState<StatementChecksRequest["kind"]>("suite");
  const [configJson, setConfigJson] = useState(defaultConfigJson);
  const [submitted, setSubmitted] = useState<StatementChecksRequest | null>(
    null,
  );
  const request =
    submitted?.modelJson === modelJson &&
    submitted.resultsJson === resultsJson &&
    submitted.configJson === configJson &&
    submitted.kind === kind
      ? submitted
      : null;
  const query = useWorkerQuery((worker) => ({
    ...workerQueryOptions({
      client: worker.client,
      queryKey: [
        "finstack",
        worker.session,
        "statement-checks",
        request,
      ] as const,
      ready: request !== null,
      missing: "Statement worker is not ready",
      queryFn: (client) => client.call("runStatementChecks", request!),
    }),
    enabled: worker.status === "ready" && request !== null,
  }));
  return (
    <section
      aria-label="Run statement checks"
      className="space-y-3 font-sans text-sm text-foreground"
    >
      <h3 className="font-semibold">Checks</h3>
      <label className="block space-y-1">
        Check runner
        <select
          className="block w-full rounded-md border border-border bg-background p-2"
          value={kind}
          onChange={(event) =>
            setKind(event.target.value as StatementChecksRequest["kind"])
          }
        >
          <option value="suite">Check suite</option>
          <option value="three-statement">Three statement mapping</option>
          <option value="credit-underwriting">
            Credit underwriting mapping
          </option>
        </select>
      </label>
      <label className="block space-y-1">
        Canonical {kind === "suite" ? "check suite" : "node mapping"} JSON
        <textarea
          className="block min-h-32 w-full rounded-md border border-border bg-background p-2 font-mono text-xs"
          value={configJson}
          onChange={(event) => setConfigJson(event.target.value)}
          spellCheck={false}
        />
      </label>
      <Button
        type="button"
        disabled={!configJson.trim() || query.workerStatus !== "ready"}
        onClick={() =>
          setSubmitted({ modelJson, resultsJson, configJson, kind })
        }
      >
        Run native checks
      </Button>
      {query.isPending && request && <p role="status">Running checks…</p>}
      {query.error && <p role="alert">{query.error.message}</p>}
      {request && query.data && (
        <StatementCheckReport report={query.data} link={link} />
      )}
    </section>
  );
}
