"use client";
import type { ExampleProps } from "./props";
import { JsonViewer } from "@/components/finstack/primitives/json-viewer/json-viewer";

export function Example({ density = "compact" }: ExampleProps) {
  return (
    <JsonViewer
      label="Original JSON"
      text={'{"seed":18446744073709551615,"amount":"1000000.123456789"}'}
      density={density}
    />
  );
}
