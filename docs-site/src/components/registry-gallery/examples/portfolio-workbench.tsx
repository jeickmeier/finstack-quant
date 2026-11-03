"use client";
import type { ExampleProps } from "./props";
import { PortfolioWorkbench } from "@/components/finstack/portfolio/blocks/portfolio-workbench/portfolio-workbench";
import data from "../data.json";
// The margined rates book (two bilateral swaps and an unmargined deposit) over
// the pricing market, with the host's metric menu, bucketed series and the
// host-parity USD-OIS scenarios and, for the Attribution tab, the market after
// a 50bp USD-OIS shock as the closing snapshot; every tab values it natively.
const { specJson, request } = data.portfolioWorkbench;
const attribution = {
  ...data.portfolioAttribution.closing["usd-ois-50bp"],
  reconciliationTolerance: data.portfolioAttribution.reconciliationTolerance,
};
const marketJson = JSON.stringify(data.pricingMarket);
export function Example({ density = "compact" }: ExampleProps) {
  return (
    <PortfolioWorkbench
      defaultSpecJson={specJson}
      marketJson={marketJson}
      asOf={request.asOf}
      metrics={request.metrics}
      seriesBase={request.seriesBase}
      scenarioJsons={request.scenarioJsons}
      attribution={attribution}
      density={density}
    />
  );
}
