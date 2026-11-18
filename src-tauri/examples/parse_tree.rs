//! Debug helper: print the raw tree-sitter node kinds of a file.
//! Usage: cargo run --example parse_tree -- <language> <file>
use tree_sitter::Parser;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let language = args[1].clone();
    let path = args[2].clone();
    let text = std::fs::read_to_string(&path).expect("read fixture");
    let lang: tree_sitter::Language = match language.as_str() {
        "C#" => tree_sitter_c_sharp::LANGUAGE.into(),
        "Java" => tree_sitter_java::LANGUAGE.into(),
        "Go" => tree_sitter_go::LANGUAGE.into(),
        "C++" => tree_sitter_cpp::LANGUAGE.into(),
        "PHP" => tree_sitter_php::LANGUAGE_PHP.into(),
        other => panic!("no direct language {other}"),
    };
    let mut parser = Parser::new();
    parser.set_language(&lang).unwrap();
    let tree = parser.parse(text.as_bytes(), None).unwrap();
    let mut cursor = tree.walk();
    let mut depth = 0;
    'outer: loop {
        let node = cursor.node();
        println!(
            "{}{} [{}..{}] named={}",
            "  ".repeat(depth),
            node.kind(),
            node.byte_range().start,
            node.byte_range().end,
            node.is_named()
        );
        if cursor.goto_first_child() {
            depth += 1;
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                break 'outer;
            }
            depth -= 1;
        }
    }
}
