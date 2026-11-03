"use client";
import { NormalizationEditor } from "@/components/finstack/statements/components/normalization-editor/normalization-editor";
import type { NormalizationResult } from "@/workers/finstack-contract";

import data from "./data.json";
// The results are the supplied native fixture: the item installs without a
// worker, so the config passes through unvalidated here.
const { config, results } = data.normalizationEditor;
const passThrough = async (json: string) => json;
export function InstalledItem() {
  return (
    <>
      <p>Supplied valid fixture; standalone normalization editor.</p>
      <NormalizationEditor
        defaultConfig={config}
        nodeIds={["ebitda", "revenue"]}
        validate={passThrough}
        onSubmit={() => {}}
        results={results as unknown as NormalizationResult[]}
      />
    </>
  );
}
