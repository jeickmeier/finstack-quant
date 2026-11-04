"use client";
import type { ExampleProps } from "./props";
import { VolCubeExplorer } from "@/components/finstack/models/components/vol-cube-explorer/vol-cube-explorer";
import { cubesMarket } from "../fixtures";
const cubes = cubesMarket;
export function Example({
  width = 880,
  variant = "default",
  presentation,
}: ExampleProps & {
  presentation?: Pick<
    import("react").ComponentProps<typeof VolCubeExplorer>,
    "title" | "subtitle" | "onSelect" | "renderTooltipBody" | "figureRefs"
  >;
}) {
  return (
    <VolCubeExplorer
      {...presentation}
      cube={cubes.vol_cubes.find((cube) =>
        variant === "shifted"
          ? cube.id === "SHIFTED-BLACK"
          : cube.id !== "SHIFTED-BLACK",
      )!}
      initialStrike={variant === "shifted" ? -0.005 : 0.05}
      initialConvention={variant === "shifted" ? "black_lognormal" : "normal"}
      colorDomains={{ normal: [0.003, 0.012], black_lognormal: [0.1, 0.7] }}
      slice={{
        initialStrikes:
          variant === "shifted"
            ? [-0.01, -0.005, 0, 0.005, 0.01, 0.02]
            : [0.04, 0.045, 0.05, 0.055, 0.06],
        initialRate: 0.04,
        repairIterations: 10,
      }}
      width={width}
      height={460}
    />
  );
}
