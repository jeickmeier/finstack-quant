"use client";
import type { ExampleProps } from "./props";
import { MeasuresGrid } from "@/components/finstack/components/measures-grid/measures-grid";
import data from "../data.json";

export function Example({ density = "compact" }: ExampleProps) {
  return (
    <MeasuresGrid
      result={data.bond.result}
      groups={data.bond.groups}
      density={density}
    />
  );
}
