"""Actual subprocess regression tests for repair and independent outcome accounting."""
import argparse
import copy
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import adaptive_tasks as adaptive
import coding_tasks as coding
import cross_source as cross
import outcome_protocol as outcome
import real_coding_corpus as real
import shared_intelligence as shared


class RepairProtocolTests(unittest.TestCase):
    def repaired_fixture(self, root):
        snapshot = root / 'snapshot'
        snapshot.mkdir()
        (snapshot / 'logic.py').write_text('def admit(scope):\n    return True\n')
        workspace = root / 'implementations/sample-test'
        cross.copy_snapshot(snapshot, workspace)
        checker = root / 'checker.py'
        checker.write_text("import json,sys\nfrom pathlib import Path\nnamespace={}\nexec(Path('logic.py').read_text(),namespace)\nprint(json.dumps({'schema_version':1,'answer_key':'NEVER_LEAK_ME','checks':[{'id':'secret_scope_vector','kind':'constraint','passed':namespace['admit']('unknown') is False},{'id':'nominal','kind':'correctness','passed':namespace['admit']('production') is True}]}))\n")
        case = {'id': 'scope', 'project':'fixture', 'task': 'Admit production scope only.',
                'editable_files':['logic.py'], 'critical_constraints':['Unknown scopes are denied.'],
                'test_command':[sys.executable,str(checker)],
                'repair_feedback':{'secret_scope_vector':'Preserve the documented scope condition.'}}
        program = """import json,sys
r=json.load(sys.stdin)
assert 'NEVER_LEAK_ME' not in json.dumps(r)
assert 'secret_scope_vector' not in json.dumps(r)
assert 'checks' not in r and 'test_command' not in r
repair='repair' in r
if repair:
    assert r['repair']['attempt']==2
    assert r['repair']['feedback']['messages']==['Preserve the documented scope condition.']
print(json.dumps({'schema_version':1,'summary':'Offline protocol test; no model inference', 'files':{'logic.py':'def admit(scope):\\n    return scope == \\"production\\"\\n' if repair else 'def admit(scope):\\n    return True\\n'},'usage':{'model_calls':0,'input_tokens':0,'output_tokens':0}}))
"""
        source = {'logic.py': (snapshot/'logic.py').read_text()}
        policy = coding.repair_policy(argparse.Namespace(timeout=10,max_attempts=4,attempt_budget_seconds=60))
        result = coding.run_attempts([sys.executable,'-c',program],case,source,None,workspace,root,'sample-test',root,10,policy,lambda *args:{})
        total = result['coding_seconds'] + result['verification_seconds'] + result['iteration_overhead_seconds']
        sample = {'sample_id':'sample-test','attempts':result['attempts'],'repair_policy':policy,
            'agent_response':result['response'],'tests':result['tests'],'attempt_stop_reason':result['stop_reason'],
            'costs':{'coding_seconds':result['coding_seconds'],'verification_seconds':result['verification_seconds'],'total_seconds':total},
            'time_to_first_correct_seconds':total}
        entry = {'case':case,'source_root':'snapshot','source_manifest':cross.fingerprint(snapshot)}
        return result,sample,entry

    def test_failed_attempt_repaired_with_allowlisted_feedback_and_stops_on_first_success(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary).resolve()
            result,sample,entry=self.repaired_fixture(root)
            self.assertEqual(len(result['attempts']),2)
            self.assertEqual(result['stop_reason'],'correct')
            self.assertFalse(result['attempts'][0]['tests']['checks'][0]['passed'])
            self.assertTrue(all(check['passed'] for check in result['tests']['checks']))
            self.assertIsNone(result['response']['usage']['billed_cost_usd'])
            self.assertGreater(result['coding_seconds'],0)
            coding.validate_attempts(root,sample,entry,None)
            self.assertFalse((root/'agent-runs/sample-test/3').exists())

    def test_attempt_feedback_or_cost_tampering_is_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary).resolve();_,sample,entry=self.repaired_fixture(root)
            for kind in ('cost','request','feedback','drop_attempt','implementation'):
                mutated=copy.deepcopy(sample)
                if kind=='cost':mutated['costs']['coding_seconds']=0
                elif kind=='request':mutated['attempts'][1]['request_sha256']='0'*64
                elif kind=='feedback':mutated['attempts'][0]['feedback']['messages']=['the hidden answer']
                elif kind=='drop_attempt':mutated['attempts']=mutated['attempts'][1:]
                else:mutated['attempts'][0]['implementation_manifest']['sha256']='0'*64
                with self.subTest(kind=kind),self.assertRaises(ValueError):coding.validate_attempts(root,mutated,entry,None)

    def test_failed_process_is_retained_as_incomplete_with_unknown_usage(self):
        for body, stage in (("raise SystemExit(3)", "agent"), ("print('{\"schema_version\":2}')", "validate_agent_response")):
            with self.subTest(stage=stage), tempfile.TemporaryDirectory() as temporary:
                root=Path(temporary).resolve();workspace=root/'workspace';workspace.mkdir()
                (workspace/'source.py').write_text('value = 1\n')
                case={'editable_files':['source.py'],'task':'Keep the value','test_command':[sys.executable]}
                policy=coding.repair_policy(argparse.Namespace(timeout=5,max_attempts=2,attempt_budget_seconds=20))
                with self.assertRaises(ValueError):
                    coding.run_attempts([sys.executable,'-c',body],case,{'source.py':'value = 1\n'},None,
                        workspace,root,'sample-failed',root,5,policy,lambda *args:{})
                record=cross.read_json(root/'attempt-invocations/sample-failed/1.json')
                self.assertEqual(record['status'],'incomplete');self.assertEqual(record['stage'],stage)
                self.assertGreater(record['elapsed_seconds'],0)
                self.assertIsNone(record['usage']['model_calls']);self.assertIsNone(record['usage']['billed_cost_usd'])
                self.assertFalse((root/'attempts').exists())

    def test_empty_duplicate_or_untyped_checker_results_never_pass(self):
        valid={'schema_version':1,'checks':[{'id':'condition','kind':'constraint','passed':True}]}
        self.assertEqual(coding.validate_checker_result(valid),[('condition','constraint')])
        for checks in ([],[valid['checks'][0],valid['checks'][0]],[{'id':'condition','kind':'constraint','passed':1}]):
            with self.assertRaises(ValueError):coding.validate_checker_result({'schema_version':1,'checks':checks})
        with self.assertRaises(ValueError):coding.validate_checker_result(valid,[('different_condition','constraint')])

    def test_feedback_is_bounded_predetermined_and_excludes_checker_diagnostics(self):
        case={'repair_feedback':{str(i):'message '+str(i) for i in range(20)}}
        checks={'secret':'answer','checks':[{'id':str(i),'passed':False,'stderr':'hidden key'} for i in range(20)]}
        response=coding.sanitized_feedback(case,checks)
        self.assertEqual(len(response['messages']),8)
        self.assertNotIn('hidden',json.dumps(response))
        for attempts in (0,6,True):
            with self.assertRaises(ValueError):coding.repair_policy(argparse.Namespace(max_attempts=attempts,timeout=10))

    def test_bootstrap_resamples_matched_tasks_and_preserves_unknown_cost(self):
        result=adaptive.paired_bootstrap([(0,0),(1,1),(1,1)],seed=17)
        self.assertEqual(result['ci95'],[0,0])
        self.assertTrue(result['small_sample'])
        self.assertEqual(result,adaptive.paired_bootstrap([(0,0),(1,1),(1,1)],seed=17))
        self.assertIsNone(adaptive.paired_bootstrap([],statistic='median_ratio')['estimate'])
        self.assertIsNone(adaptive.paired_bootstrap([(0,1)],statistic='median_ratio')['ci95'])

    def test_source_pins_and_bound_independent_review_captures(self):
        entry={'case':{'upstream':{'repository':'owner/repo','commit':'a'*40,'files_sha256':{'source.py':'b'*64}},'input_sha256':'c'*64},'source_manifest':{'sha256':'c'*64}}
        self.assertTrue(outcome.source_provenance(entry)['complete'])
        entry['case']['input_sha256']='d'*64
        self.assertFalse(outcome.source_provenance(entry)['complete'])
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary).resolve(); reviews=[]
            for index in (1,2):
                capture=root/f'review{index}.txt';capture.write_text('independent control '+str(index))
                reviews.append({'reviewer_id':f'reviewer-{index}','complete':True,'independent':True,'conflicts_declared':[],
                    'target_sha256':'a'*64,'blind_confirmed':True,'reviewed_at':'2026-10-10T00:00:00Z',
                    'capture':{'path':capture.name,'sha256':coding.hash_file(capture)}})
            self.assertTrue(outcome.independent_reviews(root,reviews,target_sha256='a'*64)['complete'])
            with self.assertRaises(ValueError):outcome.independent_reviews(root,reviews,excluded=['reviewer-1'],target_sha256='a'*64)
            reviews[1]['capture']=reviews[0]['capture']
            with self.assertRaises(ValueError):outcome.independent_reviews(root,reviews,target_sha256='a'*64)

    def test_answer_review_captures_bind_the_actual_annotation_reviewers(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary).resolve()
            sample={'sample_id':'sample-review','answer_sha256':'a'*64,'task':'task','constraints':['condition']}
            packet=adaptive.review_template(sample,'b'*64)
            annotations=[{'reviewer':'reader-1'},{'reviewer':'reader-2'}]
            self.assertFalse(adaptive.answer_review_status(root,packet,sample,'b'*64,annotations)['complete'])
            for index in (1,2):
                evidence=root/f'capture{index}.txt';evidence.write_text(f'Bound answer review {index}')
                packet['independence_evidence'].append({'reviewer_id':f'reader-{index}',
                    'complete':True,'independent':True,'conflicts_declared':[], 'blind_confirmed':True,
                    'target_sha256':packet['independence_target_sha256'],'reviewed_at':'2026-10-10T00:00:00Z',
                    'capture':{'path':evidence.name,'sha256':coding.hash_file(evidence)}})
            self.assertTrue(adaptive.answer_review_status(root,packet,sample,'b'*64,annotations)['complete'])
            with self.assertRaises(ValueError):
                adaptive.answer_review_status(root,packet,sample,'b'*64,annotations,excluded=['reader-1'])
            with self.assertRaises(ValueError):
                adaptive.answer_review_status(root,packet,sample,'b'*64,[{'reviewer':'other'},annotations[1]])

    def test_every_earlier_failed_implementation_requires_a_separate_review(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary).resolve();result,sample,entry=self.repaired_fixture(root)
            sample.update(answer_sha256='a'*64,task=entry['case']['task'],constraints=entry['case']['critical_constraints'])
            packet=adaptive.review_template(sample,'b'*64)
            self.assertEqual(len(packet['earlier_attempt_reviews']),1)
            status=adaptive.earlier_attempt_review_status(root,packet,sample,'b'*64)
            self.assertFalse(status['complete'])
            self.assertEqual(status['pending'],['sample-test-attempt-1'])
            packet['earlier_attempt_reviews']=[]
            self.assertFalse(adaptive.earlier_attempt_review_status(root,packet,sample,'b'*64)['complete'])
            packet=adaptive.review_template(sample,'b'*64)
            packet['earlier_attempt_reviews'][0]['answer_sha256']='c'*64
            with self.assertRaises(ValueError):adaptive.earlier_attempt_review_status(root,packet,sample,'b'*64)

    def test_shared_evidence_resolution_receives_the_same_private_environment(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary).resolve(); seen=[]
            environment={'LORE_INSPECTION_ROOT':str(root),'PRIVATE_TEST_CREDENTIAL':'offline-test-only'}
            def cli(binary, project, *argv, timeout, env=None):
                seen.append((argv[0],env))
                if argv[0]=='evidence':return {'evidence_id':argv[1]},0.001
                return {'schema_version':2,'task':'task','model_calls':0,
                    'evidence':[{'id':'ev_test'}]},0.001
            with patch.object(shared.bench,'subprocess_json',side_effect=cli), \
                 patch.object(coding,'registry_content',return_value={}), \
                 patch.object(shared,'context_checks',return_value={'fixture':True}):
                shared.collect('offline-fixture',root,root,'fast','task',5,6000,env=environment)
            self.assertEqual([name for name,_ in seen],['context','context','evidence'])
            self.assertTrue(all(env is environment for _,env in seen))

    def test_real_corpus_has_30_pinned_candidates_and_never_promotes_reviews(self):
        manifest=real.load_manifest()
        self.assertEqual(len(manifest['tasks']),30)
        self.assertEqual(len(manifest['projects']),3)
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary).resolve()/'candidates'
            result=real.prepare(root)
            self.assertEqual(result['inference_calls'],0)
            cases=coding.load_cases(root/'cases.json')
            self.assertTrue(cases['fixture_only']);self.assertFalse(cases['held_out'])
            for case in cases['cases']:
                source=root/case['source_root']
                self.assertFalse((source/'manifest.json').exists())
                self.assertFalse((source/'real_coding_checks.py').exists())
                self.assertIn('Study fault injection', (source/case['editable_files'][0]).read_text())

    def test_no_inference_preparation_pins_checker_bytes_before_registration(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary).resolve();source=root/'source';source.mkdir()
            (source/'source.py').write_text('value = 1\n')
            checker=root/'checker.py';checker.write_text('# original independent checker\n')
            case={'id':'pin-checker','project':'example','source_root':'source','task':'Keep value one.',
                'editable_files':['source.py'],'critical_constraints':['Keep value one.'],
                'test_command':['{python}','{manifest}/checker.py','{workspace}']}
            cases=root/'cases.json';cross.write_json(cases,{'schema_version':1,'fixture_only':True,'cases':[case]})
            with patch.object(coding,'invoke_json',side_effect=AssertionError('Preparation must not execute code')):
                prepared=coding.prepare(root/'prepared',cases)
            entry=prepared['cases'][0];snapshot=root/'prepared'/entry['source_root']
            coding.validate_prepared_checker(entry,snapshot,root)
            registration=outcome.preregistration_template(prepared,arms=list(adaptive.SETUPS),pilot=True)
            self.assertEqual(registration['cases'][0]['task_sha256'],outcome.task_contract_sha256(entry))
            self.assertIn(str(checker),registration['cases'][0]['checker_files_sha256'])
            checker.write_text('# easier checker substituted after preregistration\n')
            with self.assertRaises(ValueError):coding.validate_prepared_checker(entry,snapshot,root)

    def test_model_prompt_and_lore_configuration_pins_cannot_be_names_only(self):
        registration={'agent':{'model':'actual-fixture-model','model_revision':'fixture-revision','reasoning':'none',
            'prompt_sha256':'a'*64,'tools':[]},'lore':{'model':'fixture-lore','model_revision':'fixture-revision',
            'reasoning':'none','configuration_sha256_by_case':{'case':'b'*64}}}
        response={'provider_model':'actual-fixture-model'}
        report={'samples':[{'case_id':'case','context':{},'configuration_sha256':'b'*64,
                            'agent_response':response,'attempts':[{'response':response}]}]}
        outcome.validate_model_pins(registration,report)
        for field,value in (('model','other-model'),('prompt_sha256','not-a-digest'),('tools',['shell','shell'])):
            altered=copy.deepcopy(registration);altered['agent'][field]=value
            with self.subTest(field=field),self.assertRaises(ValueError):outcome.validate_model_pins(altered,report)
        altered=copy.deepcopy(registration);altered['lore']['configuration_sha256_by_case']['case']='c'*64
        with self.assertRaises(ValueError):outcome.validate_model_pins(altered,report)


if __name__ == '__main__':unittest.main()
