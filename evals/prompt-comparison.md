# Prompt comparison

Promptfoo evaluation `eval-QbV-2026-09-23T22:46:11`. One live run of each of four variants on
30 workflows, threshold 0.65, model `jev-1.13.0`. No LLM judge.

All 120 provider calls completed with zero execution errors. All saved graphs
were checked against the configured prefix and question template, and all
ordered non-self task pairs had an answer.

| Prefix | Question | Passes | Mean ordering F1 | Original (10) | LLM small (6) | LLM medium (7) | LLM complex (7) |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| tree-prefix | current-question | 14/30 | 0.9100 | 7 | 4 | 2 | 1 |
| current-prefix | current-question | 15/30 | 0.9136 | 8 | 5 | 1 | 1 |
| tree-prefix | legacy-question | 15/30 | 0.9391 | 7 | 4 | 3 | 1 |
| current-prefix | legacy-question | 16/30 | 0.9461 | 7 | 5 | 3 | 1 |

The current prefix plus legacy question had the highest pass count in this
run. The differences are small and this is a synthetic suite with one run per
variant; no default was selected based on this result. The existing current
prefix and current question remain the defaults.

## Prefix-only changes

These compare the current prefix to the committed prefix while holding the
question template fixed.

- current question: gained `06_config_switch`, `16_counter_snapshot`; lost `20_interleaved_side_tasks`.

- legacy question: gained `16_counter_snapshot`; lost none.

## Reproduce and inspect

```sh
promptfoo eval -c evals/promptfooconfig.prompts.yaml --no-cache -o core/out/prompt-comparison.json
promptfoo view
```

The [raw export](../core/out/prompt-comparison.json) contains all per-case results
and the full graph for every variant. Each provider output includes `graph_path`;
graphs include `prompt_config` with the actual prompt text. Generated graph and
export files are local artifacts ignored by Git. Re-running overwrites those
files; this report records the evaluation ID above.

See [prompt provenance and text diffs](../core/prompts/README.md) and
[fixture coverage](../inputs/llm/README.md). Use `--repeat N` for repeated
measurements before choosing wording on the basis of these scores.
