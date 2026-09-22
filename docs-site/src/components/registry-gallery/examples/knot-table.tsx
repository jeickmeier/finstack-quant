"use client";
import type { ExampleProps } from "./props";
import { useState } from "react";
import {
  KnotTable,
  type KnotEdit,
} from "@/components/finstack/core/primitives/knot-table/knot-table";

export function Example(_props: ExampleProps) {
  const [knots, setKnots] = useState<KnotEdit[]>([
    [1, 0.99],
    [2, 0.95],
  ]);
  return (
    <KnotTable value={knots} rowIds={["a", "b"]} onValueChange={setKnots} />
  );
}
