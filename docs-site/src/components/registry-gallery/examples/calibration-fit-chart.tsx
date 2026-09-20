"use client";
import type { ExampleProps } from "./props";
import { CalibrationFitChart } from "@/components/finstack/components/calibration-fit-chart/calibration-fit-chart";
import { useLinkedSelection } from "@/hooks/use-linked-selection/use-linked-selection";
import type { CalibrationWire } from "@/lib/finstack/generated/types/calibration";
import type { CalibrationReport as NativeReport } from "finstack-quant-wasm";
import data from "../data.json";
const calibrated = data.calibration.at(-1)!;
export function Example({
  width = 880,
  presentation,
}: ExampleProps & {
  presentation?: Pick<
    import("react").ComponentProps<typeof CalibrationFitChart>,
    "title" | "subtitle" | "onSelect" | "renderTooltipBody" | "figureRefs"
  >;
}) {
  const link = useLinkedSelection();
  const report = calibrated.result.result.step_reports[
    "USD-OIS"
  ] as unknown as NativeReport;
  return (
    <CalibrationFitChart
      {...presentation}
      stepId="USD-OIS"
      report={report}
      marketData={
        calibrated.input.market_data as CalibrationWire["market_data"]
      }
      link={link}
      width={width}
      height={460}
      caption="Returned residuals in native solver units"
      sources={[{ label: "Native calibration result" }]}
    />
  );
}
