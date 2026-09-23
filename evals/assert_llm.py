"""Jev judge. A missing TYPESAFE_KEY is a not-run failure, never a fake pass."""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import score_dag


def get_assert(output, context):
    graph, threshold, variables, envelope = score_dag.promptfoo_case(output, context)
    input_path = envelope.get("input_path") or variables.get("input_path")
    if not input_path:
        return {"pass": False, "score": 0, "reason": "LLM judge not run: input_path missing"}
    tasks = score_dag.parse_tasks(
        score_dag.resolve_repo(input_path).read_text(encoding="utf-8")
    )
    result = score_dag.score_llm(tasks, graph, score_dag.load_api_key(), threshold)
    if not result["ran"]:
        return {
            "pass": False,
            "score": 0,
            "reason": result["errors"][0],
        }
    reason = " ".join(f"{key}={value:.3f}" for key, value in sorted(result["scores"].items()))
    if result["errors"]:
        reason = f"{reason} {'; '.join(result['errors'])}".strip()
    return {"pass": result["pass"], "score": result["score"], "reason": reason or "llm judge"}
