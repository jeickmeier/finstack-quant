"use client";
import { useCallback } from "react";
import { useFinstack } from "./use-finstack";
import type { WorkerApi } from "@/workers/finstack-contract";
/** Shared transport policy; SchemaForm owns scheduling and cancellation of stale results. */
export function useNativeValidator<
  K extends "validate" | "validateMarket" | "validateCalibration",
>(operation: K, label: string) {
  const worker = useFinstack();
  return useCallback(
    async (json: string, signal?: AbortSignal) => {
      if (worker.error) throw worker.error;
      if (worker.status !== "ready" || !worker.client)
        throw new Error(`${label} validation is waiting for the worker`);
      const canonical = await worker.client.call(
        operation,
        ...([json] as Parameters<WorkerApi[K]>),
      );
      if (signal?.aborted)
        throw new DOMException("Validation superseded", "AbortError");
      return canonical;
    },
    [worker.client, worker.status, worker.error, operation, label],
  );
}
