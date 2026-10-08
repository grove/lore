use lore::domain::{AssertionProposal, KnowledgeView};

fn decision() -> (KnowledgeView, AssertionProposal) {
    let unit = KnowledgeView {
        id: "ku_old".into(),
        revision_id: "kr_old".into(),
        statement: "MySQL was selected.".into(),
        topic: "database".into(),
        topic_title: "Database".into(),
        subject: "database".into(),
        kind: "decision".into(),
        lifecycle: "accepted".into(),
        base_lifecycle: "accepted".into(),
        scope: "production".into(),
        effective_at: String::new(),
        support_state: "current_documentary_support".into(),
        evidence: vec![],
        relations: vec![],
    };
    let assertion = AssertionProposal {
        topic: unit.topic.clone(),
        topic_title: unit.topic_title.clone(),
        subject: unit.subject.clone(),
        statement: unit.statement.clone(),
        kind: unit.kind.clone(),
        lifecycle: "accepted".into(),
        scope: unit.scope.clone(),
        effective_at: String::new(),
        quote: "MySQL was selected.".into(),
    };
    (unit, assertion)
}

#[test]
fn undated_reversal_is_not_merged_into_a_superseded_decision() {
    let (mut unit, assertion) = decision();
    assert!(unit.same_semantics(&assertion));
    unit.lifecycle = "superseded".into();
    assert!(!unit.same_semantics(&assertion));
}

#[test]
fn pending_supersession_edges_are_considered_before_lifecycle_refresh() {
    let (mut unit, assertion) = decision();
    unit.relations
        .push("ku_new supersedes ku_old (current documentary evidence)".into());
    assert!(!unit.same_semantics(&assertion));
}

#[test]
fn explicit_historical_date_can_still_identify_the_same_past_decision() {
    let (mut unit, mut assertion) = decision();
    unit.lifecycle = "superseded".into();
    unit.effective_at = "January 2026".into();
    assertion.effective_at = "January 2026".into();
    assert!(unit.same_semantics(&assertion));
}
