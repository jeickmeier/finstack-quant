"use client";
import type { ExampleProps } from "./props";
import { FxMatrixGrid } from "@/components/finstack/core/components/fx-matrix-grid/fx-matrix-grid";
import type { MarketContextStateWire } from "@/lib/finstack/generated/types/market_context_state";
import data from "../data.json";
const market = data.market.supplemental as unknown as MarketContextStateWire;
export function Example(_props: ExampleProps) {
  return <FxMatrixGrid state={market.fx} />;
}
