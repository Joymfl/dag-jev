"""promptfoo provider. Every eval runs core and returns the generated graph."""

from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import score_dag


def call_api(prompt, options, context):
    del prompt
    options = options or {}
    context = context or {}
    config = options.get("config") or {}
    variables = context.get("vars") or {}
    threshold = float(variables.get("threshold") or config.get("threshold") or score_dag.DEFAULT_THRESHOLD)
    input_path = variables.get("input_path")
    control_path = variables.get("control_path")
    if not input_path or not control_path:
        return {"error": "input_path and control_path are required"}
    try:
        resolved_input = score_dag.resolve_repo(input_path)
        resolved_control = score_dag.resolve_repo(control_path)
        dest = score_dag.REPO / "core" / "out" / f"{resolved_input.stem}.json"
        graph = score_dag.run_generator(resolved_input, dest)
    except (OSError, json.JSONDecodeError, RuntimeError, ValueError) as exc:
        return {"error": str(exc)}
    envelope = {
        "graph": graph,
        "input_path": str(resolved_input),
        "control_path": str(resolved_control),
        "threshold": threshold,
    }
    return {"output": json.dumps(envelope)}
