"use client";
import type { PriceRequest } from "@/hooks/valuations/use-price-instrument/use-price-instrument";
import {
  LoanYieldCard,
  loanYieldMetrics,
} from "@/components/finstack/valuations/components/loan-yield-card/loan-yield-card";
import { useInstrumentMetrics } from "./instrument-metrics";
/** The yields, costs and default risk of the completed request's term loan. */
export function LoanPanel({
  request,
  density,
}: {
  request: PriceRequest;
  density?: "compact" | "comfortable";
}) {
  const card = useInstrumentMetrics("term_loan", loanYieldMetrics, request);
  return <LoanYieldCard {...card} density={density} />;
}
