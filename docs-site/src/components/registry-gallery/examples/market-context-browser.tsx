"use client";
import type { ExampleProps } from "./props";
import { MarketContextBrowser } from "@/components/finstack/core/components/market-context-browser/market-context-browser";
import { supplementalMarket } from "../fixtures";
const market = supplementalMarket;
export function Example(_props: ExampleProps) {
  return <MarketContextBrowser state={market} />;
}
