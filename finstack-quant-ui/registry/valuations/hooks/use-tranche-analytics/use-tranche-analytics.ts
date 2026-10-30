"use client";
import { requestSnapshot } from "@/hooks/shared/use-finstack/snapshot";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type {
  TrancheMetricsRequest,
  TrancheOasRequest,
  TranchePriceRequest,
  TrancheRequest,
} from "@/workers/finstack-contract";
export type {
  TrancheMetricsRequest,
  TrancheOasRequest,
  TranchePriceRequest,
  TrancheRequest,
} from "@/workers/finstack-contract";
/** Snapshot the deal, tranche, market, date, optional price and metric ids. Null disables the call. */
export function trancheMetricsOptions(
  client: FinstackClient | null,
  session: number,
  request: TrancheMetricsRequest | null,
) {
  const { snapshot, key } = requestSnapshot(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "trancheMetrics", key] as const,
    ready: snapshot !== null,
    missing: "Tranche metrics require an explicit request and ready worker",
    queryFn: (ready) => ready.call("trancheMetrics", snapshot!),
  });
}
/** Snapshot the deal, tranche, market, date, explicit price and optional OasConfig. Null disables the solve. */
export function trancheOasOptions(
  client: FinstackClient | null,
  session: number,
  request: TrancheOasRequest | null,
) {
  const { snapshot, key } = requestSnapshot(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "trancheOas", key] as const,
    ready: snapshot !== null,
    missing: "Tranche OAS requires an explicit price and ready worker",
    queryFn: (ready) => ready.call("trancheOas", snapshot!),
  });
}
/** Snapshot the deal, tranche, market, date and explicit price. Null disables the solve. */
export function trancheDiscountMarginOptions(
  client: FinstackClient | null,
  session: number,
  request: TranchePriceRequest | null,
) {
  const { snapshot, key } = requestSnapshot(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "trancheDiscountMargin", key] as const,
    ready: snapshot !== null,
    missing:
      "Tranche discount margin requires an explicit price and ready worker",
    queryFn: (ready) => ready.call("trancheDiscountMargin", snapshot!),
  });
}
/** Snapshot the deal, tranche, market and date. Null disables the solve. */
export function trancheBreakevenCdrOptions(
  client: FinstackClient | null,
  session: number,
  request: TrancheRequest | null,
) {
  const { snapshot, key } = requestSnapshot(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "trancheBreakevenCdr", key] as const,
    ready: snapshot !== null,
    missing:
      "Tranche break-even CDR requires an explicit request and ready worker",
    queryFn: (ready) => ready.call("trancheBreakevenCdr", snapshot!),
  });
}
/** Native TrancheMetrics or a structured failure. */
export function useTrancheMetrics(request: TrancheMetricsRequest | null) {
  return useWorkerQuery((worker) =>
    trancheMetricsOptions(worker.client, worker.session, request),
  );
}
/** Native OasResult under the supplied OasConfig, else Rust's default; pass null until the user asks for the solve. */
export function useTrancheOas(request: TrancheOasRequest | null) {
  return useWorkerQuery((worker) =>
    trancheOasOptions(worker.client, worker.session, request),
  );
}
/** Native decimal discount margin or a structured failure (fixed-rate tranches fail natively). */
export function useTrancheDiscountMargin(request: TranchePriceRequest | null) {
  return useWorkerQuery((worker) =>
    trancheDiscountMarginOptions(worker.client, worker.session, request),
  );
}
/** Native decimal break-even constant default rate or a structured failure. */
export function useTrancheBreakevenCdr(request: TrancheRequest | null) {
  return useWorkerQuery((worker) =>
    trancheBreakevenCdrOptions(worker.client, worker.session, request),
  );
}
