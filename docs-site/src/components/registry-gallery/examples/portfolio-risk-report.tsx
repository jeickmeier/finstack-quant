"use client";
import { sixPlaces, type ExampleProps } from "./props";
import { PortfolioRiskReport } from "@/components/finstack/models/components/portfolio-risk-report/portfolio-risk-report";
import { positionRisk } from "../fixtures";
// The notebook's 99% parametric report with its risk budget, and the factor
// decomposition of the host-parity book; the historical variant shows the
// 95% historical report with its tail stress attribution instead.
export function Example({ density = "compact", variant }: ExampleProps) {
  const historical = variant === "historical";
  const entry = historical ? positionRisk.historical : positionRisk.parametric;
  return (
    <PortfolioRiskReport
      report={entry.report}
      factors={historical ? null : positionRisk.factor.report}
      formatValue={sixPlaces}
      density={density}
    />
  );
}
