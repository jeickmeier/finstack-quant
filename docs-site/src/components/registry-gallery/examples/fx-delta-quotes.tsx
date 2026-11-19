"use client";
import type { ExampleProps } from "./props";
import { FxDeltaQuotes } from "@/components/finstack/core/components/fx-delta-quotes/fx-delta-quotes";
const fx: import("react").ComponentProps<typeof FxDeltaQuotes>["surface"] = {
  id: "EURUSD",
  pair: { base: "EUR", quote: "USD" },
  delta_convention: "forward_unadjusted",
  atm_convention: "delta_neutral_straddle",
  butterfly_convention: "smile",
  expiries: [0.5, 1],
  atm_vols: [0.08, 0.09],
  rr_25d: [0.01, 0.012],
  bf_25d: [0.005, 0.006],
  rr_10d: null,
  bf_10d: null,
  expiry_clock: null,
};
export function Example(_props: ExampleProps) {
  return <FxDeltaQuotes surface={fx} />;
}
