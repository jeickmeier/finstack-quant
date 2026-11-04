/**
 * The `*Table` exports turn a plain result object into the `TableEnvelope`
 * Python's `to_dataframe` reads (Rust `to_table`): the columns in order, one
 * row per result row, and each cell the result's own value. The results are
 * the cross-host goldens of `valuations_parity.golden.json`.
 *
 * Requires the wasm-pack web build: npm run build (mise run wasm-build).
 */
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { valuations } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

const golden = JSON.parse(
  readFileSync(new URL('./valuations_parity.golden.json', import.meta.url), 'utf8')
);
const { instruments } = valuations;

/** Column name -> values, in table order. */
const columns = (table) =>
  Object.fromEntries(table.columns.map((column) => [column.name, column.data.values]));

test('bondWorkoutLadderTable has one row per exit', () => {
  const { ladder } = golden.bond_workout_ladder.cases[0];
  const table = instruments.bondWorkoutLadderTable(ladder);
  const cells = columns(table);
  assert.deepEqual(Object.keys(cells), [
    'date',
    'kind',
    'redemption_price_pct',
    'redemption_amount',
    'currency',
    'make_whole',
    'yield_rate',
    'i_spread',
    'is_worst',
  ]);
  assert.equal(table.row_count, ladder.rows.length);
  assert.deepEqual(
    cells.kind,
    ladder.rows.map((row) => row.kind)
  );
  assert.deepEqual(
    cells.is_worst,
    ladder.rows.map((row) => row.is_worst)
  );
  assert.deepEqual(
    cells.yield_rate,
    ladder.rows.map((row) => row.yield_rate)
  );
  assert.deepEqual(
    cells.redemption_amount,
    ladder.rows.map((row) => Number(row.redemption_amount.amount))
  );
  assert.equal(table.columns.find((column) => column.name === 'make_whole').data.type, 'boolean');
});

test('bondQuoteSetTable is one row, a quote that does not apply null', () => {
  const { quotes } = golden.compute_quotes.cases[0];
  const table = instruments.bondQuoteSetTable(quotes);
  assert.equal(table.row_count, 1);
  assert.deepEqual(
    columns(table),
    Object.fromEntries(Object.entries(quotes).map(([name, value]) => [name, [value]]))
  );
});

test('convertibleSpotLadderTable has one row per share price', () => {
  const ladder = golden.convertible_spot_ladder.supplied;
  const cells = columns(instruments.convertibleSpotLadderTable(ladder));
  assert.deepEqual(Object.keys(cells), Object.keys(ladder.rows[0]));
  for (const name of Object.keys(cells)) {
    assert.deepEqual(
      cells[name],
      ladder.rows.map((row) => row[name]),
      name
    );
  }
});

test('poolReportTable stacks obligor, industry and rating rows', () => {
  const { report } = golden.structured.pool_report;
  const cells = columns(instruments.poolReportTable(report));
  const rows = [
    ...report.by_obligor.map((row) => ['obligor', row]),
    ...report.by_industry.map((row) => ['industry', row]),
    ...report.by_rating.map((row) => ['rating', row]),
  ];
  assert.deepEqual(Object.keys(cells), [
    'dimension',
    'key',
    'balance',
    'currency',
    'balance_pct',
    'asset_count',
  ]);
  assert.deepEqual(
    cells.dimension,
    rows.map(([dimension]) => dimension)
  );
  assert.deepEqual(
    cells.key,
    rows.map(([, row]) => row.key)
  );
  assert.deepEqual(
    cells.balance_pct,
    rows.map(([, row]) => row.balance_pct)
  );
  assert.deepEqual(
    cells.asset_count.map(Number),
    rows.map(([, row]) => row.asset_count)
  );
});

test('a table export rejects an object that is not its result', () => {
  for (const table of [
    instruments.bondWorkoutLadderTable,
    instruments.bondQuoteSetTable,
    instruments.convertibleSpotLadderTable,
    instruments.poolReportTable,
  ]) {
    assert.throws(
      () => table({}),
      (error) => error.kind === 'validation'
    );
  }
});
