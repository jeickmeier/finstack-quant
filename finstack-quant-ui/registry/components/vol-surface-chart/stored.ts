import type { MarketContextStateWire } from "@/lib/finstack/generated/types/market_context_state";
export type StoredSurface = MarketContextStateWire["surfaces"][number];
export interface SurfaceNode {
  surface: StoredSurface;
  expiry: number;
  secondary: number;
  value: number;
  key: string;
}
/** Exact row-major lookup; this adapter never interpolates or converts volatility. */
export function surfaceNodes(surface: StoredSurface): SurfaceNode[] {
  if (
    surface.vols_row_major.length !==
    surface.expiries.length * surface.strikes.length
  )
    throw new TypeError(
      "Stored surface dimensions do not match its row-major values",
    );
  return surface.expiries.flatMap((expiry, row) =>
    surface.strikes.map((secondary, column) => ({
      surface,
      expiry,
      secondary,
      value: surface.vols_row_major[row * surface.strikes.length + column]!,
      key: JSON.stringify([surface.id, expiry, secondary]),
    })),
  );
}
/** Convention labels come only from the canonical metadata, never from an identifier. */
export function surfaceLabels(surface: StoredSurface) {
  return {
    expiry: "Expiry (years)",
    secondary: surface.secondary_axis === "tenor" ? "Tenor (years)" : "Strike",
    value:
      surface.quote_type === "normal"
        ? "Normal volatility (raw)"
        : "Black/lognormal volatility (raw)",
  };
}
/** Selected stored row and column retain the exact same node references. */
export function surfaceSlices(
  nodes: readonly SurfaceNode[],
  key: string | null,
) {
  const selected = nodes.find((node) => node.key === key);
  return {
    selected,
    row: selected
      ? nodes.filter((node) => node.expiry === selected.expiry)
      : [],
    column: selected
      ? nodes.filter((node) => node.secondary === selected.secondary)
      : [],
  };
}
