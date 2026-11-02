"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { ComputeQuotesRequest } from "@/workers/finstack-contract";
export type {
  BondQuoteSet,
  ComputeQuotesRequest,
} from "@/workers/finstack-contract";
/** Key the set on the complete snapshotted request; nothing is defaulted or reordered. */
export function computeQuotesOptions(
  client: FinstackClient | null,
  session: number,
  request: ComputeQuotesRequest | null,
) {
  const snapshot = request === null ? null : structuredClone(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "computeQuotes", snapshot] as const,
    ready: snapshot !== null,
    missing: "No bond supplied",
    queryFn: (ready) => ready.call("computeQuotes", snapshot!),
  });
}
/** Native whole quote set of a bond from any one quote; supply a Finstack/Query provider. */
export function useComputeQuotes(request: ComputeQuotesRequest | null) {
  return useWorkerQuery((worker) =>
    computeQuotesOptions(worker.client, worker.session, request),
  );
}
/** Native units of a quote set's fields for the bond's currency. Null disables the call. */
export function useBondQuoteSetFieldUnits(currency: string | null) {
  return useWorkerQuery((worker) =>
    workerQueryOptions({
      client: worker.client,
      queryKey: [
        "finstack",
        worker.session,
        "bondQuoteSetFieldUnits",
        currency,
      ] as const,
      ready: currency !== null,
      missing: "Quote set units require a currency and ready worker",
      queryFn: (ready) => ready.call("bondQuoteSetFieldUnits", currency!),
    }),
  );
}
