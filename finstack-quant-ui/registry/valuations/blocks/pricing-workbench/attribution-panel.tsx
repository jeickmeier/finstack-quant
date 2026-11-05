"use client";
import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { PnlAttributionReport } from "@/components/finstack/attribution/components/pnl-attribution-report/pnl-attribution-report";
import {
  EnumField,
  type EnumOption,
} from "@/components/finstack/shared/primitives/enum-field/enum-field";
import {
  useAttributePnlMany,
  type AttributePnlManyRequest,
} from "@/hooks/attribution/use-attribute-pnl-many/use-attribute-pnl-many";
import { useDefaultWaterfallOrder } from "@/hooks/attribution/use-default-waterfall-order/use-default-waterfall-order";
import type { PriceRequest } from "@/hooks/valuations/use-price-instrument/use-price-instrument";
import type { MarketScenarioSession } from "./market-scenario-panel";

/** Attribution methods offered here; Rust requires one, so none is preselected. */
export type AttributionMethodChoice =
  "" | "parallel" | "waterfall" | "metrics_based" | "taylor";
/** Where the T1 market and date come from. */
export type AttributionT1Source = "priced" | "scenario";
type Method = AttributePnlManyRequest["inputs"]["method"];
type WaterfallOrder = Extract<Method, { waterfall: unknown }>["waterfall"];

/** The AttributionMethod wire value of a choice: waterfall in Rust's default order, Taylor with its config left for Rust to fill; null until both exist. */
function attributionMethod(
  choice: AttributionMethodChoice,
  order: readonly string[] | undefined,
): Method | null {
  if (choice === "") return null;
  if (choice === "waterfall")
    return order ? { waterfall: [...order] as WaterfallOrder } : null;
  return choice === "taylor" ? { taylor: {} } : choice;
}

/** A market snapshot and its valuation date. */
interface MarketPoint {
  readonly marketJson: string;
  readonly asOf: string;
}

/**
 * P&L explain state for one workbench: a pinned T0 market and date, the T1
 * source (the last priced request or the applied market scenario) and the
 * method. Rust attributes the last priced instrument between them; nothing
 * is differenced here.
 */
export function useAttributionSession(
  base: PriceRequest | null,
  scenario: MarketScenarioSession,
) {
  const [pinned, setPinned] = useState<MarketPoint | null>(null);
  // Loading another example drops the completed base, and with it the pin.
  useEffect(() => {
    if (base === null) setPinned(null);
  }, [base]);
  const [t1Source, setT1Source] = useState<AttributionT1Source>("priced");
  const [choice, setChoice] = useState<AttributionMethodChoice>("");
  const order = useDefaultWaterfallOrder();
  const applied = scenario.application.data;
  const t1: MarketPoint | null =
    t1Source === "scenario"
      ? scenario.applied !== null && applied
        ? { marketJson: applied.marketJson, asOf: applied.asOf }
        : null
      : base && { marketJson: base.marketJson, asOf: base.asOf };
  const method = attributionMethod(choice, order.data);
  const request: AttributePnlManyRequest | null =
    pinned && base && t1 && method
      ? {
          instrumentJsons: [base.instrumentJson],
          marketT0Json: pinned.marketJson,
          marketT1Json: t1.marketJson,
          inputs: { as_of_t0: pinned.asOf, as_of_t1: t1.asOf, method },
        }
      : null;
  const attribution = useAttributePnlMany(request);
  return {
    base,
    pinned,
    pin: () =>
      setPinned(base && { marketJson: base.marketJson, asOf: base.asOf }),
    clearPin: () => setPinned(null),
    t1Source,
    setT1Source,
    t1,
    scenarioAvailable: scenario.applied !== null && applied !== undefined,
    choice,
    setChoice,
    order,
    request,
    attribution,
  };
}
export type AttributionSession = ReturnType<typeof useAttributionSession>;

/** Results tab: pin T0, choose T1 and the method, and show Rust's attribution. */
export function AttributionResults({
  session,
  density,
}: {
  session: AttributionSession;
  density?: "compact" | "comfortable";
}) {
  const { pinned, t1, order, attribution } = session;
  const methods: EnumOption[] = [
    { value: "parallel", label: "parallel" },
    {
      value: "waterfall",
      label: "waterfall",
      description: order.data
        ? `Rust's default order: ${order.data.join(", ")}`
        : undefined,
      disabled: !order.data,
    },
    { value: "metrics_based", label: "metrics_based" },
    { value: "taylor", label: "taylor" },
  ];
  const result = attribution.data?.[0];
  return (
    <div className="flex flex-col gap-3">
      <p className="text-xs text-muted-foreground">
        Rust attributes the last priced instrument&apos;s P&amp;L from the
        pinned T0 market and date to the T1 market and date, repricing that
        instrument at both. Pinning keeps only the market and the valuation
        date, so a changed instrument is repriced at T0 too.
      </p>
      <div className="flex flex-wrap items-center gap-2">
        <Button
          type="button"
          size="sm"
          disabled={session.base === null}
          onClick={session.pin}
        >
          Pin as T0
        </Button>
        <Button
          type="button"
          size="sm"
          variant="outline"
          disabled={pinned === null}
          onClick={session.clearPin}
        >
          Clear T0
        </Button>
        <span className="text-xs text-muted-foreground">
          {pinned ? (
            <>
              T0 pinned at <span className="font-mono">{pinned.asOf}</span>
            </>
          ) : session.base ? (
            "No T0 pinned."
          ) : (
            "Price the instrument to pin T0."
          )}
        </span>
      </div>
      <div className="flex flex-wrap items-start gap-4">
        <EnumField
          label="T1 market"
          value={session.t1Source}
          onValueChange={(source) =>
            session.setT1Source(source as AttributionT1Source)
          }
          options={[
            {
              value: "priced",
              label: "Last priced market",
              description: "The market and date of the last priced request",
            },
            {
              value: "scenario",
              label: "Applied market scenario",
              description:
                "The shocked market and returned date of the applied market scenario",
              disabled: !session.scenarioAvailable,
            },
          ]}
          layout="radio"
        />
        <EnumField
          label="Attribution method"
          value={session.choice}
          onValueChange={(choice) =>
            session.setChoice(choice as AttributionMethodChoice)
          }
          options={methods}
          layout="radio"
          error={order.error?.message}
        />
      </div>
      {t1 && (
        <p className="text-xs text-muted-foreground">
          T1 at <span className="font-mono">{t1.asOf}</span>
        </p>
      )}
      {session.request === null ? (
        <p role="status" className="text-muted-foreground">
          {pinned === null
            ? "Pin T0, then reprice on another market or apply a market scenario."
            : t1 === null
              ? "Apply a market scenario to use it as T1."
              : "Choose an attribution method."}
        </p>
      ) : (
        <PnlAttributionReport
          attribution={result?.attribution}
          buckets={result?.buckets}
          explain={result?.explain}
          residualWithinTolerance={result?.residualWithinTolerance}
          loading={attribution.isFetching}
          error={attribution.error?.message}
          density={density}
        />
      )}
    </div>
  );
}
