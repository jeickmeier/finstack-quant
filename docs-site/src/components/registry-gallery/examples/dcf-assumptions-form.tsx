"use client";
import { useState } from "react";
import type { ExampleProps } from "./props";
import {
  DcfAssumptionsForm,
  type DcfAssumptions,
  type DcfAssumptionsSubmission,
} from "@/components/finstack/statements/components/dcf-assumptions-form/dcf-assumptions-form";
import data from "../data.json";

// Illustrative analyst assumptions; the form passes its submission to the
// caller, shown here instead of being valued.
const assumptions = data.statementDcfAssumptions as DcfAssumptions;
const nodeIds = Object.keys(data.statementModel.nodes);

export function Example({ density = "compact" }: ExampleProps) {
  const [submitted, setSubmitted] = useState<DcfAssumptionsSubmission | null>(
    null,
  );
  return (
    <div className="space-y-3">
      <DcfAssumptionsForm
        defaultAssumptions={assumptions}
        nodeIds={nodeIds}
        onSubmit={setSubmitted}
        density={density}
      />
      {submitted && (
        <pre
          aria-label="Submitted assumptions"
          className="overflow-x-auto rounded-md border border-border p-2 text-xs"
        >
          {JSON.stringify(submitted, null, 2)}
        </pre>
      )}
    </div>
  );
}
