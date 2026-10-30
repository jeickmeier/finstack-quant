"use client";
import { useCallback } from "react";
import { useFinstack } from "@/hooks/shared/use-finstack/use-finstack";
/** A loader of whole built-in scenario templates through the shared worker; rejects with the native error. */
export function useBuildFromTemplate() {
  const worker = useFinstack();
  return useCallback(
    async (templateId: string) => {
      if (worker.error) throw worker.error;
      if (worker.status !== "ready" || !worker.client)
        throw new Error("Scenario template is waiting for the worker");
      return worker.client.call("buildFromTemplate", templateId);
    },
    [worker.client, worker.status, worker.error],
  );
}
