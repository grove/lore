"""Offline tests for the evaluator; never invoke a model."""
from pathlib import Path
import json
import sqlite3
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import benchmark as bench

class BenchmarkTests(unittest.TestCase):
    def test_prepare_and_evolve_atlas(self):
        with tempfile.TemporaryDirectory() as temp:
            project=Path(temp)/"project"
            manifest=bench.prepare("atlas",project)
            self.assertEqual(len(manifest["files"]),9)
            gold=bench.ROOT/"gold"/"atlas.json"
            bench.manifest_gold_check(project,gold,["initial"])
            changes=bench.apply_mutations(project,"atlas")
            self.assertTrue(changes["applied"])
            self.assertEqual(len(changes["added_files"]),2)
            bench.manifest_gold_check(project,gold,["initial","after_mutation"])
            with self.assertRaises(ValueError): bench.prepare("atlas",project)

    def test_lore_self_checkpoints_track_current_docs(self):
        with tempfile.TemporaryDirectory() as temp:
            project=Path(temp)/"project"
            manifest=bench.prepare("lore-self",project)
            self.assertGreaterEqual(len(manifest["files"]),3)
            bench.manifest_gold_check(project,bench.ROOT/"gold"/"lore-self.json",["initial"])

    def test_hosted_requires_explicit_consent(self):
        with tempfile.TemporaryDirectory() as temp:
            path=Path(temp)
            with self.assertRaisesRegex(ValueError,"Hosted evaluation"):
                bench.config_for(path,"atlas","openai","model",None,None,False,None,True)
            with self.assertRaisesRegex(ValueError,"Hosted evaluation"):
                bench.config_for(path,"atlas","ollama","model","openai","gpt-6-luna",False,None,True)
            config=bench.config_for(path,"atlas","openai","model",None,None,True,None,True)
            self.assertFalse(config["privacy"]["local_only"])
            self.assertEqual(config["providers"]["openai"]["api_key_env"],"OPENAI_API_KEY")
            self.assertTrue(bench.config_for(path,"atlas","ollama","model",None,None,False,None,True)["privacy"]["local_only"])
            with self.assertRaisesRegex(ValueError,"ambiguous"):
                bench.config_for(path,"atlas","ollama","m","openai","d",True,"http://localhost",True)

    def test_reasoning_defaults_overrides_and_invalid_settings(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp)
            config=bench.config_for(root, "atlas", "openai", "gpt-6-luna",
                                    "ollama", "clef-flash", True, None, True)
            self.assertEqual(config["models"]["reasoning"],bench.REASONING_DEFAULTS)
            args=bench.parse_args([
                "run", "--target", "atlas", "--provider", "openai",
                "--model", "gpt-6-luna", "--output", str(root/"run"),
                "--reasoning-extraction", "high",
                "--reasoning-verification", "xhigh"
            ])
            policy=bench.reasoning_from_args(args)
            self.assertEqual(policy["extraction"],"high")
            self.assertEqual(policy["verification"],"xhigh")
            self.assertEqual(policy["reconciliation"],"high")
            custom=bench.config_for(root, "atlas", "openai", "gpt-6-luna",
                                    None, None, True, None, True, reasoning=policy)
            self.assertEqual(custom["models"]["reasoning"]["verification"],"xhigh")
            args=bench.parse_args([
                "run", "--target", "atlas", "--provider", "openai",
                "--model", "gpt-6-luna", "--output", str(root/"run2"),
                "--disable-reasoning"
            ])
            self.assertFalse(bench.reasoning_from_args(args)["enabled"])
            with self.assertRaises(ValueError):
                bench.config_for(root,"atlas","openai","gpt-6-luna",
                                 None,None,True,None,True,
                                 reasoning={"enabled": True,"extraction":"hyper"})
            with self.assertRaises(SystemExit):
                bench.parse_args([
                    "run", "--target", "atlas", "--provider", "openai",
                    "--model", "gpt-6-luna", "--output", str(root/"bad"),
                    "--reasoning-extraction", "hyper"
                ])

    def test_source_containment(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp)/"source";root.mkdir()
            (root/"safe.md").write_text("# Safe\n")
            out=Path(temp)/"out";out.mkdir()
            with self.assertRaisesRegex(ValueError,"Unsafe"):
                bench.collect_docs(root,["../*.md"],out)
            self.assertEqual(bench.collect_docs(root,["*.md"],out)["files"],["safe.md"])
            try:(root/"escape.md").symlink_to(root/"safe.md")
            except (OSError,NotImplementedError):return
            with self.assertRaisesRegex(ValueError,"symlink"):
                bench.collect_docs(root,["escape.md"],out)

    def test_lexical_quality_scoring_does_not_claim_truth(self):
        with tempfile.TemporaryDirectory() as temp:
            project=Path(temp)
            (project/"docs"/"decisions").mkdir(parents=True)
            (project/"docs"/"decisions"/"ADR-001.md").write_text("We selected MySQL as our primary database.\n")
            (project/".lore").mkdir()
            db=sqlite3.connect(project/".lore"/"state.db")
            db.executescript("""
            CREATE TABLE source_current(source_id TEXT);
            CREATE TABLE source_revisions(id TEXT);
            CREATE TABLE knowledge_units(id TEXT);
            CREATE TABLE knowledge_current(knowledge_id TEXT);
            CREATE TABLE evidence_snapshots(id TEXT,exact_excerpt TEXT);
            CREATE TABLE review_items(status TEXT);
            CREATE TABLE model_calls(task TEXT,provider TEXT,model TEXT,cache_hit INTEGER,duration_ms INTEGER);
            CREATE TABLE active_assertions(assertion_revision_id TEXT);
            CREATE TABLE assertion_revisions(id TEXT,source_id TEXT,modality TEXT);
            CREATE TABLE sources(id TEXT,root_id TEXT,relative_path TEXT);
            CREATE TABLE assertion_details(assertion_revision_id TEXT,proposal_json TEXT);
            CREATE TABLE assertion_evidence(assertion_revision_id TEXT,evidence_id TEXT);
            CREATE TABLE assertion_assignments(assertion_revision_id TEXT,knowledge_id TEXT);
            CREATE TABLE knowledge_relations(id TEXT,from_revision_id TEXT,to_revision_id TEXT,relation TEXT);
            CREATE TABLE knowledge_revisions(id TEXT,knowledge_id TEXT);
            CREATE TABLE relation_assertions(relation_id TEXT,assertion_revision_id TEXT);
            INSERT INTO sources VALUES('s','docs','decisions/ADR-001.md');
            INSERT INTO active_assertions VALUES('ar');
            INSERT INTO assertion_revisions VALUES('ar','s','decision');
            INSERT INTO assertion_assignments VALUES('ar','ku');
            INSERT INTO knowledge_units VALUES('ku');
            INSERT INTO knowledge_current VALUES('ku');
            INSERT INTO source_current VALUES('s');
            INSERT INTO source_revisions VALUES('rev');
            INSERT INTO assertion_evidence VALUES('ar','ev');
            INSERT INTO evidence_snapshots VALUES('ev','We selected MySQL as our primary database.');
            INSERT INTO model_calls VALUES('extract','Ollama','fixture',0,10);
            """)
            db.execute("INSERT INTO assertion_details VALUES('ar',?)",
                       (json.dumps({"kind":"decision","lifecycle":"accepted"}),))
            db.commit();db.close()
            gold=project/"gold.json"
            gold.write_text(json.dumps({"schema_version":1,"expected_assertions":[
                {"id":"choice","source":"docs:decisions/ADR-001.md","needle":"selected MySQL as our primary",
                 "kind":"decision","lifecycle":"accepted"}],"expected_relations":[]}))
            score=bench.score_project(project,gold)
            self.assertTrue(score["sqlite_integrity_ok"])
            self.assertEqual(score["gold"]["typed_coverage_proxy"],1.0)
            self.assertFalse(score["current_excerpt_failures"])
            self.assertEqual(score["model_calls_by_task"][0]["duration_ms"],10)
            (project/"docs"/"decisions"/"ADR-001.md").write_text("The quote disappeared.\n")
            score=bench.score_project(project,gold)
            self.assertEqual(score["current_excerpt_failures"][0]["reason"],"excerpt_not_in_current_source")

    def test_report_is_honest_about_cost_and_quality(self):
        self.assertTrue(bench.overlap_match("MySQL","Selected MySQL for the ledger"))
        self.assertFalse(bench.overlap_match("PostgreSQL","Selected MySQL for the ledger"))
        report={"target":"atlas","run_at":"now","provider":"ollama","model":"fixture",
                "phases":{},"billed_cost_usd":None,
                "no_op":{"no_op":True,"zero_generations":True,"pages_unchanged":True}}
        text=bench.report_markdown(report)
        self.assertIn("Not measured",text)
        self.assertIn("not",text.lower())

if __name__=="__main__":
    unittest.main()
