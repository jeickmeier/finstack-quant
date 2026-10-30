"use client";
import { useState } from "react";
import {
  MertonCalculator,
  type MertonCalculatorRequest,
  type MertonReportData,
} from "@/components/finstack/models/components/merton-calculator/merton-calculator";

import data from "./data.json";
// The caller's validation returns the text unchanged and the report is the
// supplied native fixture: the item installs without a worker.
const validate = async (json: string) => json;
const report = data.merton.model.report as unknown as MertonReportData;
export function InstalledItem() {
  const [count, setCount] = useState(0);
  return (
    <>
      <p>
        Supplied valid fixture; standalone Merton calculator with a caller
        validation callback and a supplied report.
      </p>
      <MertonCalculator
        defaultRequest={data.merton.model.request as MertonCalculatorRequest}
        validate={validate}
        onSubmit={() => setCount((n) => n + 1)}
        report={report}
      />
      <output aria-label="Submission count">{count}</output>
    </>
  );
}
