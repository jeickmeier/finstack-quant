"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type {
  DatedCashflow,
  TwrrLinkedRequest,
  TwrrPeriod,
} from "@/workers/finstack-contract";
export type {
  DatedCashflow,
  LinkedReturn,
  TwrrLinkedRequest,
  TwrrPeriod,
} from "@/workers/finstack-contract";
/** Key the link on the snapshotted returns and horizon; nothing is defaulted. */
export function twrrLinkedOptions(
  client: FinstackClient | null,
  session: number,
  request: TwrrLinkedRequest | null,
) {
  const snapshot = request === null ? null : structuredClone(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "twrrLinked", snapshot] as const,
    ready: snapshot !== null,
    missing: "No sub-period returns supplied",
    queryFn: (ready) => ready.call("twrrLinked", snapshot!),
  });
}
/** Key the return on the snapshotted period; omitted flows stay omitted. */
export function twrrModifiedDietzOptions(
  client: FinstackClient | null,
  session: number,
  period: TwrrPeriod | null,
) {
  const snapshot = period === null ? null : structuredClone(period);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "twrrModifiedDietz", snapshot] as const,
    ready: snapshot !== null,
    missing: "No period supplied",
    queryFn: (ready) => ready.call("twrrModifiedDietz", snapshot!),
  });
}
/** Key the return on the snapshotted flows, in the order supplied; Rust sorts and nets them. */
export function mwrXirrOptions(
  client: FinstackClient | null,
  session: number,
  cashflows: readonly DatedCashflow[] | null,
) {
  const snapshot = cashflows === null ? null : structuredClone(cashflows);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "mwrXirr", snapshot] as const,
    ready: snapshot !== null,
    missing: "No cash flows supplied",
    queryFn: (ready) => ready.call("mwrXirr", snapshot!),
  });
}
/** Native portfolio.twrrLinked of host-supplied returns over a horizon, or its native error; null disables the call. Supply a Finstack/Query provider. */
export function useTwrrLinked(request: TwrrLinkedRequest | null) {
  return useWorkerQuery((worker) =>
    twrrLinkedOptions(worker.client, worker.session, request),
  );
}
/** Native portfolio.twrrModifiedDietz of one host-supplied period, or its native error; null disables the call. Supply a Finstack/Query provider. */
export function useTwrrModifiedDietz(period: TwrrPeriod | null) {
  return useWorkerQuery((worker) =>
    twrrModifiedDietzOptions(worker.client, worker.session, period),
  );
}
/** Native portfolio.mwrXirr of host-supplied dated flows, or its native error; null disables the call. Supply a Finstack/Query provider. */
export function useMwrXirr(cashflows: readonly DatedCashflow[] | null) {
  return useWorkerQuery((worker) =>
    mwrXirrOptions(worker.client, worker.session, cashflows),
  );
}
