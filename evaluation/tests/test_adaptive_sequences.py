"""Offline source/revision/grant protocol tests, explicitly not live model evidence."""
import argparse
import copy
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
import adaptive_sequences as sequence
import coding_tasks as coding
import cross_source as cross
from test_coding_tasks import registry_fixture
from test_adaptive_tasks import response_fixture


class DifferentTaskSequenceTests(unittest.TestCase):
    def fixture(self, root):
        binary=root/'lore-double';binary.write_text('Offline protocol double; no model inference')
        args=argparse.Namespace(output=root/'run',cases=sequence.DEFAULT,lore_binary=binary,
            provider='ollama',model='offline-not-a-model',embedding_model=None,decision_provider=None,decision_model=None,
            generative_base_url=None,decision_base_url=None,allow_hosted=False,allow_inspection=True,
            allow_checkout_egress=False,timeout=20,max_tokens=6000,seed=7)
        calls=[]
        def cli(binary,project,*argv,timeout,env=None):
            calls.append((argv,env.get('LORE_INSPECTION_ROOT')))
            if argv[0] in ('init','update'):return {'model_calls':0},0.01
            if argv[0]=='evidence':return {'evidence_id':argv[1],'excerpt':'Documentary fixture'},0.01
            return response_fixture(project,argv[1],argv,env),0.01
        with patch.object(coding,'registry_content',return_value=registry_fixture()),patch.object(coding.bench,'subprocess_json',side_effect=cli):
            result=sequence.run(args)
        return args.output,result,calls

    def test_preparation_has_distinct_tasks_and_exact_same_size_revision(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary).resolve()/'prepared'
            report=sequence.prepare(root)
            self.assertEqual(report['inference_calls'],0)
            case=report['sequences'][0]['sequence']
            self.assertNotEqual(case['task_a'],case['task_b'])
            self.assertEqual(len(case['related_revision']['before']),len(case['related_revision']['after']))
            self.assertNotEqual(case['related_revision']['path'],case['unrelated_revision']['path'])

    def test_matched_different_task_revision_and_narrow_grant_probe(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory,result,calls=self.fixture(Path(temporary).resolve())
            self.assertTrue(result['mechanical_contracts_passed'],result['issues'])
            self.assertEqual(len(result['records']),10)
            self.assertEqual(result['actual_coding_outcomes'],'unmeasured')
            self.assertEqual(len(result['arm_totals']),2)
            for total in result['arm_totals']:
                self.assertGreaterEqual(total['total_seconds'],0.08)
                self.assertIsNone(total['total_usage']['billed_cost_usd'])
            contexts=[(argv,grant) for argv,grant in calls if argv[0]=='context']
            self.assertEqual(sum(grant is None for _,grant in contexts),2)
            self.assertEqual(sum('--no-cache' in argv for argv,_ in contexts),5)
            self.assertEqual(sum(argv[0]=='update' for argv,_ in calls),4)
            report=cross.read_json(directory/'metrics.json')
            phases=report['sequences'][0]['arms']['adaptive_reuse']['phases']
            prior=phases[1]['context']['response']['intelligence']['inspection']['observations'][0]['content_hash']
            changed=phases[2]['context']['response']['intelligence']['inspection']['observations'][0]['content_hash']
            self.assertNotEqual(prior,changed)

    def test_stale_inspection_or_narrower_grant_replay_fails_even_with_rehashed_response(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory,_,_=self.fixture(Path(temporary).resolve())
            original=cross.read_json(directory/'metrics.json')
            for kind in ('stale','grant','revision','host_grants','refresh_cost'):
                report=copy.deepcopy(original)
                phases=report['sequences'][0]['arms']['adaptive_reuse']['phases']
                if kind=='stale':
                    phases[2]['context']['response']['intelligence']['inspection']=phases[1]['context']['response']['intelligence']['inspection']
                    phases[2]['context']['response_sha256']=cross.digest(phases[2]['context']['response'])
                elif kind=='grant':
                    phases[4]['context']['response']['capabilities']['inspection']='granted'
                    phases[4]['context']['response_sha256']=cross.digest(phases[4]['context']['response'])
                elif kind=='revision':phases[2]['update']['change']['after']='MAX_RETRIES = 9'
                elif kind=='host_grants':phases[4]['context']['host_grants']['inspection_root']=str(directory)
                else:phases[2]['update']['usage']['model_calls']=100
                cross.write_json(directory/'metrics.json',report)
                with self.subTest(kind=kind),patch.object(coding,'registry_content',return_value=registry_fixture()):
                    self.assertFalse(sequence.assess(directory)['mechanical_contracts_passed'])


if __name__=='__main__':unittest.main()
