from pathlib import Path

def edit(name,old,new):
 p=Path(name);s=p.read_text();assert s.count(old)==1,(name,old[:80],s.count(old));p.write_text(s.replace(old,new))
edit('src/engine.rs','mod reconcile;','mod reconcile;\nmod overview;')
edit('tests/common/mod.rs','"synthesize" => {','"synthesize" | "overview" => {')
edit('tests/common/mod.rs','"verify" => {','"verify" | "verify_overview" => {')
edit('src/engine/render.rs','use super::{runner::Runner, timeline};','use super::{runner::Runner, timeline, overview};')
edit('src/engine/render.rs','&crate::storage::relations(runner.conn)?','&crate::storage::relation_facts(runner.conn)?')
edit('src/engine/render.rs','''        let topic_decisions: Vec<_> = decisions
            .iter()
            .filter(|r| r.touches(slug))
            .cloned()
            .collect();''','''        let topic_decisions=timeline::context_for(slug,&decisions);''')
p=Path('src/engine/render.rs');s=p.read_text();a=s.index('    let mut index = format!(');b=s.index('    Ok(pages)',a);s=s[:a]+'''    let index=overview::build(runner,knowledge,&decisions,old.get("index.md"),force).await?;
    pages.insert(index.path.clone(),index);
    let review_page=crate::reviews::page(runner.conn)?;
    pages.insert(review_page.path.clone(),review_page);
'''+s[b:];s=s.replace('&link.from_title','&link.from_label').replace('&link.to_title','&link.to_label');p.write_text(s)
edit('src/engine/timeline.rs','storage::RelationRow','storage::RelationFact')
edit('src/engine/timeline.rs','relations: &[RelationRow]','relations: &[RelationFact]')
edit('src/engine/timeline.rs','    pub from_title: String,','    pub from_title: String,\n    pub from_label: String,\n    pub to_label: String,')
edit('src/engine/timeline.rs','        let observed = from.evidence.iter().find(|e| e.active);','')
edit('src/engine/timeline.rs','            from_title: from.topic_title.clone(),','            from_title: from.topic_title.clone(),\n            from_label: source_label(&relation.source_locator),\n            to_label: predecessor_label(to),')
edit('src/engine/timeline.rs','supporting_source: observed.map(|e| e.source.clone()),','supporting_source: Some(relation.source_locator.clone()),')
edit('src/engine/timeline.rs','evidence_id: observed.map(|e| e.id.clone()),','evidence_id: Some(relation.evidence_id.clone()),')
p=Path('src/engine/timeline.rs');p.write_text(p.read_text()+'''
fn source_label(locator:&str)->String {
 let path=locator.split_once(':').map_or(locator,|(_,p)|p);
 let stem=std::path::Path::new(path).file_stem().and_then(|s|s.to_str()).unwrap_or(path);
 if stem.contains('-')&&stem.chars().any(|c|c.is_ascii_digit()){stem.to_owned()}else{path.to_owned()}
}
fn predecessor_label(unit:&KnowledgeView)->String {
 let active=unit.evidence.iter().filter(|e|e.active).collect::<Vec<_>>();
 let evidence=if active.is_empty(){unit.evidence.iter().collect::<Vec<_>>()}else{active};
 let labels=evidence.into_iter().map(|e|source_label(&e.source)).collect::<std::collections::BTreeSet<_>>();
 if labels.is_empty(){format!("Decision {}",unit.id)}else{labels.into_iter().collect::<Vec<_>>().join(" / ")}
}
/// A reaffirmation page must also see its predecessor's explicit successor.
pub(super) fn context_for(slug:&str,links:&[DecisionLink])->Vec<DecisionLink> {
 let mut ids=std::collections::BTreeSet::new();
 for link in links.iter().filter(|l|l.touches(slug)){ids.insert(link.from_id.clone());ids.insert(link.to_id.clone());}
 loop {let before=ids.len();for link in links{if ids.contains(&link.from_id)||ids.contains(&link.to_id){ids.insert(link.from_id.clone());ids.insert(link.to_id.clone());}}if before==ids.len(){break;}}
 links.iter().filter(|l|ids.contains(&l.from_id)||ids.contains(&l.to_id)).cloned().collect()
}
''')
edit('src/reviews.rs','''        let identity: (String, String) = conn.query_row(
            "SELECT source_id,source_revision_id FROM assertion_revisions WHERE id=?1",
            [assertion],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;''','''        let identity: (String,String,String,String)=conn.query_row("SELECT ar.source_id,ar.source_revision_id,ar.modality,json_extract(ad.proposal_json,'$.lifecycle') FROM assertion_revisions ar JOIN assertion_details ad ON ad.assertion_revision_id=ar.id WHERE ar.id=?1",[assertion],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?;
        // A decision relationship cannot settle a proposal or report merely
        // because both appear in the same document revision.
        if identity.2!="decision"||identity.3!="accepted" {continue;}
''')
