"use client";
import type { ExampleProps } from "./props";
import { TransitionMatrixHeatmap } from "@/components/finstack/models/components/transition-matrix-heatmap/transition-matrix-heatmap";
import { migration } from "../fixtures";
// The native results for an illustrative eight-state one-year matrix: the
// matrix, its generator (Rust's default round-trip tolerance) and cumulative
// default probabilities at 1 to 10 years; this item needs no worker.
const matrix = migration.matrix;
const projection = migration.projection;
// Host display choice: four significant digits of the returned decimals.
const fourDigits = (value: number) => String(Number(value.toPrecision(4)));
export function Example({ density = "compact" }: ExampleProps) {
  return (
    <TransitionMatrixHeatmap
      matrix={matrix}
      projection={projection}
      formatValue={fourDigits}
      density={density}
    />
  );
}
