"use client";
import type { ExampleProps } from "./props";
import { FinstackChart } from "@/components/finstack/primitives/finstack-chart/finstack-chart";
import { figureExample } from "@/components/finstack/components/figure-example/figure-data";

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
