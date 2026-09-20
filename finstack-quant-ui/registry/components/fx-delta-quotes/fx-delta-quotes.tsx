"use client";
import { serializeHost } from "@/lib/finstack/codec.mjs";
import type { MarketContextStateWire } from "@/lib/finstack/generated/types/market_context_state";
import { JsonViewer } from "../../primitives/json-viewer/json-viewer";
import { FinstackTable } from "../../primitives/finstack-table/finstack-table";
/** Raw canonical ATM/RR/BF quotes. No forward or delta-to-strike conversion is inferred. */
export function FxDeltaQuotes({
  surface,
}: {
  surface: MarketContextStateWire["fx_delta_vol_surfaces"][number];
}) {
  const columns = ["atm_vols", "rr_25d", "bf_25d", "rr_10d", "bf_10d"] as const;
  return (
    <section
      aria-label={`${surface.id} raw FX quotes`}
      className="space-y-3 font-sans text-sm text-foreground"
    >
      <h2>{surface.id} — raw FX delta quotes</h2>
      <p>
        ATM delta-neutral straddle, risk reversal and butterfly values remain in
        supplied quote units. No strike grid is inferred.
      </p>
      <FinstackTable
        data={surface.expiries.map((expiry, index) => ({ expiry, index }))}
        getRowId={(row) => String(row.index)}
        caption="Stored FX quote arrays"
        columns={[
          {
            id: "expiry",
            accessorFn: (row) => row.expiry,
            header: "Expiry (years)",
          },
          ...columns.map((key) => ({
            id: key,
            header: key,
            accessorFn: (row: { index: number }) =>
              surface[key]?.[row.index] ?? "Not supplied",
          })),
        ]}
      />
      <JsonViewer
        text={serializeHost(surface)}
        label="Complete FX quote state"
      />
    </section>
  );
}
