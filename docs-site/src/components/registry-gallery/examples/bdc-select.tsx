"use client";
import type { ExampleProps } from "./props";
import { useState } from "react";
import { BdcSelect } from "@/components/finstack/core/primitives/bdc-select/bdc-select";

export function Example(_props: ExampleProps) {
  const [bdc, setBdc] = useState("following");
  return (
    <BdcSelect
      label="Business day convention"
      value={bdc}
      onValueChange={setBdc}
    />
  );
}
