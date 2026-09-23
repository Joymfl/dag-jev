"""Score a generated Jev DAG.

Structural checks are deterministic. Control files are the ground-truth direct
edges: ``[i, j]`` or ``dep_i_j: 1`` means task i depends on task j, so the
graph edge is j -> i. ``graph.json`` stores every pairwise answer; an edge is
in the DAG only when its weight is >= threshold (core THRESHHOLD, 0.65).

The LLM judge calls the same TypeSafe Jev API the generator uses. It is a
second opinion, not a substitute for the control score. It does not run when
TYPESAFE_KEY is unset, and a missing key is not reported as a passing score.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import urllib.error
import urllib.request
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
EVALS = Path(__file__).resolve().parent
RUBRIC_PATH = EVALS / "rubrics" / "dag_judge.txt"
DEFAULT_THRESHOLD = 0.65
JUDGE_PASS = 0.7
JEV_URL = "https://api.typesafe.ai/v1/systemone"
JEV_MODEL = "jev-1.13.0"
JUDGE_QUESTIONS = {
    "no_missed_hazard": (
        "Is every direct resource hazard represented by an accepted edge from "
        "the earlier task to the later task?"
    ),
    "no_extra_edge": (
        "Does every accepted edge correspond to a direct resource hazard, "
        "rather than a transitive or invented dependency?"
    ),
    "acyclic": "Are the accepted edges acyclic?",
}


def parse_tasks(text: str) -> list[dict]:
    # Match core: str::lines enumerates every line, including blanks.
    return [{"id": index, "desc": line} for index, line in enumerate(text.splitlines())]


def parse_control(text: str) -> set[tuple[int, int]]:
    """Return (dependent, dependency) pairs. Edge direction is dependency -> dependent."""
    edges: set[tuple[int, int]] = set()
    for line_no, raw in enumerate(text.splitlines(), start=1):
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        if line.startswith("dep_"):
            left, _, right = line.partition(":")
            value = right.strip()
            if value not in {"0", "1"}:
                raise ValueError(f"control line {line_no}: expected 0 or 1, got {right!r}")
            parts = left.split("_")
            if len(parts) != 3:
                raise ValueError(f"control line {line_no}: expected dep_{{i}}_{{j}}")
            dependent, dependency = int(parts[1]), int(parts[2])
            if value == "1":
                edges.add((dependent, dependency))
            continue
        if line.startswith("[") and "]" in line:
            body = line[1 : line.index("]")]
            left, _, right = body.partition(",")
            if not right:
                raise ValueError(f"control line {line_no}: expected [i, j]")
            edges.add((int(left.strip()), int(right.strip())))
            continue
        raise ValueError(f"control line {line_no}: unrecognized {raw!r}")
    return edges


def load_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def resolve_repo(path: str | Path) -> Path:
    candidate = Path(path)
    if candidate.is_absolute():
        return candidate
    rooted = REPO / candidate
    if rooted.exists():
        return rooted
    return candidate


def promptfoo_case(output, context) -> tuple[dict, float, dict, dict]:
    """Return graph, threshold, test vars, and the provider envelope if present."""
    data = output if isinstance(output, dict) else json.loads(output)
    variables = (context or {}).get("vars") or {}
    if isinstance(data, dict) and "graph" in data and "nodes" not in data:
        threshold = float(data.get("threshold", DEFAULT_THRESHOLD))
        return data["graph"], threshold, variables, data
    return data, float(variables.get("threshold", DEFAULT_THRESHOLD)), variables, {}


def _node_ids(graph: dict) -> tuple[list[str], list[str]]:
    errors: list[str] = []
    ids: list[str] = []
    if not isinstance(graph, dict):
        return [], ["graph is not an object"]
    nodes = graph.get("nodes")
    edges = graph.get("edges")
    if not isinstance(nodes, list):
        errors.append("missing nodes")
        nodes = []
    if not isinstance(edges, list):
        errors.append("missing edges")
    seen: set[str] = set()
    for index, node in enumerate(nodes):
        if not isinstance(node, dict) or "id" not in node or "label" not in node:
            errors.append(f"node {index} missing id or label")
            continue
        node_id = str(node["id"])
        if node_id in seen:
            errors.append(f"duplicate node id {node_id}")
        seen.add(node_id)
        ids.append(node_id)
    return ids, errors


def _endpoint(value: object) -> str | None:
    if isinstance(value, bool):
        return None
    if isinstance(value, int):
        return str(value)
    if isinstance(value, float) and value.is_integer():
        return str(int(value))
    if isinstance(value, str) and value.strip():
        return value.strip()
    return None


def accepted_dependencies(graph: dict, threshold: float = DEFAULT_THRESHOLD) -> set[tuple[int, int]]:
    """Return (dependent, dependency) pairs whose edge weight meets the threshold."""
    found: set[tuple[int, int]] = set()
    for edge in graph.get("edges") or []:
        if not isinstance(edge, dict):
            continue
        weight = edge.get("weight")
        if isinstance(weight, bool) or not isinstance(weight, (int, float)):
            continue
        if float(weight) < threshold:
            continue
        source = _endpoint(edge.get("source"))
        target = _endpoint(edge.get("target"))
        if source is None or target is None:
            continue
        if not source.isdigit() or not target.isdigit():
            continue
        # source is the dependency (j), target is the dependent (i).
        found.add((int(target), int(source)))
    return found


def _has_cycle(node_ids: list[str], directed: list[tuple[str, str]]) -> bool:
    adjacent: dict[str, list[str]] = {node_id: [] for node_id in node_ids}
    for source, target in directed:
        adjacent.setdefault(source, []).append(target)
    white, gray, black = 0, 1, 2
    color = {node_id: white for node_id in adjacent}
    def visit(node_id: str) -> bool:
        color[node_id] = gray
        for nxt in adjacent.get(node_id, []):
            state = color.get(nxt, white)
            if state == gray:
                return True
            if state == white and visit(nxt):
                return True
        color[node_id] = black
        return False
    return any(color[node_id] == white and visit(node_id) for node_id in list(color))


def score_structure(graph: dict, threshold: float = DEFAULT_THRESHOLD) -> dict:
    node_ids, errors = _node_ids(graph)
    known = set(node_ids)
    directed: list[tuple[str, str]] = []
    edges = graph.get("edges") if isinstance(graph, dict) else None
    if isinstance(edges, list):
        for index, edge in enumerate(edges):
            if not isinstance(edge, dict):
                errors.append(f"edge {index} is not an object")
                continue
            missing = [field for field in ("id", "source", "target", "weight") if field not in edge]
            if missing:
                errors.append(f"edge {index} missing {', '.join(missing)}")
                continue
            weight = edge["weight"]
            if isinstance(weight, bool) or not isinstance(weight, (int, float)):
                errors.append(f"edge {index} weight is not a number")
                continue
            source = _endpoint(edge["source"])
            target = _endpoint(edge["target"])
            if source is None or target is None:
                errors.append(f"edge {index} has a non-id endpoint")
                continue
            dangling = [end for end in (source, target) if end not in known]
            if dangling:
                errors.append(
                    f"dangling edge {edge['id']}: missing node {', '.join(dangling)}"
                )
                continue
            if float(weight) >= threshold:
                directed.append((source, target))
    if known and _has_cycle(node_ids, directed):
        errors.append("cycle in accepted edges")
    ok = not errors
    return {"pass": ok, "score": 1.0 if ok else 0.0, "errors": errors}


def score_control(
    graph: dict,
    control: set[tuple[int, int]],
    threshold: float = DEFAULT_THRESHOLD,
) -> dict:
    predicted = accepted_dependencies(graph, threshold)
    true_positive = predicted & control
    missing = sorted(control - predicted)
    extra = sorted(predicted - control)
    if not control:
        recall = 1.0
    else:
        recall = len(true_positive) / len(control)
    if not predicted:
        precision = 1.0 if not control else 0.0
    else:
        precision = len(true_positive) / len(predicted)
    if precision + recall == 0:
        f1 = 0.0
    else:
        f1 = 2 * precision * recall / (precision + recall)
    exact = predicted == control
    errors: list[str] = []
    if missing:
        errors.append(f"missing {missing}")
    if extra:
        errors.append(f"extra {extra}")
    return {
        "pass": exact,
        "score": f1,
        "precision": precision,
        "recall": recall,
        "missing": missing,
        "extra": extra,
        "errors": errors,
    }

def accepted_edge_lines(graph: dict, threshold: float) -> list[str]:
    lines = []
    for edge in graph.get("edges") or []:
        weight = edge.get("weight")
        if isinstance(weight, bool) or not isinstance(weight, (int, float)):
            continue
        if float(weight) < threshold:
            continue
        lines.append(
            f"{edge['source']} -> {edge['target']} weight={float(weight):.2f}"
        )
    return lines


def build_judge_payload(
    tasks: list[dict],
    graph: dict,
    threshold: float = DEFAULT_THRESHOLD,
) -> dict:
    rubric = RUBRIC_PATH.read_text(encoding="utf-8").strip()
    task_lines = "\n".join(f"{task['id']}. {task['desc']}" for task in tasks)
    accepted = accepted_edge_lines(graph, threshold)
    edge_block = "\n".join(accepted) if accepted else "(no accepted edges)"
    state = (
        f"{rubric}\n\n"
        f"Threshold: {threshold}\n\n"
        f"Tasks:\n{task_lines}\n\n"
        f"Accepted DAG edges (source -> target):\n{edge_block}\n"
    )
    questions = {
        key: {
            "type": "noul",
            "instructions": text,
            "criteria": {"true": "Yes", "false": "No"},
        }
        for key, text in JUDGE_QUESTIONS.items()
    }
    return {"state": state, "model": JEV_MODEL, "questions": questions}


def load_api_key() -> str | None:
    key = os.environ.get("TYPESAFE_KEY")
    if key:
        return key
    env_path = REPO / ".env"
    if not env_path.exists():
        return None
    for raw in env_path.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        name, value = line.split("=", 1)
        if name.strip() == "TYPESAFE_KEY":
            cleaned = value.strip().strip('"').strip("'")
            return cleaned or None
    return None


def score_llm(
    tasks: list[dict],
    graph: dict,
    api_key: str | None,
    threshold: float = DEFAULT_THRESHOLD,
) -> dict:
    if not api_key:
        return {
            "pass": False,
            "score": None,
            "ran": False,
            "errors": ["LLM judge not run: TYPESAFE_KEY is unset"],
            "scores": {},
        }
    payload = build_judge_payload(tasks, graph, threshold)
    request = urllib.request.Request(
        JEV_URL,
        data=json.dumps(payload).encode("utf-8"),
        headers={
            "Authorization": f"Bearer {api_key}",
            "Content-Type": "application/json",
        },
        method="POST",
    )
    try:
        with urllib.request.urlopen(request, timeout=120) as response:
            body = json.loads(response.read().decode("utf-8"))
    except urllib.error.HTTPError as exc:
        detail = exc.read().decode("utf-8", errors="replace")[:500]
        return {
            "pass": False,
            "score": 0.0,
            "ran": True,
            "errors": [f"LLM judge HTTP {exc.code}: {detail}"],
            "scores": {},
        }
    except (urllib.error.URLError, TimeoutError, json.JSONDecodeError) as exc:
        return {
            "pass": False,
            "score": 0.0,
            "ran": True,
            "errors": [f"LLM judge failed: {exc}"],
            "scores": {},
        }
    answers = body.get("answers") if isinstance(body, dict) else None
    if not isinstance(answers, dict):
        return {
            "pass": False,
            "score": 0.0,
            "ran": True,
            "errors": ["LLM judge response missing answers"],
            "scores": {},
        }
    scores: dict[str, float] = {}
    errors: list[str] = []
    for key in JUDGE_QUESTIONS:
        answer = answers.get(key)
        noul = answer.get("noul") if isinstance(answer, dict) else None
        if isinstance(noul, bool) or not isinstance(noul, (int, float)):
            errors.append(f"LLM judge missing noul for {key}")
            continue
        scores[key] = float(noul)
    if errors or len(scores) != len(JUDGE_QUESTIONS):
        return {
            "pass": False,
            "score": 0.0,
            "ran": True,
            "errors": errors or ["LLM judge incomplete"],
            "scores": scores,
        }
    overall = min(scores.values())
    return {
        "pass": overall >= JUDGE_PASS and all(value >= JUDGE_PASS for value in scores.values()),
        "score": overall,
        "ran": True,
        "errors": [],
        "scores": scores,
    }


def run_generator(input_path: Path, output_path: Path) -> dict:
    output_path.parent.mkdir(parents=True, exist_ok=True)
    command = [
        "cargo",
        "run",
        "-q",
        "-p",
        "core",
        "--",
        "--input",
        str(input_path),
        "--output",
        str(output_path),
    ]
    completed = subprocess.run(command, cwd=REPO, capture_output=True, text=True)
    if completed.returncode != 0:
        detail = (completed.stderr or completed.stdout or "cargo run failed").strip()
        raise RuntimeError(detail[-2000:])
    return load_json(output_path)


def score_paths(
    graph_path: Path,
    control_path: Path | None,
    input_path: Path | None,
    threshold: float,
    use_llm: bool,
) -> dict:
    graph = load_json(graph_path)
    structure = score_structure(graph, threshold)
    result: dict = {"structure": structure, "threshold": threshold}
    control_score = None
    if control_path is not None:
        control_score = score_control(
            graph, parse_control(control_path.read_text(encoding="utf-8")), threshold
        )
        result["control"] = control_score
    llm = None
    if use_llm:
        if input_path is None:
            raise SystemExit("--llm requires --input so the judge can see the tasks")
        tasks = parse_tasks(input_path.read_text(encoding="utf-8"))
        llm = score_llm(tasks, graph, load_api_key(), threshold)
        result["llm"] = llm
    control_f1 = 1.0 if control_score is None else control_score["score"]
    result["overall"] = structure["score"] * control_f1
    if llm and llm["ran"] and llm["score"] is not None:
        result["overall_llm"] = structure["score"] * llm["score"]
    gate = structure["pass"] and (control_score is None or control_score["pass"])
    result["gate"] = gate
    return result


def _print_result(result: dict) -> None:
    structure = result["structure"]
    print(
        f"structure: {'pass' if structure['pass'] else 'fail'} {structure['score']:.3f}"
        + (f" errors={structure['errors']}" if structure["errors"] else "")
    )
    if "control" in result:
        control = result["control"]
        print(
            "control: "
            f"{'pass' if control['pass'] else 'fail'} f1={control['score']:.3f} "
            f"precision={control['precision']:.3f} recall={control['recall']:.3f}"
            + (f" errors={control['errors']}" if control["errors"] else "")
        )
    if "llm" in result:
        llm = result["llm"]
        if not llm["ran"]:
            print(f"llm: not run ({llm['errors'][0]})")
        else:
            print(
                f"llm: {'pass' if llm['pass'] else 'fail'} {llm['score']:.3f} scores={llm['scores']}"
                + (f" errors={llm['errors']}" if llm["errors"] else "")
            )
    print(f"overall: {result['overall']:.3f}")
    if "overall_llm" in result:
        print(f"overall_llm: {result['overall_llm']:.3f}")
    print(f"gate: {'pass' if result['gate'] else 'fail'}")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Score a generated Jev DAG")
    parser.add_argument("--graph", help="graph.json produced by core")
    parser.add_argument("--input", help="task file the graph was generated from")
    parser.add_argument("--control", help="control edge list or dep matrix")
    parser.add_argument("--threshold", type=float, default=DEFAULT_THRESHOLD)
    parser.add_argument("--llm", action="store_true", help="score with the Jev judge")
    parser.add_argument("--live", action="store_true", help="run core on --input, then score")
    args = parser.parse_args(argv)
    if args.live:
        if not args.input or not args.control:
            raise SystemExit("--live requires --input and --control")
        input_path = resolve_repo(args.input)
        output_path = (
            resolve_repo(args.graph)
            if args.graph
            else REPO / "core" / "out" / f"{input_path.stem}.json"
        )
        run_generator(input_path, output_path)
        args.graph = str(output_path)
    if not args.graph:
        raise SystemExit("provide --graph, or --live")
    result = score_paths(
        resolve_repo(args.graph),
        resolve_repo(args.control) if args.control else None,
        resolve_repo(args.input) if args.input else None,
        args.threshold,
        args.llm,
    )
    _print_result(result)
    return 0 if result["gate"] else 1


if __name__ == "__main__":
    sys.exit(main())
