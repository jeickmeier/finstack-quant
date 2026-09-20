"use client";
import type { ExampleProps } from "./props";
import { useState } from "react";
import { FinstackTable } from "@/components/finstack/primitives/finstack-table/finstack-table";

export function Example({ density = "compact" }: ExampleProps) {
  const [activations, setActivations] = useState(0);
  return (
    <>
      <FinstackTable
        caption="Supplied exact values"
        density={density}
        data={[
          { id: "USD", value: "1000000.123456789" },
          { id: "EUR", value: "999.99" },
        ]}
        columns={[
          { id: "value", header: "Value", accessorKey: "value" },
          {
            id: "action",
            header: "Details",
            cell: () => (
              <button onClick={() => setActivations((n) => n + 1)}>
                Open detail
              </button>
            ),
          },
        ]}
        getRowId={(row) => row.id}
        onRowActivate={() => setActivations((n) => n + 1)}
      />
      <output aria-label="Activation count" className="sr-only">
        {activations}
      </output>
    </>
  );
}
