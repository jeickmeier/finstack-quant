"use client";
import { JsonViewer } from "@/components/finstack/shared/primitives/json-viewer/json-viewer";
import { serializeHost } from "@/lib/finstack/codec.mjs";
/** Optional original valuation covenant payload; no verdict or headroom is computed. */
export function CovenantReport({
  value,
  density,
}: {
  value: unknown;
  density?: "compact" | "comfortable";
}) {
  if (value == null) return null;
  return (
    <details>
      <summary className="text-sm">Covenants</summary>
      <JsonViewer
        label="Covenant reports JSON"
        text={serializeHost(value)}
        density={density}
      />
    </details>
  );
}
