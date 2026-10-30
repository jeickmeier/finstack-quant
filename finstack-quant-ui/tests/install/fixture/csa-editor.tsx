"use client";
import { useState } from "react";
import type { CsaSpec } from "finstack-quant-wasm";
import { CsaEditor } from "@/components/finstack/margin/components/csa-editor/csa-editor";

import data from "./data.json";
// The caller's validation returns the text unchanged and the preset loader
// returns the supplied native USD regulatory CSA: the item installs without a
// worker.
const validate = async (json: string) => json;
const loadPreset = async () => data.vm.presets.usd as unknown as CsaSpec;
export function InstalledItem() {
  const [count, setCount] = useState(0);
  return (
    <>
      <p>
        Supplied valid fixture; standalone CSA editor with caller validation and
        preset callbacks.
      </p>
      <CsaEditor
        defaultJson={data.vm.csaJson}
        validate={validate}
        loadPreset={loadPreset}
        onSubmit={() => setCount((n) => n + 1)}
      />
      <output aria-label="Submission count">{count}</output>
    </>
  );
}
