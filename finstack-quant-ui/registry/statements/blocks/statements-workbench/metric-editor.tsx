"use client";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { FieldFrame } from "@/components/finstack/shared/primitives/field-frame/field-frame";
import { EnumField } from "@/components/finstack/shared/primitives/enum-field/enum-field";
import type { FinancialModelSpecWire } from "@/lib/finstack/generated/types/financial_model_spec";
import { useStatementFormulaValidator } from "@/hooks/statements/use-statement-formula-validator/use-statement-formula-validator";
import { FormulaPreview } from "@/components/finstack/statements/components/financial-model-editor/formula-preview";

/** Add a formula-backed metric to the canonical model for native validation and evaluation. */
export function MetricEditor({
  model,
  onApply,
}: {
  model: FinancialModelSpecWire;
  onApply: (json: string) => Promise<string | null>;
}) {
  const validateFormula = useStatementFormulaValidator();
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
    setSaving(true);
    try {
      await validateFormula(formula.trim());
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
          <FieldFrame label="Metric name">
            {(control) => (
              <Input
                {...control}
                value={name}
                onChange={(event) => setName(event.target.value)}
                placeholder="Net debt / EBITDA"
              />
            )}
          </FieldFrame>
          <FieldFrame label="Metric ID">
            {(control) => (
              <Input
                {...control}
                className="font-mono"
                value={id}
                onChange={(event) => setId(event.target.value)}
                placeholder="net_debt_leverage"
              />
            )}
          </FieldFrame>
          <EnumField
            label="Result type"
            value={kind}
            onValueChange={(next) => setKind(next as "scalar" | "monetary")}
            options={[
              { value: "scalar", label: "Scalar / ratio" },
              { value: "monetary", label: "Monetary amount" },
            ]}
          />
          {kind === "monetary" && (
            <FieldFrame label="Currency">
              {(control) => (
                <Input
                  {...control}
                  className="font-mono"
                  value={currency}
                  onChange={(event) =>
                    setCurrency(event.target.value.toUpperCase())
                  }
                  maxLength={3}
                />
              )}
            </FieldFrame>
          )}
          <div className="space-y-1 sm:col-span-2">
            <FieldFrame label="Formula">
              {(control) => (
                <Textarea
                  {...control}
                  rows={2}
                  className="font-mono"
                  value={formula}
                  onChange={(event) => setFormula(event.target.value)}
                  placeholder="debt_closing / ltm_ebitda"
                />
              )}
            </FieldFrame>
            <p className="text-xs text-muted-foreground">
              Available lines: {Object.keys(model.nodes).join(", ")}
            </p>
            <FormulaPreview formula={formula || null} />
          </div>
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
