# HTML extraction voting arena

A keyboard-first, blind, side-by-side evaluation interface for the fixed
100-page js-redox HTML sample, integrated into html-to-text-comparison.

Built as a small standalone Rust web binary with Axum 0.8 + SQLx 0.9 /
SQLite, following the typed state, structured configuration, CSRF/origin
boundary, and explicit readiness practices in the canonical
rust-web-fullstack reference: https://github.com/codeandsolder/rust-web-fullstack.
The browser uses a tiny local keyboard controller rather than shipping a
Leptos hydration bundle for an otherwise server-driven single-page UI.

## Usage

Compile normally with Rust 1.99 and managed sccache:

    cargo +1.99.1 build --locked --release --manifest-path vote-web/Cargo.toml

Start with:

    VOTER_DATASET=/home/jan/datasets/html-to-text-comparison/js-redox-random100-20261115 \
    VOTER_DATABASE=/home/jan/datasets/html-to-text-comparison/js-redox-random100-20261115/voting/votes.sqlite \
    VOTER_BIND=127.0.0.1:18578 \
    ./vote-web/target/release/html-extraction-voter

No network fetches are made. On boot the app validates all source and
extraction SHA-256 hashes from the manifest/report. The binary serves its own
CSS/JavaScript and never serves HTML from the corpus as executable markup.

GET /api/next issues a unique ballot; POST /api/vote writes one durable
decision (left, equal, right) to SQLite; POST /api/undo reverses the specified
ballot; GET /api/rankings replays the durable vote history into provisional
Elo-style ratings (draw=0.5); GET /api/output/:capture/:engine serves the
uncropped extraction as plain text; GET /health and GET /readyz monitor
process/database state.

The compare choices are intentionally blind: engine labels are hidden
until revealed. Use Left / Down / Right arrows or click the three voting
buttons; S skips without submitting, Z undoes the most recent vote from that
browser session. Skipping never affects ranking. Pair selection spreads votes
across pages and engines, preferring less-tested pairs. All feedback is stored
locally. It is exploratory Elo, not calibrated ground-truth quality.

The Certifex node needs one additional relative service label mapping
to localhost TCP port 18578, e.g. extract-vote = 18578, resulting in
https://extract-vote.onhir.eu on the existing private tailnet.

The database and captured HTML bodies do not belong in Git.

## Deployment

The systemd unit template is at deploy/html-extraction-voter.service. Keep
the persistent source and vote database under /home/jan/datasets/; copy only
the verified release executable to /home/jan/services/html-extraction-voter/.

On Acer, install the unit under /etc/systemd/system/ and add the exact service
entry to the existing /etc/certifex-minimus/node.toml under [services].
Certifex Iuvenis watches this file and reconciles new SANs/DNS automatically.
Do not restart an unrelated Certifex ingress merely to load a new route.

For a future executable upgrade, use an atomically staged/swap-named binary
and restart only html-extraction-voter.service, retaining the SQLite vote DB.
The underlying data is private and not part of GitHub source control.
