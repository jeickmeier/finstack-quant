"use client";
import { JsonViewer } from "../../primitives/json-viewer/json-viewer";
import { serializeHost } from "@/lib/finstack/codec.mjs";
/** Optional original trace; the valuation host declaration leaves its shape unknown. */
export function ExplanationTrace({
  value,
  density,
}: {
  value: unknown;
  density?: "compact" | "comfortable";
}) {
  if (value == null) return null;
  return (
    <details>
      <summary className="text-sm">Explanation</summary>
      <JsonViewer
        label="Explanation JSON"
        text={serializeHost(value)}
        density={density}
      />
    </details>
  );
}
