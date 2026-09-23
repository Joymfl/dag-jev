"""Fail the case when the generated graph is not a DAG of the expected shape."""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import score_dag


def get_assert(output, context):
    graph, threshold, _, _ = score_dag.promptfoo_case(output, context)
    result = score_dag.score_structure(graph, threshold)
    return {
        "pass": result["pass"],
        "score": result["score"],
        "reason": "; ".join(result["errors"]) or "structure ok",
    }
