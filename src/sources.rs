use anyhow::{Context, Result, ensure};
use globset::{Glob, GlobSetBuilder};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};
use crate::{config::ResolvedConfig, util};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub key: String,
    pub heading: String,
    pub heading_path: Vec<String>,
    pub text: String,
    pub context: String,
    pub offset: usize,
    pub input_digest: String,
}
#[derive(Debug, Clone)]
pub struct Document {
    pub root_id: String,
    pub relative_path: String,
    pub physical_path: PathBuf,
    pub text: String,
    pub digest: String,
    pub chunks: Vec<Chunk>,
}
#[derive(Debug, Clone)]
pub struct Inventory { pub documents: Vec<Document>, pub digest: String, pub warnings: Vec<String> }

pub fn scan(config: &ResolvedConfig) -> Result<Inventory> {
    let mut builder = GlobSetBuilder::new();
    for pattern in &config.config.sources.exclude { builder.add(Glob::new(pattern).context("invalid source exclude glob")?); }
    let exclusions = builder.build()?;
    let mut documents = Vec::new(); let mut warnings = Vec::new();
    for (root_id, root) in &config.roots {
        util::reject_symlinks(root)?;
        ensure!(root.is_dir(), "source root is missing or not a directory: {} (not treated as deletion)", root.display());
        let wiki = config.wiki.clone(); let state = config.state.clone();
        let walker = ignore::WalkBuilder::new(root).hidden(false).follow_links(false).require_git(false)
            .filter_entry(move |e| !e.path().starts_with(&wiki) && !e.path().starts_with(&state)
                && e.file_name() != ".git" && !e.file_name().to_string_lossy().starts_with(".lore-stage-"))
            .build();
        for entry in walker {
            let entry = entry.context("source traversal failed; no deletion inferred")?;
            let path = entry.path();
            if entry.file_type().is_some_and(|t| t.is_symlink()) {
                warnings.push(format!("Skipped symlink: {}", path.display())); continue;
            }
            if !entry.file_type().is_some_and(|t| t.is_file()) { continue; }
            let extension = path.extension().and_then(|s| s.to_str()).unwrap_or("");
            if !extension.eq_ignore_ascii_case("md") && !extension.eq_ignore_ascii_case("markdown") { continue; }
            let relative = path.strip_prefix(root)?.to_str().context("source paths must be UTF-8")?.replace('\\', "/");
            if exclusions.is_match(&relative) || relative.split('/').any(|part| exclusions.is_match(part)) { continue; }
            let text = util::read_limited(path, config.config.processing.max_file_bytes)?;
            let chunks = split_markdown(&text, root_id, &relative, config.config.processing.max_section_bytes)?;
            documents.push(Document { root_id: root_id.clone(), relative_path: relative, physical_path: path.to_owned(), digest: util::digest(&text), text, chunks });
        }
    }
    documents.sort_by(|a,b| (&a.root_id,&a.relative_path).cmp(&(&b.root_id,&b.relative_path)));
    let digest = util::json_digest(&documents.iter().map(|d| (&d.root_id,&d.relative_path,&d.digest)).collect::<Vec<_>>())?;
    Ok(Inventory { documents, digest, warnings })
}

