# Corpus

The corpus for this worked example is the original Python graphify at
commit `91f4d12` of https://github.com/safishamsi/graphify (MIT) — 20
Python modules, tests, skill files, and docs, ~0.5 MB of text.

The source is intentionally **not vendored** here: it is third-party
executable code (URL fetchers, exporters) that does not belong in this
repository's tree. Clone it and run the example yourself:

```bash
git clone https://github.com/safishamsi/graphify corpus
cd corpus && git checkout 91f4d12
nodesify-graphify run . --embed --wiki
```

All numbers in `review.md` were produced from that exact commit.
