"use client";
import {
  useCashflows,
  type CashflowRequest,
} from "@/hooks/use-cashflows/use-cashflows";
import { JsonViewer } from "../../primitives/json-viewer/json-viewer";
/** Exact native cashflow export. Pricing and model state remain owned by the caller. */
export function CashflowViewer({
  request,
  density = "compact",
}: {
  request: CashflowRequest;
  density?: "compact" | "comfortable";
}) {
  const query = useCashflows(request);
  return (
    <JsonViewer
      label="Cashflows"
      downloadName="cashflows.json"
      text={query.data}
      density={density}
      loading={query.isFetching || query.workerStatus === "starting"}
      error={
        query.error
          ? `Cashflows unavailable: ${query.error.message}`
          : undefined
      }
    />
  );
}
