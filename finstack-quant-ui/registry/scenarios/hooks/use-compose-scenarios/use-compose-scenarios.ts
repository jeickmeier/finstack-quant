"use client";
import { useCallback } from "react";
import { useFinstack } from "@/hooks/shared/use-finstack/use-finstack";
/** A composer of ScenarioSpec texts into one spec through the shared worker; rejects with the native error. */
export function useComposeScenarios() {
  const worker = useFinstack();
  return useCallback(
    async (specJsons: readonly string[]) => {
      if (worker.error) throw worker.error;
      if (worker.status !== "ready" || !worker.client)
        throw new Error("Scenario composition is waiting for the worker");
      return worker.client.call("composeScenarios", [...specJsons]);
    },
    [worker.client, worker.status, worker.error],
  );
}
