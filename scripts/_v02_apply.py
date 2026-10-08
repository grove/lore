from pathlib import Path
import json
p=Path('evaluation/benchmark.py');s=p.read_text()
def one(old,new):
 global s
 assert s.count(old)==1,(old[:80],s.count(old));s=s.replace(old,new)
one('allow_hosted: bool, base_url: str | None, verify: bool) -> dict:', 'allow_hosted: bool, base_url: str | None, verify: bool, *,\n               generative_base_url: str | None = None, decision_base_url: str | None = None) -> dict:')
one('    providers = {}\n    roles = {"generative":','''    if base_url and (generative_base_url or decision_base_url):
        err("Use either --base-url or role-specific endpoint flags, not both")
    if decision_base_url and not decision_provider:
        err("--decision-base-url requires a decision provider and model")
    if provider == decision_provider and generative_base_url and decision_base_url and generative_base_url.rstrip("/") != decision_base_url.rstrip("/"):
        err("This configuration stores one endpoint per provider; role endpoints for the same provider must agree")
    providers = {}
    roles = {"generative":''')
one('    if decision_provider:\n        roles["decision"]','''    if generative_base_url:
        providers[provider]["base_url"] = generative_base_url
    if decision_base_url:
        providers[decision_provider]["base_url"] = decision_base_url
    if decision_provider:
        roles["decision"]''')
one('args.allow_hosted, args.base_url, not args.skip_verification)','''args.allow_hosted, args.base_url, not args.skip_verification,
                                generative_base_url=getattr(args, "generative_base_url", None),
                                decision_base_url=getattr(args, "decision_base_url", None))''')
one('    run.add_argument("--base-url")','''    run.add_argument("--base-url")
    run.add_argument("--generative-base-url", help="Explicit generative endpoint; supports Foundry with a different decision provider")
    run.add_argument("--decision-base-url", help="Explicit decision-provider endpoint")''')
one('"lore_binary_sha256": binary_digest,','''"lore_binary_sha256": binary_digest,
              "configuration_sha256": hashlib.sha256(json.dumps(checked_config, sort_keys=True).encode()).hexdigest(),
              "rubric": {"version": load_json(gold_path).get("rubric_version", "legacy-v1") if gold_path else None,
                         "sha256": hashlib.sha256(gold_path.read_bytes()).hexdigest() if gold_path else None},''')
one('"model_calls_by_task": calls,','''"rubric": {"version": load_json(gold_path).get("rubric_version", "legacy-v1") if gold_path else None,
                           "sha256": hashlib.sha256(gold_path.read_bytes()).hexdigest() if gold_path else None},
                "model_calls_by_task": calls,''')
p.write_text(s)
p=Path('evaluation/gold/atlas.json');old=p.read_bytes();archive=Path('evaluation/gold/archive/atlas-v1.json');archive.parent.mkdir(parents=True,exist_ok=True);archive.write_bytes(old)
gold=json.loads(old);gold['rubric_version']='atlas-v02-design-1';gold['changes']='Introduces documented-design category, separate from future intent and runtime verification. Explicitly accepts procedure/active for an applicable mandatory approval workflow. Old rubric retained in archive/atlas-v1.json; different rubric scores are not model-quality improvement evidence.'
for a in gold['expected_assertions']:
 if a['id']=='mysql-architecture':
  a['kind']='design';a['acceptable_pairs']=[['observation','unknown'],['design','active']];a['label_rationale']='Selected documented design, not a future plan. An attributed observation also preserves documentary status; lack of runtime verification does not imply future intent.'
 if a['id']=='release-gate':
  a['acceptable_pairs']=[['procedure','unknown'],['procedure','active'],['constraint','active']];a['label_rationale']='Mandatory sign-off is a constraint; its ordered approval/rollback checklist is an applicable procedure. Neither is an implementation-completion claim.'
p.write_text(json.dumps(gold,indent=2)+'\n')
p=Path('evaluation/compare_runs.py');s=p.read_text();s=s.replace('"binary_sha256": metrics.get("lore_binary_sha256"),','"binary_sha256": metrics.get("lore_binary_sha256"),\n        "configuration_sha256": metrics.get("configuration_sha256"),\n        "rubric": metrics.get("rubric"),')
a='    models_a={(m,p) for m,p,_ in a["observed_model_pairs"]}';assert a in s
s=s.replace(a,'''    if a.get("configuration_sha256") != b.get("configuration_sha256"):
        reasons.append("Different effective provider/pipeline configurations")
    if a.get("rubric") != b.get("rubric"):
        reasons.append("Different evaluation rubrics; label scores are not comparable")
'''+a);p.write_text(s)
# Track an archived rubric as source, not as a generated evaluation artifact.
import subprocess
subprocess.run(['git','add','evaluation/gold/archive/atlas-v1.json'],check=True)
