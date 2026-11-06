"use client";
import { useCallback } from "react";
import { useFinstack } from "@/hooks/shared/use-finstack/use-finstack";
import type { CovenantTemplateRequest } from "@/workers/finstack-contract";
export type {
  CovenantSpec,
  CovenantTemplateRequest,
} from "@/workers/finstack-contract";
/** A loader of Rust covenant package templates through the shared worker; rejects with the native error. */
export function useCovenantTemplate() {
  const worker = useFinstack();
  return useCallback(
    async (request: CovenantTemplateRequest) => {
      if (worker.error) throw worker.error;
      if (worker.status !== "ready" || !worker.client)
        throw new Error("Covenant template is waiting for the worker");
      return worker.client.call("covenantTemplate", request);
    },
    [worker.client, worker.status, worker.error],
  );
}
