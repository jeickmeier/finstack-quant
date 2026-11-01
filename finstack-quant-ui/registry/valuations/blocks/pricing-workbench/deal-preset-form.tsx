"use client";
import { useState } from "react";
import { createWireCodec } from "@/lib/finstack/codec.mjs";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { linkSchema } from "@/lib/finstack/schema.mjs";
import source from "@/lib/finstack/generated/schemas/standard_deal_params.json";
import sharedSchema from "@/lib/finstack/generated/defs/shared.json";
import sharedMetadata from "@/lib/finstack/generated/meta/shared";
import localMetadata from "@/lib/finstack/generated/meta/standard_deal_params";
import {
  useNewStandardDeal,
  useStandardDealFamilies,
  type StandardDealType,
} from "@/hooks/valuations/use-tranche-analytics/use-tranche-analytics";
import { EnumField } from "@/components/finstack/shared/primitives/enum-field/enum-field";
import { SchemaForm } from "@/components/finstack/core/components/schema-form/schema-form";
const schema = linkSchema(source, sharedSchema);
/** Generated StandardDealParams contract: the inputs StructuredCredit.newStandard has no default for. */
const standardDealParamsModule = {
  schema,
  metadata: [
    ...localMetadata,
    ...sharedMetadata.filter((entry) => entry.path.startsWith("#/$defs/")),
  ],
  codec: createWireCodec(schema),
  example: source.examples[0],
};
/**
 * Builds a deal from a registry family and the StandardDealParams Rust does not
 * default. The families, each one's registry profile and its example inputs come from
 * the native call; payment frequency and the prepayment, default and recovery
 * assumptions come from the profile, so the built deal's JSON is the first place they
 * appear.
 */
export default function DealPresetForm({
  onDeal,
}: {
  onDeal(json: string): void;
}) {
  const families = useStandardDealFamilies();
  const [chosen, setChosen] = useState<StandardDealType>();
  const [error, setError] = useState<string | null>(null);
  const build = useNewStandardDeal();
  if (families.error)
    return (
      <p role="alert" className="mt-2 text-sm text-error">
        {families.error.message}
      </p>
    );
  if (!families.data)
    return (
      <p role="status" className="mt-2 text-sm text-muted-foreground">
        Loading the deal families…
      </p>
    );
  const selected =
    families.data.find((family) => family.deal_type === chosen) ??
    families.data[0]!;
  return (
    <div className="mt-2 flex flex-col gap-3">
      <p className="text-xs text-muted-foreground">
        The family selects Rust&apos;s registry profile{" "}
        <span className="font-mono">{selected.profile_id}</span>. The
        collateral, tranches, dates and curve below start from Rust&apos;s
        example for this family; edit them for the deal you mean.
      </p>
      <EnumField
        label="Deal family"
        value={selected.deal_type}
        onValueChange={(value) => setChosen(value as StandardDealType)}
        options={families.data.map(({ deal_type }) => ({
          value: deal_type,
          label: labelFor(deal_type),
        }))}
      />
      <SchemaForm
        key={selected.deal_type}
        module={standardDealParamsModule}
        defaultValues={selected.example as Record<string, unknown>}
        layout="full"
        submitLabel="Build deal"
        submitVariant="outline"
        validate={async (json) => json}
        onSubmit={async (paramsJson) => {
          setError(null);
          try {
            onDeal(await build({ dealType: selected.deal_type, paramsJson }));
          } catch (failure) {
            setError(
              failure instanceof Error ? failure.message : String(failure),
            );
          }
        }}
      />
      {error && (
        <p role="alert" className="text-sm text-error">
          {error}
        </p>
      )}
    </div>
  );
}
