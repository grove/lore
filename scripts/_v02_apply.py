"""One-shot, assertion-checked refactoring for the isolated v0.2 workbench."""
from pathlib import Path
p=Path('.')
def edit(name,old,new):
 s=(p/name).read_text(); assert s.count(old)==1,(name,old[:70],s.count(old)); (p/name).write_text(s.replace(old,new))
edit('src/lib.rs','pub mod publish;','pub mod publish;\npub mod reviews;')
edit('src/storage.rs','pub const SCHEMA_V3: &str = include_str!("../migrations/0003_reaffirmations.sql");','pub const SCHEMA_V3: &str = include_str!("../migrations/0003_reaffirmations.sql");\npub const SCHEMA_V4: &str = include_str!("../migrations/0004_review_history.sql");\npub const SCHEMA_VERSION: i64 = 4;')
edit('src/storage.rs','if version > 3 {','if version > SCHEMA_VERSION {')
edit('src/storage.rs','''        if version < 3 {
            conn.execute_batch(SCHEMA_V3)?;
        }
        Ok(())''','''        if version < 3 {
            conn.execute_batch(SCHEMA_V3)?;
        }
        if version < 4 {
            conn.execute_batch(SCHEMA_V4)?;
        }
        Ok(())''')
edit('src/storage.rs','''    let id = format!("review_{}", &util::digest(key)[7..]);
    conn.execute(
        "INSERT OR IGNORE INTO review_items VALUES(?1,?2,?3,'pending')",
        params![id, project, reason],
    )?;
    Ok(())''','''    crate::reviews::record(conn, project, key, reason)''')
facts='''#[derive(Debug, Clone, Serialize)]
pub struct RelationFact {
    pub id: String,
    pub from: String,
    pub to: String,
    pub kind: String,
    pub evidence_id: String,
    pub assertion_revision_id: String,
    pub source_id: String,
    pub source_revision_id: String,
    pub source_locator: String,
    pub active: bool,
}
pub fn relation_facts(conn: &Connection) -> Result<Vec<RelationFact>> {
    let mut q=conn.prepare("SELECT r.id,f.knowledge_id,t.knowledge_id,r.relation,r.evidence_id,ra.assertion_revision_id,ar.source_id,ar.source_revision_id,s.root_id||':'||s.relative_path,EXISTS(SELECT 1 FROM active_assertions a WHERE a.assertion_revision_id=ar.id) FROM knowledge_relations r JOIN knowledge_revisions f ON f.id=r.from_revision_id JOIN knowledge_revisions t ON t.id=r.to_revision_id JOIN relation_assertions ra ON ra.relation_id=r.id JOIN assertion_revisions ar ON ar.id=ra.assertion_revision_id JOIN sources s ON s.id=ar.source_id UNION ALL SELECT r.id,r.from_unit_id,r.to_unit_id,'reaffirms',r.evidence_id,r.assertion_revision_id,ar.source_id,ar.source_revision_id,s.root_id||':'||s.relative_path,EXISTS(SELECT 1 FROM active_assertions a WHERE a.assertion_revision_id=ar.id) FROM reaffirmation_links r JOIN assertion_revisions ar ON ar.id=r.assertion_revision_id JOIN sources s ON s.id=ar.source_id ORDER BY 1")?;
    Ok(q.query_map([],|r|Ok(RelationFact{id:r.get(0)?,from:r.get(1)?,to:r.get(2)?,kind:r.get(3)?,evidence_id:r.get(4)?,assertion_revision_id:r.get(5)?,source_id:r.get(6)?,source_revision_id:r.get(7)?,source_locator:r.get(8)?,active:r.get(9)?}))?.collect::<rusqlite::Result<_>>()?)
}
'''
edit('src/storage.rs','pub fn relations(conn: &Connection) -> Result<Vec<RelationRow>> {',facts+'\npub fn relations(conn: &Connection) -> Result<Vec<RelationRow>> {')
edit('src/domain.rs','''                "decision",
                "plan",''','''                "decision",
                "design",
                "plan",''')
