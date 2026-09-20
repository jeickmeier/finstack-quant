"use client";
import type { ExampleProps } from "./props";
import { useState } from "react";
import { FieldFrame } from "@/components/finstack/primitives/field-frame/field-frame";

export function Example(_props: ExampleProps) {
  const [text, setText] = useState("12345678901234567890.1234");
  return (
    <FieldFrame
      label="Reference"
      help="Caller-supplied reference. Escape dismisses this help."
    >
      {(control) => (
        <input
          {...control}
          value={text}
          onChange={(event) => setText(event.target.value)}
          className="finstack-field border border-border bg-background px-2"
        />
      )}
    </FieldFrame>
  );
}
