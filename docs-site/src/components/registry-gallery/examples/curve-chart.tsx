"use client";
import type { ExampleProps } from "./props";
import { CurveChart } from "@/components/finstack/core/components/curve-chart/curve-chart";
import { supplementalMarket } from "../fixtures";
const market = supplementalMarket;
export function Example({
  width = 880,
  presentation,
}: ExampleProps & {
  presentation?: Pick<
    import("react").ComponentProps<typeof CurveChart>,
    "title" | "subtitle" | "onSelect" | "renderTooltipBody" | "figureRefs"
  >;
}) {
  return (
    <CurveChart
      {...presentation}
      curves={market.curves}
      width={width}
      height={420}
    />
  );
}
