"use client";
import { useState } from "react";
import type { ExampleProps } from "./props";
import { CovenantEngineEditor } from "@/components/finstack/covenants/components/covenant-engine-editor/covenant-engine-editor";
import { useCovenantEngineValidator } from "@/hooks/covenants/use-covenant-engine-validator/use-covenant-engine-validator";
import { useCovenantTemplate } from "@/hooks/covenants/use-covenant-template/use-covenant-template";
import { WorkerPreview } from "../worker-preview";
import data from "../data.json";

// The analyst covenant engine (interest coverage throughout, leverage in a
// window) over the analyst statement model's node ids.
const engineJson = JSON.stringify(data.covenantEngine);
const metricIds = Object.keys(data.statementModel.nodes);
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
  const validate = useCovenantEngineValidator();
  const loadTemplate = useCovenantTemplate();
  // Use covenants records the canonical engine.
  const [, setEngineJson] = useState<string | null>(null);
  return (
    <CovenantEngineEditor
      defaultJson={engineJson}
      validate={validate}
      loadTemplate={loadTemplate}
      onSubmit={setEngineJson}
      metricIds={metricIds}
      density={density}
    />
  );
}
