#!/usr/bin/env python3
"""Cross-project live validation and explicit, report-bound human assessments.

Preparation and assessment do not call models. Run invokes the actual CLI.
Passing fixture tests is not a substitute for real inference and human review.
"""
from __future__ import annotations
import argparse
import hashlib
import json
from pathlib import Path
import sqlite3
import subprocess
import sys
from typing import Any
import benchmark as bench
import compare_runs

TARGETS=("atlas","lore-self","openwiki")
CRITERIA=("coverage","design_vs_intent","decision_history","uncertainty","citation_entailment","organization","usefulness")

def read_json(path:Path)->dict:
    if path.is_symlink() or path.stat().st_size>8_000_000:
        raise ValueError("Unsafe or oversized assessment input")
    value=json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value,dict):raise ValueError("Expected a JSON object")
    return value

def write_new(path:Path,value:dict)->None:
    path.parent.mkdir(parents=True,exist_ok=True)
    with path.open("x",encoding="utf-8") as output:
        json.dump(value,output,indent=2,sort_keys=True);output.write("\n")

def binding(run:Path)->dict:
    """Tie human observations to the exact report, sources and pages read."""
    metrics=run/"metrics.json";report=read_json(metrics)
    if not (run/"project"/"wiki").is_dir():raise ValueError("Completed wiki required before human assessment")
    return {"metrics_sha256":hashlib.sha256(metrics.read_bytes()).hexdigest(),
            "wiki_sha256":bench.file_hashes(run/"project"/"wiki"),
            "corpus_sha256":bench.corpus_fingerprint(run/"project"/"docs")["sha256"],
            "lore_binary_sha256":report.get("lore_binary_sha256")}

def init_review(run:Path)->Path:
    path=run/"HUMAN_REVIEW.json"
    write_new(path,{"schema_version":1,"binding":binding(run),"reviewer":None,
        "reviewed_at":None,"complete":False,"critical_errors":None,
        "criteria":{key:{"score":None,"notes":"","evidence_ids":[]} for key in CRITERIA},
        "instructions":"Read the wiki and sources. Score 0=wrong/missing, 1=poor, 2=mostly good, 3=reliable; supply notes and actual evidence IDs. Complete and attribute this record yourself. This is not independent runtime verification."})
    return path

def automated_checks(report:dict)->tuple[bool,list[str]]:
    failures=[];noop=report.get("no_op",{})
    if report.get("synthesis_verification") is not True:failures.append("Synthesis verification was disabled or not recorded")
    if not report.get("lore_binary_sha256"):failures.append("Binary identity missing")
    if not report.get("provider") or not report.get("model"):failures.append("Configured model identity missing")
    for field in ("no_op","zero_generations","pages_unchanged"):
        if noop.get(field) is not True:failures.append("No-op invariant absent or failed: "+field)
    phases=report.get("phases",{})
    if not isinstance(phases,dict) or "initial" not in phases:
        failures.append("Initial inference result missing");phases={}
    if report.get("target")=="atlas" and "after_mutation" not in phases:failures.append("Atlas mutation result missing")
    for name,entry in phases.items():
        score=entry.get("score",{})
        if score.get("sqlite_integrity_ok") is not True:failures.append(name+": database integrity not passed")
        if score.get("current_excerpt_failures")!=[]:failures.append(name+": source evidence not passed")
        if not isinstance(score.get("current_excerpt_checks"),int) or score["current_excerpt_checks"]<=0:failures.append(name+": no checked source evidence")
        # Type labels remain diagnostics rather than a proxy for human truth.
        if report.get("target")=="atlas":
            gold=score.get("gold",{});expected=3 if name=="after_mutation" else 2
            if gold.get("relation_tests_total")!=expected or gold.get("relation_tests_passed")!=expected:failures.append(name+": known decision relationships not passed")
    return not failures,failures

