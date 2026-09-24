# LLM-authored dependency fixtures

These 20 synthetic workflows and their manually specified controls were authored
by Codex. The original ten cases remain in `inputs/small` and `control/small`.
Inputs live in `inputs/llm/<size>`; matching controls in `control/llm/<size>`.

Small cases have 4–5 tasks, medium cases 8, and complex cases 12. Complexity also
increases through overlapping resource lifetimes, multiple reader barriers,
interleaved branches, and repeated replacement of shared resources.

The exhaustive `r`/`w` resource declarations and exact resource-name semantics
are the same as the original fixtures. All files read are created by an earlier
task. There are no implicit resources or side effects. Controls specify required
task ordering; redundant edges may be omitted. These are synthetic correctness
checks, not a representative estimate of production accuracy.

| Size | Case | Coverage |
| --- | --- | --- |
| small | `11_readers_before_overwrite` | Two readers must finish before a replacement |
| small | `12_write_only_chain` | Write-after-write without any reads |
| small | `13_similar_resource_names` | Exact resource identity, not substring matching |
| small | `14_multi_output_fanout` | One writer produces two resources for parallel readers |
| small | `15_same_description_independent` | Identical descriptions with disjoint resources |
| small | `16_counter_snapshot` | Read-modify-write and preservation of an earlier version |
| medium | `17_two_resource_barrier` | An atomic overwrite waits for readers of both resources |
| medium | `18_diamond_then_overwrite` | A diamond plus a second reader protects a resource |
| medium | `19_three_config_epochs` | Read barriers between three configurations |
| medium | `20_interleaved_side_tasks` | A long chain interleaved with unrelated work |
| medium | `21_reader_groups` | Three old-version readers and two new-version readers |
| medium | `22_path_identity` | Same basenames in different directories remain distinct |
| medium | `23_swap_through_snapshots` | Two writes each depend on both snapshot branches |
| complex | `24_release_matrix` | Two build branches share source and documentation |
| complex | `25_three_way_rotation` | Three snapshot barriers without a global barrier |
| complex | `26_repeated_reader_barriers` | Four versions separated by groups of readers |
| complex | `27_four_independent_pipelines` | Four interleaved chains with no join |
| complex | `28_wide_fanout_update` | A writer waits for five readers then fans out again |
| complex | `29_two_backup_rounds` | Two restore cycles with independent backup comparison |
| complex | `30_data_and_config_barriers` | Overlapping data replacement and configuration barriers |
