"use client";
import type { ExampleProps } from "./props";
import {
  PortfolioRiskReport,
  type PositionRiskData,
} from "@/components/finstack/models/components/portfolio-risk-report/portfolio-risk-report";
import data from "../data.json";
// Host display choice: six decimal places of the returned decimals.
const sixPlaces = (value: number) => value.toFixed(6);
// The notebook's 99% parametric report with its risk budget; the historical
// variant adds the tail stress attribution.
export function Example({ density = "compact", variant }: ExampleProps) {
  const entry =
    variant === "historical"
      ? data.positionRisk.historical
      : data.positionRisk.parametric;
  return (
    <PortfolioRiskReport
      report={entry.report as unknown as PositionRiskData}
      formatValue={sixPlaces}
      density={density}
    />
  );
}
