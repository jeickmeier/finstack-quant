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
        id: "Stored observations",
        expiries: [0.5, 1, 2],
        strikes: [80, 100, 120],
        secondary_axis: "strike",
        quote_type: "black_lognormal",
        interpolation_mode: "total_variance",
        vols_row_major: [0.21, 0.2, 0.23, 0.22, 0.24, 0.26, 0.25, 0.27, 0.28],
      }}
      colorDomain={[0.15, 0.3]}
      width={width}
    />
  );
}
