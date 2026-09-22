"use client";
import type { ExampleProps } from "./props";
import { useState } from "react";
import {
  TenorInput,
  type TenorEdit,
} from "@/components/finstack/core/primitives/tenor-input/tenor-input";

export function Example(_props: ExampleProps) {
  const [tenor, setTenor] = useState<TenorEdit>({ count: 6, unit: "months" });
  return <TenorInput label="Tenor" value={tenor} onValueChange={setTenor} />;
}
