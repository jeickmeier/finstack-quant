"use client";
import { useNativeValidator } from "../use-finstack/validation";
import { queryOptions } from "@tanstack/react-query";
import { useWorkerQuery, workerQueryPolicy } from "../use-finstack/query";
import type { WorkerApi } from "../../workers/finstack-contract";
import type { FinstackClient } from "../use-finstack/client";
/** Immutable complete JSON identity: quote data, prior markets and every setting remain in the key. */
export function calibrationOptions<K extends "calibrate" | "dryRun">(
  client: FinstackClient | null,
  session: number,
  operation: K,
  envelopeJson: string | null,
) {
  return queryOptions({
    queryKey: ["finstack", session, operation, envelopeJson] as const,
    queryFn: () => {
      if (!client || envelopeJson === null)
        throw new Error(
          "Calibration requires a ready worker and an explicit envelope",
        );
      return client.call(
        operation,
        ...([envelopeJson] as Parameters<WorkerApi[K]>),
      );
    },
    enabled: client !== null && envelopeJson !== null,
    ...workerQueryPolicy,
  });
}
/** Pass null until the user requests a solve; returns the unchanged native result or structured failure. */
export function useCalibrate(envelopeJson: string | null) {
  return useWorkerQuery((worker) => ({
    ...calibrationOptions(
      worker.client,
      worker.session,
      "calibrate",
      envelopeJson,
    ),
    enabled: worker.status === "ready" && envelopeJson !== null,
  }));
}
/** Static dependency/error report; no solver is invoked and returned JSON stays unchanged. */
export function useCalibrationDryRun(envelopeJson: string | null) {
  return useWorkerQuery((worker) => ({
    ...calibrationOptions(
      worker.client,
      worker.session,
      "dryRun",
      envelopeJson,
    ),
    enabled: worker.status === "ready" && envelopeJson !== null,
  }));
}
/** Native canonicalization for CalibrationForm; a superseded validation cannot accept old edits. */
export function useCalibrationValidator() {
  return useNativeValidator("validateCalibration", "Calibration");
}
