"use client";
import {
  usePoolReport,
  usePoolReportFieldUnits,
  type PoolReportRequest,
} from "@/hooks/valuations/use-deal-report/use-deal-report";
import { PoolReport } from "@/components/finstack/valuations/components/pool-report/pool-report";
/** Native pool report of the completed request's deal, in the units Rust states for its currency. */
export function PoolPanel({
  request,
  density,
}: {
  request: PoolReportRequest;
  density?: "compact" | "comfortable";
}) {
  const report = usePoolReport(request);
  const units = usePoolReportFieldUnits(
    report.data?.total_balance.currency ?? null,
  );
  return (
    <PoolReport
      report={report.data ?? null}
      units={units.data ?? null}
      loading={report.isFetching || units.isFetching}
      error={report.error?.message ?? units.error?.message}
      density={density}
    />
  );
}
