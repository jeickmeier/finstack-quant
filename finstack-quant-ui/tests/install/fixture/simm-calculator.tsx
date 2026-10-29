"use client";
import { useState } from "react";
import type { ImResult } from "finstack-quant-wasm";
import { SimmCalculator } from "@/components/finstack/margin/components/simm-calculator/simm-calculator";

import data from "./data.json";
// The caller's validation returns the text unchanged and the result is the
// supplied native fixture: the item installs without a worker.
const validate = async (json: string) => json;
const result = data.simm.example.result as unknown as ImResult;
export function InstalledItem() {
  const [asOf, setAsOf] = useState(data.simm.example.request.asOf);
  const [count, setCount] = useState(0);
  return (
    <>
      <p>
        Supplied valid fixture; standalone SIMM calculator with a caller
        validation callback and a supplied result.
      </p>
      <SimmCalculator
        defaultJson={data.simm.example.request.sensitivitiesJson}
        asOf={asOf}
        onAsOfChange={setAsOf}
        validate={validate}
        onSubmit={() => setCount((n) => n + 1)}
        result={result}
      />
      <output aria-label="Submission count">{count}</output>
    </>
  );
}
