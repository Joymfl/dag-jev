# Small decomposition inputs

Ten synthetic outputs of the user-to-LLM decomposition stage, with five tasks
each. These are inputs for dependency extraction, not control files. Task IDs
restart at zero in each file, and tasks are listed in their intended order.

Each line uses the existing input style:

```text
2. Describe the task:r:input_a,input_b:w:output_a
```

`r` lists reads and `w` lists writes. An omitted field means no accesses of that
kind. For this initial batch, the declarations are exhaustive: there are no
hidden resources or side effects. Resource names match exactly, represent
whole files, and are local to one workflow. Writes include creating, appending,
and replacing a file. A task completes its declared accesses before finishing.
No generated files exist initially; fixed text used to create them is part of
the task itself. The text files contain only task lines, with no headers or
blank lines.

| Input | Behavior to inspect |
| --- | --- |
| `01_build_release.txt` | Build prerequisites and release artifacts |
| `02_two_datasets.txt` | Independent branches that join |
| `03_analysis_fanout.txt` | A shared input feeding parallel analyses |
| `04_shared_readers.txt` | Multiple readers of one file |
| `05_results_overwrite.txt` | Copying results before their file is overwritten |
| `06_config_switch.txt` | A configuration update between two runs |
| `07_shared_report.txt` | Multiple writers followed by parallel readers |
| `08_backup_restore.txt` | Backup, mutation, inspection, and restoration |
| `09_independent_outputs.txt` | Fully independent tasks |
| `10_diamond_and_side_task.txt` | A dependency diamond and an unrelated task |

The explicit resource declarations intentionally make this first batch easy to
verify. It tests dependency extraction from structured decompositions; it does
not test discovery of implicit accesses in natural-language-only descriptions.
No dependency labels or changes to the runner are included.
