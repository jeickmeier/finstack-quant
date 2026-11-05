"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type {
  StatementMappingChecksRequest,
  StatementSuiteChecksRequest,
} from "@/workers/finstack-contract";
export type {
  StatementMappingChecksRequest,
  StatementSuiteChecksRequest,
} from "@/workers/finstack-contract";
/** One native check runner and its arguments, as the worker methods take them. */
export type StatementChecksCall =
  | {
      readonly method: "runChecks";
      readonly request: StatementSuiteChecksRequest;
    }
  | {
      readonly method:
        "runThreeStatementChecks" | "runCreditUnderwritingChecks";
      readonly request: StatementMappingChecksRequest;
    };
/** Snapshot the runner method, model, matching result and configuration. Null disables the run. */
export function runStatementChecksOptions(
  client: FinstackClient | null,
  session: number,
  call: StatementChecksCall | null,
) {
  const snapshot =
    call === null
      ? null
      : { method: call.method, request: { ...call.request } };
  return workerQueryOptions({
    client,
    queryKey: [
      "finstack",
      session,
      snapshot?.method ?? "runChecks",
      snapshot?.request ?? null,
    ] as const,
    ready: snapshot !== null,
    missing: "Statement checks require an explicit request and ready worker",
    queryFn: (ready) => {
      const current = snapshot as StatementChecksCall;
      switch (current.method) {
        case "runChecks":
          return ready.call("runChecks", current.request);
        case "runThreeStatementChecks":
        case "runCreditUnderwritingChecks":
          return ready.call(current.method, current.request);
        default: {
          const unhandled: never = current;
          throw new RangeError(String(unhandled));
        }
      }
    },
  });
}
/** Native CheckReport of the chosen runner, or its structured failure. */
export function useRunStatementChecks(call: StatementChecksCall | null) {
  return useWorkerQuery((worker) =>
    runStatementChecksOptions(worker.client, worker.session, call),
  );
}
