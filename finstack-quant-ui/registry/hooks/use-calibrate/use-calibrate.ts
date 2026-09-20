"use client";
import { useCallback } from "react";
import { queryOptions, useQuery } from "@tanstack/react-query";
import { useFinstack } from "../use-finstack/use-finstack";
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
    staleTime: Infinity,
    retry: false,
  });
}
/** Pass null until the user requests a solve; returns the unchanged native result or structured failure. */
export function useCalibrate(envelopeJson: string | null) {
  const worker = useFinstack();
  const query = useQuery({
    ...calibrationOptions(
      worker.client,
      worker.session,
      "calibrate",
      envelopeJson,
    ),
    enabled: worker.status === "ready" && envelopeJson !== null,
  });
  return {
    ...query,
    error: worker.error ?? query.error,
    workerStatus: worker.status,
  };
}
/** Static dependency/error report; no solver is invoked and returned JSON stays unchanged. */
export function useCalibrationDryRun(envelopeJson: string | null) {
  const worker = useFinstack();
  const query = useQuery({
    ...calibrationOptions(
      worker.client,
      worker.session,
      "dryRun",
      envelopeJson,
    ),
    enabled: worker.status === "ready" && envelopeJson !== null,
  });
  return {
    ...query,
    error: worker.error ?? query.error,
    workerStatus: worker.status,
  };
}
/** Native canonicalization for CalibrationForm; a superseded validation cannot accept old edits. */
export function useCalibrationValidator() {
  const worker = useFinstack();
  return useCallback(
    async (envelopeJson: string, signal?: AbortSignal) => {
      if (worker.error) throw worker.error;
      if (worker.status !== "ready" || !worker.client)
        throw new Error("Calibration validation is waiting for the worker");
      const json = await worker.client.call(
        "validateCalibration",
        envelopeJson,
      );
      if (signal?.aborted)
        throw new DOMException("Validation superseded", "AbortError");
      return json;
    },
    [worker.client, worker.status, worker.error],
  );
}
