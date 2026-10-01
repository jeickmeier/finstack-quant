// Namespaced facade for finstack-quant-wasm.
//
// The raw wasm-bindgen output (`pkg/finstack_quant_wasm.js`) is an internal artifact.
// This facade groups exports into crate-level namespaces mirroring the Rust
// umbrella crate structure.

export { default } from './pkg/finstack_quant_wasm.js';

export { core } from './exports/core.js';
export { analytics } from './exports/analytics.js';
export { attribution } from './exports/attribution.js';
export { calibration } from './exports/calibration.js';
export { cashflows } from './exports/cashflows.js';
export { covenants } from './exports/covenants.js';
export { features } from './exports/features.js';
export { models } from './exports/models.js';
export { margin } from './exports/margin.js';
export { valuations } from './exports/valuations.js';
export { statements } from './exports/statements.js';
export { statements_analytics } from './exports/statements_analytics.js';
export { portfolio } from './exports/portfolio.js';
export { scenarios } from './exports/scenarios.js';
export { schema } from './exports/schema.js';
