"use client";
import { useState } from "react";
import type { TableEnvelopeWire } from "@/lib/finstack/generated/types/table_envelope";
import type { ExampleProps } from "./props";
import {
  StatementExport,
  type StatementTableLayout,
} from "@/components/finstack/statements/components/statement-export/statement-export";
import { useStatementResultTable } from "@/hooks/statements/use-statement-result-table/use-statement-result-table";
import data from "../data.json";
import { WorkerPreview } from "../worker-preview";
const resultJson = JSON.stringify(data.statementResult);
function Live({ density }: ExampleProps) {
  const [layout, setLayout] = useState<StatementTableLayout>("wide");
  const table = useStatementResultTable({ resultJson, layout });
  return (
    <StatementExport
      layout={layout}
      onLayoutChange={setLayout}
      table={(table.data as TableEnvelopeWire | undefined) ?? null}
      loading={table.isFetching}
      error={table.error?.message}
      density={density}
    />
  );
}
// The hook sits outside the item's install closure, so the example mounts its
// own provider; the install matrix runs the item with a supplied table.
export function Example({ density = "compact" }: ExampleProps) {
  return (
    <WorkerPreview>
      <Live density={density} />
    </WorkerPreview>
  );
}
