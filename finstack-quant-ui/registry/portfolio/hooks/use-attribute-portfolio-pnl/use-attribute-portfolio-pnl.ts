"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { PortfolioAttributionRequest } from "@/workers/finstack-contract";
export type {
  PortfolioAttributionReport,
  PortfolioAttributionRequest,
  PortfolioPositionAttribution,
} from "@/workers/finstack-contract";
/** Key the attribution on the complete snapshotted spec, markets, dates, method and tolerance; nothing is parsed or defaulted. */
export function attributePortfolioPnlOptions(
  client: FinstackClient | null,
  session: number,
  request: PortfolioAttributionRequest | null,
) {
  const snapshot = request === null ? null : structuredClone(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "attributePortfolioPnl", snapshot] as const,
    ready: snapshot !== null,
    missing: "No portfolio attribution request supplied",
    queryFn: (ready) => ready.call("attributePortfolioPnl", snapshot!),
  });
}
/** Native portfolio.attributePortfolioPnl of a host-supplied book between two host-supplied markets, with Rust's explain text, reconciliation and buckets for the book and each position, or the actual native error; supply a Finstack/Query provider. */
export function useAttributePortfolioPnl(
  request: PortfolioAttributionRequest | null,
) {
  return useWorkerQuery((worker) =>
    attributePortfolioPnlOptions(worker.client, worker.session, request),
  );
}
