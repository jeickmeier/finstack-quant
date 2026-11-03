"use client";
import type { MoneyValue } from "finstack-quant-wasm";
import { PortfolioMarginReport } from "@/components/finstack/portfolio/components/portfolio-margin-report/portfolio-margin-report";
import { usePortfolioMargin } from "@/hooks/portfolio/use-portfolio-margin/use-portfolio-margin";

interface MarginPanelProps {
  /** Canonical PortfolioSpec JSON the workbench values. */
  specJson: string;
  /** MarketContextState JSON of the margin run. */
  marketJson: string;
  /** ISO date of the margin run. */
  asOf: string;
  /** IM collateral already held by CSA id; omitted is none held. */
  currentImCollateral?: Readonly<Record<string, MoneyValue>> | null;
  density: "compact" | "comfortable";
}
/**
 * The book's margin: one native portfolio.portfolioMargin, mounted (and so
 * called) when the Margin tab first opens.
 */
export function MarginPanel({
  specJson,
  marketJson,
  asOf,
  currentImCollateral,
  density,
}: MarginPanelProps) {
  const margin = usePortfolioMargin({
    specJson,
    marketJson,
    asOf,
    currentImCollateral,
  });
  return (
    <PortfolioMarginReport
      result={margin.data ?? null}
      loading={margin.isFetching}
      error={margin.error?.message}
      density={density}
    />
  );
}
