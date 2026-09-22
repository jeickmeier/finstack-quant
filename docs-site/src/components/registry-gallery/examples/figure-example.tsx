"use client";
import type { ExampleProps } from "./props";
import { LinkedFigureExample } from "@/components/finstack/shared/chart/figure-example/linked-figures";
import { FigureExample } from "@/components/finstack/shared/chart/figure-example/figure-example";

export function Example({ variant = "default" }: ExampleProps) {
  return variant === "interactive" ? (
    <LinkedFigureExample />
  ) : (
    <FigureExample />
  );
}
