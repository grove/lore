"""Offline human protocol tests. Fixture consent records are not real participants."""
import argparse
import copy
from datetime import datetime, timedelta, timezone
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
import human_onboarding as human
import coding_tasks as coding
import cross_source as cross
import shared_intelligence as shared


class HumanOnboardingTests(unittest.TestCase):
    def prepared_fixture(self, root):
        directory=root/'study'
        human.prepare(directory)
        study=cross.read_json(directory/'study.json')
        slot=next(item for item in study['slots'] if item['rounds'][0]['arm']=='without_lore' and item['rounds'][0]['project']=='releases')
        consent={'participant_id':'p_0123456789abcdef','participant_kind':'human','slot':slot['slot'],
            'study_sha256':cross.digest(study),'consented_at':(datetime.now(timezone.utc)-timedelta(minutes=1)).isoformat(),
            'informed_consent':True,'right_to_withdraw':True,'human_confirmed':True,
            'unfamiliar_projects':['payments','releases']}
        consent_path=root/'fixture-consent.json';cross.write_json(consent_path,consent)
        args=argparse.Namespace(study=directory,consent=consent_path,round=1,stage='contribution',lore_binary=None,
            provider=None,model=None,allow_inspection=False,allow_hosted=False,allow_checkout_egress=False,
            timeout=20,max_tokens=6000)
        human.run(args)
        return directory,study,args,consent

    def submission(self, root, directory, participant, stage, *, complete=True):
        session_root=human.stage_root(directory,participant,1,stage)
        session=cross.read_json(session_root/'session.json')
        finish=datetime.now(timezone.utc)
        start=human.outcome.timestamp(session['started_at'])
        value={'finished_at':finish.isoformat(),'active_seconds':(finish-start).total_seconds(),
            'pauses':[],'assistance':[],'explanation':'Offline fixture explanation: scope exceptions do not override a withdrawn authorization.',
            'perceived_clarity_1_to_5':None,'stage_complete':complete,'human_authored':True}
        path=root/f'{stage}-record.json';cross.write_json(path,value)
        return argparse.Namespace(study=directory,participant=participant,round=1,stage=stage,record=path,execute_checks=True,timeout=20)

    def test_preparation_counterbalances_slots_without_enrolling_or_calling_models(self):
        with tempfile.TemporaryDirectory() as temporary, patch.object(human.bench,'subprocess_json',side_effect=AssertionError('No inference during prepare')):
            root=Path(temporary).resolve();result=human.prepare(root/'study')
            self.assertEqual(result['enrolled_participants'],0);self.assertEqual(result['inference_calls'],0)
            study=cross.read_json(root/'study/study.json')
            patterns={}
            for slot in study['slots']:
                a,b=slot['rounds'];self.assertNotEqual(a['arm'],b['arm']);self.assertNotEqual(a['project'],b['project'])
                patterns[(a['arm'],a['project'])]=patterns.get((a['arm'],a['project']),0)+1
            self.assertEqual(set(patterns.values()),{3})
            result=human.assess(root/'study')
            self.assertEqual(result['enrolled_participant_records'],0)
            self.assertIsNone(result['verified_human_participants'])
            self.assertEqual(result['human_outcomes_status'],'unmeasured')

    def test_consent_rejects_unapproved_participants_names_and_study_changes(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary).resolve();directory,study,args,consent=self.prepared_fixture(root)
            for field,value in [('informed_consent',False),('human_confirmed',False),('participant_id','alice@example.test'),('study_sha256','0'*64)]:
                invalid=dict(consent);invalid[field]=value
                with self.subTest(field=field),self.assertRaises(ValueError):human.consent_record(invalid,study)
            invalid=dict(consent,name='Personal Name')
            with self.assertRaises(ValueError):human.consent_record(invalid,study)
            args.stage='transfer'
            with self.assertRaises(ValueError):human.run(args)
            self.assertFalse(human.stage_root(directory,consent['participant_id'],1,'transfer').exists())

    def test_real_checkers_distinguish_first_contribution_and_different_transfer_exception(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary).resolve();directory,study,args,consent=self.prepared_fixture(root)
            participant=consent['participant_id']
            first=human.stage_root(directory,participant,1,'contribution')
            (first/'workspace/src/release_gate.py').write_text("def may_release(environment, has_valid_signature, is_fixture):\n    return environment in ('production','staging') and (has_valid_signature or (environment == 'staging' and is_fixture))\n")
            recorded=human.record(self.submission(root,directory,participant,'contribution'))
            self.assertTrue(recorded['independent_checks_passed'])
            args.stage='transfer';human.run(args)
            transfer=human.stage_root(directory,participant,1,'transfer')
            wrong="def may_rollback(environment, has_valid_signature, is_fixture, authorization):\n    return environment in ('production','staging') and (has_valid_signature or (environment == 'staging' and is_fixture))\n"
            (transfer/'workspace/src/rollback_gate.py').write_text(wrong)
            first_transfer=self.submission(root,directory,participant,'transfer',complete=False)
            result=human.record(first_transfer)
            self.assertFalse(result['independent_checks_passed']);self.assertFalse(result['stage_complete'])
            correct="def may_rollback(environment, has_valid_signature, is_fixture, authorization):\n    return authorization == 'active' and environment in ('production','staging') and (has_valid_signature or (environment == 'staging' and is_fixture))\n"
            (transfer/'workspace/src/rollback_gate.py').write_text(correct)
            result=human.record(self.submission(root,directory,participant,'transfer'))
            self.assertTrue(result['independent_checks_passed']);self.assertEqual(result['attempt'],2)
            result=human.assess(directory)
            self.assertTrue(result['mechanical_contracts_passed'],result['issues'])
            self.assertEqual(result['human_outcomes_status'],'unmeasured')
            self.assertFalse(result['independent_review_complete'])
            self.assertFalse((transfer/'workspace/.lore').exists())
            self.assertFalse(any('check_rollback' in name for name in cross.fingerprint(first/'workspace')['files_sha256']))
            record_path=transfer/'records/02.json';value=cross.read_json(record_path);value['first_correct_active_seconds']=0;cross.write_json(record_path,value)
            self.assertFalse(human.assess(directory)['mechanical_contracts_passed'])

    def test_empty_checker_success_and_anonymous_review_scores_are_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary).resolve();directory,study,args,consent=self.prepared_fixture(root)
            participant=consent['participant_id']
            human.record(self.submission(root,directory,participant,'contribution'))
            phase=human.stage_root(directory,participant,1,'contribution')
            path=phase/'records/01.json';item=cross.read_json(path)
            item['tests']['checks']=[];item['passed']=True;item['stage_complete']=True
            item['first_correct_active_seconds']=item['ledger']['active_seconds'];cross.write_json(path,item)
            packet=cross.read_json(phase/'reviews/01.json');packet['target_sha256']=cross.digest(item);cross.write_json(phase/'reviews/01.json',packet)
            self.assertFalse(human.assess(directory)['mechanical_contracts_passed'])
        review={'complete':True,'reviewers':['reviewer-a','reviewer-b']}
        scores=[{'factual_explanation_0_to_3':3,'missed_exceptions':[],'patch_correct':True} for _ in range(2)]
        with self.assertRaises(ValueError):human.reviewed_scores({'scores':scores},review,'a'*64,['condition'])
        for identity,score in zip(review['reviewers'],scores):score.update(reviewer_id=identity,target_sha256='a'*64)
        self.assertEqual(len(human.reviewed_scores({'scores':scores},review,'a'*64,['condition'])),2)

    def test_record_rejects_rebound_wrong_session_before_executing_checker(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary).resolve();directory,study,args,consent=self.prepared_fixture(root)
            record_args=self.submission(root,directory,consent['participant_id'],'contribution')
            path=human.stage_root(directory,consent['participant_id'],1,'contribution')/'session.json'
            session=cross.read_json(path);session['case_id']='payments-retry';cross.write_json(path,session)
            with patch.object(coding,'execute_checks',side_effect=AssertionError('Wrong-session checker must not run')),self.assertRaises(ValueError):
                human.record(record_args)

    def test_active_time_and_assistance_ledger_reject_false_time_and_overlap(self):
        session={'started_at':'2026-01-01T00:00:00Z'}
        value={'finished_at':'2026-01-01T00:00:10Z','active_seconds':8,'pauses':[{'start':'2026-01-01T00:00:03Z','end':'2026-01-01T00:00:05Z'}],
            'assistance':[{'at':'2026-01-01T00:00:06Z','kind':'hint','reference':'scope-hint-1'}],
            'explanation':'Retain the scope exception.','perceived_clarity_1_to_5':4,'stage_complete':True,'human_authored':True}
        human.validate_ledger(value,session,7200)
        for kind in ('active','pause','assistance','ai'):
            invalid=copy.deepcopy(value)
            if kind=='active':invalid['active_seconds']=1
            elif kind=='pause':invalid['pauses'].append(dict(invalid['pauses'][0]))
            elif kind=='assistance':invalid['assistance'][0]['at']='2025-01-01T00:00:00Z'
            else:invalid['human_authored']=False
            with self.subTest(kind=kind),self.assertRaises(ValueError):human.validate_ledger(invalid,session,7200)

    def test_baseline_stage_keeps_transfer_keys_and_lore_cache_out_of_participant_input(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary).resolve();directory,study,args,consent=self.prepared_fixture(root)
            first=human.stage_root(directory,consent['participant_id'],1,'contribution')
            names=cross.fingerprint(first/'workspace')['files_sha256']
            self.assertFalse(any('rollback' in name or 'check_' in name or '.lore' in name for name in names))
            self.assertNotIn('critical_constraints',cross.read_json(first/'TASK.json'))
            args.stage='transfer'
            with self.assertRaises(ValueError):human.run(args)


if __name__=='__main__':unittest.main()