def assess_run(run:Path)->dict[str,Any]:
    run=run.resolve();report=read_json(run/"metrics.json")
    automatic,failures=automated_checks(report);manual=run/"HUMAN_REVIEW.json"
    result={"run":str(run),"target":report.get("target"),"automated_pass":automatic,
        "automated_failures":failures,"human_complete":False,"human_pass":False,
        "human_failures":[],"ready":False,"binary":report.get("lore_binary_sha256"),
        "model_roles":[report.get(k) for k in ("provider","model","decision_provider","decision_model")]}
    if not manual.is_file():result["human_failures"].append("Human assessment not supplied");return result
    human=read_json(manual);errors=[]
    if human.get("schema_version")!=1 or human.get("binding")!=binding(run):errors.append("Human review does not match the current report, sources and wiki")
    if human.get("complete") is not True:errors.append("Human review is incomplete")
    for name in ("reviewer","reviewed_at"):
        value=human.get(name)
        if not isinstance(value,str) or not value.strip():errors.append(name+" missing")
    critical=human.get("critical_errors")
    if not isinstance(critical,list):errors.append("Explicit critical-error assessment missing")
    db=run/"project"/".lore"/"state.db"
    if not db.is_file():errors.append("Evidence database unavailable");evidence=set()
    else:
        conn=sqlite3.connect(db.as_uri()+"?mode=ro",uri=True)
        try:evidence={x[0] for x in conn.execute("SELECT id FROM evidence_snapshots")}
        finally:conn.close()
    scores=[];criteria=human.get("criteria",{})
    if not isinstance(criteria,dict):criteria={}
    for name in CRITERIA:
        value=criteria.get(name,{})
        if not isinstance(value,dict):value={}
        score=value.get("score")
        if type(score) is not int or not 0<=score<=3:errors.append(name+": score 0..3 required")
        else:scores.append(score)
        if not isinstance(value.get("notes"),str) or not value["notes"].strip():errors.append(name+": evidence-based notes required")
        refs=value.get("evidence_ids")
        if not isinstance(refs,list) or not refs or any(not isinstance(x,str) or x not in evidence for x in refs):errors.append(name+": valid evidence IDs required")
    result["human_complete"]=not errors
    result["human_pass"]=not errors and critical==[] and all(x>=2 for x in scores)
    if not errors and critical:errors.append("Reviewer recorded critical errors")
    if not errors and any(x<2 for x in scores):errors.append("At least one human criterion scored below 2")
    result["human_failures"]=errors;result["ready"]=automatic and result["human_pass"]
    return result

def assess(runs:list[Path])->dict:
    entries=[assess_run(path) for path in runs];targets={x["target"] for x in entries}
    return {"schema_version":1,"results":entries,"required_targets":list(TARGETS),
        "missing_targets":sorted(set(TARGETS)-targets),
        "candidate_validated":set(TARGETS)<=targets and all(x["ready"] for x in entries) and len({x["binary"] for x in entries})==1 and len({tuple(x["model_roles"]) for x in entries})==1,
        "qualification":"Small-corpus reviewer-attested gate, not a universal accuracy guarantee or independent implementation verification. Reviewer attribution is supplied by the human, not authenticated by this tool."}

def options(args:argparse.Namespace,target:str,path:Path)->argparse.Namespace:
    return argparse.Namespace(target=target,output=path,provider=args.provider,model=args.model,
        decision_provider=args.decision_provider,decision_model=args.decision_model,
        allow_hosted=args.allow_hosted,base_url=None,generative_base_url=args.generative_base_url,
        decision_base_url=args.decision_base_url,skip_verification=False,lore_binary=str(args.lore_binary.resolve()),
        timeout=args.timeout,mutate=target=="atlas",billed_cost_usd=None,billing_source=None)

