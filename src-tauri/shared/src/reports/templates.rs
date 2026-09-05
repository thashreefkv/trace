use crate::models::{GraphQueryMode, ReasoningScope, ReportType};

pub(super) fn query_mode(report_type: ReportType, stage: &str) -> GraphQueryMode {
    match report_type {
        ReportType::Quarterly if stage == "outline" => GraphQueryMode::Global,
        ReportType::Quarterly | ReportType::DecisionMemo => GraphQueryMode::Drift,
        ReportType::Initiative if stage == "outline" => GraphQueryMode::Local,
        ReportType::Initiative => GraphQueryMode::Drift,
    }
}

pub(super) fn outline_prompt(
    report_type: ReportType,
    title: &str,
    scope: &ReasoningScope,
    initiative_titles: &[String],
) -> String {
    let coverage = scope_label(scope, initiative_titles);
    let constraint = scope_constraint(initiative_titles);
    match report_type {
        ReportType::Quarterly => format!(
            "Prepare a source-grounded outline for the quarterly report \"{title}\" ({coverage}). \
             {constraint} \
             Cover executive summary, themes, progress, changed assumptions, risks, decisions, and next-quarter recommendations. Cite every evidence-based section."
        ),
        ReportType::Initiative => format!(
            "Prepare a source-grounded outline for the initiative report \"{title}\" ({coverage}). \
             {constraint} \
             Cover objective, delivery progress, evidence, dependencies, risks, implications, and recommended next moves."
        ),
        ReportType::DecisionMemo => format!(
            "Prepare a source-grounded outline for the decision memo \"{title}\" ({coverage}). \
             {constraint} \
             Cover decision, recommendation, alternatives, evidence, tradeoffs, risks, unresolved questions, and proposed actions."
        ),
    }
}

pub(super) fn draft_prompt(
    report_type: ReportType,
    title: &str,
    scope: &ReasoningScope,
    initiative_titles: &[String],
    outline: &str,
) -> String {
    format!(
        "Draft the {} \"{}\" for {} using the approved outline below. \
         {} \
         Produce polished Markdown suitable for approval and export. Cite factual statements with evidence markers, call out unsupported assertions, and distinguish analysis from canonical work facts.\n\nApproved outline:\n{}",
        report_label(report_type),
        title,
        scope_label(scope, initiative_titles),
        scope_constraint(initiative_titles),
        outline
    )
}

pub(super) fn report_label(report_type: ReportType) -> &'static str {
    match report_type {
        ReportType::Quarterly => "Quarterly Report",
        ReportType::Initiative => "Initiative Report",
        ReportType::DecisionMemo => "Decision Memo",
    }
}

pub(super) fn scope_label(scope: &ReasoningScope, initiative_titles: &[String]) -> String {
    let mut parts: Vec<String> = Vec::new();
    if !initiative_titles.is_empty() {
        let mut titles = initiative_titles.to_vec();
        titles.sort();
        parts.push(titles.join(", "));
    }
    if let Some(quarter) = &scope.quarter {
        parts.push(quarter.clone());
    }
    if let (Some(from), Some(to)) = (&scope.date_from, &scope.date_to) {
        parts.push(format!("{from} through {to}"));
    }
    if parts.is_empty() {
        "the selected workgraph scope".to_string()
    } else {
        parts.join(" · ")
    }
}

fn scope_constraint(initiative_titles: &[String]) -> String {
    if initiative_titles.is_empty() {
        return "Cite only evidence packets that are clearly relevant to the selected scope.".to_string();
    }
    let names = initiative_titles
        .iter()
        .map(|title| format!("\"{title}\""))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "Strict scope: write about {names} ONLY. If an evidence packet is about a different initiative or project, ignore it entirely \
         — do not cite it, do not summarise it, do not mention it. Sections must cover {names}, nothing else."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quarterly_drafts_use_drift_reasoning() {
        assert_eq!(
            query_mode(ReportType::Quarterly, "draft"),
            GraphQueryMode::Drift
        );
    }
}
