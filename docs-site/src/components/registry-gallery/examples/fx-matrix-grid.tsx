"use client";
import type { ExampleProps } from "./props";
import { FxMatrixGrid } from "@/components/finstack/core/components/fx-matrix-grid/fx-matrix-grid";
import { supplementalMarket } from "../fixtures";
const market = supplementalMarket;
export function Example(_props: ExampleProps) {
  return <FxMatrixGrid state={market.fx} />;
}
