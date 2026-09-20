"use client";
import type { ExampleProps } from "./props";
import { VolCubeExplorer } from "@/components/finstack/components/vol-cube-explorer/vol-cube-explorer";
import type { MarketContextStateWire } from "@/lib/finstack/generated/types/market_context_state";
import data from "../data.json";
const cubes = data.cubes.market as unknown as MarketContextStateWire;
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
      colorDomains={{ normal: [0, 0.1], black_lognormal: [0, 2] }}
      width={width}
      height={460}
    />
  );
}
