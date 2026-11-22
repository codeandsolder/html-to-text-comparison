// Resolve direct extractor dependencies using Node's actual package exports,
// but never silently fall back to an installation above the selected assets.
const packageRoot = new URL('./node_modules/', import.meta.url).href;

export async function importLocalPackage(specifier) {
    const resolved = import.meta.resolve(specifier);
    if (!resolved.startsWith(packageRoot)) {
        throw new Error(`Package ${specifier} resolved outside the selected asset tree: ${resolved}`);
    }
    return import(resolved);
}
