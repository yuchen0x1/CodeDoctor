//! Debug helper: parse a fixture file and dump the ParseResult as JSON.
//! Usage: cargo run --example parser_dump -- <language> <file>
use codedoctor_lib::domain::types::{CoverageKind, FileRecord};
use codedoctor_lib::parse::ParserRegistry;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let language = args[1].clone();
    let path = args[2].clone();
    let text = std::fs::read_to_string(&path).expect("read fixture");
    let file = FileRecord {
        id: "debug".into(),
        scan_run_id: "debug".into(),
        project_id: "debug".into(),
        relative_path: path.clone(),
        absolute_path: path.clone(),
        encoding: "utf-8".into(),
        size_bytes: 0,
        content_hash: String::new(),
        language,
        parse_capability: CoverageKind::NotCovered,
        excluded_reason: None,
    };
    let result = ParserRegistry::parse(&file, &text);
    println!("{}", serde_json::to_string_pretty(&result).unwrap());
}
