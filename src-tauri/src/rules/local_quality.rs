use super::{IssueDraft, Rule, RuleContext};
use crate::domain::types::{CoverageKind, Severity};
use crate::parse::SourceRange;

const MAX_FINDINGS_PER_RULE: usize = 20;

fn draft(rule: &impl Rule, range: &SourceRange) -> IssueDraft {
    let mut issue = IssueDraft::new(
        rule.id(),
        rule.category(),
        rule.default_severity(),
        range.start_line,
        range.end_line,
    );
    issue.start_column = range.start_column;
    issue.end_column = range.end_column;
    issue
}

pub(crate) struct UnusedLocalBindingRule;

impl Rule for UnusedLocalBindingRule {
    fn id(&self) -> &'static str {
        "quality.unused-local-variable"
    }

    fn category(&self) -> &'static str {
        "quality"
    }

    fn default_severity(&self) -> Severity {
        Severity::Low
    }

    fn supports(&self, language: &str, coverage: CoverageKind) -> bool {
        language == "Python" && coverage == CoverageKind::Ast
    }

    fn check(&self, ctx: &RuleContext<'_>) -> Vec<IssueDraft> {
        let Some(parse) = ctx.parse else {
            return Vec::new();
        };
        parse.python_quality.unused_bindings.iter().take(MAX_FINDINGS_PER_RULE).map(|binding| {
            draft(self, &binding.range)
                .title("疑似未使用的局部变量")
                .evidence(format!("`{}` 在函数中只出现在赋值位置，未见其他同名标识符。", binding.name))
                .explanation("基于 Python 语法树检查函数体的简单赋值与标识符记录。运行时反射和外部调试工具可能观察到局部变量。")
                .impact("未引用的变量会增加阅读负担，也可能暴露遗漏的计算结果。")
                .recommendation("核对该值是否应参与业务逻辑。确认无需保留绑定后再清理，并保留初始化表达式的必要副作用。")
                .confidence(0.8)
        }).collect()
    }
}

pub(crate) struct UnreachableStatementRule;

impl Rule for UnreachableStatementRule {
    fn id(&self) -> &'static str {
        "quality.unreachable-statement"
    }

    fn category(&self) -> &'static str {
        "quality"
    }

    fn default_severity(&self) -> Severity {
        Severity::Low
    }

    fn supports(&self, language: &str, coverage: CoverageKind) -> bool {
        language == "Python" && coverage == CoverageKind::Ast
    }

    fn check(&self, ctx: &RuleContext<'_>) -> Vec<IssueDraft> {
        let Some(parse) = ctx.parse else {
            return Vec::new();
        };
        parse.python_quality.unreachable.iter().take(MAX_FINDINGS_PER_RULE).map(|statement| {
            draft(self, &statement.range)
                .title("不可达语句")
                .evidence(format!("第 {} 行的 `{}` 会结束当前执行路径，后续语句与它位于同一函数体层级。", statement.terminator_line, statement.terminator))
                .explanation("基于 Python 语法树发现直接位于无条件返回或抛出之后的同层语句。")
                .impact("该语句在当前控制流中无法执行，可能掩盖遗漏的业务步骤。")
                .recommendation("核对语句本应执行的位置。清理前检查赋值、产出语句及作用域可能影响的函数语义。")
                .confidence(0.99)
        }).collect()
    }
}
