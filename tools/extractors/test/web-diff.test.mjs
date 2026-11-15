import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { runInNewContext } from 'node:vm';
import * as Diff from 'diff';

const html = await readFile(new URL('../../../src/web_ui.html', import.meta.url), 'utf8');
const packageInfo = JSON.parse(await readFile(new URL('../package.json', import.meta.url), 'utf8'));
const browserVersion = html.match(/cdn\.jsdelivr\.net\/npm\/diff@([^/]+)\//)?.[1];
assert.equal(browserVersion, packageInfo.devDependencies.diff, 'Browser and Node jsdiff versions diverged');

function functionSource(name, next) {
    const start = html.indexOf('        function ' + name + '(');
    const end = html.indexOf('        function ' + next + '(', start);
    assert.ok(start >= 0 && end > start, 'function not found: ' + name);
    return html.slice(start, end);
}
const source =
    functionSource('normalizeLineForMatching', 'pushAlignedLines') +
    functionSource('alignChangedLines', 'pushAlignedBlock');

function align(left, right) {
    return JSON.parse(JSON.stringify(runInNewContext(
        source + '\nalignChangedLines(left, right)',
        { Diff, left, right }
    )));
}

test('normalization-aware alignment anchors unequal formatting', () => {
    assert.deepEqual(
        align(['# Header', 'one', '- bullet', 'end'], ['Header', 'inserted', '* bullet', 'end']),
        [[0, 0], [2, 2], [3, 3]]
    );
});

test('empty normalized lines are not spurious anchors', () => {
    assert.deepEqual(align(['*', 'important', ''], ['', 'important', '???']), [[1, 1]]);
});

test('large changed blocks align without a quadratic DP matrix', () => {
    const left = Array.from({ length: 900 }, (_, index) => 'line ' + index);
    const right = left.map((line, index) => index % 7 === 0 ? 'inserted ' + index : line);
    const matched = align(left, right);
    assert.ok(matched.length > 700);
    assert.deepEqual(matched[0], [1, 1]);
});
