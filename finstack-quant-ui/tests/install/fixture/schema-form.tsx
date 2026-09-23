"use client";
import { useState } from "react";
import { SchemaForm } from "@/components/finstack/core/components/schema-form/schema-form";
import schema from "./bond.schema.json";
import { createWireCodec } from "@/lib/finstack/codec.mjs";
import data from "./data.json";
const codec = createWireCodec(schema);
const bond = { schema, codec, metadata: [], example: {} };
const validate = async (json: string) => json;
export function InstalledItem() {
  const [count, setCount] = useState(0);
  return (
    <>
      <p>
        Supplied valid fixture; standalone structural form with a caller
        validation callback.
      </p>
      <SchemaForm
        module={bond}
        defaultValues={
          bond.codec.parse(data.bond.request.instrumentJson) as Record<
            string,
            unknown
          >
        }
        validate={validate}
        onSubmit={() => setCount((n) => n + 1)}
      />
      <output aria-label="Submission count">{count}</output>
    </>
  );
}
