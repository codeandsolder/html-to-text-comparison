#!/usr/bin/env bash
set -euo pipefail
if (( $# < 3 )); then
    echo "usage: $0 ASSET_ROOT INTERPRETER ADAPTER..." >&2
    exit 64
fi
assets=$(realpath "$1")
interpreter=$(realpath "$2")
shift 2
runtime=$("$interpreter" -I -B -c 'import sys; print(sys.base_prefix)')
runtime=$(realpath "$runtime")
case "$runtime" in /|/usr|/usr/local) echo "Use a relocatable Python distribution, not $runtime" >&2; exit 1;; esac
test -x "$runtime/bin/python3"
for adapter in "$@"; do
    case "$adapter" in
        html2text|markdownify|resiliparse|trafilatura|e2m) ;;
        *) echo "Unknown Python adapter: $adapter" >&2; exit 64 ;;
    esac
    dir="$assets/python/$adapter"
    test -f "$dir/requirements.txt"
    if [[ -e "$dir/packages" || -e "$dir/runtime" || -L "$dir/runtime" ]]; then
        echo "Provision into a fresh asset tree: $dir already has an environment" >&2
        exit 1
    fi
    stage=$(mktemp -d "$dir/.provision.XXXXXXXX")
    trap 'rm -rf -- "$stage"' EXIT
    uv pip install --python "$interpreter" --no-python-downloads --link-mode copy --target "$stage/packages" -r "$dir/requirements.txt"
    if [[ "$adapter" == e2m ]]; then
        "$interpreter" -I -B -c 'import sys; sys.path.insert(0,sys.argv[1]); import nltk; [nltk.download(n,download_dir=sys.argv[1]+"/nltk_data",quiet=True,raise_on_error=True) for n in ("punkt_tab","averaged_perceptron_tagger_eng")]' "$stage/packages"
    fi
    mv "$stage/packages" "$dir/packages"
    ln -s "$runtime" "$dir/runtime"
    rmdir "$stage"
    trap - EXIT
done