edit('src/domain.rs','''json!({"type":"string","enum":["decision","plan","proposal","observation","reported_outcome","constraint","question","issue_state","risk","procedure"]}),''','''json!({"type":"string","description":CLASSIFICATION_GUIDANCE,"enum":["decision","design","plan","proposal","observation","reported_outcome","constraint","question","issue_state","risk","procedure"]}),''')
edit('src/domain.rs','"enum":["elaborates","contradicts","supersedes","uncertain"]','"enum":["elaborates","contradicts","supersedes","reaffirms","uncertain"]')
guidance='''/// Classification and epistemic confidence are independent dimensions.
pub const CLASSIFICATION_GUIDANCE: &str = "Use design for an existing documented architecture/specification: components, data flows and intended system behavior described as the selected design. Lack of independent runtime verification does not make that design a future plan. Use proposal for suggestions not adopted and plan for explicit future work; preserve future-intent and uncertainty even if the document is titled Architecture. Use decision for a documented choice or reaffirmation, reported_outcome for a source report of delivery or deployment, and observation for source-described observed behavior. Use constraint for a mandatory rule or invariant; use procedure for an ordered operational workflow. A release gate can be a constraint and its operational checklist a procedure. Lifecycle describes the source's object, not model confidence: default unknown unless active, accepted, completed, proposed, rejected or superseded is documented. Never infer independently verified implementation from any kind. Classify each assertion, not the file as a whole.";
pub fn documentary_basis(kind: &str) -> &'static str {
    match kind {
        "design" => "documented_design_not_runtime_verification",
        "plan"|"proposal" => "documented_future_intent",
        "reported_outcome" => "source_report_not_independent_verification",
        "observation" => "documented_observation_not_independent_verification",
        _ => "documented_record_not_independent_verification",
    }
}
'''
edit('src/domain.rs','pub fn extraction_schema() -> Value {',guidance+'\npub fn extraction_schema() -> Value {')
edit('src/engine.rs','''                    EXTRACT_INSTRUCTIONS,
                    input,''','''                    &format!("{EXTRACT_INSTRUCTIONS} {}", domain::CLASSIFICATION_GUIDANCE),
                    input,''')
edit('src/engine.rs','''    storage::refresh_knowledge(&conn, &config.project_id)?;''','''    storage::refresh_knowledge(&conn, &config.project_id)?;
    crate::reviews::refresh(&conn)?;''')
edit('src/config.rs','"pipeline-v4"','"pipeline-v5-quality"')
for f in ['tests/schema_contracts.rs','tests/temporal_consistency.rs']:
 s=(p/f).read_text().replace('assert_eq!(version, 3);','assert_eq!(version, lore::storage::SCHEMA_VERSION);')
 (p/f).write_text(s)
edit('src/engine/render.rs','''        "plan" | "proposal" => "Proposed or planned work",''','''        "design" => "Documented design, not independently verified",
        "plan" | "proposal" => "Proposed or planned work",
        "constraint" => "Documented requirement",
        "procedure" => "Documented procedure",''')
edit('src/engine/render.rs','''"kind":u.kind,"lifecycle":u.lifecycle''','''"kind":u.kind,"basis":domain::documentary_basis(&u.kind),"lifecycle":u.lifecycle''')
edit('src/engine/render.rs','''There are {reviews} review items; run `lore audit` to inspect them.\\n\\n## Topics\\n\\n''','''See the review queue below for unresolved questions.\\n\\n## Topics\\n\\n''')
edit('src/engine/render.rs','''    let reviews: i64 = runner.conn.query_row(
        "SELECT count(*) FROM review_items WHERE status='pending'",
        [],
        |r| r.get(0),
    )?;
''','')
edit('src/engine/render.rs','''    pages.insert(
        "index.md".into(),''','''    index.push_str(&format!("\\n{}\\n", crate::reviews::status_block(runner.conn)?));
    let review_page=crate::reviews::page(runner.conn)?;
    pages.insert(review_page.path.clone(),review_page);
    pages.insert(
        "index.md".into(),''')
