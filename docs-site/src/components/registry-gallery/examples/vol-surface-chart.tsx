"use client";
import type { ExampleProps } from "./props";
import { VolSurfaceChart } from "@/components/finstack/core/components/vol-surface-chart/vol-surface-chart";

export function Example({
  width = 880,
  presentation,
}: ExampleProps & {
  presentation?: Pick<
    import("react").ComponentProps<typeof VolSurfaceChart>,
    "title" | "subtitle" | "onSelect" | "renderTooltipBody" | "figureRefs"
  >;
}) {
  return (
    <VolSurfaceChart
      {...presentation}
      surface={{
        id: "SPX implied volatility",
        expiries: [0.25, 0.5, 1, 2, 5],
        strikes: [4000, 4250, 4500, 4750, 5000],
        secondary_axis: "strike",
        quote_type: "black_lognormal",
        interpolation_mode: "total_variance",
        vols_row_major: [
          0.32, 0.28, 0.24, 0.25, 0.27, 0.3, 0.265, 0.23, 0.24, 0.26, 0.285,
          0.25, 0.22, 0.23, 0.25, 0.27, 0.24, 0.215, 0.225, 0.245, 0.255, 0.23,
          0.21, 0.22, 0.24,
        ],
      }}
      colorDomain={[0.2, 0.33]}
      width={width}
    />
  );
}
