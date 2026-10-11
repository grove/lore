//! Optional loopback-only, read-only project knowledge visualizer.
//! No provider construction, inference, migrations, source writes or external assets.
use crate::{config::ResolvedConfig, knowledge, publish, storage, util};
use anyhow::{Context, Result, ensure};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd, html};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Read, Write},
    net::{Ipv4Addr, SocketAddrV4, TcpStream},
    path::Path,
    time::Duration,
};

const HTML: &str = include_str!("visualize/assets/index.html");
const CSS: &str = include_str!("visualize/assets/style.css");
const JS: &str = include_str!("visualize/assets/app.js");
const MAX_REQUEST: usize = 4096;
const MAX_PAGE: usize = 4_000_000;
const MAX_CONNECTIONS: usize = 30_000;

pub struct Viewer {
    config: ResolvedConfig,
}
impl Viewer {
    pub fn open(config: &ResolvedConfig) -> Result<Self> {
        ensure!(
            !publish::has_pending(config),
            "publication recovery is pending; run lore update first"
        );
        util::reject_symlinks(&config.wiki)?;
        util::reject_symlinks(&config.state.join("state.db"))?;
        ensure!(
            config.wiki.is_dir(),
            "published wiki missing; run lore init first"
        );
        let conn = storage::read_only(&config.state.join("state.db"))?;
        ensure!(
            storage::meta(&conn, "initialized")?.is_some(),
            "project is not initialized"
        );
        publish::validate_existing(config, &storage::pages(&conn)?, false)?;
        Ok(Self {
            config: config.clone(),
        })
    }
    fn data(
        &self,
    ) -> Result<(
        BTreeMap<String, storage::StoredPage>,
        knowledge::KnowledgeGraph,
    )> {
        ensure!(
            !publish::has_pending(&self.config),
            "publication update is pending"
        );
        let conn = storage::read_only(&self.config.state.join("state.db"))?;
        let pages = storage::pages(&conn)?;
        publish::validate_existing(&self.config, &pages, false)?;
        let graph = knowledge::build(&conn, &knowledge::ZoomOptions::default())?;
        Ok((pages, graph))
    }
    pub fn reply(&self, path: &str) -> Result<(u16, &'static str, Vec<u8>)> {
        let (route, query) = path.split_once('?').unwrap_or((path, ""));
        match route {
            "/" | "/index.html" => Ok((200, "text/html; charset=utf-8", HTML.as_bytes().to_vec())),
            "/assets/style.css" => Ok((200, "text/css; charset=utf-8", CSS.as_bytes().to_vec())),
            "/assets/app.js" => Ok((
                200,
                "text/javascript; charset=utf-8",
                JS.as_bytes().to_vec(),
            )),
            "/api/site" | "/api/page" | "/api/concept" | "/api/record" => {
                let (pages, graph) = self.data()?;
                let value = match route {
                    "/api/site" => site(&self.config, &pages, &graph),
                    "/api/page" => {
                        let id = query_id(query)?;
                        ensure!(pages.contains_key(&id), "unknown published page");
                        let page = pages.get(&id).unwrap();
                        let source = util::read_limited(
                            &publish::page_path(&self.config.wiki, &id)?,
                            MAX_PAGE,
                        )?;
                        ensure!(
                            util::digest(&source) == page.output_digest,
                            "published page changed during read"
                        );
                        let support = graph
                            .knowledge
                            .iter()
                            .map(|record| {
                                let excerpts = record
                                    .evidence
                                    .iter()
                                    .map(|evidence| {
                                        let lines = evidence
                                            .line_start
                                            .map(|start| {
                                                format!(
                                                    " · lines {start}-{}",
                                                    evidence.line_end.unwrap_or(start)
                                                )
                                            })
                                            .unwrap_or_default();
                                        format!(
                                            "{}:{}{lines}\n{}\n{} supporting source",
                                            evidence.root_id,
                                            evidence.path,
                                            evidence.excerpt,
                                            if evidence.active {
                                                "Current"
                                            } else {
                                                "Historical"
                                            }
                                        )
                                    })
                                    .collect::<Vec<_>>()
                                    .join("\n\n");
                                (record.id.clone(), excerpts)
                            })
                            .collect();
                        json!({"id":id,"html":render_markdown(&source,&id,&pages,&support),"source_write":false})
                    }
                    "/api/concept" => {
                        let id = query_id(query)?;
                        let view = graph
                            .nodes
                            .iter()
                            .find(|v| v.id == id)
                            .context("unknown concept")?;
                        let records: Vec<_> = graph
                            .knowledge
                            .iter()
                            .filter(|k| view.knowledge_ids.contains(&k.id))
                            .take(24)
                            .collect();
                        json!({"id":id,"records":records,"total":view.knowledge_ids.len(),"omitted":view.knowledge_ids.len().saturating_sub(records.len())})
                    }
                    _ => {
                        let id = query_id(query)?;
                        let record = graph
                            .knowledge
                            .iter()
                            .find(|k| k.id == id)
                            .context("unknown record")?;
                        let relations: Vec<_> = graph
                            .relations
                            .iter()
                            .filter(|r| r.from == id || r.to == id)
                            .collect();
                        let related_records: Vec<_> = graph
                            .knowledge
                            .iter()
                            .filter(|record| {
                                relations.iter().any(|relation| {
                                    relation.from == record.id || relation.to == record.id
                                })
                            })
                            .map(|record| json!({"id":record.id,"title":record.subject}))
                            .collect();
                        json!({"record":record,"relations":relations,"related_records":related_records})
                    }
                };
                Ok((
                    200,
                    "application/json; charset=utf-8",
                    serde_json::to_vec(&value)?,
                ))
            }
            _ => Ok((404, "text/plain; charset=utf-8", b"Not found".to_vec())),
        }
    }
}
fn query_id(query: &str) -> Result<String> {
    let encoded = query.strip_prefix("id=").context("expected id query")?;
    ensure!(
        !encoded.contains('&') && encoded.len() <= 1024,
        "invalid id query"
    );
    let decoded = percent_encoding::percent_decode_str(encoded)
        .decode_utf8()?
        .to_string();
    ensure!(
        !decoded.is_empty() && !decoded.chars().any(char::is_control),
        "invalid id"
    );
    Ok(decoded)
}
fn site(
    config: &ResolvedConfig,
    pages: &BTreeMap<String, storage::StoredPage>,
    graph: &knowledge::KnowledgeGraph,
) -> Value {
    let ids: BTreeSet<_> = pages.keys().cloned().collect();
    let mut page_nodes = Vec::new();
    let mut page_edges = Vec::new();
    for (id, page) in pages.iter().take(600) {
        let title = Path::new(id)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(id)
            .strip_suffix(".md")
            .unwrap_or(id)
            .replace(['-', '_'], " ");
        page_nodes.push(json!({"id":id,"title":if id=="index.md" {"Project overview".to_string()} else {title},"kind":"page","summary":page.path}));
    }
    let selected: BTreeSet<_> = page_nodes.iter().filter_map(|p| p["id"].as_str()).collect();
    for id in &selected {
        if let Ok(source) =
            util::read_limited(&publish::page_path(&config.wiki, id).unwrap(), MAX_PAGE)
        {
            for target in page_links(&source, id, &ids) {
                if selected.contains(target.as_str()) {
                    page_edges.push(json!({"source":id,"target":target}));
                }
            }
        }
    }
    let mut concepts = Vec::new();
    for n in graph.nodes.iter().take(400) {
        concepts.push(json!({"id":n.id,"title":n.title,"kind":n.kind,"summary":n.summary}));
    }
    let cids: BTreeSet<_> = concepts.iter().filter_map(|c| c["id"].as_str()).collect();
    let concept_edges: Vec<_> = graph
        .edges
        .iter()
        .filter(|e| cids.contains(e.parent.as_str()) && cids.contains(e.child.as_str()))
        .map(|e| json!({"source":e.parent,"target":e.child}))
        .collect();
    let decisions: Vec<_> = graph.knowledge.iter().filter(|v| matches!(v.kind.as_str(),"decision"|"constraint"|"proposal"|"risk")).take(300).map(|v| json!({"id":v.id,"title":v.subject,"kind":v.kind,"lifecycle":v.lifecycle,"summary":v.statement})).collect();
    let dids: BTreeSet<_> = decisions.iter().filter_map(|d| d["id"].as_str()).collect();
    let decision_edges: Vec<_> = graph
        .relations
        .iter()
        .filter(|r| dids.contains(r.from.as_str()) && dids.contains(r.to.as_str()))
        .map(|r| json!({"source":r.from,"target":r.to,"kind":r.kind,"active":r.active}))
        .collect();
    let mut warnings = graph.report.warnings.clone();
    if pages.len() > 600 {
        warnings.push("Only the first 600 wiki pages are shown".into());
    }
    if graph.nodes.len() > 400 {
        warnings.push("Concept graph is limited to 400 visible nodes".into());
    }
    if graph.report.truncated {
        warnings.push(
            "Knowledge Zoom reports truncated navigation; use lore explore for details".into(),
        );
    }
    json!({"project":config.config.project.name,"revision":graph.revision,"pages":page_nodes,"page_edges":page_edges,
        "concepts":concepts,"concept_edges":concept_edges,"decisions":decisions,"decision_edges":decision_edges,
        "warnings":warnings,"source_write":false,"model_calls":0})
}
fn page_links(source: &str, current: &str, ids: &BTreeSet<String>) -> Vec<String> {
    let mut out = BTreeSet::new();
    for event in Parser::new(source) {
        if let Event::Start(Tag::Link { dest_url, .. }) = event
            && let Some(path) = safe_link(&dest_url, current, ids)
        {
            out.insert(path);
        }
    }
    out.into_iter().collect()
}
fn safe_link(link: &str, current: &str, ids: &BTreeSet<String>) -> Option<String> {
    if link.starts_with('#') {
        return Some(current.into());
    }
    if link.contains(':') || link.starts_with('/') || link.starts_with('\\') {
        return None;
    }
    let base = Path::new(current).parent().unwrap_or(Path::new(""));
    let raw = link.split('#').next()?.split('?').next()?;
    let decoded = percent_encoding::percent_decode_str(raw)
        .decode_utf8()
        .ok()?;
    let joined = base.join(decoded.as_ref());
    let mut parts = Vec::new();
    for c in joined.components() {
        match c {
            std::path::Component::Normal(s) => parts.push(s.to_str()?.to_string()),
            std::path::Component::CurDir => (),
            _ => return None,
        }
    }
    let path = parts.join("/");
    ids.contains(&path).then_some(path)
}
fn render_markdown(
    source: &str,
    current: &str,
    pages: &BTreeMap<String, storage::StoredPage>,
    support: &BTreeMap<String, String>,
) -> String {
    let ids: BTreeSet<_> = pages.keys().cloned().collect();
    let mut links = Vec::new();
    let mut footnote = None;
    let parser = Parser::new_ext(
        source,
        Options::ENABLE_TABLES
            | Options::ENABLE_STRIKETHROUGH
            | Options::ENABLE_TASKLISTS
            | Options::ENABLE_FOOTNOTES,
    )
    .flat_map(|event| match event {
        Event::Html(x) | Event::InlineHtml(x) => vec![Event::Text(x)],
        Event::Start(Tag::FootnoteDefinition(id)) => {
            footnote = support.get(id.as_ref());
            vec![Event::Start(Tag::FootnoteDefinition(id))]
        }
        Event::End(TagEnd::FootnoteDefinition) => {
            let mut events = Vec::new();
            if let Some(excerpts) = footnote.take().filter(|text| !text.is_empty()) {
                events.push(Event::Start(Tag::Paragraph));
                events.push(Event::Text(excerpts.clone().into()));
                events.push(Event::End(TagEnd::Paragraph));
            }
            events.push(Event::End(TagEnd::FootnoteDefinition));
            events
        }
        Event::Start(Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        }) => {
            let target = if dest_url.starts_with('#') {
                Some(format!("#{}", dest_url.trim_start_matches('#')))
            } else {
                safe_link(&dest_url, current, &ids).map(|s| {
                    format!(
                        "#page={}",
                        percent_encoding::utf8_percent_encode(
                            &s,
                            percent_encoding::NON_ALPHANUMERIC
                        )
                    )
                })
            };
            links.push(target.is_some());
            target
                .map(|target| {
                    vec![Event::Start(Tag::Link {
                        link_type,
                        dest_url: target.into(),
                        title,
                        id,
                    })]
                })
                .unwrap_or_default()
        }
        Event::End(TagEnd::Link) => {
            if links.pop().unwrap_or(false) {
                vec![Event::End(TagEnd::Link)]
            } else {
                Vec::new()
            }
        }
        Event::Start(Tag::Image {
            link_type: _,
            dest_url: _,
            title: _,
            id: _,
        }) => vec![Event::Text("Image omitted in local reader".into())],
        Event::End(TagEnd::Image) => Vec::new(),
        other => vec![other],
    });
    let mut html_out = String::new();
    html::push_html(&mut html_out, parser);
    html_out
}
fn send(stream: &mut TcpStream, status: u16, mime: &str, data: &[u8]) -> Result<()> {
    let label = match status {
        200 => "OK",
        404 => "Not Found",
        405 => "Method Not Allowed",
        _ => "Error",
    };
    write!(
        stream,
        "HTTP/1.1 {status} {label}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nX-Frame-Options: DENY\r\nContent-Security-Policy: default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; font-src 'self'; base-uri 'none'; form-action 'none'\r\nConnection: close\r\n\r\n",
        data.len()
    )?;
    stream.write_all(data)?;
    Ok(())
}
fn handle(viewer: &Viewer, mut stream: TcpStream) -> Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    let mut buf = [0u8; MAX_REQUEST];
    let mut n = 0;
    while n < buf.len() && !buf[..n].windows(4).any(|x| x == b"\r\n\r\n") {
        let read = stream.read(&mut buf[n..])?;
        if read == 0 {
            break;
        }
        n += read;
    }
    let request = std::str::from_utf8(&buf[..n]).context("invalid HTTP request")?;
    let first = request.split("\r\n").next().unwrap_or("");
    let mut words = first.split_whitespace();
    let method = words.next().unwrap_or("");
    let path = words.next().unwrap_or("");
    let protocol = words.next().unwrap_or("");
    if method != "GET" || !protocol.starts_with("HTTP/1.") {
        return send(
            &mut stream,
            405,
            "text/plain; charset=utf-8",
            b"Only GET is supported",
        );
    }
    if !request.contains("\r\n\r\n") || !path.starts_with('/') || path.len() > 2048 {
        return send(
            &mut stream,
            400,
            "text/plain; charset=utf-8",
            b"Invalid request",
        );
    }
    match viewer.reply(path) {
        Ok((status,mime,body)) => send(&mut stream,status,mime,&body),
        Err(_) => send(&mut stream,400,"application/json; charset=utf-8",br#"{"error":"Unavailable or invalid published knowledge. Run lore status or lore audit."}"#),
    }
}
pub async fn run(config: &ResolvedConfig, port: u16, no_open: bool) -> Result<()> {
    ensure!(port >= 1024, "visualizer port must be at least 1024");
    let viewer = Viewer::open(config)?;
    let listener = tokio::net::TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port))
        .await
        .with_context(|| format!("bind visualizer on 127.0.0.1:{port}"))?;
    let addr = listener.local_addr()?;
    let url = format!("http://127.0.0.1:{}", addr.port());
    println!("Lore visualizer: {url}\nRead-only; Ctrl+C to stop.");
    if !no_open {
        #[cfg(target_os = "windows")]
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", &url])
            .spawn();
        #[cfg(target_os = "macos")]
        let _ = std::process::Command::new("open").arg(&url).spawn();
        #[cfg(all(unix, not(target_os = "macos")))]
        let _ = std::process::Command::new("xdg-open").arg(&url).spawn();
    }
    // One request per connection, with bounded reads/timeouts. Localhost is not a sandbox.
    for _ in 0..MAX_CONNECTIONS {
        let (stream, _) = listener.accept().await?;
        let stream = stream.into_std()?;
        stream.set_nonblocking(false)?;
        let config = viewer.config.clone();
        let result = tokio::task::spawn_blocking(move || handle(&Viewer { config }, stream))
            .await
            .context("visualizer request handler panicked")?;
        if let Err(error) = result {
            eprintln!("Lore visualizer request failed: {error:#}");
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn link_paths_are_bounded() {
        let ids = BTreeSet::from(["index.md".to_string(), "topics/retry.md".to_string()]);
        assert_eq!(
            safe_link("topics/retry.md", "index.md", &ids),
            Some("topics/retry.md".into())
        );
        assert_eq!(safe_link("../secrets.md", "index.md", &ids), None);
        assert_eq!(safe_link("https://example.test", "index.md", &ids), None);
        assert_eq!(
            safe_link("retry.md#rules", "topics/other.md", &ids),
            Some("topics/retry.md".into())
        );
    }
    #[test]
    fn markdown_does_not_pass_literal_html() {
        let rendered = render_markdown(
            "<script>alert('x')</script>\n\n# Hello",
            "index.md",
            &BTreeMap::new(),
            &BTreeMap::new(),
        );
        assert!(!rendered.contains("<script>"));
        assert!(rendered.contains("&lt;script&gt;"));
    }
    #[test]
    fn wiki_citations_render_as_numbered_footnotes() {
        let pages = BTreeMap::from([(
            "index.md".to_string(),
            storage::StoredPage {
                path: "index.md".into(),
                input_digest: String::new(),
                output_digest: String::new(),
                content: String::new(),
            },
        )]);
        let rendered = render_markdown(
            "A documented rule.[^ku_example]\n\n## Sources\n\n[^ku_example]: Source: runbook.md, lines 4-6.\n",
            "index.md",
            &pages,
            &BTreeMap::new(),
        );
        assert!(rendered.contains("footnote-reference"));
        assert!(rendered.contains("footnote-definition"), "{rendered}");
        assert!(rendered.contains(">1</sup>"));
        assert!(!rendered.contains(">ku_example<"));
        assert!(rendered.contains("runbook.md, lines 4-6"));
    }
    #[test]
    fn source_citations_show_stored_support_without_unsafe_links() {
        let support = BTreeMap::from([(
            "ku_example".into(),
            "source-1:runbook.md · lines 4-6\nDo not retry <script>".into(),
        )]);
        let rendered = render_markdown(
            "A rule.[^ku_example]\n\n[^ku_example]: [source-1:runbook.md](../../docs/runbook.md).\n\n[Unsafe](javascript:alert) [External](https://example.test) [Missing](missing.md)",
            "index.md",
            &BTreeMap::new(),
            &support,
        );
        assert!(!rendered.contains("about:blank"));
        assert!(!rendered.contains("href=\"javascript:"));
        assert!(!rendered.contains("href=\"https:"));
        assert!(!rendered.contains("href=\"missing.md"));
        assert!(rendered.contains("source-1:runbook.md"));
        assert!(rendered.contains("Do not retry &lt;script&gt;"));
    }
}
