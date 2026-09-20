"use client";
import type { MarketContextStateWire } from "@/lib/finstack/generated/types/market_context_state";
import { serializeHost } from "@/lib/finstack/codec.mjs";
import { FinstackTable } from "../../primitives/finstack-table/finstack-table";
import { JsonViewer } from "../../primitives/json-viewer/json-viewer";
export type FxState = NonNullable<MarketContextStateWire["fx"]>;
/** Currency labels come only from supplied tuples; their ordering does not resolve source priority. */
export function fxCurrencies(state: FxState) {
  return [
    ...new Set(
      [
        ...state.quotes,
        ...state.provider_quotes,
        ...state.pinned_quotes,
      ].flatMap((quote) => [quote[0], quote[1]]),
    ),
  ];
}
/** Exact directed entries. Empty/missing cells never become inverse, cross or unit rates. */
export function FxMatrixGrid({
  state,
}: {
  state: MarketContextStateWire["fx"];
}) {
  if (state == null) return <p>FX matrix unavailable</p>;
  const currencies = fxCurrencies(state);
  return (
    <section
      aria-label="Stored FX matrices"
      className="space-y-4 font-sans text-sm text-foreground"
    >
      {(["quotes", "provider_quotes", "pinned_quotes"] as const).map(
        (source) => (
          <FinstackTable
            key={source}
            data={currencies.map((from) => ({ from }))}
            getRowId={(row) => row.from}
            caption={source}
            columns={[
              {
                id: "from",
                header: "From / To",
                accessorFn: (row) => row.from,
              },
              ...currencies.map((to) => ({
                id: to,
                header: to,
                accessorFn: (row: { from: string }) => {
                  const matches = state[source].filter(
                    (q) => q[0] === row.from && q[1] === to,
                  );
                  return matches.length
                    ? matches
                        .map((q) =>
                          q.length === 5
                            ? `${String(q[4])} · ${q[2]} · ${q[3]}`
                            : `${String(q[2])} · Undated`,
                        )
                        .join("; ")
                    : "Unavailable";
                },
              })),
            ]}
            emptyState="No stored currencies supplied"
          />
        ),
      )}
      <JsonViewer label="FX configuration" text={serializeHost(state.config)} />
      <JsonViewer
        label="Complete stored FX state"
        text={serializeHost(state)}
      />
    </section>
  );
}
