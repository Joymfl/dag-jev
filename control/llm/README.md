# LLM-authored controls

Matches `inputs/llm/{small,medium,complex}` by size and basename. Each pair
`[i,j]` means task i depends on task j (edge j -> i). Comments describe the case;
an empty list means every task is independent. Controls were specified manually
from the declared accesses, then checked against an independent all-pairs hazard
oracle in `evals/test_score_dag.py`. Equality is by task ordering (reachability),
not raw edge count. See `inputs/llm/README.md` for coverage and assumptions.
