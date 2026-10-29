"use client";
import type { ExampleProps } from "./props";
import { VolCubeSlice } from "@/components/finstack/models/components/vol-cube-slice/vol-cube-slice";
import type { MarketContextStateWire } from "@/lib/finstack/generated/types/market_context_state";
import data from "../data.json";
const cubes = data.cubes.market as unknown as MarketContextStateWire;
export function Example({ width = 880 }: ExampleProps) {
  return (
    <VolCubeSlice
      cube={cubes.vol_cubes.find((cube) => cube.id !== "SHIFTED-BLACK")!}
      initialStrikes={[0.04, 0.045, 0.05, 0.055, 0.06]}
      initialConvention="normal"
      initialRate={0.04}
      repairIterations={10}
      colorDomains={{ normal: [0.003, 0.012], black_lognormal: [0.1, 0.7] }}
      width={width}
      height={360}
    />
  );
}
