# Graph Report

**Nodes:** 1083 | **Edges:** 6823 | **Communities:** 84 | **Modularity:** 0.509

_Built by graphify v0.7.0._

## Communities

- **get** (379 nodes, cohesion 0.70)
- **walk()** (150 nodes, cohesion 0.77)
- **sample.c** (97 nodes, cohesion 0.63)
- **exists** (64 nodes, cohesion 0.45)
- **graphify.wiki** (33 nodes, cohesion 0.57)
- **skill.md** (23 nodes, cohesion 0.59)
- **validate.py** (22 nodes, cohesion 0.34)
- **test_benchmark.py** (19 nodes, cohesion 0.38)
- **import_module** (18 nodes, cohesion 0.50)
- **Architecture** (13 nodes, cohesion 0.61)

... and 74 more communities (see the MCP list_communities tool).

## Key Files

- **graphify/extract.py** (2501 edge endpoints)
- **graphify/analyze.py** (1391 edge endpoints)
- **tests/test_languages.py** (623 edge endpoints)
- **tests/test_multilang.py** (582 edge endpoints)
- **graphify/export.py** (532 edge endpoints)
- **graphify/__init__.py** (448 edge endpoints)
- **graphify/serve.py** (448 edge endpoints)
- **tests/test_analyze.py** (425 edge endpoints)
- **graphify/benchmark.py** (345 edge endpoints)
- **tests/test_serve.py** (335 edge endpoints)

## Hub Nodes (God Nodes)

- **walk()** (degree: 337, community: walk())
- **extract.py** (degree: 151, community: walk())
- **_make_id()** (degree: 132, community: walk())
- **add_node** (degree: 126, community: walk())
- **walk_calls()** (degree: 118, community: walk())
- **extract_c()** (degree: 114, community: walk())
- **to_obsidian()** (degree: 99, community: get)
- **graphify** (degree: 98, community: get)
- **serve()** (degree: 90, community: get)
- **generate()** (degree: 83, community: get)

## Surprising Connections

Top 25 of the cross-community edges, ranked by novelty (bigger, more cohesive communities joined by fewer edges score higher):

- **observer** -> **watchdog.observers** (similar_to) [sample.c -> graphify.wiki] (score: 33.00)
- **skill.md** -> **sample.md** (similar_to) [skill.md -> sample.c] (score: 23.00)
- **main()()** -> **main()** (similar_to) [sample.c -> exists] (score: 21.33)
- **watch()** -> **resolve** (calls) [sample.c -> exists] (score: 21.33)
- **watch()** -> **startswith** (calls) [sample.c -> exists] (score: 21.33)
- **ingest()** -> **valueerror** (calls) [exists -> import_module] (score: 18.00)
- **__getattr__()** -> **graphify.wiki** (similar_to) [import_module -> graphify.wiki] (score: 18.00)
- **test_watch_raises_without_watchdog()** -> **watch()** (calls) [import_module -> sample.c] (score: 18.00)
- **api/checks.py** -> **api.py** (similar_to) [validate.py -> x.py] (score: 12.00)
- **function_definition** -> **function_item** (similar_to) [method_declaration -> validate.py] (score: 12.00)
- **import_declaration** -> **use_declaration** (similar_to) [import_module -> method_declaration] (score: 12.00)
- **Find best matching node** -> **For /graphify add** (contains) [skill.md -> graphify.wiki] (score: 11.50)
- **Find best matching node** -> **For /graphify add** (contains) [skill.md -> graphify.wiki] (score: 11.50)
- **test_hooks.py** -> **not_a_repo** (references) [test_hooks.py -> sample.c] (score: 11.00)
- **test_hooks.py** -> **test_languages.py** (similar_to) [test_hooks.py -> walk()] (score: 11.00)
- **_download_binary()** -> **safe_fetch()** (calls) [exists -> safe_fetch()] (score: 10.00)
- **safe_fetch()** -> **request** (calls) [safe_fetch() -> sample.c] (score: 10.00)
- **test_claude_md.py** -> **wiki/index.md** (references) [exists -> index_md] (score: 9.00)
- **test_extract.py** -> **httpx_client** (references) [walk() -> README.md] (score: 8.00)
- ***process(const char *input)()** -> **validate** (calls) [sample.c -> validate.py] (score: 7.33)
- **Processor** -> **validate** (calls) [sample.c -> validate.py] (score: 7.33)
- **Processor** -> **validate** (calls) [sample.c -> validate.py] (score: 7.33)
- **classify_file()** -> **lower** (calls) [classify_file() -> walk()] (score: 7.00)
- **paper.md** -> **sample.md** (similar_to) [extract_pdf_text() -> sample.c] (score: 7.00)
- **run_benchmark()** -> **append** (calls) [test_benchmark.py -> walk()] (score: 6.33)

## Merged Duplicates

75 near-duplicate node(s) merged into canonical entities.

## Suggested Questions

- Why does walk() have 337 connections — shared core or coupling problem?
- observer similar_to watchdog.observers crosses a community boundary - intentional or accidental coupling?
- What are the responsibilities of the 84 communities?
