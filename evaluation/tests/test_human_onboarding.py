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
    def prepared_fixture(self, root, *, start=True):
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
        if start:
            human.run(args)
        return directory,study,args,consent

    def withdrawal(self, root, directory, consent):
        path = root / 'fixture-withdrawal.json'
        cross.write_json(path, {'participant_id': consent['participant_id'],
            'study_sha256': consent['study_sha256'], 'consent_sha256': cross.digest(consent),
            'withdrawn_at': datetime.now(timezone.utc).isoformat(), 'requested_by_participant': True})
        return argparse.Namespace(study=directory, request=path)

    def onboard_fixture_args(self, study, args, consent):
        consent['slot'] = next(slot['slot'] for slot in study['slots'] if slot['rounds'][0]['arm'] == 'with_onboard')
        cross.write_json(args.consent, consent)
        args.lore_binary = Path(sys.executable)
        args.provider, args.model, args.model_revision = 'ollama', 'offline-fixture', 'offline-fixture-revision'
        for key in ('embedding_model', 'decision_provider', 'decision_model', 'generative_base_url', 'decision_base_url'):
            setattr(args, key, None)

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

    def test_independent_transfer_requires_bound_explanation_review_and_no_assistance(self):
        for assisted in (False, True):
            with self.subTest(assisted=assisted), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve()
                directory, study, args, consent = self.prepared_fixture(root)
                participant = consent['participant_id']
                # A stopped first task is still a bound observed stage, never a fabricated pass.
                human.record(self.submission(root, directory, participant, 'contribution'))
                args.stage = 'transfer'
                human.run(args)
                transfer = human.stage_root(directory, participant, 1, 'transfer')
                (transfer / 'workspace/src/rollback_gate.py').write_text(
                    "def may_rollback(environment, has_valid_signature, is_fixture, authorization):\n"
                    "    return authorization == 'active' and environment in ('production','staging') and "
                    "(has_valid_signature or (environment == 'staging' and is_fixture))\n")
                submission = self.submission(root, directory, participant, 'transfer')
                if assisted:
                    ledger = cross.read_json(submission.record)
                    ledger['assistance'].append({'at': ledger['finished_at'], 'kind': 'hint', 'reference': 'offline-fixture-hint'})
                    cross.write_json(submission.record, ledger)
                self.assertTrue(human.record(submission)['independent_checks_passed'])

                def transfer_measurement():
                    report = human.assess(directory)
                    self.assertTrue(report['mechanical_contracts_passed'], report['issues'])
                    self.assertEqual(report['human_outcomes_status'], 'unmeasured')
                    return next(item for item in report['per_participant'][0]['measurements'] if item['stage'] == 'transfer')

                self.assertIsNone(transfer_measurement()['independent_transfer_success'])
                packet_path = transfer / 'reviews/01.json'
                packet = cross.read_json(packet_path)
                for index in (1, 2):
                    identity = f'offline-fixture-reviewer-{index}'
                    capture = transfer / f'reviews/fixture-capture-{index}.json'
                    cross.write_json(capture, {'fixture': True, 'reviewer': identity, 'target': packet['target_sha256']})
                    packet['independent_reviews'].append({'reviewer_id': identity, 'complete': True,
                        'independent': True, 'conflicts_declared': [], 'blind_confirmed': True,
                        'target_sha256': packet['target_sha256'], 'reviewed_at': datetime.now(timezone.utc).isoformat(),
                        'capture': {'path': capture.relative_to(directory).as_posix(), 'sha256': coding.hash_file(capture)}})
                    packet['scores'].append({'reviewer_id': identity, 'target_sha256': packet['target_sha256'],
                        'factual_explanation_0_to_3': 3, 'missed_exceptions': [], 'patch_correct': True})
                cross.write_json(packet_path, packet)
                self.assertEqual(transfer_measurement()['independent_transfer_success'], not assisted)
                for field, value in (('factual_explanation_0_to_3', 2),
                                     ('missed_exceptions', [packet['constraints'][0]]), ('patch_correct', False)):
                    invalid = copy.deepcopy(packet)
                    invalid['scores'][0][field] = value
                    cross.write_json(packet_path, invalid)
                    self.assertFalse(transfer_measurement()['independent_transfer_success'])

                # Evidence references use one canonical format on every host;
                # accepting native backslashes would weaken the path boundary.
                invalid = copy.deepcopy(packet)
                reference = invalid['independent_reviews'][0]['capture']
                reference['path'] = reference['path'].replace('/', '\\')
                cross.write_json(packet_path, invalid)
                report = human.assess(directory)
                self.assertFalse(report['mechanical_contracts_passed'])
                self.assertTrue(any('relative POSIX paths' in issue for issue in report['issues']))

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

    def test_readiness_is_read_only_and_zero_slots_never_become_human_outcomes(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            directory, study, args, consent = self.prepared_fixture(root, start=False)
            before = cross.fingerprint(directory)
            with patch.object(human.bench, 'subprocess_json', side_effect=AssertionError('No readiness inference')), \
                    patch.object(coding, 'execute_checks', side_effect=AssertionError('No readiness execution')):
                result = human.readiness(directory, args)
            self.assertTrue(result['session']['mechanically_ready'])
            self.assertEqual(result['session']['provider_availability'], 'not_checked')
            self.assertEqual(result['phase'], 'not_run')
            self.assertEqual(result['planned_slots'], 12)
            self.assertEqual(result['enrolled_participant_records'], 0)
            self.assertEqual(result['completed_participants'], 0)
            self.assertEqual(result['first_correct_contribution_time_status'], 'unmeasured')
            self.assertEqual(result['independent_learning_transfer_status'], 'unmeasured')
            self.assertIn('twelve_real_consented_participants', result['blocking_prerequisites'])
            self.assertIn('independent_unfamiliar_held_out_projects', result['blocking_prerequisites'])
            self.assertEqual(cross.fingerprint(directory), before)
            self.assertFalse((directory / 'participants').exists())
            args.stage = 'transfer'
            result = human.readiness(directory, args)
            self.assertFalse(result['session']['mechanically_ready'])
            self.assertNotIn('case_id', result['session'])
            self.assertEqual(cross.fingerprint(directory), before)

    def test_prepared_case_assignment_and_fixed_limits_cannot_be_rewritten(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            directory, study, args, consent = self.prepared_fixture(root, start=False)
            for change in ('task', 'case-duplicate', 'slot', 'limit', 'eligibility'):
                invalid = copy.deepcopy(study)
                if change == 'task':
                    invalid['cases'][0]['case']['task'] = 'A substituted contribution with hidden instructions'
                elif change == 'case-duplicate':
                    invalid['cases'][0] = copy.deepcopy(invalid['cases'][1])
                elif change == 'slot':
                    invalid['slots'][0]['rounds'][0]['arm'] = 'without_lore' if invalid['slots'][0]['rounds'][0]['arm'] == 'with_onboard' else 'with_onboard'
                elif change == 'limit':
                    invalid['max_attempts'] = 500
                else:
                    invalid['held_out'] = True
                cross.write_json(directory / 'study.json', invalid)
                with self.subTest(change=change), self.assertRaises(ValueError):
                    human.load_study(directory)
                self.assertFalse((directory / 'participants').exists())

    def test_declared_hidden_checker_cannot_be_copied_into_any_participant_source(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            declared = cross.read_json(human.DEFAULT_CASES)
            for case in declared['cases']:
                case['overlay'] = str((human.DEFAULT_CASES.parent / case['overlay']).resolve())
                case['test_command'] = [item.replace('{manifest}', str(human.DEFAULT_CASES.parent))
                                        for item in case['test_command']]
            contaminated = root / 'source-overlay'
            cross.copy_snapshot(Path(declared['cases'][0]['overlay']), contaminated)
            # Copy the checker from another case, so separation must cover all inputs.
            checker = Path(declared['cases'][-1]['test_command'][1])
            (contaminated / 'copied-hidden-checker.py').write_bytes(checker.read_bytes())
            declared['cases'][0]['overlay'] = str(contaminated)
            manifest = root / 'cases.json'
            cross.write_json(manifest, declared)
            with self.assertRaisesRegex(ValueError, 'hidden checker'):
                human.prepare(root / 'study', manifest)
            self.assertFalse((root / 'study/participants').exists())

            # Assessment/run must also recheck separation on an already prepared bundle.
            prepared = cross.read_json(root / 'study/prepared.json')
            with self.assertRaisesRegex(ValueError, 'hidden checker'):
                human.validate_checker_separation(root / 'study', prepared)

    def test_forged_complete_flag_never_exposes_transfer(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            directory, study, args, consent = self.prepared_fixture(root)
            first = human.stage_root(directory, consent['participant_id'], 1, 'contribution')
            cross.write_json(first / 'records/01.json', {'stage_complete': True})
            args.stage = 'transfer'
            with self.assertRaisesRegex(ValueError, 'complete bound submission'):
                human.run(args)
            self.assertFalse(human.stage_root(directory, consent['participant_id'], 1, 'transfer').exists())

    def test_withdrawal_erases_managed_data_excludes_outcomes_and_blocks_old_consent(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            directory, study, args, consent = self.prepared_fixture(root)
            participant = consent['participant_id']
            record_args = self.submission(root, directory, participant, 'contribution')
            human.record(record_args)
            person = human.participant_root(directory, participant)
            cross.write_json(person / 'private-capture.json', {'fixture': 'synthetic private assessment capture'})
            sources = cross.fingerprint(directory / 'snapshots')
            before = human.assess(directory)['participant_records_sha256']
            request = self.withdrawal(root, directory, consent)
            result = human.withdraw(request)
            self.assertTrue(result['consent_revoked'])
            self.assertTrue(result['participant_data_removed'])
            self.assertFalse(person.exists())
            self.assertTrue(human.withdraw(request)['already_revoked'])
            marker = cross.read_json(human.revocation_path(directory, participant))
            self.assertEqual(marker['consent_sha256'], cross.digest(consent))
            self.assertNotIn('explanation', marker)
            with patch.object(human.bench, 'subprocess_json', side_effect=AssertionError('No revoked inference')), \
                    patch.object(coding, 'execute_checks', side_effect=AssertionError('No revoked checking')):
                with self.assertRaisesRegex(ValueError, 'withdrawn'):
                    human.run(args)
                with self.assertRaisesRegex(ValueError, 'withdrawn'):
                    human.record(record_args)
            report = human.assess(directory)
            self.assertEqual(report['enrolled_participant_records'], 0)
            self.assertEqual(report['withdrawn_participant_records'], 1)
            self.assertEqual(report['per_participant'], [])
            self.assertNotEqual(report['participant_records_sha256'], before)
            self.assertEqual(report['human_outcomes_status'], 'unmeasured')
            self.assertEqual(cross.fingerprint(directory / 'snapshots'), sources)

    def test_withdrawal_needs_exact_request_and_survives_source_integrity_failures(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            directory, study, args, consent = self.prepared_fixture(root)
            request = self.withdrawal(root, directory, consent)
            original = cross.read_json(request.request)
            for key, value in [('requested_by_participant', False), ('consent_sha256', '0' * 64),
                               ('study_sha256', '0' * 64), ('withdrawn_at', '2000-01-01T00:00:00Z')]:
                cross.write_json(request.request, dict(original, **{key: value}))
                with self.subTest(key=key), self.assertRaises(ValueError):
                    human.withdraw(request)
                self.assertFalse(human.revocation_path(directory, consent['participant_id']).exists())
            cross.write_json(request.request, original)
            snapshot = directory / study['cases'][0]['source_root']
            (snapshot / 'corrupted.txt').write_text('Source-integrity failure fixture')
            with self.assertRaises(ValueError):
                human.load_study(directory)
            self.assertTrue(human.withdraw(request)['participant_data_removed'])

    def test_foreign_revocation_marker_cannot_enter_study_assessment(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            directory, study, args, consent = self.prepared_fixture(root)
            human.withdraw(self.withdrawal(root, directory, consent))
            marker = human.revocation_path(directory, consent['participant_id'])
            value = cross.read_json(marker)
            value['study_sha256'] = '0' * 64
            cross.write_json(marker, value)
            for check in (human.assess, human.readiness):
                with self.assertRaisesRegex(ValueError, 'different prepared study'):
                    check(directory)
            # Even an invalid marker never grants permission to replay consent.
            with self.assertRaisesRegex(ValueError, 'withdrawn'):
                human.run(args)

    def test_withdrawal_cleanup_failure_still_revokes_and_retry_finishes_erasure(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            directory, study, args, consent = self.prepared_fixture(root)
            request = self.withdrawal(root, directory, consent)
            with patch.object(human.shutil, 'rmtree', side_effect=OSError('fixture cleanup failure')):
                result = human.withdraw(request)
            self.assertTrue(result['consent_revoked'])
            self.assertFalse(result['participant_data_removed'])
            report = human.assess(directory)
            self.assertEqual(report['per_participant'], [])
            self.assertEqual(report['withdrawn_participant_records'], 1)
            self.assertFalse(report['mechanical_contracts_passed'])
            self.assertTrue(human.withdraw(request)['participant_data_removed'])

    def test_withdrawal_during_checker_removes_late_files_and_publishes_no_result(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            directory, study, args, consent = self.prepared_fixture(root)
            participant = consent['participant_id']
            request = self.withdrawal(root, directory, consent)
            record_args = self.submission(root, directory, participant, 'contribution')
            person = human.participant_root(directory, participant)
            def late_checker(*_args, **_kwargs):
                human.withdraw(request)
                cross.write_json(person / 'late-child-output.json', {'fixture': 'late result after revocation'})
                return {'checks': [{'id': 'fixture', 'kind': 'correctness', 'passed': True}]}, 0.0
            with patch.object(coding, 'execute_checks', side_effect=late_checker), self.assertRaisesRegex(ValueError, 'withdrawn'):
                human.record(record_args)
            self.assertFalse(person.exists())
            self.assertTrue(human.revocation_path(directory, participant).is_file())
            self.assertEqual(human.assess(directory)['per_participant'], [])

    def test_withdrawal_during_initialization_cannot_expose_task_or_late_artifacts(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            directory, study, args, consent = self.prepared_fixture(root, start=False)
            self.onboard_fixture_args(study, args, consent)
            request = self.withdrawal(root, directory, consent)
            person = human.participant_root(directory, consent['participant_id'])
            def late_initialization(*_args, **_kwargs):
                human.withdraw(request)
                cross.write_json(person / 'late-provider-output.json', {'fixture': 'not a real provider result'})
                return {}, 0.0
            with patch.object(human.bench, 'subprocess_json', side_effect=late_initialization), \
                    patch.object(shared, 'collect', side_effect=AssertionError('No orientation after withdrawal')), \
                    self.assertRaisesRegex(ValueError, 'withdrawn'):
                human.run(args)
            self.assertFalse(person.exists())

    def test_interrupted_check_cannot_change_or_score_the_authored_submission(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            directory, study, args, consent = self.prepared_fixture(root)
            participant = consent['participant_id']
            phase = human.stage_root(directory, participant, 1, 'contribution')
            original = cross.fingerprint(phase / 'workspace')
            record_args = self.submission(root, directory, participant, 'contribution')
            def interrupted_checker(case, submitted, *_args, **_kwargs):
                (submitted / case['editable_files'][0]).write_text('A checker must not author the human patch')
                raise KeyboardInterrupt()
            with patch.object(coding, 'execute_checks', side_effect=interrupted_checker), self.assertRaises(KeyboardInterrupt):
                human.record(record_args)
            self.assertEqual(cross.fingerprint(phase / 'workspace'), original)
            self.assertEqual(human.records(phase), [])
            self.assertFalse((phase / 'submissions/1').exists())
            self.assertFalse((directory / 'operations' / f"{consent['slot']}.lock").exists())
            self.assertEqual(human.record(record_args)['attempt'], 1)

    def test_submitted_patch_digest_and_tested_snapshot_are_independently_bound(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            directory, study, args, consent = self.prepared_fixture(root)
            participant = consent['participant_id']
            phase = human.stage_root(directory, participant, 1, 'contribution')
            result = human.record(self.submission(root, directory, participant, 'contribution'))
            record_path = phase / 'records/01.json'
            item = cross.read_json(record_path)
            self.assertEqual(result['submitted_patch_sha256'], item['submitted_patch_sha256'])
            self.assertIn(str(phase / 'submissions/1'), item['tests']['provenance']['argv'])
            self.assertNotIn(str(phase / 'workspace'), item['tests']['provenance']['argv'])
            item['submitted_patch_sha256'] = '0' * 64
            cross.write_json(record_path, item)
            report = human.assess(directory)
            self.assertFalse(report['mechanical_contracts_passed'])
            self.assertTrue(any('Submitted patch digest' in issue for issue in report['issues']))


if __name__=='__main__':unittest.main()
