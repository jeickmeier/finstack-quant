"use client";
import type { ExampleProps } from "./props";
import { FinstackChart } from "@/components/finstack/shared/chart/finstack-chart/finstack-chart";
import { figureExample } from "@/components/finstack/shared/chart/figure-example/figure-data";

export function Example({ width = 880, publication = false }: ExampleProps) {
  return (
    <>
      <FinstackChart
        {...figureExample}
        width={width}
        height={publication ? 760 : 560}
      />
    </>
  );
}
