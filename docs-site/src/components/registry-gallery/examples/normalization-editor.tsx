"use client";
import { useState, type ComponentProps } from "react";
import type { ExampleProps } from "./props";
import { NormalizationEditor } from "@/components/finstack/statements/components/normalization-editor/normalization-editor";
import { useNormalizationConfigValidator } from "@/hooks/statements/use-normalization-config-validator/use-normalization-config-validator";
import { useNormalize } from "@/hooks/statements/use-normalize/use-normalize";
import data from "../data.json";
import { WorkerPreview } from "../worker-preview";
const resultJson = JSON.stringify(data.statementResult);
const nodeIds = Object.keys(data.statementResult.nodes);
const start = data.normalizationEditor.config as unknown as ComponentProps<
  typeof NormalizationEditor
>["defaultConfig"];
function Live({ density }: ExampleProps) {
  const validate = useNormalizationConfigValidator();
  const [configJson, setConfigJson] = useState(() => JSON.stringify(start));
  const normalized = useNormalize({ resultJson, configJson });
  return (
    <NormalizationEditor
      defaultConfig={start}
      nodeIds={nodeIds}
      validate={validate}
      onSubmit={setConfigJson}
      results={normalized.data}
      loading={normalized.isFetching}
      error={normalized.error?.message}
      density={density}
    />
  );
}
// The hooks sit outside the item's install closure, so the example mounts its
// own provider; the install matrix runs the item with supplied results.
export function Example({ density = "compact" }: ExampleProps) {
  return (
    <WorkerPreview>
      <Live density={density} />
    </WorkerPreview>
  );
}
