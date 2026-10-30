"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
/** Key the built-in template list on the worker session alone; it takes no input. */
export function listBuiltinTemplateMetadataOptions(
  client: FinstackClient | null,
  session: number,
) {
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "listBuiltinTemplateMetadata"] as const,
    ready: true,
    missing: "No template request supplied",
    queryFn: (ready) => ready.call("listBuiltinTemplateMetadata"),
  });
}
/** Native scenarios.listBuiltinTemplateMetadata in registry order, or the actual native error; supply a Finstack/Query provider. */
export function useListBuiltinTemplateMetadata() {
  return useWorkerQuery((worker) =>
    listBuiltinTemplateMetadataOptions(worker.client, worker.session),
  );
}
