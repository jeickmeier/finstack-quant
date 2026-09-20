"use client";
import { useCallback } from "react";
import { useFinstack } from "../use-finstack/use-finstack";
/** Native Market construction/canonicalization in the shared worker; stale validation is ignored. */
export function useMarketValidator() {
  const worker = useFinstack();
  return useCallback(
    async (marketJson: string, signal?: AbortSignal) => {
      if (worker.error) throw worker.error;
      if (worker.status !== "ready" || !worker.client)
        throw new Error("Market validation is waiting for the worker");
      const json = await worker.client.call("validateMarket", marketJson);
      if (signal?.aborted)
        throw new DOMException("Validation superseded", "AbortError");
      return json;
    },
    [worker.client, worker.status, worker.error],
  );
}
