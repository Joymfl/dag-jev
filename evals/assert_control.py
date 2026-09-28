"""Score task ordering against the control file, allowing redundant edges."""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import score_dag


def get_assert(output, context):
    graph, threshold, variables, envelope = score_dag.promptfoo_case(output, context)
    control_path = envelope.get("control_path") or variables.get("control_path")
    if not control_path:
        return {"pass": False, "score": 0, "reason": "control_path missing"}
    control = score_dag.parse_control(
        score_dag.resolve_repo(control_path).read_text(encoding="utf-8")
    )
    result = score_dag.score_control(graph, control, threshold)
    reason = (
        f"ordering precision={result['precision']:.3f} recall={result['recall']:.3f} "
        f"f1={result['score']:.3f}"
    )
    if result["errors"]:
        reason = f"{reason} {'; '.join(result['errors'])}"
    return {"pass": result["pass"], "score": result["score"], "reason": reason}
