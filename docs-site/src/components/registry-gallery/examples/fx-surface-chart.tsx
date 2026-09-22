"use client";
import type { ExampleProps } from "./props";
import { FxSurfaceChart } from "@/components/finstack/core/components/fx-surface-chart/fx-surface-chart";
const fx = {
  id: "EURUSD",
  expiries: [0.5, 1],
  atm_vols: [0.08, 0.09],
  rr_25d: [0.01, 0.012],
  bf_25d: [0.005, 0.006],
  rr_10d: null,
  bf_10d: null,
};
export function Example({
  width = 880,
  presentation,
}: ExampleProps & {
  presentation?: Pick<
    import("react").ComponentProps<typeof FxSurfaceChart>,
    "title" | "subtitle" | "onSelect" | "renderTooltipBody" | "figureRefs"
  >;
}) {
  return (
    <FxSurfaceChart
      {...presentation}
      surface={fx}
      coordinates={[0.5, 1].flatMap((expiry) =>
        [1, 1.1, 1.2].map((strike) => ({ expiry, strike, forward: 1.12 })),
      )}
      colorDomain={[0, 1]}
      width={width}
      height={460}
    />
  );
}
