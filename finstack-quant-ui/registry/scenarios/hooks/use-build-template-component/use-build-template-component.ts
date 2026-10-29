"use client";
import { useCallback } from "react";
import { useFinstack } from "@/hooks/shared/use-finstack/use-finstack";
import type { TemplateComponentRequest } from "@/workers/finstack-contract";
export type { TemplateComponentRequest } from "@/workers/finstack-contract";
/** A loader of one component of a built-in composite scenario template through the shared worker; rejects with the native error. */
export function useBuildTemplateComponent() {
  const worker = useFinstack();
  return useCallback(
    async (request: TemplateComponentRequest) => {
      if (worker.error) throw worker.error;
      if (worker.status !== "ready" || !worker.client)
        throw new Error(
          "Scenario template component is waiting for the worker",
        );
      return worker.client.call("buildTemplateComponent", request);
    },
    [worker.client, worker.status, worker.error],
  );
}
