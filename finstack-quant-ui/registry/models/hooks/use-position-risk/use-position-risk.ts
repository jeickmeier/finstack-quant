"use client";
import { requestSnapshot } from "@/hooks/shared/use-finstack/snapshot";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { PositionRiskRequest } from "@/workers/finstack-contract";
export type {
  PositionRisk,
  PositionRiskRequest,
  PositionRiskSource,
  StressAttribution,
} from "@/workers/finstack-contract";
/** Key the report on the complete snapshotted positions, source, confidence and budget (a non-finite value stays distinct); nothing is defaulted. */
export function positionRiskOptions(
  client: FinstackClient | null,
  session: number,
  request: PositionRiskRequest | null,
) {
  const { snapshot, key } = requestSnapshot(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "positionRisk", key] as const,
    ready: snapshot !== null,
    missing: "No positions supplied",
    queryFn: (ready) => ready.call("positionRisk", snapshot!),
  });
}
/** Native models.factor.risk VaR/ES decomposition with its risk budget and tail stress attribution, or the actual native error; supply a Finstack/Query provider. */
export function usePositionRisk(request: PositionRiskRequest | null) {
  return useWorkerQuery((worker) => ({
    ...positionRiskOptions(worker.client, worker.session, request),
    enabled: worker.status === "ready" && request !== null,
  }));
}
