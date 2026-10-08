"""Offline validation of the real-run repeatability comparator."""
import json
from pathlib import Path
import sqlite3
import sys
import tempfile
import unittest

sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
import compare_runs
import benchmark


def fixture(root: Path, *, corpus="MySQL", topic="ledger",
            statement="The production database uses MySQL",
            build="same-build",model="model") -> Path:
    project=root/"project"
    docs=project/"docs"
    docs.mkdir(parents=True)
    (docs/"decision.md").write_text(corpus,encoding="utf-8")
    topics=project/"wiki"/"topics"
    topics.mkdir(parents=True)
    (topics/f"{topic}.md").write_text("# Example\n",encoding="utf-8")
    state=project/".lore"
    state.mkdir()
    conn=sqlite3.connect(state/"state.db")
    conn.executescript("""
        CREATE TABLE knowledge_current(knowledge_id TEXT,revision_id TEXT);
        CREATE TABLE knowledge_revisions(id TEXT,statement TEXT);
        CREATE TABLE model_calls(model TEXT,provider TEXT,cache_hit INTEGER);
        INSERT INTO knowledge_current VALUES('unit','revision');
    """)
    conn.execute("INSERT INTO knowledge_revisions VALUES('revision',?)",(statement,))
    conn.execute("INSERT INTO model_calls VALUES(?,'OpenAi',0)",(model,))
    conn.commit();conn.close()
    (root/"metrics.json").write_text(json.dumps({
        "provider":"openai","model":model,
        "decision_provider":None,"decision_model":None,
        "lore_binary_sha256":build
    }),encoding="utf-8")
    return root


class RepeatabilityTests(unittest.TestCase):
    def test_same_inputs_and_model_allow_lexical_comparison(self):
        with tempfile.TemporaryDirectory() as tmp:
            a=fixture(Path(tmp)/"a")
            b=fixture(Path(tmp)/"b")
            result=compare_runs.summarize([a,b])
            pair=result["comparisons"][0]
            self.assertTrue(pair["strictly_comparable"])
            self.assertEqual(pair["topic_jaccard"],1.0)
            self.assertEqual(pair["statement_lexical_jaccard"],1.0)

    def test_different_model_or_source_is_not_repeatability_evidence(self):
        with tempfile.TemporaryDirectory() as tmp:
            a=fixture(Path(tmp)/"a")
            b=fixture(Path(tmp)/"b",corpus="PostgreSQL",model="other-model")
            pair=compare_runs.summarize([a,b])["comparisons"][0]
            self.assertFalse(pair["strictly_comparable"])
            self.assertFalse(pair["same_source"])
            self.assertNotIn("topic_jaccard",pair)

    def test_unknown_build_prevents_claiming_strict_comparability(self):
        with tempfile.TemporaryDirectory() as tmp:
            a=fixture(Path(tmp)/"a")
            b=fixture(Path(tmp)/"b")
            metrics=json.loads((b/"metrics.json").read_text("utf-8"))
            metrics.pop("lore_binary_sha256")
            (b/"metrics.json").write_text(json.dumps(metrics))
            pair=compare_runs.summarize([a,b])["comparisons"][0]
            self.assertFalse(pair["strictly_comparable"])
            self.assertEqual(pair["statement_lexical_jaccard"],1.0)

    def test_atlas_gold_reaffirmation_is_distinct(self):
        gold=benchmark.load_json(benchmark.ROOT/"gold"/"atlas.json")
        rule=next(x for x in gold["expected_relations"]
                  if x["id"]=="mysql-reaffirmation-evidence")
        self.assertEqual(rule["type"],"reaffirms")
        self.assertTrue(rule["expected"])
        self.assertEqual(benchmark.corpus_fingerprint(
            benchmark.ROOT/"corpora"/"atlas")["file_count"],9)


if __name__=="__main__":
    unittest.main()
