"use client";
import { useCallback, useRef } from "react";
import { useFinstack } from "../use-finstack/use-finstack";
/** Native canonical validation for SchemaForm; all calls run in the existing shared worker. */
export function useInstrumentValidator() {
  const worker = useFinstack();
  const revision = useRef(0);
  return useCallback(
    async (instrumentJson: string, signal?: AbortSignal) => {
      if (worker.error) throw worker.error;
      if (worker.status !== "ready" || !worker.client)
        throw new Error("Instrument validation is waiting for the worker");
      const result = await worker.client.call("validate", {
        instrumentJson,
        revision: ++revision.current,
      });
      if (signal?.aborted)
        throw new DOMException("Validation superseded", "AbortError");
      return result.json;
    },
    [worker.client, worker.status, worker.error],
  );
}
