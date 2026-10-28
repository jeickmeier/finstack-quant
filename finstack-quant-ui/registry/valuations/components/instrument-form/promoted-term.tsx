"use client";
import { useId } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { errorText } from "@/lib/finstack/form";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { objectValue } from "@/components/finstack/core/components/schema-form/schema-walk";
import type {
  SchemaFormApi,
  SchemaFormProps,
  SheetControls,
} from "@/components/finstack/core/components/schema-form/schema-form";
import { INSTRUMENT_SHEET, PRICING_OVERRIDES } from "./instrument-sheet";

/** One desk-critical native input shown before the instrument term sheet. */
export interface PromotedTerm {
  path: string;
  label: string;
  unit: string;
  hint: string;
  kind:
    "market-quote" | "nullable-term" | "contract-term" | "single-fixed-tranche";
  /** Describes the source used when an optional quote is absent. */
  emptySource?: string;
  /** Other mutually exclusive market quote keys, cleared when this quote is entered. */
  exclusiveWith?: readonly string[];
  /** Set false for instruments without a secondary pricing disclosure. */
  morePricingOverrides?: boolean;
}

const marketQuotePath =
  "instrument.spec.instrument_pricing_overrides.market_quotes";

/** Only inputs consumed by the corresponding native pricer are promoted. */
export const promotedTerms: Record<string, PromotedTerm> = {
  bond: {
    path: `${marketQuotePath}.quoted_clean_price_pct`,
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
  commodity_spot_option: {
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
    path: "instrument.spec.premium_leg.coupon_bp",
    label: "Running coupon",
    unit: "bp",
    hint: "Contract premium; par spread appears in Results.",
    kind: "contract-term",
  },
  interest_rate_swap: {
    path: "instrument.spec.fixed_leg.rate",
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
    path: "instrument.spec.leg2.leg.spread_bp",
    label: "Leg 2 spread",
    unit: "bp",
    hint: "Contract spread on the second leg.",
    kind: "contract-term",
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

function atPath(value: unknown, path: string): unknown {
  return path
    .split(".")
    .reduce<unknown>((node, key) => objectValue(node)[key], value);
}

function hasValue(value: unknown): boolean {
  return value !== undefined && value !== null && String(value) !== "";
}

function singleFixedTranche(values: Record<string, unknown>) {
  const tranches = atPath(values, "instrument.spec.tranches.tranches");
  if (!Array.isArray(tranches) || tranches.length !== 1) return null;
  const tranche = objectValue(tranches[0]);
  const coupon = objectValue(tranche.coupon);
  if (
    Object.keys(coupon).length !== 1 ||
    !hasValue(objectValue(coupon.fixed).rate)
  )
    return null;
  return tranche;
}

/** Update the optional quote branch as one form edit so omitted wrappers stay omitted. */
function setMarketQuote(
  form: SchemaFormApi,
  values: Record<string, unknown>,
  term: PromotedTerm,
  text: string,
) {
  const spec = objectValue(atPath(values, "instrument.spec"));
  const overrides = {
    ...objectValue(spec.instrument_pricing_overrides),
  };
  const quotes = {
    ...objectValue(overrides.market_quotes),
  };
  const key = term.path.split(".").at(-1)!;
  if (text === "") {
    delete quotes[key];
  } else {
    for (const other of term.exclusiveWith ?? []) delete quotes[other];
    quotes[key] = text;
  }
  // Nullable quote siblings have no effect when absent; keep only supplied values.
  for (const [name, value] of Object.entries(quotes))
    if (value === null || value === undefined) delete quotes[name];
  if (Object.keys(quotes).length) overrides.market_quotes = quotes;
  else delete overrides.market_quotes;
  form.setFieldValue(
    "instrument.spec.instrument_pricing_overrides",
    Object.keys(overrides).length ? overrides : undefined,
  );
}

function PromotedTermField({
  form,
  values,
  term,
  reveal,
}: SheetControls & { term: PromotedTerm }) {
  const id = useId();
  const optional =
    term.kind === "market-quote" || term.kind === "nullable-term";
  const dealTerm = !optional;
  const trancheId =
    term.kind === "single-fixed-tranche"
      ? singleFixedTranche(values)?.id
      : undefined;
  const quotes = objectValue(
    atPath(
      values,
      "instrument.spec.instrument_pricing_overrides.market_quotes",
    ),
  );
  const competing = (term.exclusiveWith ?? []).filter((key) =>
    hasValue(quotes[key]),
  );
  const setValue = (text: string) => {
    if (term.kind === "market-quote") {
      setMarketQuote(form, values, term, text);
    } else {
      form.setFieldValue(term.path, text === "" && optional ? null : text);
    }
  };
  return (
    <section
      className="finstack-quote-ticket"
      aria-label={dealTerm ? "Deal rate" : "Desk quote"}
    >
      <header className="finstack-quote-ticket__header">
        <span className="finstack-quote-ticket__eyebrow">
          {dealTerm ? "Deal rate" : "Desk quote"}
        </span>
        <span className="finstack-quote-ticket__category">
          {dealTerm ? "Deal term" : "Market override"}
        </span>
      </header>
      <form.AppField name={term.path}>
        {(field) => {
          const error = errorText(field.state.meta.errors);
          const active = hasValue(field.state.value);
          const source = active
            ? dealTerm
              ? "Contract input"
              : "Override active"
            : (term.emptySource ?? "Not set");
          return (
            <div
              className="finstack-quote-ticket__body"
              data-field-path={term.path}
              data-active={active || undefined}
              data-invalid={Boolean(error) || undefined}
            >
              <div className="finstack-quote-ticket__description">
                <div className="finstack-quote-ticket__label-row">
                  <label htmlFor={id}>{term.label}</label>
                  {typeof trancheId === "string" && (
                    <span className="finstack-quote-ticket__tranche-id">
                      {trancheId}
                    </span>
                  )}
                </div>
                <p id={`${id}-hint`}>{term.hint}</p>
                {competing.length > 0 && (
                  <p className="finstack-quote-ticket__replacement">
                    Entering a clean price replaces{" "}
                    {competing.map(labelFor).join(", ")}.
                  </p>
                )}
              </div>
              <div className="finstack-quote-ticket__editor">
                <div className="finstack-quote-ticket__entry">
                  <div className="finstack-quote-ticket__input-wrap">
                    <Input
                      id={id}
                      type="text"
                      inputMode="decimal"
                      className="finstack-quote-ticket__input finstack-numeric"
                      value={active ? String(field.state.value) : ""}
                      placeholder={optional ? "—" : undefined}
                      aria-describedby={`${id}-hint${error ? ` ${id}-error` : ""}`}
                      aria-invalid={Boolean(error)}
                      onBlur={field.handleBlur}
                      onChange={(event) => setValue(event.target.value)}
                    />
                    <span className="finstack-quote-ticket__unit">
                      {term.unit}
                    </span>
                  </div>
                  {optional && active && (
                    <Button
                      type="button"
                      variant="ghost"
                      size="sm"
                      aria-label={`Clear ${term.label.toLowerCase()}`}
                      onClick={() => setValue("")}
                    >
                      Clear
                    </Button>
                  )}
                </div>
                <div className="finstack-quote-ticket__footer">
                  <span
                    className="finstack-quote-ticket__source"
                    data-active={active || undefined}
                  >
                    {source}
                  </span>
                  {term.morePricingOverrides !== false && (
                    <Button
                      type="button"
                      variant="link"
                      size="sm"
                      className="finstack-quote-ticket__more"
                      onClick={() =>
                        reveal(INSTRUMENT_SHEET.path, PRICING_OVERRIDES)
                      }
                    >
                      More pricing overrides
                    </Button>
                  )}
                </div>
                {error && (
                  <p
                    id={`${id}-error`}
                    role="alert"
                    className="finstack-quote-ticket__error"
                  >
                    {error}
                  </p>
                )}
              </div>
            </div>
          );
        }}
      </form.AppField>
    </section>
  );
}

/** The promoted desk quote or deal rate for an instrument type, or undefined when it has none. */
export function promotedField(type: string): SchemaFormProps["promotedField"] {
  const term = promotedTerms[type];
  if (!term) return undefined;
  return (controls) =>
    term.kind === "single-fixed-tranche" && !singleFixedTranche(controls.values)
      ? null
      : {
          path: term.path,
          node: <PromotedTermField term={term} {...controls} />,
        };
}
