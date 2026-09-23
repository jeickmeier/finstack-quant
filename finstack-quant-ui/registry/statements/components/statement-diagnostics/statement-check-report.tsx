"use client";
import type { CheckReport } from "finstack-quant-wasm";
import { Button } from "@/components/ui/button";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import { JsonViewer } from "@/components/finstack/shared/primitives/json-viewer/json-viewer";
import { serializeHost } from "@/lib/finstack/codec.mjs";
import { statementCellKey } from "../statement-grid/projection";
import type { LinkedSelection } from "@/hooks/shared/use-linked-selection/use-linked-selection";

/** Display native check outcomes, counts and findings without recomputing status. */
export function StatementCheckReport({
  report,
  link,
}: {
  report: CheckReport;
  link?: LinkedSelection;
}) {
  const { summary } = report;
  return (
    <section
      aria-label="Statement check report"
      className="space-y-3 font-sans text-sm text-foreground"
    >
      <p className="font-medium">
        {summary.passed} passed · {summary.failed} failed · {summary.errors}{" "}
        errors · {summary.warnings} warnings · {summary.infos} info ·{" "}
        {summary.total_checks} checks
      </p>
      <FinstackTable
        caption="Native statement checks"
        data={report.results}
        getRowId={(row) => row.check_id}
        columns={[
          { id: "check", header: "Check", accessorFn: (row) => row.check_name },
          {
            id: "category",
            header: "Category",
            accessorFn: (row) => row.category,
          },
          {
            id: "status",
            header: "Native status",
            accessorFn: (row) => (row.passed ? "Passed" : "Failed"),
          },
          {
            id: "findings",
            header: "Findings",
            accessorFn: (row) => row.findings.length,
          },
        ]}
        emptyState="No checks returned"
      />
      {report.results.flatMap((check) =>
        check.findings.map((finding, index) => (
          <article
            key={`${check.check_id}-${index}`}
            className="rounded-md border border-border bg-card p-3"
          >
            <p className="font-medium">
              {finding.severity} · {finding.message}
            </p>
            <p className="text-xs text-muted-foreground">
              {check.check_name} · {finding.period ?? "No period"}
            </p>
            {finding.materiality && (
              <dl className="mt-2 grid grid-cols-2 gap-1 text-xs">
                {Object.entries(finding.materiality).map(([key, value]) => (
                  <div key={key}>
                    <dt className="text-muted-foreground">{key}</dt>
                    <dd className="font-mono">{String(value)}</dd>
                  </div>
                ))}
              </dl>
            )}
            {finding.period && finding.nodes?.length ? (
              <div className="mt-2 flex flex-wrap gap-1">
                {finding.nodes.map((nodeId) => (
                  <Button
                    key={nodeId}
                    type="button"
                    variant="outline"
                    size="sm"
                    onClick={() =>
                      link?.select(statementCellKey(nodeId, finding.period!))
                    }
                  >
                    {nodeId} · {finding.period}
                  </Button>
                ))}
              </div>
            ) : null}
          </article>
        )),
      )}
      <details>
        <summary>Complete check report</summary>
        <JsonViewer
          label="Check report JSON"
          text={serializeHost(report)}
          downloadName="statement-checks.json"
        />
      </details>
    </section>
  );
}
