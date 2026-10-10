#!/usr/bin/env python3
"""Opt-in human contribution/transfer pilot. Preparation never fabricates participants."""
from __future__ import annotations
import argparse
import hashlib
import json
import math
from pathlib import Path
import random
import re
import shutil
import subprocess
import sys

import adaptive_tasks as adaptive
import benchmark as bench
import coding_tasks as coding
import cross_source as cross
import decision_tasks as decision
import outcome_protocol as outcome
import shared_intelligence as shared

ROOT = Path(__file__).resolve().parent
DEFAULT_CASES = ROOT / 'corpora/human-onboarding/cases.json'
PROTOCOL = 'human-onboarding-v1'
ARMS = ('without_lore', 'with_onboard')
STAGES = ('contribution', 'transfer')
MAX_RECORD_BYTES = 100_000


def read_bounded(path: Path) -> dict:
    cross.reject_symlink_path(path)
    if path.stat().st_size > MAX_RECORD_BYTES:
        raise ValueError('Participant record exceeds 100 KB')
    value = cross.read_json(path)
    if not isinstance(value, dict):
        raise ValueError('Expected a participant record object')
    return value


def prepare(output: Path, cases: Path = DEFAULT_CASES, participants=12, seed=7) -> dict:
    if type(participants) is not int or not 4 <= participants <= 100 or participants % 4:
        raise ValueError('Use 4..100 planned slots, divisible by four for counterbalancing')
    data = coding.load_cases(cases)
    journeys = data.get('learning_journeys', [])
    if len(journeys) != 2 or journeys[0]['project'] == journeys[1]['project']:
        raise ValueError('The counterbalanced pilot requires two journeys in distinct unfamiliar projects')
    known = {case['id']: case for case in data['cases']}
    for journey in journeys:
        first, transfer = journey['first_case'], journey['transfer_case']
        if (first == transfer or first not in known or transfer not in known
                or known[first]['project'] != journey['project'] or known[transfer]['project'] != journey['project']
                or not journey.get('required_explanation')):
            raise ValueError('Each journey needs distinct same-project contribution and transfer tasks')
    prepared = coding.prepare(output, cases)
    slots = []
    patterns = [(0, 0), (0, 1), (1, 0), (1, 1)] * (participants // 4)
    random.Random(seed).shuffle(patterns)
    for index, (first_project, first_arm) in enumerate(patterns, 1):
        rounds = []
        for number in range(2):
            journey = journeys[(first_project + number) % 2]
            rounds.append({'round': number + 1, 'arm': ARMS[(first_arm + number) % 2],
                'journey_id': journey['id'], 'project': journey['project'],
                'contribution': journey['first_case'], 'transfer': journey['transfer_case'],
                'required_explanation': journey['required_explanation']})
        slots.append({'slot': f'slot-{index:02}', 'rounds': rounds})
    study = {'schema_version': 1, 'protocol': PROTOCOL, 'phase': 'preparation_only',
        'created_at': bench.now_utc(), 'seed': seed, 'planned_slots': participants,
        'enrolled_participants': 0, 'inference_calls': 0, 'fixture_only': prepared['fixture_only'],
        'held_out': prepared['held_out'], 'cases_manifest': str(cases.resolve()),
        'cases_manifest_sha256': coding.hash_file(cases), 'cases': prepared['cases'], 'slots': slots,
        'max_attempts': 5, 'max_active_seconds': 7200,
        'transfer_assistance': 'No Lore/AI context automatically supplied; every requested hint or mentor intervention is logged.',
        'human_outcomes': 'unmeasured', 'independent_review': 'pending'}
    cross.write_json(output / 'study.json', study)
    cross.write_json(output / 'CONSENT_TEMPLATE.json', {'participant_id': '', 'participant_kind': 'human',
        'slot': '', 'study_sha256': cross.digest(study), 'consented_at': '',
        'informed_consent': False, 'right_to_withdraw': False, 'human_confirmed': False,
        'unfamiliar_projects': [journey['project'] for journey in journeys]})
    cross.write_json(output / 'HUMAN_AUDIT.json', {'schema_version': 1, 'study_sha256': cross.digest(study),
        'complete': False, 'auditor_id': '', 'consenting_real_participants_verified': False,
        'no_agent_impersonation': False, 'participant_records_sha256': None,
        'capture': {'path': '', 'sha256': ''}, 'reviewed_at': ''})
    return {'schema_version': 1, 'protocol': PROTOCOL, 'phase': 'preparation_only', 'planned_slots': participants,
        'enrolled_participants': 0, 'inference_calls': 0, 'human_outcomes': 'unmeasured',
        'fixture_only': prepared['fixture_only'], 'study_sha256': cross.digest(study)}


def load_study(directory: Path) -> dict:
    study = cross.read_json(directory / 'study.json')
    if study.get('schema_version') != 1 or study.get('protocol') != PROTOCOL:
        raise ValueError('Expected a prepared human onboarding study')
    if coding.hash_file(Path(study['cases_manifest'])) != study['cases_manifest_sha256']:
        raise ValueError('Human study task/checker manifest changed')
    for entry in study['cases']:
        if cross.fingerprint(directory / coding.relative_file(entry['source_root'])) != entry['source_manifest']:
            raise ValueError('A pinned participant source snapshot changed')
        if not entry.get('checker_files_sha256'):
            raise ValueError('Human study preparation must pin the external checker bytes')
        coding.validate_prepared_checker(entry, directory / entry['source_root'], Path(study['cases_manifest']).parent)
    return study


def consent_record(value: dict, study: dict) -> dict:
    expected = {'participant_id', 'participant_kind', 'slot', 'study_sha256', 'consented_at',
                'informed_consent', 'right_to_withdraw', 'human_confirmed', 'unfamiliar_projects'}
    if set(value) != expected:
        raise ValueError('Consent records contain only the documented pseudonymous fields')
    if not re.fullmatch(r'p_[0-9a-f]{16,32}', value['participant_id']):
        raise ValueError('Use a random p_ hexadecimal pseudonym; never a name or email')
    if value['participant_kind'] != 'human' or any(value[key] is not True for key in ('informed_consent', 'right_to_withdraw', 'human_confirmed')):
        raise ValueError('Explicit opt-in consent from an actual human participant is required')
    if value['study_sha256'] != cross.digest(study) or value['slot'] not in {slot['slot'] for slot in study['slots']}:
        raise ValueError('Consent must name this prepared study and an existing slot')
    if outcome.timestamp(value['consented_at']) > outcome.timestamp(bench.now_utc()):
        raise ValueError('Consent cannot be future-dated')
    if set(value['unfamiliar_projects']) != {item['case']['project'] for item in study['cases']}:
        raise ValueError('Participants must declare unfamiliarity with both assigned projects')
    return value


def round_for(study: dict, consent: dict, number: int) -> dict:
    slot = next(item for item in study['slots'] if item['slot'] == consent['slot'])
    if number not in (1, 2):
        raise ValueError('Round must be 1 or 2')
    return slot['rounds'][number - 1]


def stage_root(directory: Path, participant: str, number: int, stage: str) -> Path:
    if not re.fullmatch(r'p_[0-9a-f]{16,32}', participant) or number not in (1, 2) or stage not in STAGES:
        raise ValueError('Invalid participant, round or study stage')
    return directory / 'participants' / participant / f'round-{number}' / stage


def records(root: Path) -> list[dict]:
    return [cross.read_json(path) for path in sorted((root / 'records').glob('*.json'))]


def require_complete(root: Path) -> None:
    previous = records(root)
    if not previous or previous[-1]['stage_complete'] is not True:
        raise ValueError('The preceding contribution/transfer stage must finish before exposing the next task')


def validate_session(directory: Path, study: dict, consent: dict, session: dict,
                     number: int, stage: str) -> dict:
    assigned = round_for(study, consent, number)
    entry = next(item for item in study['cases'] if item['case']['id'] == assigned[stage])
    if (session.get('schema_version') != 1 or session.get('protocol') != PROTOCOL
            or session.get('study_sha256') != cross.digest(study) or session.get('consent_sha256') != cross.digest(consent)
            or session.get('participant_id') != consent['participant_id'] or session.get('arm') != assigned['arm']
            or session.get('round') != number or session.get('stage') != stage
            or session.get('case_id') != assigned[stage] or session.get('source_manifest') != entry['source_manifest']):
        raise ValueError('Participant session is not bound to its randomized condition and source')
    if not outcome.timestamp(consent['consented_at']) <= outcome.timestamp(session['started_at']) <= outcome.timestamp(bench.now_utc()):
        raise ValueError('A human session cannot precede consent or start in the future')
    orientation = session.get('orientation')
    if (orientation is not None) != (assigned['arm'] == 'with_onboard' and stage == 'contribution'):
        raise ValueError('Onboarding assistance does not match the assigned intervention')
    if orientation is not None and not all(shared.context_checks(orientation, session['orientation_experience'],
            entry['case']['task'], directory / entry['source_root']).values()):
        raise ValueError('Recorded onboarding source integrity failed')
    return entry


def reviewed_scores(packet: dict, review: dict, target_sha256: str, constraints: list[str]) -> list:
    scores = packet.get('scores', [])
    if not review['complete']:
        return []
    if not isinstance(scores, list) or len(scores) != len(review['reviewers']):
        raise ValueError('Each independent evaluator needs a separately bound factual score')
    identities = set()
    for score in scores:
        identity = score.get('reviewer_id', '').strip().casefold()
        if identity not in review['reviewers'] or identity in identities or score.get('target_sha256') != target_sha256:
            raise ValueError('Factual scores must name their reviewer and the exact reviewed submission')
        identities.add(identity)
        if (type(score.get('factual_explanation_0_to_3')) is not int or not 0 <= score['factual_explanation_0_to_3'] <= 3
                or not isinstance(score.get('missed_exceptions'), list)
                or len(score['missed_exceptions']) != len(set(score['missed_exceptions']))
                or any(item not in constraints for item in score['missed_exceptions'])
                or type(score.get('patch_correct')) is not bool):
            raise ValueError('Independent explanation and exception scores are incomplete')
    return scores


def run(args: argparse.Namespace) -> dict:
    directory = cross.selected_path(args.study)
    study = load_study(directory)
    consent = consent_record(read_bounded(args.consent), study)
    participant = consent['participant_id']
    assigned = round_for(study, consent, args.round)
    requires_onboard = assigned['arm'] == 'with_onboard' and args.stage == 'contribution'
    if requires_onboard and (args.lore_binary is None or not args.provider or not args.model):
        raise ValueError('The onboard arm requires an actual Lore binary and pinned provider/model')
    if args.allow_checkout_egress and not (args.allow_inspection and args.allow_hosted):
        raise ValueError('Checkout egress requires explicit hosted and inspection grants')
    if args.stage == 'transfer':
        require_complete(stage_root(directory, participant, args.round, 'contribution'))
    elif args.round == 2:
        require_complete(stage_root(directory, participant, 1, 'transfer'))
    root = stage_root(directory, participant, args.round, args.stage)
    if root.exists():
        raise ValueError('This participant stage already exists; record its next attempt instead')
    participant_root = directory / 'participants' / participant
    for path in (directory / 'participants').glob('*/consent.json'):
        other = cross.read_json(path)
        if other['slot'] == consent['slot'] and other['participant_id'] != participant:
            raise ValueError('A counterbalance slot cannot be assigned to two participants')
    if (participant_root / 'consent.json').exists() and cross.read_json(participant_root / 'consent.json') != consent:
        raise ValueError('Participant consent binding changed')
    cross.write_json(participant_root / 'consent.json', consent)
    entry = next(item for item in study['cases'] if item['case']['id'] == assigned[args.stage])
    snapshot = directory / entry['source_root']
    workspace = root / 'workspace'
    cross.copy_snapshot(snapshot, workspace)
    orientation, initialization = None, None
    if requires_onboard:
        binary = str(args.lore_binary.resolve(strict=True))
        project = root / 'orientation-project'
        cross.copy_snapshot(snapshot, project)
        config = coding.config_for_case(args, entry['case'], project)
        cross.write_json(project / 'lore.yml', config)
        env = shared.invocation_environment(project, allow_inspection=args.allow_inspection,
            allow_hosted=args.allow_hosted, allow_checkout_egress=args.allow_checkout_egress)
        init, seconds = bench.subprocess_json(binary, project, 'init', timeout=args.timeout, env=env)
        initialization = {'response': init, 'response_sha256': cross.digest(init), 'seconds': seconds,
                          'binary_sha256': coding.hash_file(Path(binary)), 'configuration_sha256': cross.digest(config)}
        experience = 'onboard' if args.allow_inspection else 'onboard_no_inspect'
        orientation = shared.collect(binary, project, snapshot, experience, entry['case']['task'], args.timeout, args.max_tokens, env=env)
        if not all(shared.context_checks(orientation, experience, entry['case']['task'], snapshot).values()):
            raise ValueError('Onboarding output failed source, capability or output integrity checks')
        cross.write_json(root / 'ONBOARD.json', orientation['response'])
    session = {'schema_version': 1, 'protocol': PROTOCOL, 'participant_id': participant,
        'study_sha256': cross.digest(study), 'consent_sha256': cross.digest(consent),
        'round': args.round, 'stage': args.stage, 'arm': assigned['arm'], 'case_id': entry['case']['id'],
        'started_at': bench.now_utc(), 'source_manifest': entry['source_manifest'],
        'orientation': orientation, 'initialization': initialization,
        'orientation_experience': ('onboard' if args.allow_inspection else 'onboard_no_inspect') if orientation else None}
    cross.write_json(root / 'session.json', session)
    cross.write_json(root / 'TASK.json', {'task': entry['case']['task'], 'editable_files': entry['case']['editable_files'],
        'instruction': 'Work on this task, then explain the relevant principle and exceptions. Requesting hints is optional.'})
    cross.write_json(root / 'RECORD_TEMPLATE.json', {'finished_at': '', 'active_seconds': None,
        'pauses': [], 'assistance': [], 'explanation': '', 'perceived_clarity_1_to_5': None,
        'stage_complete': False, 'human_authored': True})
    return {'participant_id': participant, 'round': args.round, 'stage': args.stage,
            'workspace': str(workspace), 'task': str(root / 'TASK.json'), 'session_sha256': cross.digest(session),
            'onboard_supplied': orientation is not None, 'human_outcome': 'pending'}


def validate_ledger(value: dict, session: dict, max_active_seconds: int) -> dict:
    expected = {'finished_at', 'active_seconds', 'pauses', 'assistance', 'explanation',
                'perceived_clarity_1_to_5', 'stage_complete', 'human_authored'}
    if set(value) != expected or value['human_authored'] is not True or type(value['stage_complete']) is not bool:
        raise ValueError('Record must use the documented opt-in human submission fields')
    start, finish = outcome.timestamp(session['started_at']), outcome.timestamp(value['finished_at'])
    elapsed = (finish - start).total_seconds()
    if not 0 <= elapsed <= 86400 or finish > outcome.timestamp(bench.now_utc()):
        raise ValueError('Invalid participant session interval')
    if (type(value['active_seconds']) not in (int, float) or not math.isfinite(value['active_seconds'])
            or not 0 <= value['active_seconds'] <= min(elapsed, max_active_seconds)):
        raise ValueError('Active time exceeds the observed interval or study limit')
    if not isinstance(value['pauses'], list) or len(value['pauses']) > 200:
        raise ValueError('Pause ledger exceeds its bound')
    pause_seconds, previous = 0.0, start
    for pause in value['pauses']:
        left, right = outcome.timestamp(pause['start']), outcome.timestamp(pause['end'])
        if not previous <= left <= right <= finish:
            raise ValueError('Pause intervals overlap or escape the session')
        pause_seconds += (right - left).total_seconds()
        previous = right
    if abs(elapsed - pause_seconds - value['active_seconds']) > 1:
        raise ValueError('Active time must equal session elapsed time minus documented pauses')
    if not isinstance(value['assistance'], list) or len(value['assistance']) > 200:
        raise ValueError('Assistance ledger exceeds its bound')
    for event in value['assistance']:
        if set(event) != {'at', 'kind', 'reference'} or event['kind'] not in ('hint', 'mentor', 'ai_assistance'):
            raise ValueError('Assistance needs a timestamp, kind and opaque hint/mentor reference')
        if not start <= outcome.timestamp(event['at']) <= finish or not isinstance(event['reference'], str) or not 1 <= len(event['reference']) <= 200:
            raise ValueError('Invalid assistance timestamp or reference')
    explanation = value['explanation']
    if not isinstance(explanation, str) or not explanation.strip() or len(explanation.encode('utf-8')) > 16000:
        raise ValueError('A bounded task explanation is required; do not include personal information')
    clarity = value['perceived_clarity_1_to_5']
    if clarity is not None and (type(clarity) is not int or not 1 <= clarity <= 5):
        raise ValueError('Optional perceived clarity must be 1..5')
    return value


def record(args: argparse.Namespace) -> dict:
    if not args.execute_checks:
        raise ValueError('Recording a patch requires --execute-checks in an intentionally isolated study environment')
    directory = cross.selected_path(args.study)
    study = load_study(directory)
    root = stage_root(directory, args.participant, args.round, args.stage)
    session = cross.read_json(root / 'session.json')
    consent = consent_record(cross.read_json(directory / 'participants' / args.participant / 'consent.json'), study)
    entry = validate_session(directory, study, consent, session, args.round, args.stage)
    value = validate_ledger(read_bounded(args.record), session, study['max_active_seconds'])
    previous = records(root)
    if len(previous) >= study['max_attempts'] or (previous and previous[-1]['stage_complete']):
        raise ValueError('The stage finished or exhausted its attempt allowance')
    if previous and (value['active_seconds'] < previous[-1]['ledger']['active_seconds']
                     or value['assistance'][:len(previous[-1]['ledger']['assistance'])] != previous[-1]['ledger']['assistance']):
        raise ValueError('Cumulative active time and assistance cannot be removed during repairs')
    workspace = root / 'workspace'
    before = cross.fingerprint(workspace)
    allowed = set(entry['case']['editable_files'])
    if (set(before['files_sha256']) != set(entry['source_manifest']['files_sha256'])
            or any(before['files_sha256'].get(name) != digest for name, digest in entry['source_manifest']['files_sha256'].items() if name not in allowed)):
        raise ValueError('Participant changed files outside the allowed contribution')
    checks, seconds = coding.execute_checks(entry['case'], workspace, Path(study['cases_manifest']).parent, args.timeout,
        env=shared.invocation_environment(workspace, allow_inspection=False, allow_hosted=False, allow_checkout_egress=False))
    if cross.fingerprint(workspace) != before:
        raise ValueError('Independent checking changed the participant submission')
    if previous:
        coding.validate_checker_result(checks, coding.validate_checker_result(previous[-1]['tests']))
    number = len(previous) + 1
    revision_root = root / 'submissions' / str(number)
    cross.copy_snapshot(workspace, revision_root)
    passed = all(check['passed'] for check in checks['checks'])
    result = {'schema_version': 1, 'session_sha256': cross.digest(session), 'number': number,
        'ledger': value, 'implementation_manifest': before, 'tests': checks, 'verification_seconds': seconds,
        'passed': passed, 'stage_complete': passed or value['stage_complete'] or number == study['max_attempts'],
        'first_correct_active_seconds': value['active_seconds'] if passed else None}
    cross.write_json(root / 'records' / f'{number:02}.json', result)
    cross.write_json(root / 'reviews' / f'{number:02}.json', {'schema_version': 1,
        'target_sha256': cross.digest(result), 'task': entry['case']['task'],
        'constraints': entry['case']['critical_constraints'], 'independent_reviews': [],
        'scores': [], 'qualification': 'Two blinded independent evaluators must review explanation, patch and important exceptions. Do not infer mastery from a correct patch alone.'})
    return {'participant_id': args.participant, 'round': args.round, 'stage': args.stage,
        'attempt': number, 'independent_checks_passed': passed, 'stage_complete': result['stage_complete'],
        'active_seconds': value['active_seconds'], 'human_review': 'pending'}


def assess(directory: Path) -> dict:
    directory = cross.selected_path(directory)
    study = load_study(directory)
    people, issues, per_participant, occupied_slots = [], [], [], set()
    all_reviewed = True
    for participant_root in sorted((directory / 'participants').glob('p_*')):
        try:
            consent = consent_record(cross.read_json(participant_root / 'consent.json'), study)
            participant = consent['participant_id']
            if participant_root.name != participant:
                raise ValueError('Pseudonymous participant path changed')
            if consent['slot'] in occupied_slots:
                raise ValueError('Two participants occupy one counterbalance slot')
            occupied_slots.add(consent['slot'])
            people.append(participant)
            measurements = []
            for number in (1, 2):
                assigned = round_for(study, consent, number)
                for stage in STAGES:
                    root = stage_root(directory, participant, number, stage)
                    if not (root / 'session.json').exists():
                        continue
                    session = cross.read_json(root / 'session.json')
                    entry = validate_session(directory, study, consent, session, number, stage)
                    attempts = records(root)
                    if len(attempts) > study['max_attempts']:
                        raise ValueError('Participant attempts exceed the registered bound')
                    previous, checker_contract, scores = None, None, []
                    reviewed = True
                    for index, item in enumerate(attempts, 1):
                        if previous and previous['stage_complete']:
                            raise ValueError('Participant submissions continued after completion')
                        validate_ledger(item['ledger'], session, study['max_active_seconds'])
                        if item['number'] != index or item['session_sha256'] != cross.digest(session):
                            raise ValueError('Participant attempt order or session binding changed')
                        if cross.fingerprint(root / 'submissions' / str(index)) != item['implementation_manifest']:
                            raise ValueError('Tested participant patch bytes changed')
                        actual = item['implementation_manifest']['files_sha256']
                        protected = entry['source_manifest']['files_sha256']
                        if (set(actual) != set(protected) or any(actual[name] != value for name, value in protected.items()
                                if name not in entry['case']['editable_files'])):
                            raise ValueError('A retained submission modified protected sources')
                        checker_contract = coding.validate_checker_record(item['tests'], entry['case'], root / 'workspace',
                            Path(study['cases_manifest']).parent, checker_contract)
                        passed = all(check['passed'] for check in item['tests']['checks'])
                        if (item['passed'] != passed or item['first_correct_active_seconds'] != (item['ledger']['active_seconds'] if passed else None)
                                or item['stage_complete'] != (passed or item['ledger']['stage_complete'] or index == study['max_attempts'])):
                            raise ValueError('Recorded participant success/time differs from executable checks')
                        if previous and (item['ledger']['active_seconds'] < previous['ledger']['active_seconds']
                                or item['ledger']['assistance'][:len(previous['ledger']['assistance'])] != previous['ledger']['assistance']):
                            raise ValueError('A later attempt removed prior time or assistance')
                        packet = cross.read_json(root / 'reviews' / f'{index:02}.json')
                        if packet['target_sha256'] != cross.digest(item):
                            raise ValueError('Independent review no longer binds the submission')
                        review = outcome.independent_reviews(directory, packet['independent_reviews'], excluded=[participant], target_sha256=cross.digest(item))
                        scores = reviewed_scores(packet, review, cross.digest(item), entry['case']['critical_constraints'])
                        reviewed &= review['complete']
                        previous = item
                    all_reviewed &= reviewed and bool(attempts)
                    if attempts:
                        last = attempts[-1]
                        measurements.append({'round': number, 'stage': stage, 'arm': assigned['arm'],
                            'case_id': assigned[stage], 'passed': last['passed'], 'stage_complete': last['stage_complete'],
                            'attempts': len(attempts), 'active_seconds': last['ledger']['active_seconds'],
                            'first_correct_active_seconds': last['first_correct_active_seconds'],
                            'hints': sum(event['kind'] == 'hint' for event in last['ledger']['assistance']),
                            'mentor_interventions': sum(event['kind'] == 'mentor' for event in last['ledger']['assistance']),
                            'ai_assistance': sum(event['kind'] == 'ai_assistance' for event in last['ledger']['assistance']),
                            'mean_factual_explanation_0_to_3': sum(score['factual_explanation_0_to_3'] for score in scores) / len(scores) if scores else None,
                            'mean_missed_exceptions': sum(len(score['missed_exceptions']) for score in scores) / len(scores) if scores else None,
                            'perceived_clarity_1_to_5': last['ledger']['perceived_clarity_1_to_5'],
                            'independent_reviews_complete': reviewed})
            per_participant.append({'participant_id': participant, 'measurements': measurements})
        except (ValueError, KeyError, OSError, TypeError) as error:
            issues.append(f'{participant_root.name}: {error}')
    paired = {}
    for stage in STAGES:
        paired[stage] = {}
        for metric, statistic in (('passed', 'difference'), ('active_seconds', 'median_ratio'),
                                  ('hints', 'difference'), ('mentor_interventions', 'difference'),
                                  ('mean_factual_explanation_0_to_3', 'difference'), ('mean_missed_exceptions', 'difference')):
            values = []
            for person in per_participant:
                arms = {item['arm']: item for item in person['measurements'] if item['stage'] == stage and item['stage_complete']}
                if set(arms) == set(ARMS) and all(arms[arm][metric] is not None for arm in ARMS):
                    values.append((arms[ARMS[0]][metric], arms[ARMS[1]][metric]))
            paired[stage][metric] = adaptive.paired_bootstrap(values, seed=study['seed'], statistic=statistic)
            paired[stage][metric]['unit'] = 'counterbalanced participant'
    audit = cross.read_json(directory / 'HUMAN_AUDIT.json')
    participant_binding = cross.digest({path.name: cross.fingerprint(path)['sha256'] for path in sorted((directory / 'participants').glob('p_*'))})
    audited = False
    if audit.get('complete') is True:
        try:
            if (audit['study_sha256'] != cross.digest(study) or not audit['auditor_id'].strip()
                    or audit['consenting_real_participants_verified'] is not True or audit['no_agent_impersonation'] is not True
                    or audit.get('participant_records_sha256') != participant_binding):
                raise ValueError('Human participant audit is incomplete or unbound')
            outcome.timestamp(audit['reviewed_at'])
            outcome.capture(directory, audit['capture'])
            audited = True
        except (ValueError, KeyError, TypeError, OSError) as error:
            issues.append(str(error))
    completed = sum(len(person['measurements']) == 4 and all(item['stage_complete'] for item in person['measurements']) for person in per_participant)
    known = coding.bundled_fingerprints() | cross.bundled_fingerprints()
    fixture_only = (study.get('fixture_only') is not False or any('base_project' in entry['case']
                    or entry['source_manifest']['sha256'] in known for entry in study['cases']))
    measured = not issues and completed >= 12 and all_reviewed and audited and not fixture_only and study['held_out']
    return {'schema_version': 1, 'protocol': PROTOCOL, 'mechanical_contracts_passed': not issues,
        'planned_slots': study['planned_slots'], 'enrolled_participant_records': len(people),
        'completed_participants': completed, 'verified_human_participants': len(people) if audited else None,
        'participant_records_sha256': participant_binding,
        'fixture_only': fixture_only, 'independent_review_complete': bool(people) and all_reviewed,
        'human_outcomes_status': 'independently_reviewed_counterbalanced_pilot' if measured else 'unmeasured',
        'learning_benefit_established': False, 'issues': issues, 'per_participant': per_participant,
        'paired_outcomes': paired, 'qualification': 'Twelve participants form a pilot. Prepared slots are not participants, simulated learners are not human evidence, and a correct patch alone is not mastery. Real identity/consent verification requires the independent audit capture.'}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    prepare_parser = commands.add_parser('prepare')
    prepare_parser.add_argument('--output', type=Path, required=True)
    prepare_parser.add_argument('--cases', type=Path, default=DEFAULT_CASES)
    prepare_parser.add_argument('--participants', type=int, default=12)
    prepare_parser.add_argument('--seed', type=int, default=7)
    run_parser = commands.add_parser('run')
    run_parser.add_argument('--study', type=Path, required=True)
    run_parser.add_argument('--consent', type=Path, required=True)
    run_parser.add_argument('--round', type=int, choices=(1, 2), required=True)
    run_parser.add_argument('--stage', choices=STAGES, required=True)
    run_parser.add_argument('--lore-binary', type=Path)
    run_parser.add_argument('--provider', choices=('ollama', 'openai'))
    run_parser.add_argument('--model')
    for option in ('embedding-model','decision-provider','decision-model','generative-base-url','decision-base-url'):
        run_parser.add_argument('--'+option)
    for option in ('allow-inspection','allow-hosted','allow-checkout-egress'):
        run_parser.add_argument('--'+option, action='store_true')
    run_parser.add_argument('--timeout', type=int, default=3600)
    run_parser.add_argument('--max-tokens', type=int, default=6000)
    bench.add_reasoning_options(run_parser)
    record_parser = commands.add_parser('record')
    record_parser.add_argument('--study', type=Path, required=True)
    record_parser.add_argument('--participant', required=True)
    record_parser.add_argument('--round', type=int, choices=(1, 2), required=True)
    record_parser.add_argument('--stage', choices=STAGES, required=True)
    record_parser.add_argument('--record', type=Path, required=True)
    record_parser.add_argument('--execute-checks', action='store_true')
    record_parser.add_argument('--timeout', type=int, default=60)
    assess_parser = commands.add_parser('assess')
    assess_parser.add_argument('study', type=Path)
    args = parser.parse_args(argv)
    try:
        result = prepare(args.output, args.cases, args.participants, args.seed) if args.command == 'prepare' else run(args) if args.command == 'run' else record(args) if args.command == 'record' else assess(args.study)
        print(json.dumps(result, indent=2, sort_keys=True))
        return 0 if result.get('mechanical_contracts_passed', True) else 2
    except (ValueError, OSError, KeyError, TypeError, subprocess.SubprocessError) as error:
        print(f'Human onboarding evaluation failed: {error}', file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
