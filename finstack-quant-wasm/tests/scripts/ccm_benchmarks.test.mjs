import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import {
  CCM_NODE_CASES,
  captureOwned,
  ccmSettings,
  disposeAll,
  measureCheckedBatches,
  parseBenchmarkMode,
} from '../../benchmarks/ccm-measure.mjs';

test('selectors reject typos and combined modes before importing a runtime', () => {
  assert.equal(parseBenchmarkMode([]).mode, 'all');
  for (const mode of ['--core-models-only', '--valuation-only', '--materialization-only']) {
    assert.equal(parseBenchmarkMode([mode]).mode, mode);
  }
  assert.equal(parseBenchmarkMode(['--help']).help, true);
  assert.throws(() => parseBenchmarkMode(['--core-model-only']), /Unknown benchmark argument/);
  assert.throws(
    () => parseBenchmarkMode(['--core-models-only', '--valuation-only']),
    /mutually exclusive/
  );
  assert.throws(
    () => parseBenchmarkMode(['--core-models-only', '--core-models-only']),
    /mutually exclusive/
  );
  const file = fileURLToPath(new URL('../../benchmarks/bench.mjs', import.meta.url));
  const rejected = spawnSync(process.execPath, [file, '--misspelled-mode'], { encoding: 'utf8' });
  assert.notEqual(rejected.status, 0);
  assert.match(rejected.stderr, /Unknown benchmark argument/);
  assert.doesNotMatch(rejected.stderr, /Node build not found/);
});

test('host sampling is explicit, rejects malformed controls and ignores Criterion settings', () => {
  assert.deepEqual(ccmSettings({ FQ_BENCH_SAMPLE_SIZE: '99' }), { samples: 10, warmup_samples: 1 });
  assert.deepEqual(ccmSettings({ FQ_CCM_JS_SAMPLES: '3', FQ_CCM_JS_WARMUP_SAMPLES: '0' }), {
    samples: 3,
    warmup_samples: 0,
  });
  for (const value of ['0', '-1', '1.5', '1e2', 'NaN', ' 10', '', '9007199254740992']) {
    assert.throws(() => ccmSettings({ FQ_CCM_JS_SAMPLES: value }), /integer at least 1/);
  }
  assert.throws(() => ccmSettings({ FQ_CCM_JS_WARMUP_SAMPLES: '-1' }), /integer at least 0/);
  assert.equal(CCM_NODE_CASES.length, 22);
  assert.equal(new Set(CCM_NODE_CASES.map((row) => row.id)).size, 22);
});

test('fixed batches observe every output and normalize raw elapsed time per call', () => {
  let calls = 0;
  let live = 0;
  let peak = 0;
  let checked = 0;
  const disposed = [];
  const ticks = [100, 112, 200, 218];
  const result = measureCheckedBatches({
    operation: () => {
      calls++;
      live++;
      peak = Math.max(peak, live);
      return { sequence: calls, live: true };
    },
    validate: (value) => {
      assert.equal(value.live, true);
      checked++;
    },
    snapshot: (value) => ({ sequence: value.sequence }),
    cleanup: (value) => {
      assert.equal(value.live, true);
      value.live = false;
      live--;
      disposed.push(value.sequence);
    },
    batchSize: 3,
    samples: 2,
    warmupSamples: 1,
    clock: () => ticks.shift(),
  });
  assert.equal(calls, 10); // one preflight, three warmup, six measured calls
  assert.equal(checked, calls);
  assert.equal(live, 0);
  assert.equal(peak, 3);
  assert.equal(disposed.length, calls);
  assert.deepEqual(result.batch_milliseconds, [12, 18]);
  assert.deepEqual(result.samples, [4, 6]);
  assert.equal(result.median, 5);
  assert.equal(result.iterations, 6);
  assert.deepEqual(result.preflight_output, { sequence: 1 });
  assert.deepEqual(result.observed_outputs, [{ sequence: 7 }, { sequence: 10 }]);
  assert.deepEqual(JSON.parse(JSON.stringify(result)).observed_outputs, result.observed_outputs);
  assert.equal(ticks.length, 0);
});

test('a failed operation disposes the successful prefix and supplies no observations', () => {
  let calls = 0;
  const disposed = [];
  assert.throws(
    () =>
      measureCheckedBatches({
        operation: () => {
          calls++;
          if (calls === 3) throw new Error('intentional operation failure');
          return calls;
        },
        validate: () => {},
        cleanup: (value) => disposed.push(value),
        batchSize: 3,
        samples: 1,
        warmupSamples: 0,
        clock: () => 10,
      }),
    /intentional operation failure/
  );
  assert.deepEqual(disposed, [1, 2]);
});

test('a failed output check disposes the entire measured batch', () => {
  let calls = 0;
  let tick = 0;
  const disposed = [];
  assert.throws(
    () =>
      measureCheckedBatches({
        operation: () => ++calls,
        validate: (value) => {
          if (value === 2) throw new Error('intentional invalid output');
        },
        cleanup: (value) => disposed.push(value),
        batchSize: 3,
        samples: 1,
        warmupSamples: 0,
        clock: () => ++tick,
      }),
    /intentional invalid output/
  );
  assert.deepEqual(disposed, [1, 2, 3, 4]);
});

test('preflight failure cleans its result before timing or warmup', () => {
  let clocks = 0;
  let cleaned = 0;
  assert.throws(
    () =>
      measureCheckedBatches({
        operation: () => ({ invalid: true }),
        validate: () => {
          throw new Error('invalid preflight');
        },
        cleanup: () => {
          cleaned++;
        },
        batchSize: 3,
        samples: 2,
        warmupSamples: 1,
        clock: () => ++clocks,
      }),
    /invalid preflight/
  );
  assert.equal(cleaned, 1);
  assert.equal(clocks, 0);
});

test('cleanup attempts every handle and preserves the original operation failure', () => {
  const disposed = [];
  const handle = (name, throws = false) => ({
    free() {
      disposed.push(name);
      if (throws) throw new Error('intentional destructor failure');
    },
  });
  assert.throws(() => disposeAll([handle('a', true), handle('b')]), /destructor failure/);
  assert.deepEqual(disposed, ['a', 'b']);
  assert.throws(
    () =>
      captureOwned((own) => {
        own(handle('c'));
        own(handle('d', true));
        throw new Error('original construction failure');
      }),
    /original construction failure/
  );
  assert.deepEqual(disposed, ['a', 'b', 'd', 'c']);
});

test('invalid timing cannot become an accepted raw record', () => {
  for (const duration of [0, -1, Number.NaN, Number.POSITIVE_INFINITY]) {
    let tick = 0;
    let cleaned = 0;
    assert.throws(
      () =>
        measureCheckedBatches({
          operation: () => 1,
          validate: () => {},
          cleanup: () => {
            cleaned++;
          },
          batchSize: 2,
          samples: 1,
          warmupSamples: 0,
          clock: () => (tick++ === 0 ? 0 : duration),
        }),
      /positive finite batch duration/
    );
    assert.equal(cleaned, 3);
  }
});
