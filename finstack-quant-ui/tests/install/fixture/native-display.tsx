"use client";
import { useState } from "react";
import {
  Outcome,
  Pairs,
  ReportSection,
  TextField,
  UndefinedValue,
} from "@/components/finstack/shared/primitives/native-display/native-display";

// The gallery shows Outcome's error state; an installed page must render without one.
export function InstalledItem() {
  const [id, setId] = useState("ACME-SR-2031");
  return (
    <div className="space-y-6">
      <TextField
        label="Claim id"
        help="Caller-defined claim identifier."
        value={id}
        onValueChange={setId}
      />
      <Outcome
        state={{ result: 0.4235 }}
        empty="No result"
        working="Calculating…"
      >
        {(rate) => (
          <ReportSection title="Returned values">
            <Pairs
              label="Returned values"
              rows={[
                ["Recovery rate", rate.toFixed(4)],
                [
                  "Mode",
                  <UndefinedValue key="mode" reason="No interior mode" />,
                ],
              ]}
            />
          </ReportSection>
        )}
      </Outcome>
    </div>
  );
}
