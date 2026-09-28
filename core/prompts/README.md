# Prompt variants

These text files separate the state prefix from the per-question instructions.
The CLI embeds `prefix.current.txt` and `question.current.txt` as its defaults.

| File | Origin |
| --- | --- |
| `prefix.current.txt` | Working-tree wording at the start of this change, including the explanation of `r`, `w`, and `:` |
| `prefix.tree.txt` | Committed prefix at `63836da` (introduced by `2544dd7`) |
| `question.current.txt` | Current and `63836da` question, with Rust's positional placeholders renamed to `{i}` and `{j}` |
| `question.legacy.txt` | Question from `7b89522`, before simplification in `2544dd7`; same placeholder conversion |

`{i}` is the dependent task; `{j}` is its potential dependency. Both placeholders
are required, may repeat, and are expanded for every ordered pair except self
pairs. The question template is the entire `instructions` field. File templates
have trailing whitespace removed; prefix files retain their text. A missing
newline between a nonempty prefix and the first task is inserted automatically.

Compare the source text directly:

```sh
diff -u core/prompts/prefix.tree.txt core/prompts/prefix.current.txt
diff -u core/prompts/question.current.txt core/prompts/question.legacy.txt
```

The prefix change adds:

```text
the resources read are marked by r, and written by w. Delimited by :
```

The historical question adds the run-to-completion statement and the instruction
to assume a dependency when unsure. It is an experimental comparison, not a new
default. Current question text and HEAD question text are identical.

All four combinations are wired into `evals/promptfooconfig.prompts.yaml` as
separate provider columns. That config uses control ordering and structure as
its scores, holding the judge out of this comparison. The prefix belongs to
request `state`, and the template belongs to each question's `instructions`,
matching the [TypeSafe request contract](https://docs.typesafe.ai/primitives/noul).
