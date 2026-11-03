"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
  workerQueryPolicy,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type {
  RecoveryWaterfallRequest,
  SeniorityRecoveryRequest,
  WorkoutLgdRequest,
} from "@/workers/finstack-contract";
export type {
  RecoveryWaterfallRequest,
  SeniorityRecoveryRequest,
  SeniorityRecoveryStats,
  WorkoutLgdRequest,
} from "@/workers/finstack-contract";
/** Key the waterfall on the complete snapshotted estate and claims; nothing is defaulted or reordered. */
export function allocateRecoveryOptions(
  client: FinstackClient | null,
  session: number,
  request: RecoveryWaterfallRequest | null,
) {
  const snapshot = request === null ? null : structuredClone(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "allocateRecovery", snapshot] as const,
    ready: snapshot !== null,
    missing: "No recovery claims supplied",
    queryFn: (ready) => ready.call("allocateRecovery", snapshot!),
  });
}
/** Key the workout LGD on the complete snapshotted inputs; nothing is defaulted or reordered. */
export function workoutLgdOptions(
  client: FinstackClient | null,
  session: number,
  request: WorkoutLgdRequest | null,
) {
  const snapshot = request === null ? null : structuredClone(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "workoutLgd", snapshot] as const,
    ready: snapshot !== null,
    missing: "No workout supplied",
    queryFn: (ready) => ready.call("workoutLgd", snapshot!),
  });
}
/** Key the statistics on the seniority class and the exact calibration id. */
export function seniorityRecoveryStatsOptions(
  client: FinstackClient | null,
  session: number,
  request: SeniorityRecoveryRequest | null,
) {
  const snapshot = request === null ? null : structuredClone(request);
  return workerQueryOptions({
    client,
    queryKey: [
      "finstack",
      session,
      "seniorityRecoveryStats",
      snapshot,
    ] as const,
    ready: snapshot !== null,
    missing: "No seniority class supplied",
    queryFn: (ready) => ready.call("seniorityRecoveryStats", snapshot!),
  });
}
/** Native absolute-priority allocation of an estate across claims, or the actual native error; supply a Finstack/Query provider. */
export function useAllocateRecovery(request: RecoveryWaterfallRequest | null) {
  return useWorkerQuery((worker) =>
    allocateRecoveryOptions(worker.client, worker.session, request),
  );
}
/** Native workout net recovery, LGD and recovery rate, or the actual native error; supply a Finstack/Query provider. */
export function useWorkoutLgd(request: WorkoutLgdRequest | null) {
  return useWorkerQuery((worker) =>
    workoutLgdOptions(worker.client, worker.session, request),
  );
}
/** Native historical Beta recovery statistics of one seniority class under one calibration, or the actual native error; supply a Finstack/Query provider. */
export function useSeniorityRecoveryStats(
  request: SeniorityRecoveryRequest | null,
) {
  return useWorkerQuery((worker) =>
    seniorityRecoveryStatsOptions(worker.client, worker.session, request),
  );
}
/** The registry's seniority calibration ids, default first, immutable within one worker session. */
export function useSeniorityCalibrationIds() {
  return useWorkerQuery((worker) => ({
    queryKey: ["finstack", worker.session, "seniorityCalibrationIds"],
    queryFn: () => worker.client!.call("seniorityCalibrationIds"),
    ...workerQueryPolicy,
  }));
}
