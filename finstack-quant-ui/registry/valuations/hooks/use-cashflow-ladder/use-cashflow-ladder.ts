"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { CashflowEnvelope } from "@/workers/finstack-contract";
export type {
  CashflowEnvelope,
  CurrencyLadder,
} from "@/workers/finstack-contract";
/** Key the ladder on the complete supplied envelope; the worker reads its flows unchanged. */
export function cashflowLadderOptions(
  client: FinstackClient | null,
  session: number,
  cashflows: CashflowEnvelope | null | undefined,
) {
  return workerQueryOptions({
    client,
    queryKey: [
      "finstack",
      session,
      "cashflowLadder",
      cashflows ?? null,
    ] as const,
    ready: cashflows != null,
    missing: "No cashflow envelope supplied",
    queryFn: (ready) =>
      ready.call("cashflowLadder", cashflows as CashflowEnvelope),
  });
}
/** Native calendar-year ladder per row currency of a supplied cashflow envelope, or the actual native error; supply a Finstack/Query provider. */
export function useCashflowLadder(
  cashflows: CashflowEnvelope | null | undefined,
) {
  return useWorkerQuery((worker) =>
    cashflowLadderOptions(worker.client, worker.session, cashflows),
  );
}
