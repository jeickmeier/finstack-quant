"use client";
import type { ComponentProps } from "react";
import { RelativeValueTable } from "@/components/finstack/statements/components/relative-value-table/relative-value-table";
import type {
  CompanyMetrics,
  RelativeValueReport,
} from "@/workers/finstack-contract";

import data from "./data.json";
// The report is the supplied native fixture: the item installs without a
// worker, so nothing is compared here.
const { subject, universe, cases } = data.relativeValueTable;
export function InstalledItem() {
  return (
    <>
      <p>Supplied valid fixture; standalone relative value table.</p>
      <RelativeValueTable
        subject={subject as CompanyMetrics}
        universe={universe as CompanyMetrics[]}
        defaultConfig={
          cases.full.config as unknown as ComponentProps<
            typeof RelativeValueTable
          >["defaultConfig"]
        }
        onSubmit={() => {}}
        report={cases.full.report as unknown as RelativeValueReport}
      />
    </>
  );
}
