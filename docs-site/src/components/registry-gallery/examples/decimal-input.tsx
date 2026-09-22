"use client";
import type { ExampleProps } from "./props";
import { useState } from "react";
import { DecimalInput } from "@/components/finstack/core/primitives/decimal-input/decimal-input";

export function Example(_props: ExampleProps) {
  const [text, setText] = useState("12345678901234567890.1234");
  return (
    <DecimalInput label="Exact decimal" value={text} onValueChange={setText} />
  );
}
