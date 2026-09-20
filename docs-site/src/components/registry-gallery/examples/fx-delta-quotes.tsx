"use client";
import type { ExampleProps } from "./props";
import { FxDeltaQuotes } from "@/components/finstack/components/fx-delta-quotes/fx-delta-quotes";
const fx = {
  id: "EURUSD",
  expiries: [0.5, 1],
  atm_vols: [0.08, 0.09],
  rr_25d: [0.01, 0.012],
  bf_25d: [0.005, 0.006],
  rr_10d: null,
  bf_10d: null,
};
export function Example(_props: ExampleProps) {
  return <FxDeltaQuotes surface={fx} />;
}
