/// Human generated comments, not AI, I promise
///NOTE:  if task i depends on j, then edge goes from j to i
use dotenvy::dotenv;
use petgraph::{
    Graph,
    algo::{condensation, tarjan_scc, toposort},
    dot::{Config, Dot},
    graph::Node,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::{BTreeMap, HashMap},
    env, fs,
    path::Path,
};

const DEFAULT_PROMPT_PREFIX: &str = include_str!("../prompts/prefix.current.txt");
const DEFAULT_QUESTION_TEMPLATE: &str = include_str!("../prompts/question.current.txt");

#[derive(Clone, Debug, Serialize)]
struct PromptConfig {
    prefix: String,
    question_template: String,
}

impl Default for PromptConfig {
    fn default() -> Self {
        Self {
            prefix: DEFAULT_PROMPT_PREFIX.to_string(),
            question_template: DEFAULT_QUESTION_TEMPLATE.trim_end().to_string(),
        }
    }
}

impl PromptConfig {
    fn state(&self, tasks: &str) -> String {
        let separator = if self.prefix.is_empty() || self.prefix.ends_with('\n') {
            ""
        } else {
            "\n"
        };
        format!("{}{separator}{tasks}", self.prefix)
    }

    fn instruction(&self, i: usize, j: usize) -> Result<String, String> {
        if !self.question_template.contains("{i}") || !self.question_template.contains("{j}") {
            return Err(
                "question template must contain both {i} (dependent) and {j} (dependency)".into(),
            );
        }
        let rendered = self
            .question_template
            .replace("{i}", &i.to_string())
            .replace("{j}", &j.to_string());
        if rendered.contains(['{', '}']) {
            return Err("question template supports only {i} and {j} placeholders".into());
        }
        Ok(rendered)
    }
}
const THRESHHOLD: f64 = 0.65; //arbitrary confidence threshold. Will tweak based on testing
//This is as low as 0.65 because it's better to get a bad dependency,
//than to actually parallelize something that (could) have a
//dependency. Tarjan's should catch cycles, if the low values does cause them

struct Task<'a> {
    pub id: usize,
    pub desc: &'a str,
}

#[derive(Deserialize, Debug)]
struct JevResponseChoice {
    model: Option<String>,
    answers: HashMap<String, ChoiceRsesponse>,
    usage: CostResponse,
}

#[derive(Deserialize, Debug)]
struct JevResponseNoul {
    model: Option<String>,
    answers: HashMap<String, NoulResponse>,
    usage: CostResponse,
}
#[derive(Deserialize, Debug)]
struct NoulResponse {
    #[serde(rename = "type")]
    kind: String,
    noul: f64,
}

#[derive(Deserialize, Debug)]
struct CostResponse {
    input_tokens: usize,
    output_tokens: usize,
}
#[derive(Deserialize, Debug)]
struct ChoiceRsesponse {
    #[serde(rename = "type")]
    kind: String,
    choice: String,
    confidence: f64,
}
#[derive(Serialize)]
struct Payload {
    state: String,
    model: String,
    questions: BTreeMap<String, Question>,
}
// hardcoded to "choice" type of question for first pass
#[derive(Serialize)]
struct Question {
    #[serde(rename = "type")]
    kind: String,
    instructions: String, // Although api mentions an enum of types, hardcoding it to string for
    // this test
    criteria: BTreeMap<String, String>,
}

#[derive(Serialize)]
struct GraphJson {
    nodes: Vec<NodeJson>,
    edges: Vec<EdgeJson>,
    prompt_config: PromptConfig,
}

#[derive(Serialize)]
struct NodeJson {
    id: String,
    label: String,
}
#[derive(Serialize)]
struct EdgeJson {
    id: String,
    source: usize,
    target: usize,
    weight: f64,
}

impl Payload {
    fn new(state: String, questions: BTreeMap<String, Question>) -> Self {
        Self {
            state,
            // instruction: "the question will always ask if a task depends on another. RAW = Read after Write, WAR = Write after read, WAW = Write after write, decoupled = free node".to_string(),
            model: "jev-1.13.0".to_string(),
            questions,
        }
    }
}

