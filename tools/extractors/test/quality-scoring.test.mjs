import test from 'node:test';
import assert from 'node:assert/strict';
import { referenceFromHtml, scoreOutput, shingles } from '../../quality/evaluate.mjs';

test('independent semantic source catches main text dropped by a navigation-only extractor', () => {
  const html = '<html><body><nav>Shop store cart products delivery times invoices menu</nav>' +
    '<main><article><p>This experimental engine burns low emissions biofuel during summer road tests.</p>' +
    '<p>Measured voltage ripple stayed stable throughout the entire thermal cycling experiment.</p>' +
    '<p>Input protection circuits successfully survived repeated electrostatic discharge conditions.</p>' +
    '</article></main></body></html>';
  const ref = referenceFromHtml(html);
  assert.ok(ref.eligible);
  const good = scoreOutput(ref, 'This experimental engine burns low emissions biofuel during summer road tests. Measured voltage ripple stayed stable throughout the entire thermal cycling experiment. Input protection circuits successfully survived repeated electrostatic discharge conditions.');
  const bad = scoreOutput(ref, 'Shop store cart products delivery times invoices menu');
  assert.ok(good.main_recall > 0.95, good.main_recall);
  assert.equal(bad.main_recall, 0);
  assert.ok(bad.nav_density > good.nav_density);
});

test('missing semantic main is not silently treated as a quality ranking reference', () => {
  const ref = referenceFromHtml('<html><body><div>Some text inside a generic layout.</div></body></html>');
  assert.equal(ref.eligible, false);
  assert.equal(scoreOutput(ref, 'Some text inside a generic layout.').main_recall, null);
});

test('access challenge is excluded from article relevance scoring', () => {
  const ref = referenceFromHtml('<html><head><title>Just a moment...</title></head><body><main><p>Verify you are human: geo.captcha-delivery.com challenge.</p></main></body></html>');
  assert.equal(ref.blocked, true);
  assert.equal(ref.eligible, false);
});

test('unicode normalization and interleaved markdown markup retain textual shingles', () => {
  const a = shingles('Solar-array voltage and high-speed charging buses');
  const b = shingles('**Solar array voltage**, and high speed charging buses');
  assert.ok(a.size > 0);
  assert.equal(a.size, b.size);
  assert.deepEqual(a, b);
});

test('failed extractors count as zero recall on eligible pages', () => {
  const html = '<html><body><main>' +
    '<p>Magnetic field gradients disturb the control loop during the first calibration cycle.</p>' +
    '<p>Additional measurement samples establish a clear improvement after calibration feedback.</p>' +
    '<p>Repeated measurements confirm the earlier result in multiple independent conditions.</p>' +
    '</main></body></html>';
  const ref = referenceFromHtml(html);
  assert.equal(ref.eligible, true);
  const failed = scoreOutput(ref, '[ERROR] converter crashed', 'failed');
  assert.equal(failed.main_recall, 0);
  assert.equal(failed.nav_density, null);
});

test('headings and table cell text retention are measured independently', () => {
  const ref = referenceFromHtml('<html><body><main><article>' +
    '<h2>Latency analysis over five trials</h2>' +
    '<p>The experimental device reported stable response times over the complete collection.</p>' +
    '<p>Battery current remained almost unchanged during the several repeat runs.</p>' +
    '<table><tr><th>Peak voltage at load</th><td>Thirty volts under load</td></tr></table>' +
    '</article></main></body></html>');
  const complete = scoreOutput(ref,
    '# Latency analysis over five trials\nThe experimental device reported stable response times over the complete collection.\n' +
    'Battery current remained almost unchanged during the several repeat runs.\nPeak voltage at load Thirty volts under load');
  const noTable = scoreOutput(ref,
    'Latency analysis over five trials The experimental device reported stable response times over the complete collection.\n' +
    'Battery current remained almost unchanged during the several repeat runs.');
  assert.equal(complete.heading_retention, 1);
  assert.equal(complete.table_cell_retention, 1);
  assert.equal(noTable.table_cell_retention, 0);
});
