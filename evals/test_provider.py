"""Verify Promptfoo configuration reaches the Cargo CLI without API calls."""

import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import provider
import score_dag


class ProviderTests(unittest.TestCase):
    def context(self, **extra):
        return {"vars": {
            "input_path": "inputs/small/01_build_release.txt",
            "control_path": "control/small/01_build_release.txt",
            **extra,
        }}

    def test_prompt_files_reach_generator_and_variants_do_not_collide(self):
        paths = []
        with patch.object(score_dag, "run_generator", return_value={"nodes": [], "edges": []}) as run:
            for prefix in ("tree", "current"):
                for question in ("current", "legacy"):
                    result = provider.call_api("unused", {"config": {
                        "prompt_prefix_file": f"core/prompts/prefix.{prefix}.txt",
                        "question_template_file": f"core/prompts/question.{question}.txt",
                    }}, self.context())
                    self.assertNotIn("error", result)
                    paths.append(json.loads(result["output"])["graph_path"])
                    self.assertEqual(run.call_args.kwargs["prompt_prefix"],
                        (score_dag.REPO / f"core/prompts/prefix.{prefix}.txt").read_text())
                    self.assertEqual(run.call_args.kwargs["question_template"],
                        (score_dag.REPO / f"core/prompts/question.{question}.txt").read_text().rstrip())
        self.assertEqual(len(set(paths)), 4)

    def test_empty_prefix_and_zero_threshold_are_preserved(self):
        with patch.object(score_dag, "run_generator", return_value={}) as run:
            result = provider.call_api("", {"config": {"prompt_prefix": "configured"}},
                self.context(prompt_prefix="", threshold=0))
            self.assertEqual(run.call_args.kwargs["prompt_prefix"], "")
            self.assertEqual(json.loads(result["output"])["threshold"], 0)

    def test_selected_prompt_is_forwarded_as_prefix(self):
        with patch.object(score_dag, "run_generator", return_value={}) as run:
            for text in ("Selected prefix\n", ""):
                result = provider.call_api(text, {"config": {"prompt_target": "prompt_prefix"}}, self.context())
                self.assertNotIn("error", result)
                self.assertEqual(run.call_args.kwargs["prompt_prefix"], text)

    def test_selected_prompt_is_forwarded_as_question_template(self):
        with patch.object(score_dag, "run_generator", return_value={}) as run:
            result = provider.call_api("Must {j} finish before {i}?\n", {"config": {
                "prompt_target": "question_template",
                "prompt_prefix_file": "core/prompts/prefix.current.txt",
            }}, self.context())
            self.assertNotIn("error", result)
            self.assertEqual(run.call_args.kwargs["question_template"], "Must {j} finish before {i}?")
            self.assertEqual(run.call_args.kwargs["prompt_prefix"],
                (score_dag.REPO / "core/prompts/prefix.current.txt").read_text())

    def test_selected_prompt_cannot_be_silently_overridden(self):
        with patch.object(score_dag, "run_generator") as run:
            for override in ({"prompt_prefix": "hidden"}, {"prompt_prefix_file": "hidden.txt"},
                             {"prompt_prefix_addition": "hidden"}):
                result = provider.call_api("visible", {"config": {
                    "prompt_target": "prompt_prefix", **override,
                }}, self.context())
                self.assertIn("error", result)
            result = provider.call_api("visible", {"config": {"prompt_target": "typo"}}, self.context())
            self.assertIn("error", result)
            run.assert_not_called()

    def test_conflicting_sources_fail_before_generation(self):
        with patch.object(score_dag, "run_generator") as run:
            result = provider.call_api("", {"config": {
                "prompt_prefix": "text", "prompt_prefix_file": "core/prompts/prefix.tree.txt",
            }}, self.context())
            self.assertIn("choose prompt_prefix", result["error"])
            run.assert_not_called()

    def test_editing_prompt_content_changes_artifact_path(self):
        with tempfile.TemporaryDirectory() as tmp, patch.object(score_dag, "run_generator", return_value={}):
            path = Path(tmp) / "prefix.txt"
            options = {"config": {"prompt_prefix_file": str(path)}}
            path.write_text("first")
            first = provider.call_api("", options, self.context())
            path.write_text("second")
            second = provider.call_api("", options, self.context())
            self.assertNotEqual(json.loads(first["output"])["graph_path"],
                json.loads(second["output"])["graph_path"])

    def test_generator_forwards_literal_cli_arguments(self):
        with tempfile.TemporaryDirectory() as tmp:
            output = Path(tmp) / "graph.json"
            output.write_text('{"nodes": [], "edges": []}')
            with patch.object(score_dag.subprocess, "run") as run:
                run.return_value.returncode = 0
                score_dag.run_generator(Path("tasks.txt"), output,
                    prompt_prefix="", prompt_prefix_addition="A\nB",
                    question_template="Does {i} depend on {j}?")
                command = run.call_args.args[0]
                self.assertEqual(command[command.index("--prompt-prefix") + 1], "")
                self.assertEqual(command[command.index("--prompt-prefix-addition") + 1], "A\nB")
                self.assertEqual(command[command.index("--question-template") + 1], "Does {i} depend on {j}?")
                self.assertEqual(run.call_args.kwargs["cwd"], score_dag.REPO)

    def test_all_input_fixtures_registered_once(self):
        config = (score_dag.EVALS / "cases.yaml").read_text()
        inputs = list((score_dag.REPO / "inputs").rglob("*.txt"))
        self.assertEqual(len(inputs), 30)
        for path in inputs:
            relative = path.relative_to(score_dag.REPO)
            self.assertEqual(config.count(f"input_path: {relative}\n"), 1)
            control = Path("control") / path.relative_to(score_dag.REPO / "inputs")
            self.assertEqual(config.count(f"control_path: {control}\n"), 1)


if __name__ == "__main__":
    unittest.main()
