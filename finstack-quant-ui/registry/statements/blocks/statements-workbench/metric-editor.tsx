"use client";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import type { FinancialModelSpecWire } from "@/lib/finstack/generated/types/financial_model_spec";
import { useFinstack } from "@/hooks/shared/use-finstack/use-finstack";
import { FormulaPreview } from "@/components/finstack/statements/components/financial-model-editor/formula-preview";

/** Add a formula-backed metric to the canonical model for native validation and evaluation. */
export function MetricEditor({
  model,
  onApply,
}: {
  model: FinancialModelSpecWire;
  onApply: (json: string) => Promise<string | null>;
}) {
  const { client, status } = useFinstack();
  const [open, setOpen] = useState(false);
  const [id, setId] = useState("");
  const [name, setName] = useState("");
  const [formula, setFormula] = useState("");
  const [kind, setKind] = useState<"scalar" | "monetary">("scalar");
  const [currency, setCurrency] = useState(
    typeof model.meta?.reporting_currency === "string"
      ? model.meta.reporting_currency
      : "USD",
  );
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const add = async () => {
    const metricId = id.trim();
    if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(metricId))
      return setError(
        "Use a metric ID with letters, numbers, and underscores, starting with a letter or underscore.",
      );
    if (model.nodes[metricId])
      return setError("A line with this metric ID already exists.");
    if (!name.trim() || !formula.trim())
      return setError("Enter a name and formula.");
    if (kind === "monetary" && !/^[A-Z]{3}$/.test(currency.trim()))
      return setError("Enter a three-letter currency code.");
    if (!client || status !== "ready")
      return setError("Statement runtime is not ready.");
    setSaving(true);
    try {
      await client.call("validateStatementFormula", formula.trim());
      const next = structuredClone(model);
      next.nodes[metricId] = {
        node_id: metricId,
        name: name.trim(),
        node_type: "calculated",
        tags: ["metric", ...(kind === "scalar" ? ["ratio"] : [])],
        value_type:
          kind === "scalar"
            ? { type: "scalar" }
            : ({ type: "monetary", currency: currency.trim() } as NonNullable<
                FinancialModelSpecWire["nodes"][string]["value_type"]
              >),
        formula_text: formula.trim(),
      };
      const message = await onApply(JSON.stringify(next));
      setError(message);
      if (!message) {
        setOpen(false);
        setId("");
        setName("");
        setFormula("");
      }
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setSaving(false);
    }
  };
  return (
    <div className="rounded-lg border border-border bg-card p-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h3 className="font-semibold">Calculated metrics</h3>
          <p className="text-xs text-muted-foreground">
            Use model line IDs in a native statement formula. WASM calculates
            every period.
          </p>
        </div>
        <Button
          type="button"
          size="sm"
          onClick={() => setOpen((value) => !value)}
        >
          {open ? "Close" : "Add metric"}
        </Button>
      </div>
      {open && (
        <div className="mt-4 grid gap-3 sm:grid-cols-2">
          <label className="text-xs">
            Metric name
            <input
              className="mt-1 w-full rounded border border-border bg-background px-3 py-2 text-sm"
              value={name}
              onChange={(event) => setName(event.target.value)}
              placeholder="Net debt / EBITDA"
            />
          </label>
          <label className="text-xs">
            Metric ID
            <input
              className="mt-1 w-full rounded border border-border bg-background px-3 py-2 font-mono text-sm"
              value={id}
              onChange={(event) => setId(event.target.value)}
              placeholder="net_debt_leverage"
            />
          </label>
          <label className="text-xs">
            Result type
            <select
              className="mt-1 w-full rounded border border-border bg-background px-3 py-2 text-sm"
              value={kind}
              onChange={(event) =>
                setKind(event.target.value as "scalar" | "monetary")
              }
            >
              <option value="scalar">Scalar / ratio</option>
              <option value="monetary">Monetary amount</option>
            </select>
          </label>
          {kind === "monetary" && (
            <label className="text-xs">
              Currency
              <input
                className="mt-1 w-full rounded border border-border bg-background px-3 py-2 font-mono text-sm"
                value={currency}
                onChange={(event) =>
                  setCurrency(event.target.value.toUpperCase())
                }
                maxLength={3}
              />
            </label>
          )}
          <label className="text-xs sm:col-span-2">
            Formula
            <textarea
              aria-label="Formula"
              className="mt-1 w-full rounded border border-border bg-background px-3 py-2 font-mono text-sm"
              rows={2}
              value={formula}
              onChange={(event) => setFormula(event.target.value)}
              placeholder="debt_closing / ltm_ebitda"
            />
            <span className="block pt-1 text-muted-foreground">
              Available lines: {Object.keys(model.nodes).join(", ")}
            </span>
            <FormulaPreview formula={formula || null} />
          </label>
          {error && (
            <p role="alert" className="text-xs text-error sm:col-span-2">
              {error}
            </p>
          )}
          <div className="sm:col-span-2">
            <Button
              type="button"
              size="sm"
              disabled={saving}
              onClick={() => void add()}
            >
              {saving ? "Calculating…" : "Add and calculate"}
            </Button>
          </div>
        </div>
      )}
    </div>
  );
}