review_api='''/// Change review disposition without inference or graph mutation. Uses the
/// same recoverable publication protocol as knowledge compilation.
pub fn change_review(config: &ResolvedConfig, id: &str, state: &str, reason: &str, actor: &str) -> Result<Value> {
    let _lock=publish::ProjectLock::acquire(config)?;
    publish::recover(config)?;
    let inventory=sources::scan(config)?;
    let old_path=config.state.join("state.db");
    let old=storage::read_only(&old_path)?;
    let plan=make_plan(config,&inventory,Some(&old))?;
    ensure!(plan.status.initialized && !plan.status.needs_update,"run lore update before changing review state: sources, configuration and output must match the baseline");
    let old_pages=storage::pages(&old)?;
    publish::validate_existing(config,&old_pages,false)?;
    drop(old);
    publish::cleanup_abandoned(config)?;
    let generation=util::id("run");
    let _stage=publish::StageGuard::new(config,&generation)?;
    let conn=storage::stage_database(&old_path,&publish::stage_dir(config,&generation).join("state.db"))?;
    conn.execute_batch("BEGIN IMMEDIATE")?;
    let changed=crate::reviews::manual(&conn,id,state,reason,actor)?;
    if !changed {
        conn.execute_batch("ROLLBACK")?;
        return Ok(json!({"changed":false,"review_id":id,"status":state,"model_calls":0}));
    }
    let detail=crate::reviews::show(&conn,id)?;
    let mut pages=old_pages.clone();
    let rp=crate::reviews::page(&conn)?;
    pages.insert(rp.path.clone(),rp);
    let index=pages.get_mut("index.md").context("missing project overview")?;
    let start=index.content.find(crate::reviews::START).context("run lore update to add review status to the overview")?;
    let end=index.content[start..].find(crate::reviews::END).context("invalid overview review marker")?+start+crate::reviews::END.len();
    index.content.replace_range(start..end,&crate::reviews::status_block(&conn)?);
    index.output_digest=util::digest(&index.content);
    storage::save_pages(&conn,&pages)?;
    conn.execute("INSERT INTO runs(id,project_id,phase,source_inventory_digest,started_at,finished_at) VALUES(?1,?2,'completed',?3,?4,?4)",params![generation,config.project_id,inventory.digest,util::now()])?;
    let wiki_digest=util::json_digest(&pages.iter().map(|(p,v)|(p,&v.output_digest)).collect::<Vec<_>>())?;
    conn.execute("INSERT INTO publications VALUES(?1,?2,?3,?4)",params![generation,generation,wiki_digest,util::now()])?;
    storage::set_meta(&conn,"generation",&generation)?;
    ensure!(sources::scan(config)?.digest==inventory.digest,"source documents changed while updating a review");
    publish::validate_existing(config,&old_pages,false)?;
    conn.execute_batch("COMMIT")?;
    conn.close().map_err(|(_,e)|e)?;
    publish::commit(config,&generation,&pages)?;
    Ok(json!({"changed":true,"review_id":id,"status":state,"model_calls":0,"detail":detail}))
}

'''
edit('src/engine.rs','pub fn evidence(config: &ResolvedConfig, id: &str) -> Result<Value> {',review_api+'pub fn evidence(config: &ResolvedConfig, id: &str) -> Result<Value> {')
edit('src/main.rs','''    /// List unresolved reconciliation questions without changing their status.
    Review,''','''    /// Inspect and disposition review questions; never edits source knowledge.
    Review {
        #[command(subcommand)]
        action: Option<ReviewCommand>,
    },''')
cmd='''#[derive(Subcommand)]
enum ReviewCommand {
    /// List pending questions, or all retained records with --all.
    List { #[arg(long)] all: bool },
    /// Show one review, its evidence binding and immutable history.
    Show { id: String },
    /// Record a human resolution without changing the knowledge graph.
    Resolve { id: String, #[arg(long)] reason: String, #[arg(long, default_value="user")] actor: String },
    /// Dismiss a false-positive question with a recorded reason.
    Dismiss { id: String, #[arg(long)] reason: String, #[arg(long, default_value="user")] actor: String },
    /// Reopen a retained review for investigation.
    Reopen { id: String, #[arg(long)] reason: String, #[arg(long, default_value="user")] actor: String },
}
'''
edit('src/main.rs','#[tokio::main]\nasync fn main()',cmd+'\n#[tokio::main]\nasync fn main()')
s=(p/'src/main.rs').read_text();start=s.index('        Command::Review => {');end=s.index('        Command::Purge {',start)
s=s[:start]+'''        Command::Review { action } => {
            match action {
                None | Some(ReviewCommand::List { all: false }) => {
                    let conn=storage::read_only(&config.state.join("state.db"))?;
                    emit(json!({"reviews":lore::reviews::list(&conn,false)?}),cli.json)?;
                }
                Some(ReviewCommand::List { all: true }) => {
                    let conn=storage::read_only(&config.state.join("state.db"))?;
                    emit(json!({"reviews":lore::reviews::list(&conn,true)?}),cli.json)?;
                }
                Some(ReviewCommand::Show { id }) => {
                    let conn=storage::read_only(&config.state.join("state.db"))?;
                    emit(lore::reviews::show(&conn,&id)?,cli.json)?;
                }
                Some(ReviewCommand::Resolve { id, reason, actor }) => emit(engine::change_review(&config,&id,"resolved",&reason,&actor)?,cli.json)?,
                Some(ReviewCommand::Dismiss { id, reason, actor }) => emit(engine::change_review(&config,&id,"dismissed",&reason,&actor)?,cli.json)?,
                Some(ReviewCommand::Reopen { id, reason, actor }) => emit(engine::change_review(&config,&id,"pending",&reason,&actor)?,cli.json)?,
            }
        }
''' +s[end:]
s=s.replace('let path = if topic == "index" {\n                "index.md".into()', 'let path = if topic == "index" || topic == "reviews" {\n                format!("{topic}.md")')
(p/'src/main.rs').write_text(s)
