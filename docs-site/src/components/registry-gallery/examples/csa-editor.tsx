"use client";
import { useState } from "react";
import type { ExampleProps } from "./props";
import { CsaEditor } from "@/components/finstack/margin/components/csa-editor/csa-editor";
import { useCsaValidator } from "@/hooks/margin/use-csa-validator/use-csa-validator";
import { useCsaPreset } from "@/hooks/margin/use-csa-preset/use-csa-preset";
import { WorkerPreview } from "../worker-preview";
// The hooks sit outside the item's install closure, so the example mounts its
// own provider; the install matrix runs the item with caller stubs.
export function Example({ density = "compact" }: ExampleProps) {
  return (
    <WorkerPreview>
      <Editor density={density} />
    </WorkerPreview>
  );
}
function Editor({ density }: ExampleProps) {
  const validate = useCsaValidator();
  const loadPreset = useCsaPreset();
  // Starts from the csa_spec schema example; Use CSA records the canonical text.
  const [, setCsaJson] = useState<string | null>(null);
  return (
    <CsaEditor
      validate={validate}
      loadPreset={loadPreset}
      onSubmit={setCsaJson}
      density={density}
    />
  );
}
