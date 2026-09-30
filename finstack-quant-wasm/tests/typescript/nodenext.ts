// Consumer view of the published types under `module`/`moduleResolution`
// NodeNext with `skipLibCheck: false`. Imports go through the package name
// (self-reference via package.json `exports`), exactly as a dependent project
// resolves them. The `@ts-expect-error` lines fail the gate if a type silently
// degrades to `any`.
import type {
  CalibrationEnvelope,
  CalibrationResultEnvelope,
  CalibrationValidationReport,
  MaterializationReport,
  PriorMarketObject,
  ValuationResult,
} from 'finstack-quant-wasm';
import type { calibration, portfolio } from 'finstack-quant-wasm/types';

// F380: every serde-defaulted plan and settings field is optional.
const minimal: CalibrationEnvelope = {
  schema: 'finstack_quant.calibration/1',
  plan: { id: 'smoke', steps: [] },
};
const documented: CalibrationEnvelope = {
  schema: 'finstack_quant.calibration/1',
  plan: { id: 'smoke', description: null, quote_sets: {}, steps: [], settings: {} },
};

// F403: u64 fields inside JSON envelopes are numbers.
const freshness: calibration.MarketFreshnessPolicy = {
  snapshot_timestamp: '2026-01-02T00:00:00Z',
  max_age_seconds: 3600,
  quote_side: 'mid',
};
// @ts-expect-error JSON envelopes carry numbers, never bigint.
const bigFreshness: calibration.MarketFreshnessPolicy = { max_age_seconds: 3600n };

// F404: pillars, tenors and CDS convention keys use their serde object shapes.
const deposit: calibration.MarketDatum = {
  kind: 'rate_quote',
  type: 'deposit',
  id: 'USD-SOFR-DEP-1M',
  index: 'USD-SOFR',
  pillar: { tenor: { count: 1, unit: 'months' } },
  rate: 0.0525,
};
const cds: calibration.MarketDatum = {
  kind: 'cds_quote',
  type: 'cds_par_spread',
  id: 'ACME-CDS-5Y',
  entity: 'ACME',
  convention: { currency: 'USD', doc_clause: 'cr14' },
  pillar: { date: '2031-06-20' },
  spread_bp: 120,
  recovery_rate: 0.4,
};
const stringPillar: calibration.MarketDatum = {
  kind: 'rate_quote',
  type: 'deposit',
  id: 'USD-SOFR-DEP-1M',
  index: 'USD-SOFR',
  // @ts-expect-error Rust rejects the bare tenor string form.
  pillar: '1M',
  rate: 0.0525,
};

// F405: notional positions carry `{ notional: currency | null }`.
const position: portfolio.MaterializedPosition = {
  id: 'p',
  entity_id: 'e',
  instrument_id: 'i',
  artifact_id: 'a',
  quantity: 1,
  unit: { notional: 'USD' },
};
const unpricedPosition: portfolio.MaterializedPosition = { ...position, unit: { notional: null } };
// @ts-expect-error The bare "notional" string is rejected by the serde wire.
const badPosition: portfolio.MaterializedPosition = { ...position, unit: 'notional' };

// F408: internally tagged payloads keep their fields.
// @ts-expect-error A bare discriminator is not a discount curve.
const barePrior: PriorMarketObject = { kind: 'discount_curve' };

// F412: an absent dependency block may be written as null.
const artifact: portfolio.InstrumentArtifact = {
  artifact_id: 'a',
  envelope: {
    schema: 'finstack_quant.instrument/1',
    instrument: {
      type: 'equity',
      spec: {
        id: 'EQ',
        ticker: 'EQ',
        currency: 'USD',
        attributes: {},
        discount_curve_id: 'USD-OIS',
      },
    },
  },
  dependencies: null,
};

// F413: closed Rust enums are string-literal unions.
const interpolation: calibration.InterpStyle = 'log_linear';
// @ts-expect-error Unknown interpolation styles are rejected.
const typoInterpolation: calibration.InterpStyle = 'log-linear';

// F407/F410: result sub-documents are typed; skipped fields are optional.
declare const result: CalibrationResultEnvelope;
const success: boolean = result.result.report.success;
const stepReports: Record<string, calibration.CalibrationReport> = result.result.step_reports;
const validationError: string | null | undefined = result.result.report.validation_error;
declare const dryRun: CalibrationValidationReport;
const firstError: calibration.EnvelopeError | undefined = dryRun.errors[0];
const reads: string[] | undefined = dryRun.dependency_graph.nodes[0]?.reads;
declare const materialization: MaterializationReport;
const parseNanos: number = materialization.phase_nanos.parse;
declare const valuation: ValuationResult;

// @ts-expect-error A published type must never degrade to `any`.
const notAnEnvelope: CalibrationEnvelope = 42;
// @ts-expect-error A published type must never degrade to `any`.
const notAReport: MaterializationReport = 'x';
// @ts-expect-error The valuation contract keeps its structure.
const notMeta: string = valuation.meta;

void [minimal, documented, freshness, bigFreshness, deposit, cds, stringPillar];
void [position, unpricedPosition, badPosition, barePrior, artifact];
void [interpolation, typoInterpolation, success, stepReports, validationError];
void [firstError, reads, parseNanos, notAnEnvelope, notAReport, notMeta];
