"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { PortfolioMarginRequest } from "@/workers/finstack-contract";
export type { PortfolioMarginRequest } from "@/workers/finstack-contract";
/** Key the margin run on the complete snapshotted spec, market, date and collateral; nothing is parsed or defaulted. */
export function portfolioMarginOptions(
  client: FinstackClient | null,
  session: number,
  request: PortfolioMarginRequest | null,
) {
  const snapshot = request === null ? null : structuredClone(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "portfolioMargin", snapshot] as const,
    ready: snapshot !== null,
    missing: "No portfolio margin request supplied",
    queryFn: (ready) => ready.call("portfolioMargin", snapshot!),
  });
}
/** Native portfolio.portfolioMargin result for a host-supplied book, market and date, or the actual native error; supply a Finstack/Query provider. */
export function usePortfolioMargin(request: PortfolioMarginRequest | null) {
  return useWorkerQuery((worker) => ({
    ...portfolioMarginOptions(worker.client, worker.session, request),
    enabled: worker.status === "ready" && request !== null,
  }));
}
