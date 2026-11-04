"use client";
import { SurfaceArbitrageBadge } from "@/components/finstack/models/components/surface-arbitrage-badge/surface-arbitrage-badge";
import type { ExampleProps } from "./props";
import { calibration } from "../fixtures";
/** AAPL-EQUITY-VOL, calibrated from market_bootstrap/08_equity_vol_surface.json. */
const surface = calibration
  .find((entry) => entry.source.includes("08_equity"))!
  .result.result.final_market.surfaces.find(
    (entry) => entry.id === "AAPL-EQUITY-VOL",
  )!;
/** The 0.5y at-the-money node raised to 0.40, which breaks convexity and calendar monotonicity. */
const bumped = {
  ...surface,
  vols_row_major: surface.vols_row_major.map((vol, index) =>
    index === 4 ? 0.4 : vol,
  ),
};
/** The example's AAPL spot price, supplied by the host as one forward broadcast to every expiry. */
const forwards = [175];
export function Example({ density = "compact" }: ExampleProps) {
  return (
    <ul className="grid gap-3">
      <li className="grid gap-1">
        <span className="text-xs text-muted-foreground">
          Calibrated surface, host forward 175
        </span>
        <SurfaceArbitrageBadge
          surface={surface}
          forwards={forwards}
          density={density}
        />
      </li>
      <li className="grid gap-1">
        <span className="text-xs text-muted-foreground">
          0.5y at-the-money volatility raised to 0.40
        </span>
        <SurfaceArbitrageBadge
          surface={bumped}
          forwards={forwards}
          density={density}
        />
      </li>
      <li className="grid gap-1">
        <span className="text-xs text-muted-foreground">
          The same nodes stored as normal quotes
        </span>
        <SurfaceArbitrageBadge
          surface={{ ...surface, quote_type: "normal" }}
          forwards={forwards}
          density={density}
        />
      </li>
      <li className="grid gap-1">
        <span className="text-xs text-muted-foreground">
          No forwards from the host
        </span>
        <SurfaceArbitrageBadge surface={surface} density={density} />
      </li>
    </ul>
  );
}
