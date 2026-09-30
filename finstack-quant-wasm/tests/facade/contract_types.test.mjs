/**
 * Runtime outputs satisfy the published, schema-generated TypeScript types.
 *
 * Runs the shipped calibration examples, a dry run, a canonicalized envelope,
 * two portfolio materializations (including a notional-unit position) and a
 * Monte Carlo valuation, writes each value as a typed literal
 * (`const x: CalibrationResultEnvelope = {...}`) and compiles the file with
 * `tsc --strict` under NodeNext with `skipLibCheck: false`. A generated type
 * that disagrees with the serde wire fails here.
 */
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import init, { calibration, portfolio, valuations } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

const PACKAGE = fileURLToPath(new URL('../../', import.meta.url));
const EXAMPLES = new URL(
  '../../../finstack-quant/calibration/examples/market_bootstrap/',
  import.meta.url
);
const MATERIALIZATION = new URL(
  '../../../finstack-quant/portfolio/tests/data/canonical/portfolio_materialization.json',
  import.meta.url
);
const BARRIER = new URL(
  '../../../finstack-quant/valuations/tests/golden/data/pricing/quantlib/fx_barrier_option/eurusd_up_out_call_3m_quantlib.json',
  import.meta.url
);

/** TypeScript source for a runtime value; `bigint` stays a bigint literal. */
function literal(value) {
  const marker = '__bigint__';
  return JSON.stringify(value, (_key, item) =>
    typeof item === 'bigint' ? `${marker}${item}` : item
  ).replace(new RegExp(`"${marker}(-?\\d+)"`, 'g'), '$1n');
}

function typecheck(declarations) {
  const dir = mkdtempSync(join(tmpdir(), 'fq-contract-types-'));
  try {
    const source = [
      `import type * as fq from ${JSON.stringify(join(PACKAGE, 'index.js'))};`,
      `import type * as types from ${JSON.stringify(join(PACKAGE, 'types/generated/index.js'))};`,
      ...declarations.map(
        ([name, type, value], i) => `export const v${i}_${name}: ${type} = ${literal(value)};`
      ),
    ].join('\n');
    writeFileSync(join(dir, 'probe.mts'), source);
    writeFileSync(
      join(dir, 'tsconfig.json'),
      JSON.stringify({
        compilerOptions: {
          target: 'ES2022',
          module: 'NodeNext',
          moduleResolution: 'NodeNext',
          lib: ['ES2022', 'DOM'],
          strict: true,
          noEmit: true,
          skipLibCheck: false,
          types: [],
        },
        files: ['probe.mts'],
      })
    );
    const tsc = join(PACKAGE, 'node_modules/typescript/bin/tsc');
    const run = spawnSync(process.execPath, [tsc, '-p', dir], { encoding: 'utf8' });
    assert.equal(run.status, 0, `tsc rejected runtime outputs:\n${run.stdout}${run.stderr}`);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

test('calibration envelopes, results, dry runs and canonical JSON match the generated types', () => {
  const declarations = [];
  for (const file of readdirSync(EXAMPLES)
    .filter((name) => name.endsWith('.json'))
    .sort()) {
    const name = file.replace(/\W/g, '_');
    const envelope = JSON.parse(readFileSync(new URL(file, EXAMPLES), 'utf8'));
    declarations.push([`${name}_input`, 'fq.CalibrationEnvelope', envelope]);
    const canonical = JSON.parse(calibration.validateCalibrationJson(envelope));
    declarations.push([`${name}_canonical`, 'fq.CalibrationEnvelope', canonical]);
    declarations.push([
      `${name}_dry_run`,
      'fq.CalibrationValidationReport',
      calibration.dryRun(envelope),
    ]);
    declarations.push([
      `${name}_result`,
      'fq.CalibrationResultEnvelope',
      calibration.calibrate(envelope),
    ]);
  }
  assert.equal(declarations.length, 48, 'all twelve shipped examples are exercised');
  const broken = {
    schema: 'finstack_quant.calibration/1',
    plan: {
      id: 'p',
      steps: [
        {
          id: 's',
          quote_set: 'missing',
          kind: 'discount',
          curve_id: 'USD-OIS',
          currency: 'USD',
          base_date: '2026-01-02',
        },
      ],
    },
  };
  const findings = calibration.dryRun(broken);
  assert.ok(findings.errors.length > 0, 'the dry run reports the undefined quote set');
  declarations.push(['dry_run_errors', 'fq.CalibrationValidationReport', findings]);
  typecheck(declarations);
});

test('materialization bundles and reports match the generated types', () => {
  const bundle = JSON.parse(readFileSync(MATERIALIZATION, 'utf8'));
  const notional = structuredClone(bundle);
  notional.positions[0].unit = { notional: 'USD' };
  notional.instruments[0].dependencies = null;
  const declarations = [];
  for (const [name, input] of [
    ['canonical', bundle],
    ['notional', notional],
  ]) {
    const text = JSON.stringify(input);
    const loaded = portfolio.Portfolio.fromMaterialization(text);
    try {
      declarations.push([
        `${name}_bundle`,
        'types.portfolio.PortfolioMaterializationEnvelope',
        input,
      ]);
      declarations.push([`${name}_report`, 'fq.MaterializationReport', loaded.report]);
      declarations.push([
        `${name}_validation`,
        'fq.MaterializationReport | fq.ValidationReport',
        portfolio.Portfolio.validateMaterialization(text),
      ]);
    } finally {
      loaded.portfolio.free();
    }
  }
  typecheck(declarations);
});

test('a Monte Carlo valuation matches the bigint host contract', () => {
  const golden = JSON.parse(readFileSync(BARRIER, 'utf8'));
  const envelope = structuredClone(golden.instrument);
  Object.assign(envelope.instrument.spec, {
    monitoring: { type: 'discrete', observation_dates: ['2026-05-29', '2026-06-30', '2026-07-30'] },
    monitoring_start_date: '2026-05-29',
    instrument_pricing_overrides: { model_config: { mc_paths: 256 } },
  });
  const result = valuations.instruments.priceInstrument(
    JSON.stringify(envelope),
    JSON.stringify(golden.market.data),
    '2026-04-30',
    'monte_carlo_gbm',
    []
  );
  assert.equal(typeof result.details.data.seed, 'bigint');
  typecheck([['valuation', 'fq.ValuationResult', result]]);
});
