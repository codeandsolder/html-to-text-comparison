import test from 'node:test';
import assert from 'node:assert/strict';
import { parseSnapshot } from '../snapshot-parsers.mjs';

const articleBody = 'The primary content of the article is present and readable. '.repeat(25);
const fixture = `<html><head><title>Fixture</title></head><body><nav>Navigation should not be copied</nav><article><h1>Fixture article</h1><p>${articleBody}</p></article></body></html>`;

test('Defuddle extracts only local snapshot in non-fetching mode', async () => {
    const text = await parseSnapshot('defuddle', fixture, 'https://example.com/article');
    assert.match(text, /primary content of the article/);
    assert.doesNotMatch(text, /Navigation should not be copied/);
});

test('node-html-markdown converts the supplied full document', async () => {
    const text = await parseSnapshot('node-html-markdown', '<h1>Header</h1><p>A <strong>strong</strong> line</p>', 'https://example.com');
    assert.match(text, /# Header/);
    assert.match(text, /\*\*strong\*\*/);
});

test('node-html-markdown preserves defaults when only selected options change', async () => {
    const text = await parseSnapshot('node-html-markdown', '<h1>Header</h1><p>A <strong>strong</strong> line</p>', 'https://example.com', {
        strong_delimiter: '__',
        use_inline_links: false,
        max_consecutive_newlines: 0,
    });
    assert.match(text, /# Header/);
    assert.match(text, /__strong__/);
    assert.doesNotMatch(text, /undefined/);
});

test('webforai reads the same snapshot and preserves technical tables and code', async () => {
    const { readFile } = await import('node:fs/promises');
    const raw = await readFile(new URL('../../../fixtures/technical-reference.html', import.meta.url), 'utf8');
    const text = await parseSnapshot('webforai', raw, 'https://example.com/docs/registers');
    assert.match(text, /\\| CTRL_A \\| 0x18 \\| 7:0 \\|/);
    assert.match(text, /0x00ff_0000/);
    assert.ok(text.includes(String.fromCharCode(96).repeat(3)), 'fenced code block missing');
    assert.doesNotMatch(text, /IRRELEVANT_DOCS_NAVIGATION/);
});

test('webforai ignores navigation but retains all article paragraphs', async () => {
    const { readFile } = await import('node:fs/promises');
    const raw = await readFile(new URL('../../../fixtures/article-with-navigation.html', import.meta.url), 'utf8');
    const text = await parseSnapshot('webforai', raw, 'https://example.com/article');
    for (const marker of ['CONTENT_SENTINEL_ALPHA', 'CONTENT_SENTINEL_BETA', 'CONTENT_SENTINEL_GAMMA']) {
        assert.ok(text.includes(marker), 'missing ' + marker);
    }
    assert.doesNotMatch(text, /UNRELATED_NAVIGATION_HELP_CENTER/);
    assert.doesNotMatch(text, /UNRELATED_SIDEBAR_SPONSORED_LINKS/);
});

test('unknown parser fails explicitly', async () => {
    await assert.rejects(parseSnapshot('missing', fixture, 'https://example.com'), /Unknown snapshot parser/);
});

test('Turndown preserves options and removes explicitly selected elements', async () => {
    const html = '<h1>Title</h1><p><strong>Body</strong></p><script>discardMe()</script>';
    const text = await parseSnapshot('turndown', html, undefined, {
        options: { headingStyle: 'atx', strongDelimiter: '__' },
        remove_tags: ['script'],
    });
    assert.match(text, /# Title/);
    assert.match(text, /__Body__/);
    assert.doesNotMatch(text, /discardMe/);
});

test('Mozilla Readability parses the acquired snapshot without navigation', async () => {
    const text = await parseSnapshot('mozilla-readability', fixture, undefined, {
        char_threshold: 500,
        nb_top_candidates: 5,
    });
    assert.match(text, /primary content of the article/);
    assert.doesNotMatch(text, /Navigation should not be copied/);
});

test('URL-dependent parsers require a source URL explicitly', async () => {
    for (const kind of ['defuddle', 'webforai']) {
        await assert.rejects(parseSnapshot(kind, fixture, undefined), /requires the source URL/);
    }
});

test('direct packages cannot silently resolve from an ancestor installation', async (t) => {
    const { copyFile, mkdir, mkdtemp, rm, writeFile } = await import('node:fs/promises');
    const { tmpdir } = await import('node:os');
    const { join } = await import('node:path');
    const { pathToFileURL } = await import('node:url');
    const root = await mkdtemp(join(tmpdir(), 'extractor-assets-'));
    t.after(() => rm(root, { recursive: true, force: true }));
    const assets = join(root, 'assets');
    const ancestorPackage = join(root, 'node_modules', 'test-package');
    await mkdir(assets);
    await mkdir(ancestorPackage, { recursive: true });
    const loader = join(assets, 'local-package.mjs');
    await copyFile(new URL('../local-package.mjs', import.meta.url), loader);
    await writeFile(join(ancestorPackage, 'package.json'), '{"exports":"./index.mjs"}');
    await writeFile(join(ancestorPackage, 'index.mjs'), 'export default "ancestor";');
    const { importLocalPackage } = await import(pathToFileURL(loader));
    await assert.rejects(importLocalPackage('test-package'), /outside the selected asset tree/);
    // Use another loader URL so Node's cached failed/successful resolution
    // cannot obscure the selected local installation.
    const localAssets = join(root, 'local-assets');
    const localPackage = join(localAssets, 'node_modules', 'test-package');
    await mkdir(localPackage, { recursive: true });
    await writeFile(join(localPackage, 'package.json'), '{"exports":"./index.mjs"}');
    await writeFile(join(localPackage, 'index.mjs'), 'export default "local";');
    const secondLoader = join(localAssets, 'local-package.mjs');
    await copyFile(loader, secondLoader);
    const local = await import(pathToFileURL(secondLoader));
    assert.equal((await local.importLocalPackage('test-package')).default, 'local');
});