pub fn split_markdown(text: &str, root: &str, path: &str, limit: usize) -> Result<Vec<Chunk>> {
    ensure!(limit >= 512, "section limit too small");
    let front_end = frontmatter_end(text);
    let body = &text[front_end..];
    let mut headings = Vec::<(usize, String, usize)>::new();
    let mut current: Option<(usize, String, usize)> = None;
    for (event, range) in Parser::new_ext(body, Options::all()).into_offset_iter() {
        match event {
            Event::Start(Tag::Heading { level, .. }) => current = Some((front_end + range.start, String::new(), level as usize)),
            Event::End(TagEnd::Heading(_)) => if let Some(h) = current.take() { headings.push(h); },
            Event::Text(t) | Event::Code(t) => if let Some((_, title, _)) = current.as_mut() { title.push_str(&t); },
            Event::SoftBreak | Event::HardBreak => if let Some((_, title, _)) = current.as_mut() { title.push(' '); },
            _ => {},
        }
    }
    let first = headings.first().map(|h| h.0).unwrap_or(text.len());
    // A changed document preamble/front matter invalidates all descendant chunks.
    // Sibling content is reconciled across the knowledge graph, not copied here.
    let context = text[..first].to_owned();
    ensure!(context.len() <= limit * 2 || headings.is_empty(), "document preamble exceeds context limit: {path}");
    let mut segments = Vec::new();
    if first > 0 && !text[..first].trim().is_empty() { segments.push((0, first, vec!["Preamble".to_string()])); }
    let mut hierarchy: Vec<(usize, String)> = Vec::new();
    for (i, (start, title, level)) in headings.iter().enumerate() {
        while hierarchy.last().is_some_and(|(l,_)| l >= level) { hierarchy.pop(); }
        hierarchy.push((*level, title.clone()));
        let end = headings.get(i+1).map(|h| h.0).unwrap_or(text.len());
        segments.push((*start, end, hierarchy.iter().map(|(_,s)| s.clone()).collect()));
    }
    if segments.is_empty() && !text.trim().is_empty() { segments.push((0, text.len(), vec!["Document".into()])); }
    let stem = std::path::Path::new(path).file_stem().and_then(|s| s.to_str()).unwrap_or(path);
    let mut occurrences = BTreeMap::<String, usize>::new(); let mut chunks = Vec::new();
    for (start, end, heading_path) in segments {
        let base_key = serde_json::to_string(&heading_path)?;
        let occurrence = occurrences.entry(base_key.clone()).or_default(); let ordinal = *occurrence; *occurrence += 1;
        let mut offset = start; let mut part = 0;
        while offset < end {
            let mut boundary = (offset + limit).min(end);
            while !text.is_char_boundary(boundary) { boundary -= 1; }
            if boundary < end {
                if let Some(newline) = text[offset..boundary].rfind('\n') { if newline > limit / 2 { boundary = offset + newline + 1; } }
            }
            let section = &text[offset..boundary];
            if !section.trim().is_empty() {
                let ctx = if headings.is_empty() { String::new() } else { context.clone() };
                let input_digest = util::json_digest(&("markdown-context-v1", root, stem, &heading_path, &ctx, section))?;
                chunks.push(Chunk { key: format!("{base_key}:{ordinal}:{part}"), heading: heading_path.last().cloned().unwrap_or_default(), heading_path: heading_path.clone(), text: section.to_owned(), context: ctx, offset, input_digest });
            }
            ensure!(boundary > offset, "unable to split Markdown on UTF-8 boundary");
            offset = boundary; part += 1;
        }
    }
    Ok(chunks)
}
fn frontmatter_end(text: &str) -> usize {
    let first = if text.starts_with("---\r\n") { 5 } else if text.starts_with("---\n") { 4 } else { return 0; };
    let mut offset = first;
    for line in text[first..].split_inclusive('\n') {
        offset += line.len();
        if matches!(line.trim_end_matches(['\r','\n']), "---" | "...") { return offset; }
    }
    0
}

/// Resolve an exact, unambiguous quote in the captured bytes, never in a later
/// filesystem read. This is the boundary between model prose and real evidence.
pub fn locate_quote(document: &Document, chunk: &Chunk, quote: &str) -> Result<(usize, usize, String, String)> {
    ensure!(!quote.trim().is_empty() && quote.len() <= chunk.text.len(), "empty or oversized evidence quote");
    let matches: Vec<_> = chunk.text.match_indices(quote).collect();
    ensure!(matches.len() == 1, "evidence quote is missing or ambiguous; use a longer verbatim passage");
    let start = chunk.offset + matches[0].0; let end = start + quote.len();
    ensure!(document.text.get(start..end) == Some(quote), "evidence does not match captured source bytes");
    let line_start = document.text[..start].bytes().filter(|b| *b == b'\n').count() + 1;
    let line_end = line_start + quote.bytes().filter(|b| *b == b'\n').count();
    let mut before = start.saturating_sub(256); while !document.text.is_char_boundary(before) { before += 1; }
    let mut after = (end + 256).min(document.text.len()); while !document.text.is_char_boundary(after) { after -= 1; }
    Ok((line_start, line_end, document.text[before..start].to_owned(), document.text[end..after].to_owned()))
}