def run(args:argparse.Namespace)->dict:
    out=args.output.resolve()
    if out.exists():raise ValueError("Choose a fresh suite output directory; previous results are retained")
    if not 1<=args.repeats<=10 or not 60<=args.timeout<=86400:raise ValueError("Repeats must be 1..10 and timeout 60..86400")
    if not args.lore_binary.is_file():raise ValueError("Build the Lore binary first")
    preview=options(args,args.targets[0],out/"preview")
    bench.config_for(preview.output/"project",preview.target,preview.provider,preview.model,
        preview.decision_provider,preview.decision_model,preview.allow_hosted,None,True,
        generative_base_url=preview.generative_base_url,decision_base_url=preview.decision_base_url)
    out.mkdir(parents=True)
    summary={"schema_version":1,"phase":"real_inference","started_at":bench.now_utc(),"runs":[],"human_validation":"not performed"}
    for target in args.targets:
        successful=[]
        for repeat in range(1,args.repeats+1):
            directory=out/f"{target}-{repeat:02d}"
            try:
                bench.run_command(options(args,target,directory));init_review(directory)
                summary["runs"].append({"target":target,"repeat":repeat,"path":directory.name,"status":"completed"});successful.append(directory)
            except (ValueError,OSError,subprocess.SubprocessError,sqlite3.Error) as error:
                summary["runs"].append({"target":target,"repeat":repeat,"path":directory.name,"status":"failed","error_type":type(error).__name__,"note":"Inspect the local retained workspace; no raw provider errors or excerpts logged."})
            (out/"suite.json").write_text(json.dumps(summary,indent=2)+"\n",encoding="utf-8")
        if len(successful)>1:write_new(out/f"{target}-repeatability.json",compare_runs.summarize(successful))
    summary["completed_at"]=bench.now_utc();(out/"suite.json").write_text(json.dumps(summary,indent=2)+"\n",encoding="utf-8")
    return summary

def main(argv:list[str]|None=None)->int:
    parser=argparse.ArgumentParser(description=__doc__);sub=parser.add_subparsers(dest="command",required=True)
    prepare=sub.add_parser("prepare",help="Prepare selected corpora; no inference")
    prepare.add_argument("--targets",nargs="+",choices=TARGETS,default=list(TARGETS));prepare.add_argument("--output",type=Path,required=True)
    execute=sub.add_parser("run",help="Run the actual CLI with independent caches")
    execute.add_argument("--targets",nargs="+",choices=TARGETS,default=list(TARGETS));execute.add_argument("--repeats",type=int,default=1)
    execute.add_argument("--provider",choices=("ollama","openai"),required=True);execute.add_argument("--model",required=True)
    execute.add_argument("--decision-provider",choices=("ollama","openai","typesafe"));execute.add_argument("--decision-model")
    execute.add_argument("--generative-base-url");execute.add_argument("--decision-base-url");execute.add_argument("--allow-hosted",action="store_true")
    execute.add_argument("--lore-binary",type=Path,required=True);execute.add_argument("--timeout",type=int,default=3600);execute.add_argument("--output",type=Path,required=True)
    init=sub.add_parser("init-review",help="Create an unscored human review template");init.add_argument("run",type=Path)
    check=sub.add_parser("assess",help="Check bound human assessments and completed live runs");check.add_argument("runs",nargs="+",type=Path);check.add_argument("--output",type=Path)
    args=parser.parse_args(argv)
    try:
        if args.command=="prepare":
            if args.output.exists():raise ValueError("Preparation requires a new directory")
            args.output.mkdir(parents=True);result={"phase":"preparation_only","inference_calls":0,"targets":{}}
            for target in dict.fromkeys(args.targets):
                project=args.output/target;manifest=bench.prepare(target,project);spec=bench.target_spec(target)
                bench.manifest_gold_check(project,bench.ROOT/spec["gold"] if spec.get("gold") else None,["initial"]);result["targets"][target]=manifest
            write_new(args.output/"prepared.json",result)
        elif args.command=="run":args.targets=list(dict.fromkeys(args.targets));result=run(args)
        elif args.command=="init-review":result={"template":str(init_review(args.run.resolve())),"human_scores":"not supplied"}
        else:
            result=assess(args.runs)
            if args.output:write_new(args.output,result)
        print(json.dumps(result,indent=2))
        if args.command=="assess" and not result["candidate_validated"]:return 2
        if args.command=="run" and any(x["status"]=="failed" for x in result["runs"]):return 1
        return 0
    except (ValueError,OSError,sqlite3.Error,subprocess.SubprocessError) as error:
        print(f"Validation error: {error}",file=sys.stderr);return 1
if __name__=="__main__":raise SystemExit(main())
