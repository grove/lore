from pathlib import Path
p=Path('tests/compiler.rs')
s=p.read_text()
a='serde_json::json!(["elaborates", "contradicts", "supersedes", "uncertain"])'
assert s.count(a)==1
p.write_text(s.replace(a,'serde_json::json!(["elaborates", "contradicts", "supersedes", "reaffirms", "uncertain"])'))
p=Path('tests/temporal_consistency.rs');s=p.read_text();s=s.replace('        use lore::inference::GenerativeModel;\n','');p.write_text(s)
