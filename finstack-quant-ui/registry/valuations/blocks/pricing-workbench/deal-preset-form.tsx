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
/** The families StructuredCredit.newStandard accepts; the native call selects each one's registry assumptions. */
const families: readonly StandardDealType[] = ["abs", "clo", "cmbs", "rmbs"];
/**
 * Builds a deal from a registry family and the StandardDealParams Rust does not
 * default. The form starts from the schema's Rust example; payment frequency and
 * the prepayment, default and recovery assumptions come from the family's registry
 * profile, so the built deal's JSON is the first place they appear.
 */
export default function DealPresetForm({
  onDeal,
}: {
  onDeal(json: string): void;
}) {
  const [family, setFamily] = useState<StandardDealType>("clo");
  const [error, setError] = useState<string | null>(null);
  const build = useNewStandardDeal();
  return (
    <div className="mt-2 flex flex-col gap-3">
      <p className="text-xs text-muted-foreground">
        The family selects Rust&apos;s registry assumptions. The collateral,
        tranches, dates and curve below start from Rust&apos;s example CLO
        inputs; edit them for the deal you mean.
      </p>
      <EnumField
        label="Deal family"
        value={family}
        onValueChange={(value) => setFamily(value as StandardDealType)}
        options={families.map((value) => ({
          value,
          label: labelFor(value),
        }))}
      />
      <SchemaForm
        module={standardDealParamsModule}
        defaultValues={
          standardDealParamsModule.example as Record<string, unknown>
        }
        layout="full"
        submitLabel="Build deal"
        submitVariant="outline"
        validate={async (json) => json}
        onSubmit={async (paramsJson) => {
          setError(null);
          try {
            onDeal(await build({ dealType: family, paramsJson }));
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
