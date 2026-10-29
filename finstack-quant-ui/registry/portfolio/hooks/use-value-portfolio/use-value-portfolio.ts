"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { PortfolioValuationRequest } from "@/workers/finstack-contract";
export type {
  PortfolioValuationReport,
  PortfolioValuationRequest,
} from "@/workers/finstack-contract";
/** Key the valuation on the complete snapshotted spec, market and options; nothing is defaulted. */
export function valuePortfolioOptions(
  client: FinstackClient | null,
  session: number,
  request: PortfolioValuationRequest | null,
) {
  const snapshot = request === null ? null : structuredClone(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "valuePortfolio", snapshot] as const,
    ready: snapshot !== null,
    missing: "No portfolio supplied",
    queryFn: (ready) => ready.call("valuePortfolio", snapshot!),
  });
}
/** Native portfolio valuation and aggregated metrics of a host-supplied book and market, or the actual native error; supply a Finstack/Query provider. */
export function useValuePortfolio(request: PortfolioValuationRequest | null) {
  return useWorkerQuery((worker) => ({
    ...valuePortfolioOptions(worker.client, worker.session, request),
    enabled: worker.status === "ready" && request !== null,
  }));
}
