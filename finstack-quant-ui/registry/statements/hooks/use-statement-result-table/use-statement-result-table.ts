"use client";
import { requestSnapshot } from "@/hooks/shared/use-finstack/snapshot";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
/** One evaluated result and the table layout the user picked. */
export interface StatementResultTableRequest {
  /** Host-serialized StatementResult from evaluateStatement or rollupStatement. */
  readonly resultJson: string;
  readonly layout: "wide" | "long";
}
/** Snapshot the result and layout, which picks the native call. Null disables the query. */
export function statementResultTableOptions(
  client: FinstackClient | null,
  session: number,
  request: StatementResultTableRequest | null,
) {
  const { snapshot, key } = requestSnapshot(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "statementResultTable", key] as const,
    ready: snapshot !== null,
    missing: "A statement table requires an explicit request and ready worker",
    queryFn: (ready) =>
      snapshot!.layout === "wide"
        ? ready.call("statementResultToTableWide", snapshot!.resultJson)
        : ready.call("statementResultToTableLong", snapshot!.resultJson),
  });
}
/** Native TableEnvelope of an evaluated result in the requested layout, or the native error. */
export function useStatementResultTable(
  request: StatementResultTableRequest | null,
) {
  return useWorkerQuery((worker) =>
    statementResultTableOptions(worker.client, worker.session, request),
  );
}
