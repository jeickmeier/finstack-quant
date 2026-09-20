"use client";
import { JsonViewer } from "../../primitives/json-viewer/json-viewer";
/** Raw payload from the host adapter; no financial shape is inferred. */
export function RawDetails({
  text,
  label,
  density,
}: {
  text: string;
  label: string;
  density?: "compact" | "comfortable";
}) {
  return <JsonViewer label={label} text={text} density={density} />;
}
