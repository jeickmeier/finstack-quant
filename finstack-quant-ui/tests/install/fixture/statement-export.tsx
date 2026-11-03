"use client";
import { useState } from "react";
import type { TableEnvelopeWire } from "@/lib/finstack/generated/types/table_envelope";
import {
  StatementExport,
  type StatementTableLayout,
} from "@/components/finstack/statements/components/statement-export/statement-export";

import data from "./data.json";
// The tables are the supplied native fixture: the item installs without a worker.
const { analyst } = data.statementExport;
export function InstalledItem() {
  const [layout, setLayout] = useState<StatementTableLayout>("wide");
  return (
    <>
      <p>Supplied valid fixture; standalone statement export.</p>
      <StatementExport
        layout={layout}
        onLayoutChange={setLayout}
        table={analyst[layout] as unknown as TableEnvelopeWire}
      />
    </>
  );
}
