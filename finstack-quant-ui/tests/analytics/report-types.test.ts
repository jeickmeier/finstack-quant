import { expectTypeOf, it } from "vitest";
import type {
  PerformanceReport,
  BenchmarkReport,
} from "@/workers/finstack-contract";
import type {
  PerformanceReportData,
  BenchmarkReportData,
  BenchmarkTickerData,
} from "@/components/finstack/analytics/lib/analytics-report-types";
import type { PerformanceReportProps } from "@/components/finstack/analytics/components/performance-report/performance-report";
import type { BenchmarkReportProps } from "@/components/finstack/analytics/components/benchmark-report/benchmark-report";

it("accepts native reports and readonly display inputs without transport-only fields", () => {
  expectTypeOf<PerformanceReport>().toExtend<PerformanceReportData>();
  expectTypeOf<BenchmarkReport>().toExtend<BenchmarkReportData>();
  expectTypeOf<
    Omit<PerformanceReport, "correlation">
  >().toExtend<PerformanceReportData>();
  expectTypeOf<
    PerformanceReportProps["report"]
  >().toEqualTypeOf<PerformanceReportData | null>();
  expectTypeOf<
    BenchmarkReportProps["report"]
  >().toEqualTypeOf<BenchmarkReportData | null>();
  expectTypeOf<PerformanceReportData["tickerNames"]>().toEqualTypeOf<
    readonly string[]
  >();
  expectTypeOf<BenchmarkReportData["tickers"]>().toEqualTypeOf<
    readonly BenchmarkTickerData[]
  >();
  expectTypeOf<{ ok: false; error: { message: string } }>().toExtend<
    BenchmarkTickerData["multiFactor"]
  >();
});
