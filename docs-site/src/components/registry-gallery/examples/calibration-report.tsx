"use client";
import type { ExampleProps } from "./props";
import { CalibrationReport } from "@/components/finstack/calibration/components/calibration-report/calibration-report";
import { useLinkedSelection } from "@/hooks/shared/use-linked-selection/use-linked-selection";
import type { CalibrationReport as NativeReport } from "finstack-quant-wasm";
import data from "../data.json";
const calibrated = data.calibration.at(-1)!;
export function Example(_props: ExampleProps) {
  const link = useLinkedSelection();
  const report = calibrated.result.result.step_reports[
    "USD-OIS"
  ] as unknown as NativeReport;
  return <CalibrationReport stepId="USD-OIS" report={report} link={link} />;
}
