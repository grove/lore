"""The matched baseline fixture retains exact bytes and rejects changed inputs."""
from contextlib import closing
import hashlib
import json
from pathlib import Path
import shutil
import sqlite3
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import restore_zoom_fixture as fixture


class RetainedFixtureTests(unittest.TestCase):
    def test_exact_registry_is_restored_without_running_a_model(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "restored"
            result = fixture.restore(output)
            data = (output / ".lore/state.db").read_bytes()
            self.assertEqual(hashlib.sha256(data).hexdigest(), result["registry_sha256"])
            self.assertEqual(result["provider_calls"], 0)
            # SQLite's transaction context manager does not close its file handle.
            with closing(sqlite3.connect(output / ".lore/state.db")) as connection:
                self.assertEqual(connection.execute("PRAGMA integrity_check").fetchone()[0], "ok")
                self.assertEqual(connection.execute("SELECT count(*) FROM knowledge_current").fetchone()[0], 12)
            self.assertEqual(len(json.loads((output / "cases.json").read_text())), 8)

    def test_source_mutation_rejected_before_creating_an_output(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            corpus = root / "corpus"
            shutil.copytree(fixture.CORPUS, corpus)
            (corpus / "docs/emergency.md").write_text("An exception has been removed.\n")
            with self.assertRaises(ValueError):
                fixture.restore(root / "output", corpus)
            self.assertFalse((root / "output").exists())

    def test_existing_output_and_oversized_registry_are_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with self.assertRaises(ValueError):
                fixture.restore(root)
            corpus = root / "corpus"
            shutil.copytree(fixture.CORPUS, corpus)
            manifest = json.loads((corpus / "manifest.json").read_text())
            manifest["registry_bytes"] = fixture.MAX_DATABASE_BYTES + 1
            (corpus / "manifest.json").write_text(json.dumps(manifest))
            with self.assertRaises(ValueError):
                fixture.restore(root / "output", corpus)


if __name__ == "__main__":
    unittest.main()
