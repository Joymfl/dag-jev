///NOTE:  if task i depends on j, then edge goes from j to i
use dotenvy::dotenv;
use petgraph::{
    Graph,
    algo::{tarjan_scc, toposort},
    dot::{Config, Dot},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{collections::HashMap, env, fs, hash::Hash, path::Path};

const INSTRUCTION_TEMPLATE: &'static str = "Does {} depend on {}?";
const THRESHHOLD: f64 = 0.9; // arbitrary confidence threshold. Will tweak based on testing

struct Task<'a> {
    pub id: usize,
    pub desc: &'a str,
}

#[derive(Deserialize, Debug)]
struct JevResponseChoice {
    model: String,
    answers: HashMap<String, ChoiceRsesponse>,
    usage: CostResponse,
}

#[derive(Deserialize, Debug)]
struct JevResponseNoul {
    model: String,
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
    instruction: String,
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

impl Payload {
    fn new(state: String, questions: HashMap<String, Question>) -> Self {
        Self {
            state,
            instruction: "the question will always ask if a task depends on another. RAW = Read after Write, WAR = Write after read, WAW = Write after write, decoupled = free node".to_string(),
            model: "jev-latest".to_string(),
            questions,
        }
    }
}
//POST https://api.typesafe.ai/v1/systemone
//Authorization: Bearer <API_KEY>
//Content-Type: application/json
fn main() {
    let input_file_path = "input.txt";
    let contents = fs::read_to_string(input_file_path).unwrap(); // just an experiment don't
    // care about unwrap here
    dotenv().ok();
    let bearer_token = env::var("TYPESAFE_KEY").expect("typesafe api key. Set it");
    let mut task_list: Vec<Task> = Vec::new();
    contents.lines().enumerate().for_each(|(index, line)| {
        task_list.push(Task {
            id: index,
            desc: line,
        })
    });
    // let task_list: Vec<Task> = contents
    //     .lines()
    //     .map(|line| Task {
    //         id: ,
    //         dependencies: todo!(),
    //         task: todo!(),
    //     })
    // .collect();
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
            let instruction_string = format!("Does task {} depend on {}", i, j);
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
                    ]), // criteria: HashMap::from([
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
    let des_response = response.json::<JevResponseNoul>().unwrap();

    // graph builder
    let mut graph = Graph::<usize, f64>::new();
    let nodes: Vec<_> = task_list
        .iter()
        .map(|task| graph.add_node(task.id))
        .collect();

    for (key, answer) in &des_response.answers {
        let (i, j) = pairs[key];
        if answer.noul >= THRESHHOLD {
            graph.add_edge(nodes[j], nodes[i], answer.noul);
        }
    }

    // Every cycle, should require a human to resolve
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
    if cycles.is_empty() {
        let order = toposort(&graph, None).unwrap();
        for n in order {
            println!("{}", task_list[graph[n]].desc);
        }
        let out_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../out");
        fs::create_dir_all(&out_dir).unwrap();
        let dot = Dot::with_config(&graph, &[Config::EdgeNoLabel]);
        fs::write(out_dir.join("graph.dot"), format!("{:?}", dot)).unwrap();
    }
}
