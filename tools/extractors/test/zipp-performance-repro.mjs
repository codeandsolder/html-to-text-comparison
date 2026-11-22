import { Defuddle } from 'defuddle/node';
import { parseHTML } from 'linkedom';
import { htmlToMarkdown } from 'webforai';

const kind = process.argv[2] || 'defuddle';
const html = '<!doctype html><html><head><title>Engineering notes</title></head><body><nav>Navigation <a href="/index">Index</a></nav><main><article><h1>Engineering notes</h1>' +
  Array.from({length: 48}, (_, i) =>
    '<section><h2>Experiment ' + i + '</h2><p>We measured the response of the circuit over a range of frequencies. The amplifier gain changes as the load and the supply voltage vary. The results illustrate the interaction between impedance and bandwidth.</p><ul><li>Frequency: ' + (100+i) + ' kHz</li><li><a href="/notes/' + i + '">Measurement notes</a></li></ul><pre><code>sample = input * gain;</code></pre></section>'
  ).join('') + '</article></main><footer>Footer</footer></body></html>';
const url = 'https://example.com/article';
const parse = async () => {
  if (kind === 'defuddle') {
    const { document } = parseHTML(html);
    const result = await Defuddle(document, url, { markdown: true, useAsync: false });
    return result?.contentMarkdown ?? result?.content ?? '';
  }
  return htmlToMarkdown(html, {baseUrl:url});
};
const now = () => Number(process.hrtime.bigint()) / 1e6;
const start = now();
const ref = await parse();
const firstMs = now() - start;
const repeatMs = [];
for (let i=0; i<7; ++i) {
  const t = now();
  const result = await parse();
  repeatMs.push(now()-t);
  if (result !== ref) throw Error('unstable output');
}
console.log(JSON.stringify({kind, htmlBytes:Buffer.byteLength(html), outputBytes:Buffer.byteLength(ref), firstMs, repeatMs}));
