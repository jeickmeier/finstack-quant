"use client";
import type { StatementExplanationRequest } from "@/workers/finstack-contract";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import { JsonViewer } from "@/components/finstack/shared/primitives/json-viewer/json-viewer";
import { serializeHost } from "@/lib/finstack/codec.mjs";

/** Native structured formula breakdown plus opaque text and dependency trace. */
export function StatementExplanation({
  request,
}: {
  request: StatementExplanationRequest | null;
}) {
  const snapshot = request === null ? null : Object.freeze({ ...request });
  const explanation = useWorkerQuery((worker) => ({
    ...workerQueryOptions({
      client: worker.client,
      queryKey: [
        "finstack",
        worker.session,
        "statement-explanation",
        snapshot,
      ] as const,
      ready: snapshot !== null,
      missing: "Statement worker is not ready",
      queryFn: (client) => client.call("explainStatement", snapshot!),
    }),
    enabled: worker.status === "ready" && snapshot !== null,
  }));
  const text = useWorkerQuery((worker) => ({
    ...workerQueryOptions({
      client: worker.client,
      queryKey: [
        "finstack",
        worker.session,
        "statement-explanation-text",
        snapshot,
      ] as const,
      ready: snapshot !== null,
      missing: "Statement worker is not ready",
      queryFn: (client) => client.call("explainStatementText", snapshot!),
    }),
    enabled: worker.status === "ready" && snapshot !== null,
  }));
  const trace = useWorkerQuery((worker) => ({
    ...workerQueryOptions({
      client: worker.client,
      queryKey: [
        "finstack",
        worker.session,
        "statement-trace",
        snapshot?.modelJson,
        snapshot?.nodeId,
      ] as const,
      ready: snapshot !== null,
      missing: "Statement worker is not ready",
      queryFn: (client) =>
        client.call("traceStatement", snapshot!.modelJson, snapshot!.nodeId),
    }),
    enabled: worker.status === "ready" && snapshot !== null,
  }));
  if (!snapshot)
    return <p role="status">Select a statement cell to inspect its formula.</p>;
  const data = explanation.data;
  return (
    <section
      aria-label="Statement explanation"
      className="space-y-3 font-sans text-sm text-foreground"
    >
      <h3 className="font-semibold">
        {snapshot.nodeId} · {snapshot.period}
      </h3>
      {explanation.isPending && <p role="status">Explaining selected value…</p>}
      {explanation.error && <p role="alert">{explanation.error.message}</p>}
      {data && (
        <>
          <p>
            Returned value:{" "}
            <span className="font-mono">{String(data.final_value)}</span> ·{" "}
            {data.node_type}
          </p>
          {data.formula_text && (
            <p>
              Formula: <code>{data.formula_text}</code>
            </p>
          )}
          <FinstackTable
            caption="Native formula breakdown"
            data={data.breakdown.map((step, index) => ({ step, index }))}
            getRowId={(row) => String(row.index)}
            columns={[
              {
                id: "component",
                header: "Component",
                accessorFn: (row) => row.step.component,
              },
              {
                id: "operation",
                header: "Operation",
                accessorFn: (row) => row.step.operation ?? "—",
              },
              {
                id: "value",
                header: "Returned contribution",
                accessorFn: (row) => String(row.step.value),
              },
            ]}
            emptyState="No formula components returned"
          />
          <details>
            <summary>Complete explanation</summary>
            <JsonViewer
              label="Formula explanation JSON"
              text={serializeHost(data)}
            />
          </details>
        </>
      )}
      {trace.error && (
        <p role="alert">Dependency trace: {trace.error.message}</p>
      )}
      {trace.data && (
        <details>
          <summary>Dependency trace</summary>
          <pre className="overflow-auto whitespace-pre-wrap font-mono text-xs">
            {trace.data}
          </pre>
        </details>
      )}
      {text.error && <p role="alert">Text explanation: {text.error.message}</p>}
      {text.data && (
        <details>
          <summary>Native text explanation</summary>
          <pre className="overflow-auto whitespace-pre-wrap font-mono text-xs">
            {text.data}
          </pre>
        </details>
      )}
    </section>
  );
}
