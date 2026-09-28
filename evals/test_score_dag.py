"""Offline regressions for evaluating scheduling constraints, not edge counts."""

import json
import tempfile
import unittest
from pathlib import Path

import assert_control
import score_dag


def graph_for(pairs, count=5, weight=0.9):
    return {
        "nodes": [{"id": str(i), "label": f"task {i}"} for i in range(count)],
        "edges": [
            {"id": f"e{j}_{i}", "source": j, "target": i, "weight": weight}
            for i, j in sorted(pairs)
        ],
    }


class ControlScoringTests(unittest.TestCase):
    def test_build_release_redundant_hazard_passes(self):
        control = {(1, 0), (2, 1), (3, 2), (4, 3)}
        graph = graph_for(control | {(3, 1)})
        result = score_dag.score_control(graph, control)
        self.assertTrue(result["pass"])
        self.assertEqual(result["score"], 1.0)
        self.assertEqual(result["extra"], [])

    def test_redundant_control_edges_are_equivalent(self):
        graph = graph_for({(1, 0), (2, 1)})
        self.assertTrue(score_dag.score_control(graph, {(1, 0), (2, 1), (2, 0)})["pass"])

    def test_missing_order_is_not_hidden_by_shortcut(self):
        result = score_dag.score_control(graph_for({(1, 0), (2, 0)}), {(1, 0), (2, 1)})
        self.assertFalse(result["pass"])
        self.assertIn((2, 1), result["missing"])

    def test_extra_order_between_parallel_tasks_fails(self):
        control = {(1, 0), (2, 0)}
        result = score_dag.score_control(graph_for(control | {(2, 1)}), control)
        self.assertFalse(result["pass"])
        self.assertIn((2, 1), result["extra"])

    def test_reversed_edge_fails(self):
        self.assertFalse(score_dag.score_control(graph_for({(0, 1)}), {(1, 0)})["pass"])

    def test_cycles_still_fail(self):
        graph = graph_for({(1, 0), (0, 1)})
        self.assertFalse(score_dag.score_structure(graph)["pass"])
        self.assertFalse(score_dag.score_control(graph, {(1, 0)})["pass"])

    def test_threshold_applied_before_reachability(self):
        control = {(1, 0), (2, 1)}
        graph = graph_for(control, weight=0.65)
        self.assertTrue(score_dag.score_control(graph, control)["pass"])
        self.assertFalse(score_dag.score_control(graph, control, threshold=0.66)["pass"])

    def test_empty_graph_has_no_constraints(self):
        self.assertTrue(score_dag.score_control(graph_for(set()), set())["pass"])
        self.assertFalse(score_dag.score_control(graph_for({(1, 0)}), set())["pass"])

    def test_all_controls_match_declared_hazard_ordering(self):
        # Independent oracle from the exhaustive resource declarations. Controls
        # intentionally omit edges whose ordering is enforced by another path.
        for path in sorted((score_dag.REPO / "inputs").rglob("*.txt")):
            with self.subTest(case=str(path.relative_to(score_dag.REPO))):
                tasks = []
                created = set()
                for task_id, line in enumerate(path.read_text().splitlines()):
                    self.assertTrue(line.startswith(f"{task_id}. "))
                    fields = line.split(":")
                    access = {"r": set(), "w": set()}
                    for index in range(1, len(fields), 2):
                        access[fields[index]].update(fields[index + 1].split(","))
                    self.assertTrue(access["r"] <= created, f"read before creation: {access['r'] - created}")
                    created.update(access["w"])
                    tasks.append(access)
                hazards = {
                    (i, j)
                    for i, later in enumerate(tasks)
                    for j, earlier in enumerate(tasks[:i])
                    if later["r"] & earlier["w"]
                    or later["w"] & earlier["r"]
                    or later["w"] & earlier["w"]
                }
                control = score_dag.parse_control(
                    (score_dag.REPO / "control" / path.relative_to(score_dag.REPO / "inputs")).read_text()
                )
                self.assertTrue(all(0 <= j < i < len(tasks) for i, j in control))
                result = score_dag.score_control(graph_for(hazards, count=len(tasks)), control)
                self.assertTrue(result["pass"], result["errors"])

    def test_promptfoo_and_cli_score_the_same_artifact(self):
        graph = graph_for({(1, 0), (2, 1), (2, 0)})
        with tempfile.TemporaryDirectory() as tmp:
            control_path = Path(tmp) / "control.txt"
            control_path.write_text("[1,0]\n[2,1]\n")
            graph_path = Path(tmp) / "graph.json"
            graph_path.write_text(json.dumps(graph))
            envelope = json.dumps({"graph": graph, "control_path": str(control_path), "threshold": 0.65})
            assertion = assert_control.get_assert(envelope, {})
            cli = score_dag.score_paths(graph_path, control_path, None, 0.65, False)
            self.assertTrue(assertion["pass"])
            self.assertTrue(cli["gate"])
            self.assertEqual(assertion["score"], cli["control"]["score"])


if __name__ == "__main__":
    unittest.main()
