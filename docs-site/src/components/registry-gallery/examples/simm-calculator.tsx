"use client";
import { useState } from "react";
import type { ExampleProps } from "./props";
import { SimmCalculator } from "@/components/finstack/margin/components/simm-calculator/simm-calculator";
import { useSimmIm } from "@/hooks/margin/use-simm-im/use-simm-im";
import { useSimmSensitivitiesValidator } from "@/hooks/margin/use-simm-sensitivities-validator/use-simm-sensitivities-validator";
import data from "../data.json";
import { WorkerPreview } from "../worker-preview";
// The hooks sit outside the item's install closure, so the example mounts its
// own provider; the install matrix runs the item with a caller validate stub.
export function Example({ density = "compact" }: ExampleProps) {
  return (
    <WorkerPreview>
      <Calculator density={density} />
    </WorkerPreview>
  );
}
function Calculator({ density }: ExampleProps) {
  const validate = useSimmSensitivitiesValidator();
  const [asOf, setAsOf] = useState(data.simm.example.request.asOf);
  // Starts from the schema example; Calculate replaces the sensitivities.
  const [sensitivitiesJson, setSensitivitiesJson] = useState(
    data.simm.example.request.sensitivitiesJson,
  );
  const im = useSimmIm({ sensitivitiesJson, asOf });
  return (
    <SimmCalculator
      defaultJson={sensitivitiesJson}
      asOf={asOf}
      onAsOfChange={setAsOf}
      validate={validate}
      onSubmit={setSensitivitiesJson}
      result={im.data ?? null}
      loading={im.isFetching}
      error={im.error?.message}
      density={density}
    />
  );
}
