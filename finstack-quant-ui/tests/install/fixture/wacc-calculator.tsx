"use client";
import {
  WaccCalculator,
  type WaccInputs,
} from "@/components/finstack/statements/components/wacc-calculator/wacc-calculator";

import data from "./data.json";
// The rate is the supplied native fixture: the item installs without a worker.
const { blended } = data.waccCalculator.cases;
export function InstalledItem() {
  return (
    <>
      <p>Supplied valid fixture; standalone WACC calculator.</p>
      <WaccCalculator
        defaultInputs={blended.inputs as WaccInputs}
        onSubmit={() => {}}
        rate={blended.rate}
        onApply={() => {}}
      />
    </>
  );
}
