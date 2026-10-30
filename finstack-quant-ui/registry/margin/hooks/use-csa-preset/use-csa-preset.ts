"use client";
import { useCallback } from "react";
import { useFinstack } from "@/hooks/shared/use-finstack/use-finstack";
import type { CsaPresetRequest } from "@/workers/finstack-contract";
export type { CsaPresetRequest } from "@/workers/finstack-contract";
/** A loader of Rust regulatory CSA presets through the shared worker; rejects with the native error. */
export function useCsaPreset() {
  const worker = useFinstack();
  return useCallback(
    async (request: CsaPresetRequest) => {
      if (worker.error) throw worker.error;
      if (worker.status !== "ready" || !worker.client)
        throw new Error("CSA preset is waiting for the worker");
      return worker.client.call("csaPreset", request);
    },
    [worker.client, worker.status, worker.error],
  );
}
