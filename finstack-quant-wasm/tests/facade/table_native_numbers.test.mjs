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
    periods: [
      { id: '2025Q1', start: '2025-01-01', end: '2025-04-01', is_actual: false },
      { id: '2025Q2', start: '2025-04-01', end: '2025-07-01', is_actual: false },
    ],
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

test('statement table outputs retain native nonfinite numbers and literal sentinel-like identities', () => {
  const input = result();
  input.nodes.nan = { '2025Q1': 'nan', '2025Q2': 'inf' };
  input.nodes.other = { '2025Q1': '-inf' };
  const wide = statements.statementResultToTableWide(input);
  assert.ok(Number.isNaN(values(wide, 'nan')[0]));
  assert.equal(values(wide, 'nan')[1], Infinity);
  assert.equal(values(wide, 'other')[0], -Infinity);
  assert.ok(Number.isNaN(values(wide, 'other')[1]));

  const long = statements.statementResultToTableLong(input);
  const nodeIds = values(long, 'node_id');
  const periods = values(long, 'period_id');
  const numeric = values(long, 'value');
  const cell = (node, period) =>
    numeric[nodeIds.findIndex((id, index) => id === node && periods[index] === period)];
  assert.deepEqual(
    nodeIds.filter((id) => id === 'nan'),
    ['nan', 'nan']
  );
  assert.ok(Number.isNaN(cell('nan', '2025Q1')));
  assert.equal(cell('nan', '2025Q2'), Infinity);
  assert.equal(cell('other', '2025Q1'), -Infinity);
  assert.deepEqual(values(long, 'currency'), [null, null, null]);
});

test('Monte Carlo nested path table retains numeric storage and literal metric identities', () => {
  const evaluator = new statements.Evaluator();
  try {
    const output = evaluator.evaluateMonteCarlo(model(), {
      n_paths: 2,
      seed: 7,
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
