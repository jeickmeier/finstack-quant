"use client";
import { useMemo, useState } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import type { models as modelsWire } from "finstack-quant-wasm/types";
import type { RecoveryWaterfallRequest } from "@/workers/finstack-contract";
import { Button } from "@/components/ui/button";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { DecimalInput } from "@/components/finstack/core/primitives/decimal-input/decimal-input";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import {
  Outcome,
  shownValue,
  TextField,
  typedNumber,
  type OutcomeState,
} from "@/components/finstack/shared/primitives/native-display/native-display";
import { claimHelp } from "./recovery-contracts";
import {
  defaultValue,
  FieldTable,
  row,
  useRows,
  type Density,
} from "./recovery-fields";

export interface RecoveryWaterfallProps {
  /** The initial estate and claims; remount to load others. */
  defaultTerms: RecoveryWaterfallRequest;
  /** Receives the estate and claims on Allocate; connect it to the useAllocateRecovery request. */
  onSubmit(terms: RecoveryWaterfallRequest): void;
  /** The useAllocateRecovery state. */
  state: OutcomeState<modelsWire.RecoveryWaterfallResult>;
  /** Display text of a finite returned number; omitted shows four decimals. Every digit stays in the cell's title. */
  formatValue?: (value: number) => string;
  density?: Density;
}
const text = (value: number | null) => (value === null ? "" : String(value));
/** The RecoveryClaim amounts, by wire name; each labelled and described from the contract. */
const claimFields = [
  "principal",
  "accrued",
  "penalties",
  "collateral_value",
  "collateral_haircut",
] as const;
/**
 * How an estate is allocated down a stack of claims under absolute priority,
 * as Rust returned it. Nothing is summed, divided or ranked here.
 */
export function RecoveryWaterfall({
  defaultTerms,
  onSubmit,
  state,
  formatValue = defaultValue,
  density = "compact",
}: RecoveryWaterfallProps) {
  const [estate, setEstate] = useState(String(defaultTerms.estateValue));
  const claims = useRows(() =>
    defaultTerms.claims.map((claim) =>
      row({
        id: claim.id,
        seniority: claim.seniority,
        priority: String(claim.priority),
        principal: String(claim.principal),
        accrued: String(claim.accrued),
        penalties: String(claim.penalties),
        collateral_value: text(claim.collateral_value ?? null),
        collateral_haircut: String(claim.collateral_haircut),
      }),
    ),
  );
  return (
    <section
      aria-label="Recovery waterfall"
      data-density={density}
      className="@container space-y-4 font-sans text-sm text-foreground"
    >
      <h3 className="text-sm font-medium">Recovery waterfall</h3>
      <form
        aria-label="Estate and claims"
        className="space-y-4"
        onSubmit={(event) => {
          event.preventDefault();
          onSubmit({
            estateValue: typedNumber(estate),
            claims: claims.rows.map((claim) => ({
              id: claim.id ?? "",
              seniority: claim.seniority ?? "",
              priority: typedNumber(claim.priority),
              principal: typedNumber(claim.principal),
              accrued: typedNumber(claim.accrued),
              penalties: typedNumber(claim.penalties),
              // An empty collateral value is an unsecured claim, never zero collateral.
              collateral_value: claim.collateral_value?.trim()
                ? typedNumber(claim.collateral_value)
                : null,
              collateral_haircut: typedNumber(claim.collateral_haircut),
            })),
          });
        }}
      >
        <div className="max-w-xs">
          <DecimalInput
            label="Estate value"
            help="Distributable estate including pledged collateral, in one monetary unit you choose."
            value={estate}
            onValueChange={setEstate}
          />
        </div>
        {claims.rows.map((claim, index) => (
          <fieldset
            key={claim.key}
            aria-label={`Claim ${claim.id || index}`}
            className="grid gap-4 border-t border-border pt-4 @lg:grid-cols-4"
          >
            <TextField
              label={labelFor("id")}
              help={claimHelp("id")}
              value={claim.id ?? ""}
              onValueChange={(value) => claims.set(claim.key, "id", value)}
            />
            <TextField
              label={labelFor("seniority")}
              help={claimHelp("seniority")}
              value={claim.seniority ?? ""}
              onValueChange={(value) =>
                claims.set(claim.key, "seniority", value)
              }
            />
            <DecimalInput
              label={labelFor("priority")}
              help={claimHelp("priority")}
              value={claim.priority ?? ""}
              onValueChange={(value) =>
                claims.set(claim.key, "priority", value)
              }
            />
            {claimFields.map((field) => (
              <DecimalInput
                key={field}
                label={labelFor(field)}
                help={claimHelp(field)}
                value={claim[field] ?? ""}
                onValueChange={(value) => claims.set(claim.key, field, value)}
              />
            ))}
            <div className="flex items-end">
              <Button
                type="button"
                variant="outline"
                onClick={() => claims.remove(claim.key)}
              >
                Remove claim
              </Button>
            </div>
          </fieldset>
        ))}
        <div className="flex flex-wrap gap-3">
          <Button
            type="button"
            variant="outline"
            onClick={() =>
              claims.add({
                id: "",
                seniority: "",
                priority: "",
                principal: "",
                accrued: "",
                penalties: "",
                collateral_value: "",
                collateral_haircut: "",
              })
            }
          >
            Add claim
          </Button>
          <Button type="submit">Allocate</Button>
        </div>
      </form>
      <Outcome state={state} empty="No waterfall" working="Allocating…">
        {(result) => (
          <div className="space-y-3">
            <AllocationTable
              allocations={result.allocations}
              format={formatValue}
              density={density}
            />
            <FieldTable
              caption="Estate"
              rows={[
                ["total_distributed", result.total_distributed],
                ["undistributed_estate", result.undistributed_estate],
                ["apr_satisfied", result.apr_satisfied],
              ]}
              format={formatValue}
              density={density}
            />
            <p className="text-xs text-muted-foreground">
              Rows in Rust&apos;s allocation order: priority, then entry order.
              Amounts in the unit entered; recovery rate is a fraction of the
              allowed claim.
            </p>
          </div>
        )}
      </Outcome>
    </section>
  );
}
const allocationFields = [
  "seniority",
  "priority",
  "total_claim",
  "collateral_recovery",
  "general_recovery",
  "total_recovery",
  "recovery_rate",
  "deficiency",
] as const satisfies readonly (keyof modelsWire.RecoveryAllocation)[];
function AllocationTable({
  allocations,
  format,
  density,
}: {
  allocations: modelsWire.RecoveryAllocation[];
  format: (value: number) => string;
  density: Density;
}) {
  const columns = useMemo<ColumnDef<{}, modelsWire.RecoveryAllocation, any>[]>(
    () => [
      {
        id: "id",
        header: labelFor("claim"),
        accessorFn: (allocation) => allocation.id,
        meta: { rowHeader: true },
      },
      ...allocationFields.map(
        (field): ColumnDef<{}, modelsWire.RecoveryAllocation, any> => ({
          id: field,
          header: labelFor(field),
          accessorFn: (allocation) => allocation[field],
          cell: ({ row }) => {
            const value = row.original[field];
            return (
              <span title={String(value)}>{shownValue(value, format)}</span>
            );
          },
          meta:
            field === "seniority"
              ? {}
              : { className: "text-right finstack-numeric" },
        }),
      ),
    ],
    [format],
  );
  return (
    <FinstackTable
      data={allocations}
      columns={columns}
      getRowId={(allocation) => allocation.id}
      caption="Recovery by claim"
      density={density}
      emptyState="No claims"
    />
  );
}
