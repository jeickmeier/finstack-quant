"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
  workerQueryPolicy,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type {
  ExchangeOfferRequest,
  LmeRequest,
} from "@/workers/finstack-contract";
export type {
  ExchangeOfferRequest,
  LmeRequest,
} from "@/workers/finstack-contract";
/** Key the analysis on the complete snapshotted terms; nothing is defaulted or reordered. */
export function exchangeOfferOptions(
  client: FinstackClient | null,
  session: number,
  request: ExchangeOfferRequest | null,
) {
  const snapshot = request === null ? null : structuredClone(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "analyzeExchangeOffer", snapshot] as const,
    ready: snapshot !== null,
    missing: "No exchange offer supplied",
    queryFn: (ready) => ready.call("analyzeExchangeOffer", snapshot!),
  });
}
/** Key the analysis on the complete snapshotted terms; nothing is defaulted or reordered. */
export function lmeOptions(
  client: FinstackClient | null,
  session: number,
  request: LmeRequest | null,
) {
  const snapshot = request === null ? null : structuredClone(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "analyzeLme", snapshot] as const,
    ready: snapshot !== null,
    missing: "No LME terms supplied",
    queryFn: (ready) => ready.call("analyzeLme", snapshot!),
  });
}
/** Native hold-versus-tender economics of an exchange offer, including Rust's recommendation, or the actual native error; supply a Finstack/Query provider. */
export function useExchangeOffer(request: ExchangeOfferRequest | null) {
  return useWorkerQuery((worker) =>
    exchangeOfferOptions(worker.client, worker.session, request),
  );
}
/** Native issuer-side economics of a liability management exercise, or the actual native error; supply a Finstack/Query provider. */
export function useLme(request: LmeRequest | null) {
  return useWorkerQuery((worker) =>
    lmeOptions(worker.client, worker.session, request),
  );
}
/** The hurdle multiple Rust applies to the hold-out value, immutable within one worker session. */
export function useTenderRecommendationHurdle() {
  return useWorkerQuery((worker) => ({
    queryKey: ["finstack", worker.session, "tenderRecommendationHurdle"],
    queryFn: () => worker.client!.call("tenderRecommendationHurdle"),
    ...workerQueryPolicy,
  }));
}
