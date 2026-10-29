"use client";
import type { ExampleProps } from "./props";
import {
  PortfolioRiskReport,
  type FactorRiskData,
  type PositionRiskData,
} from "@/components/finstack/models/components/portfolio-risk-report/portfolio-risk-report";
import data from "../data.json";
// Host display choice: six decimal places of the returned decimals.
const sixPlaces = (value: number) => value.toFixed(6);
// The notebook's 99% parametric report with its risk budget, and the factor
// decomposition of the host-parity book; the historical variant shows the
// 95% historical report with its tail stress attribution instead.
export function Example({ density = "compact", variant }: ExampleProps) {
  const historical = variant === "historical";
  const entry = historical
    ? data.positionRisk.historical
    : data.positionRisk.parametric;
  return (
    <PortfolioRiskReport
      report={entry.report as unknown as PositionRiskData}
      factors={
        historical
          ? null
          : (data.positionRisk.factor.report as unknown as FactorRiskData)
      }
      formatValue={sixPlaces}
      density={density}
    />
  );
}
