"use client";
import { useState } from "react";
import type { covenants } from "finstack-quant-wasm/types";
import { CovenantEngineEditor } from "@/components/finstack/covenants/components/covenant-engine-editor/covenant-engine-editor";

import data from "./data.json";
// The caller's validation returns the text unchanged and the template loader
// returns the supplied analyst engine's specs: the item installs without a
// worker.
const validate = async (json: string) => json;
const loadTemplate = async () =>
  data.covenantEngine.specs as unknown as covenants.CovenantSpec[];
const metricIds = Object.keys(data.statementModel.nodes);
export function InstalledItem() {
  const [count, setCount] = useState(0);
  return (
    <>
      <p>
        Supplied valid fixture; standalone covenant engine editor with caller
        validation and template callbacks.
      </p>
      <CovenantEngineEditor
        defaultJson={JSON.stringify(data.covenantEngine)}
        validate={validate}
        loadTemplate={loadTemplate}
        onSubmit={() => setCount((n) => n + 1)}
        metricIds={metricIds}
      />
      <output aria-label="Submission count">{count}</output>
    </>
  );
}