#[derive(PartialEq, PartialOrd)]
enum RunType {
    Default, // Default run type. Keeping it as the api + dag construction + condensation pass for now
    TestPairs, // Read pairs from response and just print out. Mostly for manual checks and
             // eyeballing differences between prompts for now
}
const USAGE: &str = "usage: core [test-pairs] [--input PATH] [--output PATH]
  --prompt-prefix TEXT             Replace the prefix before the task list (empty disables it)
  --prompt-prefix-file PATH        Read the replacement prefix from a text file
  --prompt-prefix-addition TEXT    Append text to the chosen prefix, before the task list
  --question-template TEXT         Per-question instructions with {i} and {j} placeholders
  --question-template-file PATH    Read per-question instructions from a text file
  --dump-request PATH             Write the exact JSON request and exit without calling Jev";

struct Cli {
    run_type: RunType,
    input: String,
    output: Option<String>,
    prompt: PromptConfig,
    dump_request: Option<String>,
}

fn parse_args(args: Vec<String>) -> Result<Cli, String> {
    let mut cli = Cli {
        run_type: RunType::Default,
        input: "input.txt".into(),
        output: None,
        prompt: PromptConfig::default(),
        dump_request: None,
    };
    let mut prefix_set = false;
    let mut template_set = false;
    let mut additions = Vec::new();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        if arg == "test-pairs" {
            cli.run_type = RunType::TestPairs;
            continue;
        }
        if !matches!(
            arg.as_str(),
            "--input"
                | "--output"
                | "--prompt-prefix"
                | "--prompt-prefix-file"
                | "--prompt-prefix-addition"
                | "--question-template"
                | "--question-template-file"
                | "--dump-request"
        ) {
            return Err(format!("unsupported arg: {arg}"));
        }
        let value = args.next().ok_or_else(|| format!("{arg} needs a value"))?;
        match arg.as_str() {
            "--input" => cli.input = value,
            "--output" => cli.output = Some(value),
            "--dump-request" => cli.dump_request = Some(value),
            "--prompt-prefix-addition" => additions.push(value),
            "--prompt-prefix" | "--prompt-prefix-file" => {
                if prefix_set {
                    return Err("choose only one prefix override".into());
                }
                prefix_set = true;
                cli.prompt.prefix = if arg.ends_with("-file") {
                    fs::read_to_string(&value).map_err(|e| format!("{value}: {e}"))?
                } else {
                    value
                };
            }
            "--question-template" | "--question-template-file" => {
                if template_set {
                    return Err("choose only one question template override".into());
                }
                template_set = true;
                cli.prompt.question_template = if arg.ends_with("-file") {
                    fs::read_to_string(&value)
                        .map_err(|e| format!("{value}: {e}"))?
                        .trim_end()
                        .to_string()
                } else {
                    value
                };
            }
            _ => unreachable!(),
        }
    }
    for addition in additions {
        if !addition.is_empty() {
            if !cli.prompt.prefix.is_empty() && !cli.prompt.prefix.ends_with('\n') {
                cli.prompt.prefix.push('\n');
            }
            cli.prompt.prefix.push_str(&addition);
        }
    }
    cli.prompt.instruction(0, 1)?;
    Ok(cli)
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{USAGE}");
        return;
    }
    let cli = parse_args(args).unwrap_or_else(|err| {
        eprintln!("{err}\n{USAGE}");
        std::process::exit(2);
    });
    if let Err(err) = test_routine(cli) {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn build_request(task_state: &str, prompt: &PromptConfig) -> Result<Payload, String> {
    let mut request = Payload::new(prompt.state(task_state), BTreeMap::new());
    let count = task_state.lines().count();
    for i in 0..count {
        for j in 0..count {
            if i == j {
                continue;
            }
            request.questions.insert(
                format!("dep_{i}_{j}"),
                Question {
                    kind: "noul".into(),
                    instructions: prompt.instruction(i, j)?,
                    criteria: BTreeMap::from([
                        ("true".into(), "Yes".into()),
                        ("false".into(), "No".into()),
                    ]),
                },
            );
        }
    }
    Ok(request)
}

fn test_routine(cli: Cli) -> Result<(), String> {
    let task_state = fs::read_to_string(&cli.input).map_err(|e| format!("{}: {e}", cli.input))?;
    let request = build_request(&task_state, &cli.prompt)?;
    if let Some(path) = cli.dump_request {
        fs::write(&path, serde_json::to_string_pretty(&request).unwrap())
            .map_err(|e| format!("{path}: {e}"))?;
        return Ok(());
    }
    dotenv().ok();
    let bearer_token = env::var("TYPESAFE_KEY").expect("typesafe api key. Set it");
    let task_list: Vec<_> = task_state
        .lines()
        .enumerate()
        .map(|(id, desc)| Task { id, desc })
        .collect();
    for task in &task_list {
        println!("Task: {}", task.desc);
    }
    let mut pairs = HashMap::new();
    for i in 0..task_list.len() {
        for j in 0..task_list.len() {
            if i != j {
                pairs.insert(format!("dep_{i}_{j}"), (i, j));
            }
        }
    }
    // let json = serde_json::to_string(&request).unwrap();
    //
    // NOTE: I don't think the blocking call will matter here? I should take a look when I'm
    // planning to make this into a pipeline
    let client = reqwest::blocking::Client::new();

    let response = client
        .post("https://api.typesafe.ai/v1/systemone")
        .json(&request)
        .header("Authorization", format!("Bearer {}", bearer_token))
        .send()
        .unwrap();
    // eprintln!("response status: {}", response.status());

    let body = response.text().unwrap();
    // eprintln!("response body: {}", body);
    let des_response = serde_json::from_str::<JevResponseNoul>(&body).unwrap();

    if cli.run_type == RunType::TestPairs {
        for answer in des_response.answers.iter() {
            println!("Question: {} Answer: {:?}", answer.0, answer.1);
        }
        return Ok(());
    }

    // graph builder
    let mut graph = Graph::<usize, f64>::new();
    // we'll piggyback off the graph builder for now.
    // TODO: Seperate out the graph json output and the actual scc builder
    let mut graph_json: GraphJson = GraphJson {
        nodes: Vec::new(),
        edges: Vec::new(),
        prompt_config: cli.prompt,
    };

    let nodes: Vec<_> = task_list
        .iter()
        .map(|task| {
            graph_json.nodes.push(NodeJson {
                id: task.id.to_string(),
                label: task.desc.to_string(),
            });
            graph.add_node(task.id)
        })
        .collect();

    for (key, answer) in &des_response.answers {
        let (i, j) = pairs[key];
        // Record graph out of threshold check. Needed to test if the threshold is right
        graph_json.edges.push(EdgeJson {
            id: format!("e{}_{}", j, i),
            source: j,
            target: i,
            weight: answer.noul,
        });

        if answer.noul >= THRESHHOLD {
            // TODO: What does the weight truly represent here? Right now we just check whether
            // connection exists or not
            graph.add_edge(nodes[j], nodes[i], answer.noul);
        }
    }

    // cycle pass
    // NOTE: Design choice. Every cycle, should require a human to resolve
    let cycles: Vec<_> = tarjan_scc(&graph)
        .into_iter()
        .filter(|scc| scc.len() > 1)
        .collect();
    for scc in &cycles {
        let ids: Vec<_> = scc.iter().map(|n| graph[*n]).collect();
        println!("cycle: {:?}", ids);
        for &a in scc {
            for &b in scc {
                if let Some(e) = graph.find_edge(a, b) {
                    println!(" {} -> {} p={:.2}", graph[a], graph[b], graph[e])
                }
            }
        }
    }

    let out_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("./out");
    fs::create_dir_all(&out_dir).unwrap();
    let dot = Dot::with_config(&graph, &[Config::EdgeNoLabel]);
    fs::write(out_dir.join("graph_tarjan.dot"), format!("{:?}", dot)).unwrap();
    let default_graph = out_dir.join("graph.json");
    let graph_path = match cli.output.as_deref() {
        Some(path) => Path::new(path),
        None => default_graph.as_path(),
    };
    if let Some(parent) = graph_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).unwrap();
        }
    }
    fs::write(
        graph_path,
        serde_json::to_string_pretty(&graph_json).unwrap(),
    )
    .unwrap();

    // condensation pass. this is for actually generating parallelism
    let condensed = condensation(graph, true);
    if cycles.is_empty() {
        let mut in_degree: Vec<usize> = condensed
            .node_indices()
            .map(|n| {
                condensed
                    .neighbors_directed(n, petgraph::Direction::Incoming)
                    .count()
            })
            .collect();
        let mut done = vec![false; condensed.node_count()];
        let mut level = 0;

        loop {
            let ready: Vec<_> = condensed
                .node_indices()
                .filter(|&n| !done[n.index()] && in_degree[n.index()] == 0)
                .collect();
            if ready.is_empty() {
                break;
            }
            println!("level {}", level);
            for &n in &ready {
                let group = &condensed[n];
                if group.len() > 1 {
                    println!(" Cycle, Needs review {:?}", group);
                } else {
                    println!(" {}", task_list[group[0]].desc);
                }
                done[n.index()] = true;
            }
            for &n in &ready {
                for succ in condensed.neighbors_directed(n, petgraph::Direction::Outgoing) {
                    in_degree[succ.index()] -= 1;
                }
            }
            level += 1;
        }
        println!("Critical path: {} levels", level);
    }
    let labelled = condensed.map(
        |_, group| {
            group
                .iter()
                .map(|&id| task_list[id].desc)
                .collect::<Vec<_>>()
                .join("\n")
        },
        |_, &p| p,
    );
    let dot = Dot::with_config(&labelled, &[Config::EdgeNoLabel]);
    fs::write(out_dir.join("condensed.dot"), format!("{:?}", dot)).unwrap();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Result<Cli, String> {
        parse_args(values.iter().map(|s| s.to_string()).collect())
    }

    #[test]
    fn default_request_preserves_current_prompts_and_task_zero() {
        let tasks = "0. Create:w:a\n1. Read:r:a:w:b\n";
        let request = build_request(tasks, &PromptConfig::default()).unwrap();
        assert_eq!(request.state, format!("{DEFAULT_PROMPT_PREFIX}{tasks}"));
        assert_eq!(request.questions.len(), 2);
        assert_eq!(
            request.questions["dep_1_0"].instructions,
            "Does task 1 depend on 0?"
        );
        assert_eq!(
            request.questions["dep_0_1"].instructions,
            "Does task 0 depend on 1?"
        );
    }

    #[test]
    fn overrides_are_independent_and_addition_precedes_tasks() {
        let cli = args(&[
            "--prompt-prefix-addition",
            "Extra rule.",
            "--prompt-prefix",
            "Prefix",
            "--question-template",
            "Must {j} finish before {i}? Compare {i} with {j}.",
        ])
        .unwrap();
        let request = build_request("a\nb\nc\n", &cli.prompt).unwrap();
        assert_eq!(request.state, "Prefix\nExtra rule.\na\nb\nc\n");
        assert_eq!(request.questions.len(), 6);
        assert_eq!(
            request.questions["dep_2_0"].instructions,
            "Must 0 finish before 2? Compare 2 with 0."
        );
    }

    #[test]
    fn empty_prefix_removes_it_without_changing_ids() {
        let cli = args(&["--prompt-prefix", ""]).unwrap();
        let request = build_request("a\nb\n", &cli.prompt).unwrap();
        assert_eq!(request.state, "a\nb\n");
        assert!(request.questions.contains_key("dep_0_1"));
    }

    #[test]
    fn templates_require_both_known_placeholders() {
        for template in [
            "Always yes",
            "Does {i} depend on task?",
            "{i} {j} {unknown}",
        ] {
            assert!(args(&["--question-template", template]).is_err());
        }
    }

    #[test]
    fn conflicting_and_missing_options_fail() {
        assert!(args(&["--prompt-prefix", "a", "--prompt-prefix-file", "b"]).is_err());
        assert!(
            args(&[
                "--question-template",
                "{i} {j}",
                "--question-template-file",
                "b"
            ])
            .is_err()
        );
        assert!(args(&["--input"]).is_err());
        assert!(args(&["--unknown"]).is_err());
    }

    #[test]
    fn prompt_files_and_request_serialization_are_diffable() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let prefix = root.join("prompts/prefix.tree.txt");
        let question = root.join("prompts/question.legacy.txt");
        let cli = args(&[
            "--prompt-prefix-file",
            prefix.to_str().unwrap(),
            "--question-template-file",
            question.to_str().unwrap(),
        ])
        .unwrap();
        let request = build_request("a\nb\n", &cli.prompt).unwrap();
        assert!(
            request.questions["dep_1_0"]
                .instructions
                .contains("If unsure")
        );
        assert!(!request.state.contains("Delimited by"));
        let again = build_request("a\nb\n", &cli.prompt).unwrap();
        assert_eq!(
            serde_json::to_string(&request).unwrap(),
            serde_json::to_string(&again).unwrap()
        );
    }
}
