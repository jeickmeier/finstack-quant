"use client";
import { useCallback } from "react";
import { useFinstack } from "./use-finstack";
/** Shared transport policy; SchemaForm owns scheduling and cancellation of stale results. */
export function useNativeValidator(
  operation: "validate" | "validateMarket" | "validateCalibration",
  label: string,
) {
  const worker = useFinstack();
  return useCallback(
    async (json: string, signal?: AbortSignal) => {
      if (worker.error) throw worker.error;
      if (worker.status !== "ready" || !worker.client)
        throw new Error(`${label} validation is waiting for the worker`);
      const canonical = await worker.client.call(operation, json);
      if (signal?.aborted)
        throw new DOMException("Validation superseded", "AbortError");
      return canonical;
    },
    [worker.client, worker.status, worker.error, operation, label],
  );
}
