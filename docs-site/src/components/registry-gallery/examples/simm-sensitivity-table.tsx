"use client";
import type { SimmSensitivitiesJson } from "finstack-quant-wasm";
import type { ExampleProps } from "./props";
import { SimmSensitivityTable } from "@/components/finstack/margin/components/simm-sensitivity-table/simm-sensitivity-table";
import data from "../data.json";
// The canonical text Rust returns for the simm_sensitivities_json schema
// example (IR, equity and FX deltas in USD); this item needs no worker.
const sensitivities = JSON.parse(
  data.simm.example.canonical,
) as SimmSensitivitiesJson;
export function Example({ density = "compact" }: ExampleProps) {
  return (
    <SimmSensitivityTable sensitivities={sensitivities} density={density} />
  );
}
