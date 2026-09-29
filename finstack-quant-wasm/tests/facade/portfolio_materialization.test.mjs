/**
 * Portfolio materialization browser-runtime API coverage.
 *
 * Requires the wasm-pack web build: npm run build.
 */

import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const WASM_BG = join(__dirname, '..', '..', 'pkg', 'finstack_quant_wasm_bg.wasm');

if (!existsSync(WASM_BG)) {
  throw new Error(`WASM web build not found at ${WASM_BG}. Generate it with: npm run build`);
}

const facade = await import('../../index.js');
await facade.default({ module_or_path: readFileSync(WASM_BG) });
const { portfolio } = facade;

const MAX_MATERIALIZATION_BYTES = 64 * 1024 * 1024;
const MAX_MATERIALIZATION_ARTIFACTS = 100_000;

const EMPTY_BUNDLE = {
  schema: 'finstack_quant.portfolio_materialization/1',
  portfolio: {
    id: 'materialized-empty',
    base_currency: 'USD',
    as_of: '2025-01-01',
    entities: {},
  },
  instruments: [],
  positions: [],
};

const EMPTY_MARKET = JSON.stringify({
  collateral: {},
  credit_indices: [],
  curves: [],
  dividends: [],
  fx: null,
  fx_delta_vol_surfaces: [],
  hierarchy: { roots: {} },
  inflation_indices: [],
  prices: {},
  series: [],
  surfaces: [],
  schema_version: 1,
  vol_cubes: [],
});

const DEPOSIT_BUNDLE = {
  schema: 'finstack_quant.portfolio_materialization/1',
  portfolio: {
    id: 'materialized-deposit',
    base_currency: 'USD',
    as_of: '2025-01-01',
    entities: { entity: { id: 'entity', name: null } },
  },
  instruments: [
    {
      artifact_id: 'artifact-0',
      envelope: {
        schema: 'finstack_quant.instrument/1',
        instrument: {
          type: 'deposit',
          spec: {
            id: 'DEP-0',
            notional: { amount: '1000000', currency: 'USD' },
            start_date: '2025-01-01',
            maturity: '2025-02-01',
            day_count: 'act_360',
            discount_curve_id: 'USD-OIS',
            attributes: {},
          },
        },
      },
    },
  ],
  positions: [
    {
      id: 'position-0',
      entity_id: 'entity',
      instrument_id: 'DEP-0',
      artifact_id: 'artifact-0',
      quantity: 1,
      unit: 'units',
    },
  ],
};

test('fromMaterialization accepts string and Uint8Array', () => {
  for (const bundle of [
    JSON.stringify(EMPTY_BUNDLE),
    new TextEncoder().encode(JSON.stringify(EMPTY_BUNDLE)),
  ]) {
    const loaded = portfolio.Portfolio.fromMaterialization(bundle);
    assert.equal(loaded.portfolio.id, 'materialized-empty');
    assert.equal(loaded.portfolio.numPositions(), 0);
    assert.equal(loaded.report.positions, 0);
    assert.deepEqual(loaded.report.report.diagnostics, []);
    loaded.portfolio.free();
  }
});

test('materialization depth scan ignores delimiters inside strings', () => {
  const bundle = structuredClone(EMPTY_BUNDLE);
  bundle.portfolio.meta = {
    quoted: `${'[{'.repeat(200)}\\"escaped\\"]}${'}]'.repeat(200)}`,
  };

  const loaded = portfolio.Portfolio.fromMaterialization(JSON.stringify(bundle));

  assert.equal(loaded.portfolio.id, 'materialized-empty');
  assert.deepEqual(loaded.report.report.diagnostics, []);
  loaded.portfolio.free();
});

test('materialization timings are available non-fabricated JavaScript numbers', () => {
  const loaded = portfolio.Portfolio.fromMaterialization(JSON.stringify(DEPOSIT_BUNDLE));

  assert.equal(loaded.report.timing_available, true);
  const phases = Object.values(loaded.report.phase_nanos);
  for (const elapsed of phases) {
    assert.equal(typeof elapsed, 'number');
    assert.equal(Number.isSafeInteger(elapsed), true);
    assert.ok(elapsed >= 0);
  }
  assert.ok(phases.some((elapsed) => elapsed > 0));
  loaded.portfolio.free();
});

test('validateMaterialization returns structured diagnostics', () => {
  const report = portfolio.Portfolio.validateMaterialization(
    new TextEncoder().encode('{"schema":')
  );

  assert.equal(report.truncated, false);
  assert.equal(report.diagnostics[0].code, 'contract/parse-error');
  assert.equal(report.diagnostics[0].pointer, null);
});

test('validateMaterialization validates without portfolio build phases', () => {
  const cache = new portfolio.InstrumentArtifactCache(1);
  const report = portfolio.Portfolio.validateMaterialization(JSON.stringify(DEPOSIT_BUNDLE), cache);

  assert.equal(report.unique_instruments, 1);
  assert.equal(report.positions, 1);
  assert.ok(report.dependencies > 0);
  assert.equal(report.phase_nanos.build_positions, 0);
  assert.equal(report.phase_nanos.index_build, 0);
  cache.free();
});

