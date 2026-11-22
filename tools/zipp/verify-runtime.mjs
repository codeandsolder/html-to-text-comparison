import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';

const [selected, ...extra] = process.argv.slice(2);
if (!selected || extra.length) throw new Error('usage: node tools/zipp/verify-runtime.mjs /absolute/path/to/zipp');
const zipp = resolve(selected);
const root = mkdtempSync(join(tmpdir(), 'zipp-runtime-probe-'));
try {
    const files = {
        'node_modules/@fixture/dual/package.json': JSON.stringify({
            name: '@fixture/dual',
            type: 'module',
            exports: { '.': {import:'./import.mjs', require:'./require.cjs'}, './sub':'./sub.mjs' },
        }),
        'node_modules/@fixture/dual/import.mjs': 'export default "import";',
        'node_modules/@fixture/dual/require.cjs': 'module.exports = "require";',
        'node_modules/@fixture/dual/sub.mjs': 'export default "sub";',
        'nested/file with space.mjs': 'export default "space";',
        'nested/helper.mjs': 'export const resolveFromNested = import.meta.resolve;',
        'main.mjs': [
            'import chosen from "@fixture/dual";',
            'import { resolveFromNested } from "./nested/helper.mjs";',
            'console.log(chosen);',
            'console.log(import.meta.resolve("@fixture/dual"));',
            'console.log(import.meta.resolve("@fixture/dual/sub"));',
            'console.log(import.meta.resolve("node:fs"));',
            'console.log(resolveFromNested("./file with space.mjs"));',
            'try { import.meta.resolve("@fixture/missing"); console.log("unexpected success"); } catch { console.log("missing package"); }',
            'await Promise.resolve();',
            'process.on("exit", c => console.log("exit", c));',
            'process.exitCode = 17;',
        ].join('\n'),
    };
    for (const [relative, text] of Object.entries(files)) {
        const path = join(root, relative);
        mkdirSync(join(path, '..'), {recursive:true});
        writeFileSync(path, text);
    }
    const run = executable => spawnSync(executable, [join(root,'main.mjs')], {
        cwd:root, encoding:'utf8', timeout:30_000, maxBuffer:1_000_000,
    });
    const expected = run(process.execPath);
    const actual = run(zipp);
    assert.equal(expected.status, 17, expected.stderr);
    assert.equal(actual.status, expected.status, actual.stderr);
    assert.equal(actual.stdout, expected.stdout);
    assert.equal(actual.stderr, expected.stderr);
    console.log('Package export conditions, scoped subpaths, builtin URL, detached nested resolver, percent encoding, missing-package error and module exit status match Node.');
} finally {
    rmSync(root, {recursive:true, force:true});
}
