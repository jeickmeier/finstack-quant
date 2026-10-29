"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type {
  DecomposeFactorsRequest,
  FactorSensitivitiesRequest,
} from "@/workers/finstack-contract";
export type {
  DecomposeFactorsRequest,
  FactorSensitivitiesRequest,
} from "@/workers/finstack-contract";
/** A book's factor sensitivities and their decomposition under a factor covariance; omitted options are left to Rust. */
export interface FactorRiskRequest
  extends
    FactorSensitivitiesRequest,
    Omit<DecomposeFactorsRequest, "sensitivitiesJson"> {}
/** Key the chain on the complete snapshotted book, market, factors, covariance and options; nothing is defaulted. */
export function factorRiskOptions(
  client: FinstackClient | null,
  session: number,
  request: FactorRiskRequest | null,
) {
  const snapshot = request === null ? null : structuredClone(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "factorRisk", snapshot] as const,
    ready: snapshot !== null,
    missing: "No portfolio factor model supplied",
    queryFn: async (ready) => {
      const r = snapshot!;
      const sensitivities = await ready.call("computeFactorSensitivities", {
        specJson: r.specJson,
        marketJson: r.marketJson,
        factorsJson: r.factorsJson,
        asOf: r.asOf,
        bumpConfigJson: r.bumpConfigJson,
      });
      // The returned matrix is handed to Rust unchanged, as its JSON text.
      const decomposition = await ready.call("decomposeFactors", {
        sensitivitiesJson: JSON.stringify(sensitivities),
        covarianceJson: r.covarianceJson,
        riskMeasureJson: r.riskMeasureJson,
      });
      return { sensitivities, decomposition };
    },
  });
}
/** Native portfolio.computeFactorSensitivities of a host-supplied book, then models.factor.risk.decomposeFactors of that matrix, or the first native error; supply a Finstack/Query provider. */
export function useFactorRisk(request: FactorRiskRequest | null) {
  return useWorkerQuery((worker) => ({
    ...factorRiskOptions(worker.client, worker.session, request),
    enabled: worker.status === "ready" && request !== null,
  }));
}
