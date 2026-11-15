# Immutable parser quality fixtures

These synthetic HTML snapshots are intentionally independent of live sites, network time and licensing. They are designed to expose distinct extraction failures: removal of real content, leakage of unrelated boilerplate, and structural destruction (tables and code). Keep relevant text and negative sentinels stable across dependency upgrades. Add new cases only when they exercise a genuinely distinct HTML shape.

- article-with-navigation.html: article main-content retention, nav/sidebar/footer contamination.
- technical-reference.html: documentation with registers, a table and a preformatted code block.

The Resiliparse regression tests currently exercise both. Similar tests for other extractors should preserve the source HTML and score content coverage separately from boilerplate leakage and formatting fidelity. Results from these two tiny samples do not constitute a representative real-world ranking.
