"use client";
import {
  useWorkerQuery,
  workerQueryPolicy,
} from "@/hooks/shared/use-finstack/query";
/** Rust's default waterfall factor order (attribution.defaultWaterfallOrder), immutable within one worker session; supply a Finstack/Query provider. */
export function useDefaultWaterfallOrder() {
  return useWorkerQuery((worker) => ({
    queryKey: ["finstack", worker.session, "defaultWaterfallOrder"] as const,
    queryFn: () => worker.client!.call("defaultWaterfallOrder"),
    ...workerQueryPolicy,
  }));
}
