"use client";
import type { DependencyTree } from "finstack-quant-wasm";
import {
  useExplainStatement,
  type StatementExplanationRequest,
} from "@/hooks/statements/use-explain-statement/use-explain-statement";
import { useTraceStatement } from "@/hooks/statements/use-trace-statement/use-trace-statement";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import { JsonViewer } from "@/components/finstack/shared/primitives/json-viewer/json-viewer";
import { serializeHost } from "@/lib/finstack/codec.mjs";

/** One dependency tree node at its pre-order position and nesting depth. */
interface TraceRow {
  index: number;
  depth: number;
  node: DependencyTree;
}
/** Flatten the native tree pre-order; a recurring dependency keeps one row per occurrence. */
function traceRows(root: DependencyTree): TraceRow[] {
  const rows: TraceRow[] = [];
  const visit = (node: DependencyTree, depth: number) => {
    rows.push({ index: rows.length, depth, node });
    for (const child of node.children) visit(child, depth + 1);
  };
  visit(root, 0);
  return rows;
}

/** Native structured formula breakdown plus dependency trace. */
export function StatementExplanation({
  request,
  density,
}: {
  request: StatementExplanationRequest | null;
  /** Density of the breakdown and trace tables and the JSON viewer; undefined inherits the surrounding surface. */
  density?: "compact" | "comfortable";
}) {
  const explanation = useExplainStatement(request);
  const trace = useTraceStatement(
    request && { modelJson: request.modelJson, nodeId: request.nodeId },
  );
  if (!request)
    return <p role="status">Select a statement cell to inspect its formula.</p>;
  const data = explanation.data;
  return (
    <section
      aria-label="Statement explanation"
      className="space-y-3 font-sans text-sm text-foreground"
    >
      <h3 className="font-semibold">
        {request.nodeId} · {request.period}
      </h3>
      {explanation.isPending && <p role="status">Explaining selected value…</p>}
      {explanation.error && <p role="alert">{explanation.error.message}</p>}
      {data && (
        <>
          <p>
            Returned value:{" "}
            <span className="font-mono">{String(data.final_value)}</span> ·{" "}
            {data.node_type}
            {data.source ? ` · ${data.source}` : ""}
          </p>
          {data.formula_text && (
            <p>
              Formula: <code>{data.formula_text}</code>
            </p>
          )}
          {data.forecast && (
            <p>
              Forecast method: <code>{data.forecast.method}</code>
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
            density={density}
          />
          <details>
            <summary>Complete explanation</summary>
            <JsonViewer
              label="Formula explanation JSON"
              text={serializeHost(data)}
              density={density}
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
          <FinstackTable
            caption="Native dependency tree"
            data={traceRows(trace.data)}
            getRowId={(row) => String(row.index)}
            columns={[
              {
                id: "node",
                header: "Node",
                accessorFn: (row) => row.node.node_id,
                cell: ({ row }: { row: { original: TraceRow } }) => (
                  <span
                    className="font-mono"
                    style={{ paddingInlineStart: `${row.original.depth}rem` }}
                  >
                    {row.original.node.node_id}
                  </span>
                ),
              },
              {
                id: "formula",
                header: "Formula",
                accessorFn: (row) => row.node.formula ?? "—",
              },
            ]}
            emptyState="No dependencies returned"
            density={density}
          />
        </details>
      )}
    </section>
  );
}
