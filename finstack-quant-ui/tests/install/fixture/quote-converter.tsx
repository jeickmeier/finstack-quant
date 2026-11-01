"use client";
import {
  QuoteConverter,
  type QuoteConverterProps,
} from "@/components/finstack/valuations/components/quote-converter/quote-converter";

import data from "./data.json";
// The set is the supplied native fixture: the item installs without a worker.
const { cases, metadata } = data.quoteConverter;
export function InstalledItem() {
  return (
    <>
      <p>Supplied valid fixture; standalone quote converter.</p>
      <QuoteConverter
        defaultInput={
          cases.clean.request.quoteInput as QuoteConverterProps["defaultInput"]
        }
        onSubmit={() => {}}
        quotes={cases.clean.quotes as QuoteConverterProps["quotes"]}
        metadata={metadata as QuoteConverterProps["metadata"]}
      />
    </>
  );
}
