"use client";
import { requestSnapshot } from "@/hooks/shared/use-finstack/snapshot";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { DcfSensitivityRequest } from "@/workers/finstack-contract";
export type { DcfSensitivityRequest } from "@/workers/finstack-contract";
export type { DcfSensitivityResult } from "finstack-quant-wasm";
/** Snapshot the model and every DCF assumption. Null disables the sensitivity. A non-finite input stays distinct in the key and reaches Rust as entered. */
export function dcfSensitivityOptions(
  client: FinstackClient | null,
  session: number,
  request: DcfSensitivityRequest | null,
) {
  const { snapshot, key } = requestSnapshot(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "dcfSensitivity", key] as const,
    ready: snapshot !== null,
    missing:
      "The DCF sensitivity requires an explicit request and ready worker",
    queryFn: (ready) => ready.call("dcfSensitivity", snapshot!),
  });
}
/** Native statements_analytics.dcfSensitivity enterprise-value tornado or structured failure. */
export function useDcfSensitivity(request: DcfSensitivityRequest | null) {
  return useWorkerQuery((worker) =>
    dcfSensitivityOptions(worker.client, worker.session, request),
  );
}
