"""Offline fixtures validate assessment bookkeeping, not model quality."""
import json
from pathlib import Path
import sqlite3
import sys
import tempfile
import unittest
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
import benchmark as bench
import suite

def completed(root:Path,target='atlas')->Path:
 root.mkdir(parents=True);project=root/'project';(project/'docs').mkdir(parents=True)
 (project/'docs'/'example.md').write_text('Original evidence.\n')
 (project/'wiki').mkdir();(project/'wiki'/'index.md').write_text('Grounded fixture overview.\n')
 (project/'.lore').mkdir();db=sqlite3.connect(project/'.lore'/'state.db')
 db.executescript("CREATE TABLE evidence_snapshots(id TEXT); INSERT INTO evidence_snapshots VALUES('ev-fixture');");db.commit();db.close()
 score={'sqlite_integrity_ok':True,'current_excerpt_failures':[],'current_excerpt_checks':1,'gold':{'relation_tests_total':2,'relation_tests_passed':2}}
 after={**score,'gold':{'relation_tests_total':3,'relation_tests_passed':3}}
 (root/'metrics.json').write_text(json.dumps({'schema_version':1,'target':target,'lore_binary_sha256':'fixture-only','synthesis_verification':True,'provider':'fixture','model':'not-real','no_op':{'no_op':True,'zero_generations':True,'pages_unchanged':True},'phases':{'initial':{'score':score},'after_mutation':{'score':after}}}))
 return root

def reviewed(run:Path):
 path=suite.init_review(run);data=json.loads(path.read_text())
 data.update({'reviewer':'test-fixture-not-a-real-review','reviewed_at':'2026-10-09','complete':True,'critical_errors':[]})
 for value in data['criteria'].values():value.update({'score':2,'notes':'Offline test fixture only.','evidence_ids':['ev-fixture']})
 path.write_text(json.dumps(data))

class SuiteTests(unittest.TestCase):
 def test_mixed_foundry_and_ollama_explicit_endpoints(self):
  c=bench.config_for(Path('.'),'atlas','openai','test','ollama','clef',True,None,True,generative_base_url='https://example.test/openai/v1',decision_base_url='http://127.0.0.1:11434')
  self.assertEqual(c['providers']['openai']['base_url'],'https://example.test/openai/v1');self.assertEqual(c['providers']['ollama']['base_url'],'http://127.0.0.1:11434')
  with self.assertRaises(ValueError):bench.config_for(Path('.'),'atlas','openai','m','ollama','d',False,None,True,generative_base_url='https://example.test')
  with self.assertRaises(ValueError):bench.config_for(Path('.'),'atlas','openai','m','openai','d',True,None,True,generative_base_url='https://one.test',decision_base_url='https://two.test')
  with self.assertRaises(ValueError):bench.config_for(Path('.'),'atlas','openai','m',None,None,True,None,True,decision_base_url='https://two.test')
 def test_unsigned_template_cannot_pass(self):
  with tempfile.TemporaryDirectory() as temp:
   run=completed(Path(temp)/'a');suite.init_review(run);result=suite.assess_run(run)
   self.assertTrue(result['automated_pass']);self.assertFalse(result['human_complete']);self.assertFalse(result['ready'])
   with self.assertRaises(FileExistsError):suite.init_review(run)
 def test_review_bound_to_pages_report_and_evidence(self):
  with tempfile.TemporaryDirectory() as temp:
   run=completed(Path(temp)/'a');reviewed(run);self.assertTrue(suite.assess_run(run)['ready'])
   (run/'project'/'wiki'/'index.md').write_text('Changed page.\n');self.assertFalse(suite.assess_run(run)['ready'])
 def test_critical_errors_low_scores_and_invented_evidence_fail(self):
  with tempfile.TemporaryDirectory() as temp:
   run=completed(Path(temp)/'a');reviewed(run);path=run/'HUMAN_REVIEW.json';human=json.loads(path.read_text())
   human['critical_errors']=['Wrong decision history'];path.write_text(json.dumps(human));self.assertFalse(suite.assess_run(run)['ready'])
   human['critical_errors']=[];human['criteria']['coverage']['score']=1;path.write_text(json.dumps(human));self.assertFalse(suite.assess_run(run)['ready'])
   human['criteria']['coverage']['score']=2;human['criteria']['coverage']['evidence_ids']=['invented'];path.write_text(json.dumps(human));self.assertFalse(suite.assess_run(run)['human_complete'])
 def test_gate_requires_all_three_projects(self):
  with tempfile.TemporaryDirectory() as temp:
   runs=[completed(Path(temp)/target,target) for target in suite.TARGETS]
   for run in runs:reviewed(run)
   self.assertFalse(suite.assess(runs[:1])['candidate_validated']);self.assertTrue(suite.assess(runs)['candidate_validated'])
   report=json.loads((runs[0]/'metrics.json').read_text());report['no_op']['zero_generations']=False;(runs[0]/'metrics.json').write_text(json.dumps(report));self.assertFalse(suite.assess(runs)['candidate_validated'])
 def test_rubric_preserves_prior_labels_and_prohibits_plan_for_design(self):
  current=bench.load_json(bench.ROOT/'gold'/'atlas.json');prior=bench.load_json(bench.ROOT/'gold'/'archive'/'atlas-v1.json')
  design=next(x for x in current['expected_assertions'] if x['id']=='mysql-architecture');original=next(x for x in prior['expected_assertions'] if x['id']=='mysql-architecture')
  self.assertEqual(design['kind'],'design');self.assertEqual(original['kind'],'observation');self.assertNotIn('plan',[x[0] for x in design['acceptable_pairs']])
  gate=next(x for x in current['expected_assertions'] if x['id']=='release-gate');self.assertIn(['procedure','active'],gate['acceptable_pairs']);self.assertTrue(gate['label_rationale'])
 def test_missing_model_identity_and_disabled_verification_cannot_pass(self):
  with tempfile.TemporaryDirectory() as temp:
   run=completed(Path(temp)/'a');reviewed(run)
   path=run/'metrics.json';report=json.loads(path.read_text())
   report.pop('model');self.assertFalse(suite.automated_checks(report)[0])
   report['model']='fixture';report['synthesis_verification']=False
   self.assertFalse(suite.automated_checks(report)[0])
   report['synthesis_verification']=True;report['lore_binary_sha256']=None
   self.assertFalse(suite.automated_checks(report)[0])

if __name__=='__main__':unittest.main()
