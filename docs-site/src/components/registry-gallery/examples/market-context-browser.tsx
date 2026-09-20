"use client";
import type { ExampleProps } from "./props";
import { MarketContextBrowser } from "@/components/finstack/components/market-context-browser/market-context-browser";
import type { MarketContextStateWire } from "@/lib/finstack/generated/types/market_context_state";
import data from "../data.json";
const market = data.market.supplemental as unknown as MarketContextStateWire;
export function Example(_props: ExampleProps) {
  return <MarketContextBrowser state={market} />;
}
