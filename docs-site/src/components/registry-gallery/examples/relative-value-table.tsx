"use client";
import { useState, type ComponentProps } from "react";
import type { ExampleProps } from "./props";
import { RelativeValueTable } from "@/components/finstack/statements/components/relative-value-table/relative-value-table";
import { useRelativeValueReport } from "@/hooks/statements/use-relative-value-report/use-relative-value-report";
import type {
  CompanyMetrics,
  RelativeValueReportRequest,
} from "@/workers/finstack-contract";
import data from "../data.json";
import { WorkerPreview } from "../worker-preview";
// A host-supplied universe of ten issuers in two sectors, compared natively.
const fixture = data.relativeValueTable;
const subject = fixture.subject as CompanyMetrics;
const universe = fixture.universe as CompanyMetrics[];
const start = fixture.cases.full.config as unknown as ComponentProps<
  typeof RelativeValueTable
>["defaultConfig"];
const initial: RelativeValueReportRequest = {
  subjectJson: JSON.stringify(subject),
  universeJson: JSON.stringify(universe),
  configJson: JSON.stringify(start),
};
function Live({ density }: ExampleProps) {
  const [request, setRequest] = useState(initial);
  const report = useRelativeValueReport(request);
  return (
    <RelativeValueTable
      subject={subject}
      universe={universe}
      defaultConfig={start}
      onSubmit={setRequest}
      report={report.data}
      loading={report.isFetching}
      error={report.error?.message}
      density={density}
    />
  );
}
// The hook sits outside the item's install closure, so the example mounts its
// own provider; the install matrix runs the item with a supplied report.
export function Example({ density = "compact" }: ExampleProps) {
  return (
    <WorkerPreview>
      <Live density={density} />
    </WorkerPreview>
  );
}
