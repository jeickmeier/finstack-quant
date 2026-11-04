"use client";
import type { ExampleProps } from "./props";
import { CalibrationReport } from "@/components/finstack/calibration/components/calibration-report/calibration-report";
import { useLinkedSelection } from "@/hooks/shared/use-linked-selection/use-linked-selection";
import { calibration } from "../fixtures";
const calibrated = calibration.at(-1)!;
export function Example(_props: ExampleProps) {
  const link = useLinkedSelection();
  const report = calibrated.result.result.step_reports["USD-OIS"];
  return <CalibrationReport stepId="USD-OIS" report={report} link={link} />;
}
