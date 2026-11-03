"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type {
  FactorSensitivitiesRequest,
  PrimitiveExposuresRequest,
} from "@/workers/finstack-contract";
export type {
  FactorSensitivitiesRequest,
  PortfolioPrimitiveExposureReport,
  PrimitiveExposuresRequest,
} from "@/workers/finstack-contract";
export type { SensitivityMatrixJson } from "finstack-quant-wasm";
/** Key the decomposition on the snapshotted book, market and metric ids; nothing is defaulted. */
export function primitiveExposuresOptions(
  client: FinstackClient | null,
  session: number,
  request: PrimitiveExposuresRequest | null,
) {
  const snapshot = request === null ? null : structuredClone(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "primitiveExposures", snapshot] as const,
    ready: snapshot !== null,
    missing: "No book supplied",
    queryFn: (ready) => ready.call("primitiveExposures", snapshot!),
  });
}
/** Key the matrix on the snapshotted book, market, factors, date and bump config; an omitted bump config stays omitted. */
export function factorSensitivitiesOptions(
  client: FinstackClient | null,
  session: number,
  request: FactorSensitivitiesRequest | null,
) {
  const snapshot = request === null ? null : structuredClone(request);
  return workerQueryOptions({
    client,
    queryKey: [
      "finstack",
      session,
      "computeFactorSensitivities",
      snapshot,
    ] as const,
    ready: snapshot !== null,
    missing: "No factor definitions supplied",
    queryFn: (ready) => ready.call("computeFactorSensitivities", snapshot!),
  });
}
/** Native portfolio.primitiveExposures of a host-supplied book over a host-supplied market, or its native error; null disables the call. Supply a Finstack/Query provider. */
export function usePrimitiveExposures(
  request: PrimitiveExposuresRequest | null,
) {
  return useWorkerQuery((worker) =>
    primitiveExposuresOptions(worker.client, worker.session, request),
  );
}
/** Native portfolio.computeFactorSensitivities of a host-supplied book against host-supplied factor definitions, or its native error; null disables the call. Supply a Finstack/Query provider. */
export function useFactorSensitivities(
  request: FactorSensitivitiesRequest | null,
) {
  return useWorkerQuery((worker) =>
    factorSensitivitiesOptions(worker.client, worker.session, request),
  );
}
