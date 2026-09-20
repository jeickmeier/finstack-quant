"use client";
import type { ExampleProps } from "./props";
import { StampBadge } from "@/components/finstack/primitives/stamp-badge/stamp-badge";
import data from "../data.json";

export function Example(_props: ExampleProps) {
  return <StampBadge meta={data.bond.result.meta} />;
}