test('validateMaterialization matches portfolio metadata diagnostics', () => {
  const book = (id, parentId, positionIds, childBookIds) => ({
    id,
    name: null,
    parent_id: parentId,
    position_ids: positionIds,
    child_book_ids: childBookIds,
  });
  const cases = [
    [
      (bundle) => {
        bundle.positions[0].entity_id = 'missing-entity';
      },
      'portfolio/unknown-entity',
    ],
    [
      (bundle) => {
        bundle.portfolio.books = {
          a: book('a', null, ['missing-position'], []),
        };
      },
      'portfolio/book-missing-position',
    ],
    [
      (bundle) => {
        bundle.portfolio.books = {
          a: book('a', 'missing-parent', [], []),
        };
      },
      'portfolio/book-missing-parent',
    ],
    [
      (bundle) => {
        bundle.portfolio.books = {
          a: book('a', 'b', [], ['b']),
          b: book('b', 'a', [], ['a']),
        };
      },
      'portfolio/book-cycle',
    ],
    [
      (bundle) => {
        bundle.portfolio.books = {
          a: book('a', null, ['position-0'], []),
          b: book('b', null, ['position-0'], []),
        };
      },
      'portfolio/position-multiple-books',
    ],
  ];

  for (const [mutate, expectedCode] of cases) {
    const bundle = structuredClone(DEPOSIT_BUNDLE);
    mutate(bundle);
    const report = portfolio.Portfolio.validateMaterialization(JSON.stringify(bundle));
    assert.ok(
      report.diagnostics.some(({ code }) => code === expectedCode),
      `expected ${expectedCode}; received ${report.diagnostics.map(({ code }) => code).join(', ')}`
    );
  }
});

test('fromMaterialization throws typed errors with reports', () => {
  assert.throws(
    () => portfolio.Portfolio.fromMaterialization('{"schema":'),
    (error) => {
      assert.equal(error.name, 'ContractValidationError');
      assert.equal(error.kind, 'validation');
      assert.equal(error.code, 'report');
      assert.equal(error.report.diagnostics[0].code, 'contract/parse-error');
      assert.equal(error.report.diagnostics[0].pointer, null);
      return true;
    }
  );
});

test('fromMaterialization maps only max input size to limit_exceeded', () => {
  const payload = ' '.repeat(MAX_MATERIALIZATION_BYTES + 1);

  assert.throws(
    () => portfolio.Portfolio.fromMaterialization(payload),
    (error) => {
      assert.equal(error.name, 'ContractValidationError');
      assert.equal(error.kind, 'validation');
      assert.equal(error.code, 'limit_exceeded');
      assert.match(error.message, /bytes/);
      return true;
    }
  );
});

test('fromMaterialization maps max artifact count to limit_exceeded', () => {
  const bundle = structuredClone(EMPTY_BUNDLE);
  bundle.instruments = Array.from({ length: MAX_MATERIALIZATION_ARTIFACTS + 1 }, (_, index) => ({
    artifact_id: `artifact-${index}`,
    envelope: {},
  }));
  const payload = JSON.stringify(bundle);
  assert.ok(payload.length < MAX_MATERIALIZATION_BYTES);

  assert.throws(
    () => portfolio.Portfolio.fromMaterialization(payload),
    (error) => {
      assert.equal(error.name, 'ContractValidationError');
      assert.equal(error.kind, 'validation');
      assert.equal(error.code, 'limit_exceeded');
      assert.match(error.message, /artifacts/);
      return true;
    }
  );
});

test('materialization cache handle is reusable', () => {
  const cache = new portfolio.InstrumentArtifactCache();
  const first = portfolio.Portfolio.fromMaterialization(JSON.stringify(DEPOSIT_BUNDLE), cache);
  const second = portfolio.Portfolio.fromMaterialization(JSON.stringify(DEPOSIT_BUNDLE), cache);

  assert.equal(first.report.cache_hits, 0);
  assert.equal(second.report.cache_hits, 1);
  assert.equal(cache.decodeCount, 1);
  assert.equal(cache.size, 1);
  first.portfolio.free();
  second.portfolio.free();
  cache.free();
});

test('distinct artifact IDs with identical content share one cache entry', () => {
  const bundle = structuredClone(DEPOSIT_BUNDLE);
  const alias = structuredClone(bundle.instruments[0]);
  alias.artifact_id = 'artifact-alias';
  bundle.instruments.push(alias);
  const aliasPosition = structuredClone(bundle.positions[0]);
  aliasPosition.id = 'position-alias';
  aliasPosition.artifact_id = 'artifact-alias';
  bundle.positions.push(aliasPosition);
  const cache = new portfolio.InstrumentArtifactCache();

  const loaded = portfolio.Portfolio.fromMaterialization(JSON.stringify(bundle), cache);

  assert.equal(loaded.portfolio.numPositions(), 2);
  assert.equal(loaded.report.cache_hits, 1);
  assert.equal(cache.decodeCount, 1);
  assert.equal(cache.size, 1);
  const warning = loaded.report.report.diagnostics.find(
    ({ code }) => code === 'portfolio/duplicate-artifact-content'
  );
  assert.equal(warning.severity, 'warning');
  assert.equal(warning.pointer, '/instruments/1/envelope');
  assert.equal(warning.revision_id, 'artifact-alias');
  assert.match(warning.artifact_hash, /^sha256:/);
  loaded.portfolio.free();
  cache.free();
});

test('materialized portfolio handle supports repeated valuation without re-materialization', () => {
  const cache = new portfolio.InstrumentArtifactCache();
  const loaded = portfolio.Portfolio.fromMaterialization(JSON.stringify(EMPTY_BUNDLE), cache);
  const decodeCount = cache.decodeCount;

  // `valuePortfolioBuilt` hands back a structured object, not a JSON string.
  const first = portfolio.valuePortfolioBuilt(loaded.portfolio, EMPTY_MARKET, false);
  const second = portfolio.valuePortfolioBuilt(loaded.portfolio, EMPTY_MARKET, false);
  assert.equal(typeof first, 'object');
  assert.ok(JSON.stringify(first).length > 2, 'valuation must stringify to a real payload');

  assert.deepEqual(second, first);
  assert.equal(cache.decodeCount, decodeCount);
  loaded.portfolio.free();
  cache.free();
});
