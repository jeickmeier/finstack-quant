import { instruments } from "finstack-quant-ui/instruments";

export const instrumentTypes = () => instruments.map((entry) => entry.type);

// Instrument presentation contracts only; no WASM calls or financial arithmetic.
export async function measureInstrument(type) {
  const entry = instruments.find((item) => item.type === type);
  if (!entry) throw new Error(`Unknown instrument: ${type}`);
  const before = new Set(
    performance.getEntriesByType("resource").map((entry) => entry.name),
  );
  const started = performance.now();
  const module = await entry.loader();
  const loadAndConversionMs = performance.now() - started;
  const { createWireCodec } = await import("finstack-quant-ui/codec");
  const conversionStarted = performance.now();
  // Separate reconstruction measurement excludes network and runs after the
  // module's first cached construction. It is explicitly a warm-engine sample.
  createWireCodec(module.schema);
  const warmReconstructionMs = performance.now() - conversionStarted;
  const broken = structuredClone(module.example);
  broken.instrument.type = "__invalid__";
  const validationStarted = performance.now();
  module.validator.parse(module.example);
  for (let index = 0; index < 20; index++) {
    if (
      module.validator.safeParse(broken).success ||
      module.validator.safeParse({}).success
    )
      throw new Error(`${type}: invalid input accepted`);
  }
  const errorHeavyValidationMs = performance.now() - validationStarted;
  const resources = performance
    .getEntriesByType("resource")
    .filter((entry) => !before.has(entry.name))
    .map((entry) => ({
      path: new URL(entry.name).pathname,
      encodedBytes: entry.encodedBodySize,
      decodedBytes: entry.decodedBodySize,
      transferBytes: entry.transferSize,
    }));
  return {
    type,
    loadAndConversionMs,
    warmReconstructionMs,
    errorHeavyValidationMs,
    rejectedInputs: 40,
    resources,
  };
}
