"use client";
import { Button } from "@/components/ui/button";
import { useEffect, useState } from "react";
import { instruments } from "@/lib/finstack/generated/instruments";
import { InstrumentSelector } from "./instrument-selector";
import { JsonViewer } from "@/components/finstack/shared/primitives/json-viewer/json-viewer";
import {
  SchemaForm,
  type InstrumentModule,
  type PromotedTerm,
  type SchemaFormProps,
} from "@/components/finstack/core/components/schema-form/schema-form";

const marketQuotePath =
  "instrument.spec.instrument_pricing_overrides.market_quotes";

/** Only inputs consumed by the corresponding native pricer are promoted. */
const promotedTerms: Record<string, PromotedTerm> = {
  bond: {
    path: `${marketQuotePath}.quoted_clean_price`,
    label: "Bond clean price",
    unit: "% of par",
    hint: "99.5 = 99.5% of par; overrides model PV.",
    emptySource: "Model price",
    kind: "market-quote",
    exclusiveWith: [
      "quoted_dirty_price_currency",
      "quoted_ytm",
      "quoted_ytw",
      "quoted_z_spread",
      "quoted_oas",
      "quoted_discount_margin",
      "quoted_i_spread",
      "quoted_asw_market",
      "quoted_japanese_simple_yield",
    ],
  },
  equity_option: {
    path: `${marketQuotePath}.implied_volatility`,
    label: "Implied volatility",
    unit: "decimal σ",
    hint: "0.20 = 20%; flat volatility replaces the surface.",
    emptySource: "Volatility surface",
    kind: "market-quote",
  },
  commodity_option: {
    path: `${marketQuotePath}.implied_volatility`,
    label: "Implied volatility",
    unit: "decimal σ",
    hint: "0.20 = 20%; flat volatility replaces the surface.",
    emptySource: "Volatility surface",
    kind: "market-quote",
  },
  fx_option: {
    path: `${marketQuotePath}.implied_volatility`,
    label: "Implied volatility",
    unit: "decimal σ",
    hint: "0.20 = 20%; flat volatility replaces the surface.",
    emptySource: "Volatility surface",
    kind: "market-quote",
  },
  credit_default_swap: {
    path: "instrument.spec.premium.spread_bp",
    label: "Running spread",
    unit: "bp",
    hint: "Contract premium; par spread appears in Results.",
    kind: "contract-term",
  },
  interest_rate_swap: {
    path: "instrument.spec.fixed.rate",
    label: "Fixed coupon",
    unit: "decimal rate",
    hint: "0.04 = 4%; par rate appears in Results.",
    kind: "contract-term",
  },
  fx_swap: {
    path: "instrument.spec.far_rate",
    label: "Far FX rate",
    unit: "quote / base",
    hint: "Far-leg outright rate, quote per base.",
    emptySource: "Forward curves",
    kind: "nullable-term",
  },
  xccy_swap: {
    path: "instrument.spec.leg2.spread_bp",
    label: "Leg 2 spread",
    unit: "bp",
    hint: "Contract spread on the second leg.",
    kind: "contract-term",
    morePricingOverrides: false,
  },
  structured_credit: {
    path: "instrument.spec.tranches.tranches[0].coupon.fixed.rate",
    label: "Tranche coupon",
    unit: "decimal rate",
    hint: "0.06 = 6%; fixed coupon for this tranche.",
    kind: "single-fixed-tranche",
    morePricingOverrides: false,
  },
};
/** Independently installed all-instrument editor; generated modules convert only when selected. */
export function InstrumentForm(props: {
  type: string;
  onTypeChange(type: string): void;
  allowedTypes?: readonly string[];
  /** Whether the generated catalogue example can replace the supplied document. */
  showGeneratedExample?: boolean;
  /** Initial canonical JSON. Remount the component to load another document; active edits are retained. */
  defaultJson?: string;
  validate: SchemaFormProps["validate"];
  onSubmit: SchemaFormProps["onSubmit"];
  /** Native canonical result, or null as soon as an edit invalidates the prior validation. */
  onValidated?: (json: string | null) => void;
  layout?: SchemaFormProps["layout"];
  submitVariant?: SchemaFormProps["submitVariant"];
}) {
  const [loaded, setLoaded] = useState<{
    type: string;
    module: InstrumentModule;
  } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [example, setExample] = useState(0);
  const [initial] = useState({ type: props.type, json: props.defaultJson });
  useEffect(() => {
    let active = true;
    setError(null);
    const entry = instruments.find((entry) => entry.type === props.type);
    if (entry) {
      void entry.loader().then(
        (module) => {
          if (active) setLoaded({ type: entry.type, module });
        },
        (error) => {
          if (active) setError(String(error));
        },
      );
    }
    return () => {
      active = false;
    };
  }, [props.type]);
  const module = loaded?.type === props.type ? loaded.module : null;
  let defaults: Record<string, unknown> | undefined;
  let parseError: string | undefined;
  if (module && initial.type === props.type && initial.json && example === 0) {
    try {
      defaults = module.codec.parse(initial.json) as Record<string, unknown>;
    } catch (error) {
      parseError = error instanceof Error ? error.message : String(error);
    }
  }
  return (
    <section
      aria-label="Instrument form"
      className="space-y-3 font-sans text-foreground"
    >
      <header className="finstack-instrument-header">
        <InstrumentSelector
          value={props.type}
          allowedTypes={props.allowedTypes}
          onValueChange={(type) => {
            props.onValidated?.(null);
            props.onTypeChange(type);
          }}
        />
        {module && props.showGeneratedExample !== false && (
          <Button
            variant="outline"
            size="sm"
            type="button"

            onClick={() => {
              props.onValidated?.(null);
              setExample((n) => n + 1);
            }}
          >
            Load example
          </Button>
        )}
      </header>
      {!instruments.some((entry) => entry.type === props.type) ? (
        <p role="alert" className="text-sm text-error">
          Unknown instrument type: {props.type}
        </p>
      ) : (
        <>
          {error && (
            <p role="alert" className="text-sm text-error">
              {error}
            </p>
          )}
          {!module && !error && <p role="status">Loading instrument schema…</p>}
          {module && (
            <>
              {parseError ? (
                <p role="alert" className="text-sm text-error">
                  {parseError}
                </p>
              ) : (
                <SchemaForm
                  key={`${props.type}:${example}`}
                  module={module}
                  defaultValues={defaults}
                  validate={props.validate}
                  onSubmit={props.onSubmit}
                  onValidated={props.onValidated}
                  layout={props.layout}
                  submitVariant={props.submitVariant}
                  promotedTerm={promotedTerms[props.type]}
                />
              )}
              {props.showGeneratedExample !== false && (
                <details>
                  <summary className="text-sm">Example JSON</summary>
                  <JsonViewer
                    label="Generated example JSON"
                    text={module.codec.stringify(module.example)}
                  />
                </details>
              )}
            </>
          )}
        </>
      )}
    </section>
  );
}
