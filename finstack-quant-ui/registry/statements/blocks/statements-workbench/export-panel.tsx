"use client";
import { useMemo, useState } from "react";
import type { StatementResult } from "finstack-quant-wasm";
import { serializeHost } from "@/lib/finstack/codec.mjs";
import {
  StatementExport,
  type StatementTableLayout,
} from "@/components/finstack/statements/components/statement-export/statement-export";
import { useStatementResultTable } from "@/hooks/statements/use-statement-result-table/use-statement-result-table";

function ExportBody({
  result,
  density,
}: {
  result: StatementResult | null;
  density: "compact" | "comfortable";
}) {
  const [layout, setLayout] = useState<StatementTableLayout>("wide");
  const resultJson = useMemo(
    () => (result ? serializeHost(result) : null),
    [result],
  );
  const table = useStatementResultTable(
    resultJson ? { resultJson, layout } : null,
  );
  return (
    <StatementExport
      layout={layout}
      onLayoutChange={setLayout}
      table={table.data ?? null}
      loading={table.isFetching}
      error={table.error?.message}
      density={density}
    />
  );
}
/**
 * The result the grids show (reported or the native annual rollup) as a table
 * and CSV, in a disclosure whose native call runs when it is first opened. It
 * stays mounted across a rollup, when `result` is null, so it stays open.
 */
export function ExportPanel({
  result,
  density,
}: {
  result: StatementResult | null;
  density: "compact" | "comfortable";
}) {
  const [open, setOpen] = useState(false);
  return (
    <details onToggle={(event) => setOpen(event.currentTarget.open)}>
      <summary>Export table and CSV</summary>
      {open && (
        <div className="mt-3">
          <ExportBody result={result} density={density} />
        </div>
      )}
    </details>
  );
}
