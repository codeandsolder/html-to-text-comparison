import { readFile } from 'node:fs/promises';

async function parseSnapshot(kind, html, url) {
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
            return NodeHtmlMarkdown.translate(html);
        }
        default:
            throw new Error(`Unknown snapshot parser: ${kind}`);
    }
}

export { parseSnapshot };

if (process.argv[1] && (await import('node:path')).resolve(process.argv[1]) === (await import('node:url')).fileURLToPath(import.meta.url)) {
    const [kind, path, url] = process.argv.slice(2);
    if (!kind || !path || !url) {
        console.error('Usage: snapshot-parsers.mjs <parser> <html-file> <source-url>');
        process.exitCode = 2;
    } else {
        try {
            const text = await parseSnapshot(kind, await readFile(path, 'utf8'), url);
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
