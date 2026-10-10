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
    def fixture(self, root, manifest=sequence.DEFAULT):
        binary=root/'lore-double';binary.write_text('Offline protocol double; no model inference')
        args=argparse.Namespace(output=root/'run',cases=manifest,lore_binary=binary,
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

    def test_accepted_adr_and_deep_revision_remain_matched_and_fully_charged(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory,result,calls=self.fixture(Path(temporary).resolve(),sequence.REVISION_DEFAULT)
            self.assertTrue(result['mechanical_contracts_passed'],result['issues'])
            self.assertEqual(len(result['records']),10)
            self.assertEqual(sum(argv[0]=='update' for argv,_ in calls),4)
            report=cross.read_json(directory/'metrics.json')
            arms=report['sequences'][0]['arms']
            for arm in sequence.ARMS:
                phases=arms[arm]['phases']
                self.assertEqual([item['phase'] for item in phases],
                    ['task_a','task_b','accepted_adr','deep_revision','narrower_grant'])
                prior=phases[1]['context']['source_manifest']['files_sha256']
                accepted=phases[2]['context']['source_manifest']['files_sha256']
                deep=phases[3]['context']['source_manifest']['files_sha256']
                self.assertNotIn('docs/ADR-018.md',prior)
                self.assertIn('docs/ADR-018.md',accepted)
                path='src/payments/gateways/providers/legacy/retry_policy.py'
                self.assertNotEqual(accepted[path],deep[path])
                self.assertEqual(phases[4]['context']['host_grants']['inspection_root'],None)
            totals={row['arm']:row['total_seconds'] for row in result['arm_totals']}
            self.assertEqual(result['net_revalidation_comparison'][0]['total_seconds_reuse_minus_no_reuse'],
                             totals['adaptive_reuse']-totals['adaptive_no_reuse'])
            self.assertEqual(result['actual_coding_outcomes'],'unmeasured')

    def test_new_adr_cannot_overwrite_old_policy_or_target_state_and_deep_edit_is_exact(self):
        data=sequence.load_cases(sequence.REVISION_DEFAULT)
        case=data['sequences'][0]
        old=case['accepted_adr']['previous_statement']
        files={'docs/ADR-017.md':old,
               case['deep_revision']['path']:case['deep_revision']['before']}
        newer=sequence.revise_text(case,'accepted_adr',files)
        self.assertEqual(newer['docs/ADR-017.md'],old)
        with self.assertRaises(ValueError):sequence.revise_text(case,'accepted_adr',newer)
        changed=sequence.revise_text(case,'deep_revision',newer)
        with self.assertRaises(ValueError):sequence.revise_text(case,'deep_revision',changed)
        altered=copy.deepcopy(case)
        altered['accepted_adr']['path']='.lore/forbidden.md'
        with self.assertRaises(ValueError):sequence.revise_text(altered,'accepted_adr',files)

    def test_dropping_accepted_adr_or_replaying_pre_mutation_source_fails_assessment(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory,_,_=self.fixture(Path(temporary).resolve(),sequence.REVISION_DEFAULT)
            original=cross.read_json(directory/'metrics.json')
            for kind in ('dropped_adr','old_deep','grant'):
                report=copy.deepcopy(original)
                phases=report['sequences'][0]['arms']['adaptive_reuse']['phases']
                if kind=='dropped_adr':
                    phases[2]['context']['source_manifest']['files_sha256'].pop('docs/ADR-018.md')
                elif kind=='old_deep':
                    phases[3]['context']['source_manifest']=phases[2]['context']['source_manifest']
                else:
                    phases[4]['policy']['allow_inspection']=True
                cross.write_json(directory/'metrics.json',report)
                with self.subTest(kind=kind),patch.object(coding,'registry_content',return_value=registry_fixture()):
                    self.assertFalse(sequence.assess(directory)['mechanical_contracts_passed'])


if __name__=='__main__':unittest.main()
