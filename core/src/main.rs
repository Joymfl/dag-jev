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
use std::{collections::HashMap, env, fs, hash::Hash, path::Path};

const INSTRUCTION_TEMPLATE: &'static str = "Does {} depend on {}?";
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
    questions: HashMap<String, Question>,
}
// hardcoded to "choice" type of question for first pass
#[derive(Serialize)]
struct Question {
    #[serde(rename = "type")]
    kind: String,
    instructions: String, // Although api mentions an enum of types, hardcoding it to string for
    // this test
    criteria: HashMap<String, String>,
}

#[derive(Serialize)]
struct GraphJson {
    nodes: Vec<NodeJson>,
    edges: Vec<EdgeJson>,
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
    fn new(state: String, questions: HashMap<String, Question>) -> Self {
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
fn main() {
    let args: Vec<String> = env::args().collect();
    let mut run_type = RunType::Default;
    if args.len() == 2 {
        let run_type_arg = &args[1];
        if run_type_arg == "test-pairs" {
            run_type = RunType::TestPairs;
        }
    }
    if args.len() > 2 {
        eprintln!("Unsupported arg count");
        //TODO: Add usage helper here
        return;
    }
    test_routine(run_type);
}

fn test_routine(run_type: RunType) -> Result<(), String> {
    let input_file_path = "input.txt";
    let task_state = fs::read_to_string(input_file_path).unwrap(); // just an experiment don't
    //
    let prompt_prefix = "#the question will always ask if first task depends on second task. Only depends on RTC. If you're unsure always respond with yes.\n".to_string();
    let contents = format!("{}{}", prompt_prefix, task_state);
    // care about unwrap here
    dotenv().ok();
    let bearer_token = env::var("TYPESAFE_KEY").expect("typesafe api key. Set it");
    let mut task_list: Vec<Task> = Vec::new();
    task_state.lines().enumerate().for_each(|(index, line)| {
        if let Some(char) = line.chars().next() {
            if char == '#' {
                return;
            }
        }
        task_list.push(Task {
            id: index,
            desc: line,
        })
    });
    for task in &task_list {
        println!("Task: {}", task.desc);
    }
    // request builder
    let mut request = Payload::new(contents.to_string(), HashMap::new());
    // lookup pairs
    let mut pairs: HashMap<String, (usize, usize)> = HashMap::new();
    for i in 0..task_list.len() {
        for j in 0..task_list.len() {
            if i == j {
                continue;
            }
            let instruction_string = format!(
                "Does task {} depend on {}. Each task will run to completion before the next is scheduled. If unsure, always respond with a true dependency",
                i, j
            );
            let question_string = format!("dep_{}_{}", i, j);
            pairs.insert(question_string.clone(), (i, j));
            request.questions.insert(
                question_string,
                Question {
                    kind: "noul".to_string(),
                    instructions: instruction_string,
                    criteria: HashMap::from([
                        ("true".to_string(), "Yes".to_string()),
                        ("false".to_string(), "No".to_string()),
                    ]),
                    // NOTE: keeping this around, because this is needed for testing the task
                    // renaming strategy later
                    // criteria: HashMap::from([
                    //     ("raw".to_string(), "RAW hazard".to_string()),
                    //     ("war".to_string(), "WAR hazard".to_string()),
                    //     ("waw".to_string(), "WAW hazard".to_string()),
                    //     ("decoupled".to_string(), "decoupled".to_string()),
                    // ]),
                },
            );
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

    if run_type == RunType::TestPairs {
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
    fs::write(
        out_dir.join("graph.json"),
        serde_json::to_string_pretty(&graph_json).unwrap(),
    );

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
