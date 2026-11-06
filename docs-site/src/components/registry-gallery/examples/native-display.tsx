"use client";
import type { ExampleProps } from "./props";
import { useState } from "react";
import {
  Outcome,
  Pairs,
  ReportSection,
  TextField,
  UndefinedValue,
} from "@/components/finstack/shared/primitives/native-display/native-display";

export function Example(_props: ExampleProps) {
  const [id, setId] = useState("ACME-SR-2031");
  return (
    <div className="space-y-6">
      <TextField
        label="Claim id"
        help="Caller-defined claim identifier."
        value={id}
        onValueChange={setId}
      />
      <ReportSection title="Returned values">
        <Pairs
          label="Returned values"
          rows={[
            ["Recovery rate", "0.4235"],
            ["Mode", <UndefinedValue key="mode" reason="No interior mode" />],
          ]}
        />
      </ReportSection>
      <Outcome
        state={{ result: null, error: "estate value must be positive" }}
        empty="No result"
        working="Calculating…"
      >
        {() => null}
      </Outcome>
    </div>
  );
}
