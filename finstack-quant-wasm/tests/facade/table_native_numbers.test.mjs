import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import init, { statements } from '../../index.js';

await init({
  module_or_path: readFileSync(new URL('../../pkg/finstack_quant_wasm_bg.wasm', import.meta.url)),
});

function model() {
  return {
    id: 'native-table-results',
    schema_version: 1,
    periods: {
      calendar: 'gregorian',
      periods: [
        { id: '2025Q1', start: '2025-01-01', end: '2025-04-01', is_actual: false },
        { id: '2025Q2', start: '2025-04-01', end: '2025-07-01', is_actual: false },
      ],
    },
    nodes: {
      nan: { node_id: 'nan', node_type: 'value', values: { '2025Q1': 100, '2025Q2': 110 } },
      other: { node_id: 'other', node_type: 'value', values: { '2025Q1': 1, '2025Q2': 2 } },
    },
  };
}

function result() {
  const evaluator = new statements.Evaluator();
  try {
    return evaluator.evaluate(model());
  } finally {
    evaluator.free();
  }
}
const values = (table, name) => table.columns.find((column) => column.name === name).data.values;

test('statement table outputs keep native numbers, nulls for valueless cells and literal identities', () => {
  const input = result();
  input.cells.nan['2025Q2'] = { status: 'missing' };
  input.cells.other['2025Q1'] = { status: 'unavailable', reason: 'missing_mask' };
  const wide = statements.statementResultToTableWide(input);
  assert.deepEqual(values(wide, 'nan'), [100, null]);
  assert.deepEqual(values(wide, 'other'), [null, 2]);

  const long = statements.statementResultToTableLong(input);
  const rows = values(long, 'node_id')
    .map((node, index) =>
      ['period_id', 'status', 'value', 'reason', 'currency'].reduce(
        (row, name) => [...row, values(long, name)[index]],
        [node]
      )
    )
    .sort();
  assert.deepEqual(rows, [
    ['nan', '2025Q1', 'value', 100, null, null],
    ['nan', '2025Q2', 'missing', null, null, null],
    ['other', '2025Q1', 'unavailable', null, 'missing_mask', null],
    ['other', '2025Q2', 'value', 2, null, null],
  ]);
});

test('statement tables reject a value cell that is not finite', () => {
  const input = result();
  input.cells.nan['2025Q1'] = { status: 'value', value: 'nan' };
  assert.throws(
    () => statements.statementResultToTableLong(input),
    (e) => e.kind === 'validation'
  );
});

test('Monte Carlo nested path table retains numeric storage and literal metric identities', () => {
  const evaluator = new statements.Evaluator();
  try {
    const output = evaluator.evaluateMonteCarlo(model(), {
      n_paths: 2,
      seed: '7',
      metrics: ['nan', 'other'],
      include_path_data: true,
    });
    assert.ok(output.path_data.columns.some((column) => column.data.type === 'float64'));
    for (const column of output.path_data.columns) {
      if (column.data.type === 'float64')
        assert.ok(column.data.values.every((value) => typeof value === 'number'));
    }
    assert.ok(
      output.path_data.columns.some(
        (column) => column.data.type === 'string' && column.data.values.includes('nan')
      )
    );
  } finally {
    evaluator.free();
  }
});
