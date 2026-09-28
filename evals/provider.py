"""promptfoo provider. Every eval runs core and returns the generated graph."""

from __future__ import annotations

import json
import hashlib
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import score_dag


def call_api(prompt, options, context):
    options = options or {}
    context = context or {}
    config = options.get("config") or {}
    variables = context.get("vars") or {}
    input_path = variables.get("input_path")
    control_path = variables.get("control_path")
    if not input_path or not control_path:
        return {"error": "input_path and control_path are required"}
    try:
        threshold = float(variables.get("threshold", config.get("threshold", score_dag.DEFAULT_THRESHOLD)))
        resolved_input = score_dag.resolve_repo(input_path)
        resolved_control = score_dag.resolve_repo(control_path)
        prompt_options = {name: variables.get(name, config.get(name)) for name in score_dag.PROMPT_OPTIONS}
        # Promptfoo's prompts list owns the selected component. Never silently
        # score provider-config text different from the prompt shown in the UI.
        target = config.get("prompt_target")
        if target is not None:
            if target not in ("prompt_prefix", "question_template"):
                raise ValueError("prompt_target must be prompt_prefix or question_template")
            if prompt_options[target] is not None or prompt_options[target + "_file"] is not None:
                raise ValueError(f"{target} is supplied by Promptfoo's selected prompt; remove its override")
            if target == "prompt_prefix" and prompt_options["prompt_prefix_addition"] is not None:
                raise ValueError("put prefix additions in the Promptfoo prompt text")
            if not isinstance(prompt, str):
                raise ValueError("the selected Promptfoo prompt must be text")
            prompt_options[target] = prompt.rstrip() if target == "question_template" else prompt
        # Read files before hashing and running so each artifact records the exact
        # prompt content, even when a variant file is edited between evals.
        for base in ("prompt_prefix", "question_template"):
            file_name = base + "_file"
            path = prompt_options.pop(file_name)
            if path is not None:
                if prompt_options[base] is not None:
                    raise ValueError(f"choose {base} or {file_name}, not both")
                content = score_dag.resolve_repo(path).read_text(encoding="utf-8")
                prompt_options[base] = content.rstrip() if base == "question_template" else content
        prompt_options = {key: value for key, value in prompt_options.items() if value is not None}
        if prompt_options:
            variant = hashlib.sha256(json.dumps(prompt_options, sort_keys=True).encode()).hexdigest()[:16]
            # Include the full input path: two inputs can have the same basename.
            input_id = hashlib.sha256(str(resolved_input).encode()).hexdigest()[:8]
            dest = score_dag.REPO / "core/out/variants" / f"{resolved_input.stem}-{input_id}" / f"{variant}.json"
        else:
            dest = score_dag.REPO / "core" / "out" / f"{resolved_input.stem}.json"
        graph = score_dag.run_generator(resolved_input, dest, **prompt_options)
    except (OSError, json.JSONDecodeError, RuntimeError, ValueError) as exc:
        return {"error": str(exc)}
    envelope = {
        "graph": graph,
        "input_path": str(resolved_input),
        "control_path": str(resolved_control),
        "threshold": threshold,
        "graph_path": str(dest),
    }
    return {"output": json.dumps(envelope)}
