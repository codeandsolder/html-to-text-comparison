# Lightpanda: acquisition backend (not an extractor)

Lightpanda's `fetch` CLI **navigates to the supplied URL**, executes JavaScript and obtains a rendered DOM. Running it after ordinary HTTP acquisition would fetch the page a second time and make the output incomparable with the rest of the snapshot-based extractors.

Since the single-acquisition redesign, Lightpanda is offered only as an explicit page acquisition mode:

```sh
cargo run --release -- --acquire lightpanda https://example.com
```

The CLI uses `docker exec lightpanda lightpanda fetch --dump html <url>`. The resulting **rendered HTML snapshot** is delivered to every selected converter, including Mozilla Readability, which operates locally. Selecting `--acquire http` instead uses one HTTP GET and gives all converters the unrendered response. Neither path performs a second root-page acquisition through another extractor.

Lightpanda naturally loads script/style/image subresources during its single browser navigation. Those subrequests are not independent top-level navigations, and they can make the rendered snapshot differ substantially from the plain HTTP result. Compare acquisitions separately rather than mixing them in one score. Requires an already-running Docker container named `lightpanda`; the program does not create one.
