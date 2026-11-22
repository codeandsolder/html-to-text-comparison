import { readFile } from 'node:fs/promises';

async function parseSnapshot(kind, html, url, config = {}) {
    switch (kind) {
        case 'defuddle': {
            const { Defuddle } = await import('defuddle/node');
            const { parseHTML } = await import('linkedom');
            const { document } = parseHTML(html);
            // Never enable asynchronous fallback APIs, which can re-acquire the page.
            const result = await Defuddle(document, url, { markdown: true, useAsync: false });
            return result?.contentMarkdown ?? result?.content ?? '';
        }
        case 'webforai': {
            const { htmlToMarkdown } = await import('webforai');
            return htmlToMarkdown(html, { baseUrl: url });
        }
        case 'node-html-markdown': {
            const { NodeHtmlMarkdown } = await import('node-html-markdown');
            const options = {
                codeFence: config.code_fence,
                bulletMarker: config.bullet_marker,
                indent: config.indent,
                codeBlockStyle: config.code_block_style,
                emDelimiter: config.em_delimiter,
                strongDelimiter: config.strong_delimiter,
                strikeDelimiter: config.strike_delimiter,
                maxConsecutiveNewlines: config.max_consecutive_newlines,
                keepDataImages: config.keep_data_images,
                useLinkReferenceDefinitions: config.use_link_reference_definitions,
                useInlineLinks: config.use_inline_links,
                ignore: config.ignore?.length ? config.ignore : undefined,
                blockElements: config.block_elements?.length ? config.block_elements : undefined,
            };
            return NodeHtmlMarkdown.translate(html, Object.fromEntries(
                Object.entries(options).filter(([, value]) => value !== undefined)
            ));
        }
        default:
            throw new Error(`Unknown snapshot parser: ${kind}`);
    }
}

export { parseSnapshot };

if (process.argv[1] && (await import('node:path')).resolve(process.argv[1]) === (await import('node:url')).fileURLToPath(import.meta.url)) {
    const [kind, path, url, optionsJson] = process.argv.slice(2);
    if (!kind || !path || !url) {
        console.error('Usage: snapshot-parsers.mjs <parser> <html-file> <source-url>');
        process.exitCode = 2;
    } else {
        try {
            const text = await parseSnapshot(kind, await readFile(path, 'utf8'), url, JSON.parse(optionsJson || '{}'));
            if (!text) {
                console.error(`${kind} returned no content`);
                process.exitCode = 3;
            } else {
                process.stdout.write(text);
            }
        } catch (error) {
            console.error(error);
            process.exitCode = 1;
        }
    }
}
