//! Issues derived from evidence on concrete dependency edges.

use super::imports::GraphEdge;
use super::{
    GraphIssue, GraphSnapshot, HIGH_FAN_IN_THRESHOLD, HIGH_FAN_OUT_THRESHOLD, is_dependency,
};
use crate::domain::types::Severity;
use crate::rules::IssueDraft;
use std::collections::{BTreeMap, HashMap};

pub fn snapshot_issues(snapshot: &GraphSnapshot) -> Vec<GraphIssue> {
    let mut issues = Vec::new();
    let cycle_indexes: HashMap<&str, usize> = snapshot
        .cycles
        .iter()
        .enumerate()
        .flat_map(|(i, cycle)| cycle.paths.iter().map(move |path| (path.as_str(), i)))
        .collect();
    let mut cycle_evidence: Vec<Option<(&GraphEdge, f32)>> = vec![None; snapshot.cycles.len()];
    let mut incoming = HashMap::new();
    let mut outgoing = HashMap::new();
    let mut dependency_pairs = BTreeMap::new();
    for edge in &snapshot.edges {
        if is_dependency(&edge.kind) && edge.source != edge.target {
            let pair = (edge.source.as_str(), edge.target.as_str());
            let strongest = dependency_pairs.entry(pair).or_insert(edge);
            if edge.confidence > strongest.confidence
                || (edge.confidence == strongest.confidence
                    && edge.range.start_byte < strongest.range.start_byte)
            {
                *strongest = edge;
            }
        }
    }
    for ((source, target), edge) in dependency_pairs {
        incoming.entry(target).or_insert(edge);
        outgoing.entry(source).or_insert(edge);
        if let Some(&index) = cycle_indexes.get(source)
            && cycle_indexes.get(target) == Some(&index)
        {
            let evidence = cycle_evidence[index].get_or_insert((edge, edge.confidence));
            evidence.1 = evidence.1.min(edge.confidence);
        }
    }

    for (cycle, evidence) in snapshot.cycles.iter().zip(cycle_evidence) {
        let Some((edge, confidence)) = evidence else {
            continue;
        };
        issues.push(anchored_issue(
            edge,
            "architecture.cycle",
            Severity::High,
            |draft| {
                draft
                    .title("循环依赖")
                    .evidence(format!("相互可达的文件：{}", cycle.paths.join("、")))
                    .explanation(format!(
                        "当前静态依赖图中有 {} 个文件相互可达。示例依赖指向 `{}`。{}",
                        cycle.paths.len(),
                        edge.target,
                        if snapshot.incomplete_coverage {
                            "本次扫描存在部分解析覆盖或未解析导入。"
                        } else {
                            ""
                        },
                    ))
                    .impact("循环依赖让模块难以独立修改和测试，构建或加载顺序也可能受影响。")
                    .recommendation("核对组内依赖，考虑提取共用接口或调整依赖方向。")
                    .confidence(confidence)
            },
        ));
    }

    for node in &snapshot.nodes {
        if node.fan_out > HIGH_FAN_OUT_THRESHOLD
            && let Some(&edge) = outgoing.get(node.path.as_str())
        {
            issues.push(anchored_issue(edge, "architecture.high-fan-out", Severity::Medium, |draft| {
                draft
                    .title("模块扇出过高")
                    .evidence(format!("{} 依赖 {} 个模块", node.path, node.fan_out))
                    .explanation(format!(
                        "`{}` 直接依赖 {} 个模块，超过阈值 {HIGH_FAN_OUT_THRESHOLD}。示例依赖指向 `{}`。",
                        node.path, node.fan_out, edge.target,
                    ))
                    .impact("下游模块的变动可能波及该模块。")
                    .recommendation("核对各依赖的职责和边界，按业务聚合紧密相关的依赖。")
                    .confidence(edge.confidence.min(0.85))
            }));
        }
        if node.fan_in > HIGH_FAN_IN_THRESHOLD
            && let Some(&edge) = incoming.get(node.path.as_str())
        {
            issues.push(anchored_issue(edge, "architecture.hotspot", Severity::Medium, |draft| {
                draft
                    .title("高扇入热点模块")
                    .evidence(format!("{} 被 {} 个模块依赖", node.path, node.fan_in))
                    .explanation(format!(
                        "`{}` 被 {} 个模块直接依赖，超过阈值 {HIGH_FAN_IN_THRESHOLD}。当前位置为其中一处引用。",
                        node.path, node.fan_in,
                    ))
                    .impact("热点模块变更需要回归多个调用方。")
                    .recommendation("核对接口稳定性，并在演进前覆盖关键调用方的行为。")
                    .confidence(edge.confidence.min(0.85))
            }));
        }
    }
    issues
}

fn anchored_issue(
    edge: &GraphEdge,
    rule_id: &str,
    severity: Severity,
    describe: impl FnOnce(IssueDraft) -> IssueDraft,
) -> GraphIssue {
    let span = &edge.range;
    let mut draft = IssueDraft::new(
        rule_id,
        "architecture",
        severity,
        span.start_line,
        span.end_line,
    );
    draft.start_column = span.start_column.max(1);
    draft.end_column = span.end_column.max(1);
    GraphIssue {
        draft: describe(draft),
        path: edge.source.clone(),
        coverage: edge.coverage,
    }
}
