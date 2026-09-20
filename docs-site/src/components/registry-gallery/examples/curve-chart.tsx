"use client";
import type { ExampleProps } from "./props";
import { CurveChart } from "@/components/finstack/components/curve-chart/curve-chart";
import type { MarketContextStateWire } from "@/lib/finstack/generated/types/market_context_state";
import data from "../data.json";
const market = data.market.supplemental as unknown as MarketContextStateWire;
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
