"use client";
import type { ExampleProps } from "./props";
import { FxSurfaceChart } from "@/components/finstack/models/components/fx-surface-chart/fx-surface-chart";
const fx = {
  id: "EURUSD",
  expiries: [0.25, 0.5, 1, 2],
  atm_vols: [0.092, 0.09, 0.088, 0.09],
  rr_25d: [0.007, 0.008, 0.009, 0.01],
  bf_25d: [0.0035, 0.004, 0.0045, 0.005],
  rr_10d: [0.014, 0.016, 0.018, 0.02],
  bf_10d: [0.007, 0.008, 0.009, 0.01],
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
      coordinates={[
        { expiry: 0.25, forward: 1.102 },
        { expiry: 0.5, forward: 1.104 },
        { expiry: 1, forward: 1.108 },
        { expiry: 2, forward: 1.116 },
      ].flatMap(({ expiry, forward }) =>
        [1.05, 1.1, 1.15].map((strike) => ({ expiry, strike, forward })),
      )}
      colorDomain={[0.07, 0.13]}
      width={width}
      height={460}
    />
  );
}
