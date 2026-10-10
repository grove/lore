#!/usr/bin/env python3
"""Opt-in human contribution/transfer pilot. Preparation never fabricates participants."""
from __future__ import annotations
import argparse
from contextlib import contextmanager
import json
import math
import os
from pathlib import Path
import random
import re
import secrets
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
PARTICIPANT = re.compile(r'p_[0-9a-f]{16,32}')


def read_bounded(path: Path) -> dict:
    cross.reject_symlink_path(path)
    if path.stat().st_size > MAX_RECORD_BYTES:
        raise ValueError('Participant record exceeds 100 KB')
    value = cross.read_json(path)
    if not isinstance(value, dict):
        raise ValueError('Expected a participant record object')
    return value


def write_atomic(path: Path, value: dict) -> None:
    """Publish a complete private record; interruption never publishes half JSON."""
    cross.reject_symlink_path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f'.{path.name}-{secrets.token_hex(8)}.tmp')
    try:
        with temporary.open('x', encoding='utf-8') as stream:
            if os.name != 'nt':
                os.chmod(temporary, 0o600)
            stream.write(json.dumps(value, indent=2, sort_keys=True) + '\n')
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, path)
        if os.name != 'nt':
            descriptor = os.open(path.parent, os.O_RDONLY)
            try:
                os.fsync(descriptor)
            finally:
                os.close(descriptor)
    finally:
        temporary.unlink(missing_ok=True)


def slot_plan(data: dict, participants: int, seed: int) -> list[dict]:
    if type(participants) is not int or not 4 <= participants <= 100 or participants % 4:
        raise ValueError('Use 4..100 planned slots, divisible by four for counterbalancing')
    if type(seed) is not int:
        raise ValueError('The counterbalance seed must be an integer')
    journeys = data.get('learning_journeys', [])
    if len(journeys) != 2 or journeys[0]['project'] == journeys[1]['project']:
        raise ValueError('The counterbalanced pilot requires two journeys in distinct unfamiliar projects')
    known = {case['id']: case for case in data['cases']}
    for journey in journeys:
        first, transfer = journey['first_case'], journey['transfer_case']
        if (first == transfer or first not in known or transfer not in known
                or known[first]['project'] != journey['project'] or known[transfer]['project'] != journey['project']
                or not isinstance(journey.get('required_explanation'), str) or not journey['required_explanation'].strip()):
            raise ValueError('Each journey needs distinct same-project contribution and transfer tasks')
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
    return slots


def prepare(output: Path, cases: Path = DEFAULT_CASES, participants=12, seed=7) -> dict:
    data = coding.load_cases(cases)
    slots = slot_plan(data, participants, seed)
    prepared = coding.prepare(output, cases)
    validate_checker_separation(output, prepared)
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
        'unfamiliar_projects': [journey['project'] for journey in data['learning_journeys']]})
    cross.write_json(output / 'WITHDRAWAL_TEMPLATE.json', {'participant_id': '',
        'study_sha256': cross.digest(study), 'consent_sha256': '', 'withdrawn_at': '',
        'requested_by_participant': False})
    cross.write_json(output / 'HUMAN_AUDIT.json', {'schema_version': 1, 'study_sha256': cross.digest(study),
        'complete': False, 'auditor_id': '', 'consenting_real_participants_verified': False,
        'no_agent_impersonation': False, 'participant_access_boundaries_verified': False,
        'assistance_and_timing_verified': False, 'withdrawal_state_verified': False,
        'participant_records_sha256': None,
        'capture': {'path': '', 'sha256': ''}, 'reviewed_at': ''})
    return {'schema_version': 1, 'protocol': PROTOCOL, 'phase': 'preparation_only', 'planned_slots': participants,
        'enrolled_participants': 0, 'inference_calls': 0, 'human_outcomes': 'unmeasured',
        'fixture_only': prepared['fixture_only'], 'study_sha256': cross.digest(study)}


def validate_checker_separation(directory: Path, study: dict) -> None:
    """Reject declared checker/helper bytes in any participant source input."""
    manifest_dir = Path(study['cases_manifest']).parent
    roots = [(directory / entry['source_root']).resolve() for entry in study['cases']]
    roots.extend((manifest_dir / entry['case'].get('overlay', entry['case'].get('source_root', ''))).resolve()
                 for entry in study['cases'])
    source_hashes = {digest for entry in study['cases']
                     for digest in entry['source_manifest']['files_sha256'].values()}
    for entry in study['cases']:
        if not entry.get('checker_files_sha256'):
            raise ValueError('Human study preparation must pin the external checker bytes')
        for name, digest in entry['checker_files_sha256'].items():
            path = Path(name)
            cross.reject_symlink_path(path)
            if any(path.resolve().is_relative_to(root) for root in roots) or digest in source_hashes:
                raise ValueError('A declared hidden checker or answer-bearing helper is in participant source input')


def load_study(directory: Path) -> dict:
    cross.reject_symlink_path(directory / 'study.json')
    study = cross.read_json(directory / 'study.json')
    if study.get('schema_version') != 1 or study.get('protocol') != PROTOCOL:
        raise ValueError('Expected a prepared human onboarding study')
    if coding.hash_file(Path(study['cases_manifest'])) != study['cases_manifest_sha256']:
        raise ValueError('Human study task/checker manifest changed')
    declared = coding.load_cases(Path(study['cases_manifest']))
    expected = {case['id']: case for case in declared['cases']}
    actual = [entry['case'] for entry in study['cases']]
    if (len(actual) != len(expected) or len({case['id'] for case in actual}) != len(actual)
            or any(expected.get(case['id']) != case for case in actual)):
        raise ValueError('Prepared tasks must exactly match the unique pinned manifest cases')
    if study['slots'] != slot_plan(declared, study['planned_slots'], study['seed']):
        raise ValueError('Counterbalanced slots no longer match the pinned journeys and seed')
    prepared = cross.read_json(directory / 'prepared.json')
    if (any(study[key] != prepared[key] for key in ('cases', 'fixture_only', 'held_out', 'cases_manifest_sha256'))
            or study['max_attempts'] != 5 or study['max_active_seconds'] != 7200):
        raise ValueError('Study source/task pins, eligibility or fixed attempt/time limits changed after preparation')
    for entry in study['cases']:
        if entry['source_root'] != f"snapshots/{entry['case']['id']}":
            raise ValueError('A case must use its own pinned source snapshot')
        if cross.fingerprint(directory / coding.relative_file(entry['source_root'])) != entry['source_manifest']:
            raise ValueError('A pinned participant source snapshot changed')
        if not entry.get('checker_files_sha256'):
            raise ValueError('Human study preparation must pin the external checker bytes')
        coding.validate_prepared_checker(entry, directory / entry['source_root'], Path(study['cases_manifest']).parent)
    validate_checker_separation(directory, study)
    return study


def consent_record(value: dict, study: dict) -> dict:
    expected = {'participant_id', 'participant_kind', 'slot', 'study_sha256', 'consented_at',
                'informed_consent', 'right_to_withdraw', 'human_confirmed', 'unfamiliar_projects'}
    if set(value) != expected:
        raise ValueError('Consent records contain only the documented pseudonymous fields')
    if not isinstance(value['participant_id'], str) or not PARTICIPANT.fullmatch(value['participant_id']):
        raise ValueError('Use a random p_ hexadecimal pseudonym; never a name or email')
    if value['participant_kind'] != 'human' or any(value[key] is not True for key in ('informed_consent', 'right_to_withdraw', 'human_confirmed')):
        raise ValueError('Explicit opt-in consent from an actual human participant is required')
    if value['study_sha256'] != cross.digest(study) or value['slot'] not in {slot['slot'] for slot in study['slots']}:
        raise ValueError('Consent must name this prepared study and an existing slot')
    if outcome.timestamp(value['consented_at']) > outcome.timestamp(bench.now_utc()):
        raise ValueError('Consent cannot be future-dated')
    if (not isinstance(value['unfamiliar_projects'], list) or len(value['unfamiliar_projects']) != 2
            or set(value['unfamiliar_projects']) != {item['case']['project'] for item in study['cases']}):
        raise ValueError('Participants must declare unfamiliarity with both assigned projects')
    return value


def participant_root(directory: Path, participant: str) -> Path:
    if not isinstance(participant, str) or not PARTICIPANT.fullmatch(participant):
        raise ValueError('Use the enrolled pseudonymous participant identifier')
    root = directory / 'participants' / participant
    cross.reject_symlink_path(root.parent)
    return root


def revocation_path(directory: Path, participant: str) -> Path:
    participant_root(directory, participant)
    path = directory / 'revocations' / f'{participant}.json'
    cross.reject_symlink_path(path)
    return path


def revocation_record(value: dict) -> dict:
    expected = {'schema_version', 'protocol', 'participant_id', 'study_sha256',
                'consent_sha256', 'withdrawn_at', 'requested_by_participant'}
    if (set(value) != expected or value['schema_version'] != 1 or value['protocol'] != PROTOCOL
            or not isinstance(value['participant_id'], str) or not PARTICIPANT.fullmatch(value['participant_id'])
            or value['requested_by_participant'] is not True
            or any(not isinstance(value[key], str) or not outcome.SHA256.fullmatch(value[key])
                   for key in ('study_sha256', 'consent_sha256'))
            or outcome.timestamp(value['withdrawn_at']) > outcome.timestamp(bench.now_utc())):
        raise ValueError('Withdrawal must bind the actual participant request, study and original consent')
    return value


def revocations(directory: Path, *, study_sha256: str | None = None) -> dict[str, dict]:
    cross.reject_symlink_path(directory / 'revocations')
    result = {}
    for path in sorted((directory / 'revocations').glob('*.json')):
        value = revocation_record(read_bounded(path))
        if path.name != f"{value['participant_id']}.json":
            raise ValueError('Withdrawal pseudonym does not match its retained marker')
        if study_sha256 is not None and value['study_sha256'] != study_sha256:
            raise ValueError('A retained withdrawal marker belongs to a different prepared study')
        result[value['participant_id']] = value
    return result


def erase_participant(directory: Path, participant: str) -> None:
    root = participant_root(directory, participant)
    if root.is_symlink():
        root.unlink()
    elif root.exists():
        # rmtree does not follow contained symlinks. Original project snapshots
        # and the durable revocation marker are outside this managed subtree.
        shutil.rmtree(root)


def require_active_consent(directory: Path, consent: dict, *, remove_late_files=False) -> None:
    path = revocation_path(directory, consent['participant_id'])
    if path.exists():
        if remove_late_files:
            erase_participant(directory, consent['participant_id'])
        # A malformed marker also fails closed; it is never treated as consent.
        raise ValueError('Consent was withdrawn; this pseudonym cannot start, submit or re-enroll with old consent')


@contextmanager
def session_operation(directory: Path, slot: str):
    if not re.fullmatch(r'slot-[0-9]{2,3}', slot):
        raise ValueError('Invalid counterbalance slot')
    path = directory / 'operations' / f'{slot}.lock'
    cross.reject_symlink_path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    try:
        stream = path.open('x', encoding='utf-8')
    except FileExistsError as error:
        raise ValueError('This slot has an in-flight or interrupted operation; verify it has stopped before removing its operation lock') from error
    try:
        with stream:
            stream.write(str(os.getpid()) + '\n')
        yield
    finally:
        path.unlink(missing_ok=True)


def withdraw(args: argparse.Namespace) -> dict:
    directory = cross.selected_path(args.study)
    supplied = read_bounded(args.request)
    if set(supplied) != {'participant_id', 'study_sha256', 'consent_sha256', 'withdrawn_at', 'requested_by_participant'}:
        raise ValueError('Use the documented withdrawal request fields, without personal information')
    value = revocation_record({'schema_version': 1, 'protocol': PROTOCOL, **supplied})
    participant = value['participant_id']
    marker = revocation_path(directory, participant)
    existing = marker.exists()
    if existing:
        previous = revocation_record(read_bounded(marker))
        if any(value[key] != previous[key] for key in ('participant_id', 'study_sha256', 'consent_sha256')):
            raise ValueError('Withdrawal does not match the original revoked consent')
        value = previous
    else:
        # Revocation must remain available if task/checker/source pins have
        # changed or gone offline. Validate against retained original consent,
        # without requiring load_study's execution/assessment prerequisites.
        consent = read_bounded(participant_root(directory, participant) / 'consent.json')
        if (consent.get('participant_id') != participant or consent.get('study_sha256') != value['study_sha256']
                or cross.digest(consent) != value['consent_sha256']
                or outcome.timestamp(value['withdrawn_at']) < outcome.timestamp(consent['consented_at'])):
            raise ValueError('Withdrawal must match the enrolled study, pseudonym and exact original consent')
        write_atomic(marker, value)
    removed = True
    try:
        erase_participant(directory, participant)
    except OSError:
        removed = False
    return {'schema_version': 1, 'protocol': PROTOCOL, 'participant_id': participant,
        'consent_revoked': True, 'already_revoked': existing, 'withdrawal_sha256': cross.digest(value),
        'participant_data_removed': removed, 'mechanical_contracts_passed': removed,
        'retained': 'Minimal study/consent/pseudonym-bound revocation marker; no contribution, explanation or score.',
        'next_step': 'Exclude this pseudonym from all outcomes. Remove separately retained audit captures and backups according to the consent agreement.' if removed else
                     'Consent is already revoked. Retry the same withdrawal request to finish managed participant-data cleanup.'}


def round_for(study: dict, consent: dict, number: int) -> dict:
    slot = next(item for item in study['slots'] if item['slot'] == consent['slot'])
    if number not in (1, 2):
        raise ValueError('Round must be 1 or 2')
    return slot['rounds'][number - 1]


def stage_root(directory: Path, participant: str, number: int, stage: str) -> Path:
    if not isinstance(participant, str) or not PARTICIPANT.fullmatch(participant) or number not in (1, 2) or stage not in STAGES:
        raise ValueError('Invalid participant, round or study stage')
    return directory / 'participants' / participant / f'round-{number}' / stage


def records(root: Path) -> list[dict]:
    return [cross.read_json(path) for path in sorted((root / 'records').glob('*.json'))]


def previous_stage(number: int, stage: str):
    return (number, 'contribution') if stage == 'transfer' else (1, 'transfer') if number == 2 else None


def require_complete(directory: Path, study: dict, consent: dict, number: int, stage: str) -> dict:
    root = stage_root(directory, consent['participant_id'], number, stage)
    if not (root / 'session.json').is_file():
        raise ValueError('The preceding contribution/transfer session is missing')
    session = cross.read_json(root / 'session.json')
    entry = validate_session(directory, study, consent, session, number, stage)
    try:
        attempts, _, _ = validate_attempts(directory, study, consent, session, entry, number, stage, reviews=False)
    except (KeyError, TypeError) as error:
        raise ValueError('The preceding stage lacks a complete bound submission and executable checker record') from error
    if not attempts or attempts[-1]['stage_complete'] is not True:
        raise ValueError('The preceding contribution/transfer stage must finish before exposing the next task')
    predecessor = previous_stage(number, stage)
    if predecessor:
        earlier = require_complete(directory, study, consent, *predecessor)
        if outcome.timestamp(session['started_at']) < outcome.timestamp(earlier['ledger']['finished_at']):
            raise ValueError('A later task was exposed before the preceding stage finished')
    return attempts[-1]


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
    if orientation is not None:
        initialization = session.get('initialization', {})
        if (not isinstance(initialization, dict)
                or any(not isinstance(initialization.get(key), str) or not initialization[key].strip()
                       for key in ('provider', 'model', 'model_revision'))
                or not outcome.SHA256.fullmatch(initialization.get('binary_sha256', ''))
                or initialization.get('response_sha256') != cross.digest(initialization.get('response'))):
            raise ValueError('Onboarding initialization and declared binary/model pins are incomplete')
        config_path = stage_root(directory, consent['participant_id'], number, stage) / 'orientation-project/lore.yml'
        config = read_bounded(config_path)
        if (cross.digest(config) != initialization.get('configuration_sha256')
                or config['models']['generative']['provider'] != initialization['provider']
                or config['models']['generative']['model'] != initialization['model']):
            raise ValueError('Recorded onboarding configuration no longer matches its actual session pins')
    elif session.get('initialization') is not None or session.get('orientation_experience') is not None:
        raise ValueError('A baseline or transfer session cannot contain initialization or onboarding assistance')
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


def session_plan(args: argparse.Namespace, directory: Path, study: dict, consent: dict) -> dict:
    require_active_consent(directory, consent)
    participant = consent['participant_id']
    assigned = round_for(study, consent, args.round)
    root = stage_root(directory, participant, args.round, args.stage)
    requires_onboard = assigned['arm'] == 'with_onboard' and args.stage == 'contribution'
    if args.allow_checkout_egress and not (args.allow_inspection and args.allow_hosted):
        raise ValueError('Checkout egress requires explicit hosted and inspection grants')
    if type(args.timeout) is not int or not 1 <= args.timeout <= 14400:
        raise ValueError('Session timeout must be between 1 and 14400 seconds')
    if type(args.max_tokens) is not int or not 512 <= args.max_tokens <= 32768:
        raise ValueError('Session context budget must be between 512 and 32768 tokens')
    predecessor = previous_stage(args.round, args.stage)
    if predecessor:
        require_complete(directory, study, consent, *predecessor)
    if root.exists():
        raise ValueError('This participant stage already exists; record its next attempt instead')
    person = participant_root(directory, participant)
    for path in (directory / 'participants').glob('*/consent.json'):
        other = consent_record(read_bounded(path), study)
        if other['slot'] == consent['slot'] and other['participant_id'] != participant:
            raise ValueError('A counterbalance slot cannot be assigned to two participants')
    if (person / 'consent.json').exists() and read_bounded(person / 'consent.json') != consent:
        raise ValueError('Participant consent binding changed')
    entry = next(item for item in study['cases'] if item['case']['id'] == assigned[args.stage])
    binary, config = None, None
    if requires_onboard:
        revision = getattr(args, 'model_revision', None)
        if (args.lore_binary is None or not args.provider or not args.model
                or not isinstance(revision, str) or not revision.strip() or len(revision) > 256
                or any(ord(char) < 32 for char in revision)):
            raise ValueError('The onboard arm requires an actual Lore binary, provider, model and --model-revision')
        binary = args.lore_binary.resolve(strict=True)
        if not binary.is_file() or not os.access(binary, os.X_OK):
            raise ValueError('The selected Lore binary is not an executable file')
        config = coding.config_for_case(args, entry['case'], root / 'orientation-project')
    return {'assigned': assigned, 'entry': entry, 'root': root, 'binary': binary, 'config': config,
            'binary_sha256': coding.hash_file(binary) if binary else None}


def run(args: argparse.Namespace) -> dict:
    directory = cross.selected_path(args.study)
    study = load_study(directory)
    consent = consent_record(read_bounded(args.consent), study)
    participant = consent['participant_id']
    with session_operation(directory, consent['slot']):
        plan = session_plan(args, directory, study, consent)
        root, entry = plan['root'], plan['entry']
        person = participant_root(directory, participant)
        require_active_consent(directory, consent, remove_late_files=True)
        write_atomic(person / 'consent.json', consent)
        snapshot = directory / entry['source_root']
        workspace = root / 'workspace'
        try:
            if (cross.copy_snapshot(snapshot, workspace) != entry['source_manifest']
                    or cross.fingerprint(workspace) != entry['source_manifest']):
                raise ValueError('Participant source changed while the session was prepared')
            orientation, initialization = None, None
            if plan['binary'] is not None:
                binary, config = plan['binary'], plan['config']
                project = root / 'orientation-project'
                cross.copy_snapshot(snapshot, project)
                write_atomic(project / 'lore.yml', config)
                env = shared.invocation_environment(project, allow_inspection=args.allow_inspection,
                    allow_hosted=args.allow_hosted, allow_checkout_egress=args.allow_checkout_egress)
                require_active_consent(directory, consent, remove_late_files=True)
                init, seconds = bench.subprocess_json(str(binary), project, 'init', timeout=args.timeout, env=env)
                require_active_consent(directory, consent, remove_late_files=True)
                initialization = {'response': init, 'response_sha256': cross.digest(init), 'seconds': seconds,
                    'binary_sha256': plan['binary_sha256'], 'configuration_sha256': cross.digest(config),
                    'provider': args.provider, 'model': args.model, 'model_revision': args.model_revision}
                experience = 'onboard' if args.allow_inspection else 'onboard_no_inspect'
                orientation = shared.collect(str(binary), project, snapshot, experience, entry['case']['task'], args.timeout, args.max_tokens, env=env)
                require_active_consent(directory, consent, remove_late_files=True)
                if coding.hash_file(binary) != plan['binary_sha256']:
                    raise ValueError('The selected Lore binary changed during orientation')
                if not all(shared.context_checks(orientation, experience, entry['case']['task'], snapshot).values()):
                    raise ValueError('Onboarding output failed source, capability or output integrity checks')
                write_atomic(root / 'ONBOARD.json', orientation['response'])
            require_active_consent(directory, consent, remove_late_files=True)
            session = {'schema_version': 1, 'protocol': PROTOCOL, 'participant_id': participant,
                'study_sha256': cross.digest(study), 'consent_sha256': cross.digest(consent),
                'round': args.round, 'stage': args.stage, 'arm': plan['assigned']['arm'], 'case_id': entry['case']['id'],
                'started_at': bench.now_utc(), 'source_manifest': entry['source_manifest'],
                'orientation': orientation, 'initialization': initialization,
                'orientation_experience': ('onboard' if args.allow_inspection else 'onboard_no_inspect') if orientation else None}
            write_atomic(root / 'session.json', session)
            write_atomic(root / 'TASK.json', {'task': entry['case']['task'], 'editable_files': entry['case']['editable_files'],
                'instruction': 'Work on this task, then explain the relevant principle and exceptions. Requesting hints is optional.'})
            write_atomic(root / 'RECORD_TEMPLATE.json', {'finished_at': '', 'active_seconds': None,
                'pauses': [], 'assistance': [], 'explanation': '', 'perceived_clarity_1_to_5': None,
                'stage_complete': False, 'human_authored': True})
            require_active_consent(directory, consent, remove_late_files=True)
            return {'participant_id': participant, 'round': args.round, 'stage': args.stage,
                'workspace': str(workspace), 'task': str(root / 'TASK.json'), 'session_sha256': cross.digest(session),
                'onboard_supplied': orientation is not None, 'human_outcome': 'pending'}
        except BaseException:
            if revocation_path(directory, participant).exists():
                erase_participant(directory, participant)
            elif root.exists():
                shutil.rmtree(root)
            raise


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


def cumulative_ledger(value: dict, previous: dict) -> None:
    if (value['active_seconds'] < previous['active_seconds']
            or outcome.timestamp(value['finished_at']) < outcome.timestamp(previous['finished_at'])
            or any(value[key][:len(previous[key])] != previous[key] for key in ('assistance', 'pauses'))):
        raise ValueError('Cumulative finish time, active time, pauses and assistance cannot be removed during repairs')


def submitted_patch_sha256(entry: dict, implementation: dict) -> str:
    original = entry['source_manifest']['files_sha256']
    changed = {name: implementation['files_sha256'][name] for name in entry['case']['editable_files']
               if implementation['files_sha256'][name] != original[name]}
    return cross.digest({'source_sha256': entry['source_manifest']['sha256'], 'files_sha256': changed})


def validate_attempts(directory: Path, study: dict, consent: dict, session: dict, entry: dict,
                      number: int, stage: str, *, reviews: bool) -> tuple[list, bool, list]:
    root = stage_root(directory, consent['participant_id'], number, stage)
    attempts = records(root)
    if len(attempts) > study['max_attempts']:
        raise ValueError('Participant attempts exceed the registered bound')
    previous, checker_contract, scores = None, None, []
    reviewed = bool(attempts)
    for index, item in enumerate(attempts, 1):
        if previous and previous['stage_complete']:
            raise ValueError('Participant submissions continued after completion')
        validate_ledger(item['ledger'], session, study['max_active_seconds'])
        if item['number'] != index or item['session_sha256'] != cross.digest(session):
            raise ValueError('Participant attempt order or session binding changed')
        submitted = root / 'submissions' / str(index)
        if cross.fingerprint(submitted) != item['implementation_manifest']:
            raise ValueError('Tested participant patch bytes changed')
        actual = item['implementation_manifest']['files_sha256']
        protected = entry['source_manifest']['files_sha256']
        if (set(actual) != set(protected) or any(actual[name] != value for name, value in protected.items()
                if name not in entry['case']['editable_files'])):
            raise ValueError('A retained submission modified protected sources')
        if item.get('submitted_patch_sha256') != submitted_patch_sha256(entry, item['implementation_manifest']):
            raise ValueError('Submitted patch digest no longer binds the original source and changed file bytes')
        if item.get('checker_workspace') != f'submissions/{index}':
            raise ValueError('Independent checks must bind the exact retained submitted snapshot')
        checker_contract = coding.validate_checker_record(item['tests'], entry['case'], submitted,
            Path(study['cases_manifest']).parent, checker_contract)
        passed = all(check['passed'] for check in item['tests']['checks'])
        if (item['passed'] != passed or item['first_correct_active_seconds'] != (item['ledger']['active_seconds'] if passed else None)
                or item['stage_complete'] != (passed or item['ledger']['stage_complete'] or index == study['max_attempts'])):
            raise ValueError('Recorded participant success/time differs from executable checks')
        if previous:
            cumulative_ledger(item['ledger'], previous['ledger'])
        if reviews:
            packet = cross.read_json(root / 'reviews' / f'{index:02}.json')
            if packet['target_sha256'] != cross.digest(item):
                raise ValueError('Independent review no longer binds the submission')
            review = outcome.independent_reviews(directory, packet['independent_reviews'],
                excluded=[consent['participant_id']], target_sha256=cross.digest(item))
            scores = reviewed_scores(packet, review, cross.digest(item), entry['case']['critical_constraints'])
            reviewed &= review['complete']
        previous = item
    return attempts, reviewed if reviews else False, scores


def record(args: argparse.Namespace) -> dict:
    if not args.execute_checks:
        raise ValueError('Recording a patch requires --execute-checks in an intentionally isolated study environment')
    directory = cross.selected_path(args.study)
    study = load_study(directory)
    if revocation_path(directory, args.participant).exists():
        raise ValueError('Consent was withdrawn; no further submission or checking is permitted')
    consent = consent_record(read_bounded(participant_root(directory, args.participant) / 'consent.json'), study)
    with session_operation(directory, consent['slot']):
        require_active_consent(directory, consent, remove_late_files=True)
        root = stage_root(directory, args.participant, args.round, args.stage)
        session = cross.read_json(root / 'session.json')
        entry = validate_session(directory, study, consent, session, args.round, args.stage)
        value = validate_ledger(read_bounded(args.record), session, study['max_active_seconds'])
        previous, _, _ = validate_attempts(directory, study, consent, session, entry, args.round, args.stage, reviews=False)
        if len(previous) >= study['max_attempts'] or (previous and previous[-1]['stage_complete']):
            raise ValueError('The stage finished or exhausted its attempt allowance')
        if previous:
            cumulative_ledger(value, previous[-1]['ledger'])
        workspace = root / 'workspace'
        before = cross.fingerprint(workspace)
        allowed = set(entry['case']['editable_files'])
        if (set(before['files_sha256']) != set(entry['source_manifest']['files_sha256'])
                or any(before['files_sha256'].get(name) != digest for name, digest in entry['source_manifest']['files_sha256'].items() if name not in allowed)):
            raise ValueError('Participant changed files outside the allowed contribution')
        number = len(previous) + 1
        revision_root = root / 'submissions' / str(number)
        record_path = root / 'records' / f'{number:02}.json'
        review_path = root / 'reviews' / f'{number:02}.json'
        try:
            if (revision_root.exists() or cross.copy_snapshot(workspace, revision_root) != before
                    or cross.fingerprint(revision_root) != before):
                raise ValueError('The exact submission could not be captured without source drift')
            require_active_consent(directory, consent, remove_late_files=True)
            checks, seconds = coding.execute_checks(entry['case'], revision_root, Path(study['cases_manifest']).parent, args.timeout,
                env=shared.invocation_environment(revision_root, allow_inspection=False, allow_hosted=False, allow_checkout_egress=False))
            require_active_consent(directory, consent, remove_late_files=True)
            if cross.fingerprint(workspace) != before or cross.fingerprint(revision_root) != before:
                raise ValueError('Independent checking changed the participant submission')
            if previous:
                coding.validate_checker_result(checks, coding.validate_checker_result(previous[-1]['tests']))
            passed = all(check['passed'] for check in checks['checks'])
            result = {'schema_version': 1, 'session_sha256': cross.digest(session), 'number': number,
                'ledger': value, 'implementation_manifest': before, 'tests': checks, 'verification_seconds': seconds,
                'submitted_patch_sha256': submitted_patch_sha256(entry, before), 'checker_workspace': f'submissions/{number}',
                'passed': passed, 'stage_complete': passed or value['stage_complete'] or number == study['max_attempts'],
                'first_correct_active_seconds': value['active_seconds'] if passed else None}
            require_active_consent(directory, consent, remove_late_files=True)
            write_atomic(review_path, {'schema_version': 1,
                'target_sha256': cross.digest(result), 'task': entry['case']['task'],
                'constraints': entry['case']['critical_constraints'], 'independent_reviews': [],
                'scores': [], 'qualification': 'Two blinded independent evaluators must review explanation, patch and important exceptions. Do not infer mastery from a correct patch alone.'})
            write_atomic(record_path, result)
            require_active_consent(directory, consent, remove_late_files=True)
            return {'participant_id': args.participant, 'round': args.round, 'stage': args.stage,
                'attempt': number, 'submitted_patch_sha256': result['submitted_patch_sha256'],
                'independent_checks_passed': passed, 'stage_complete': result['stage_complete'],
                'active_seconds': value['active_seconds'], 'human_review': 'pending'}
        except BaseException:
            if revocation_path(directory, args.participant).exists():
                erase_participant(directory, args.participant)
            elif not record_path.exists():
                if revision_root.exists():
                    shutil.rmtree(revision_root)
                review_path.unlink(missing_ok=True)
            raise


def assess(directory: Path) -> dict:
    directory = cross.selected_path(directory)
    study = load_study(directory)
    withdrawn = revocations(directory, study_sha256=cross.digest(study))
    withdrawal_binding = cross.digest(withdrawn)
    people, issues, per_participant, occupied_slots = [], [], [], set()
    all_reviewed = True
    for participant_root in sorted((directory / 'participants').glob('p_*')):
        if participant_root.name in withdrawn:
            issues.append(f'{participant_root.name}: withdrawn data awaits managed cleanup; excluded from all outcomes')
            continue
        try:
            consent = consent_record(read_bounded(participant_root / 'consent.json'), study)
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
                    predecessor = previous_stage(number, stage)
                    if predecessor:
                        earlier = require_complete(directory, study, consent, *predecessor)
                        if outcome.timestamp(session['started_at']) < outcome.timestamp(earlier['ledger']['finished_at']):
                            raise ValueError('A later task was exposed before the preceding stage finished')
                    attempts, reviewed, scores = validate_attempts(directory, study, consent, session, entry,
                        number, stage, reviews=True)
                    all_reviewed &= reviewed and bool(attempts)
                    if attempts:
                        last = attempts[-1]
                        measurements.append({'round': number, 'stage': stage, 'arm': assigned['arm'],
                            'case_id': assigned[stage], 'passed': last['passed'], 'stage_complete': last['stage_complete'],
                            'attempts': len(attempts), 'active_seconds': last['ledger']['active_seconds'],
                            'first_correct_active_seconds': last['first_correct_active_seconds'],
                            'submitted_patch_sha256': last['submitted_patch_sha256'],
                            'hints': sum(event['kind'] == 'hint' for event in last['ledger']['assistance']),
                            'mentor_interventions': sum(event['kind'] == 'mentor' for event in last['ledger']['assistance']),
                            'ai_assistance': sum(event['kind'] == 'ai_assistance' for event in last['ledger']['assistance']),
                            'mean_factual_explanation_0_to_3': sum(score['factual_explanation_0_to_3'] for score in scores) / len(scores) if scores else None,
                            'mean_missed_exceptions': sum(len(score['missed_exceptions']) for score in scores) / len(scores) if scores else None,
                            'perceived_clarity_1_to_5': last['ledger']['perceived_clarity_1_to_5'],
                            'independent_transfer_success': (
                                last['passed'] and not last['ledger']['assistance']
                                and all(score['patch_correct'] and score['factual_explanation_0_to_3'] == 3
                                        and not score['missed_exceptions'] for score in scores)
                                ) if stage == 'transfer' and reviewed and scores else None,
                            'independent_reviews_complete': reviewed})
            per_participant.append({'participant_id': participant, 'measurements': measurements})
        except (ValueError, KeyError, OSError, TypeError) as error:
            issues.append(f'{participant_root.name}: {error}')
    paired = {}
    for stage in STAGES:
        paired[stage] = {}
        metrics = [('passed', 'difference'), ('active_seconds', 'median_ratio'),
                                  ('first_correct_active_seconds', 'median_ratio'),
                                  ('hints', 'difference'), ('mentor_interventions', 'difference'),
                                  ('mean_factual_explanation_0_to_3', 'difference'), ('mean_missed_exceptions', 'difference')]
        if stage == 'transfer':
            metrics.append(('independent_transfer_success', 'difference'))
        for metric, statistic in metrics:
            values = []
            for person in per_participant:
                arms = {item['arm']: item for item in person['measurements'] if item['stage'] == stage and item['stage_complete']}
                if set(arms) == set(ARMS) and all(arms[arm][metric] is not None for arm in ARMS):
                    values.append((arms[ARMS[0]][metric], arms[ARMS[1]][metric]))
            paired[stage][metric] = adaptive.paired_bootstrap(values, seed=study['seed'], statistic=statistic)
            paired[stage][metric]['unit'] = 'counterbalanced participant'
    audit = cross.read_json(directory / 'HUMAN_AUDIT.json')
    participant_binding = cross.digest({'active_participants': {
        path.name: cross.fingerprint(path)['sha256'] for path in sorted((directory / 'participants').glob('p_*'))
        if path.name not in withdrawn}, 'revocations': withdrawn})
    audited = False
    if audit.get('complete') is True:
        try:
            if (audit['study_sha256'] != cross.digest(study) or not audit['auditor_id'].strip()
                    or audit['consenting_real_participants_verified'] is not True or audit['no_agent_impersonation'] is not True
                    or any(audit.get(key) is not True for key in ('participant_access_boundaries_verified',
                                                                'assistance_and_timing_verified', 'withdrawal_state_verified'))
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
    if cross.digest(revocations(directory, study_sha256=cross.digest(study))) != withdrawal_binding:
        raise ValueError('Consent was withdrawn during assessment; discard the incomplete assessment and repeat it')
    return {'schema_version': 1, 'protocol': PROTOCOL, 'mechanical_contracts_passed': not issues,
        'phase': 'not_run' if not people and not withdrawn else 'records_collected',
        'planned_slots': study['planned_slots'], 'enrolled_participant_records': len(people),
        'withdrawn_participant_records': len(withdrawn), 'withdrawn_data_excluded': True,
        'completed_participants': completed, 'verified_human_participants': len(people) if audited else None,
        'participant_records_sha256': participant_binding,
        'fixture_only': fixture_only, 'held_out': study['held_out'], 'human_audit_complete': audited,
        'independent_review_complete': bool(people) and all_reviewed,
        'human_outcomes_status': 'independently_reviewed_counterbalanced_pilot' if measured else 'unmeasured',
        'first_correct_contribution_time_status': 'measured' if measured else 'unmeasured',
        'independent_learning_transfer_status': 'measured' if measured else 'unmeasured',
        'learning_benefit_established': False, 'issues': issues, 'per_participant': per_participant,
        'paired_outcomes': paired, 'qualification': 'Twelve participants form a pilot. Prepared slots are not participants, simulated learners are not human evidence, and a correct patch alone is not mastery. Real identity/consent verification requires the independent audit capture.'}


def readiness(directory: Path, args: argparse.Namespace | None = None) -> dict:
    """Read-only readiness. No enrollment, inference, checks or task publication."""
    directory = cross.selected_path(directory)
    report = assess(directory)
    study = load_study(directory)
    requirements = [
        ('pinned_source_task_checker_integrity', report['mechanical_contracts_passed'],
         'Keep the exact prepared source snapshots, manifest tasks, counterbalanced assignments and checker programs intact, with declared checker/helper paths and bytes outside every participant source input.'),
        ('independent_unfamiliar_held_out_projects', not report['fixture_only'] and study['held_out'],
         'Prepare two independently selected unfamiliar projects with distinct contribution/transfer cases and external hidden checkers via --cases; bundled fixtures are ineligible.'),
        ('twelve_real_consented_participants', report['verified_human_participants'] is not None and report['verified_human_participants'] >= 12,
         'Enroll at least twelve actual consenting humans with study-bound pseudonyms; verify participation and current consent through independent retained audit capture.'),
        ('four_completed_stages_per_participant', report['completed_participants'] >= 12,
         'Collect both counterbalanced rounds, each with a first contribution and different transfer, including unsuccessful stops, exact patches, cumulative time and assistance.'),
        ('independent_submission_and_explanation_reviews', report['independent_review_complete'],
         'Obtain two distinct blinded, conflict-free reviews and source-derived factual/exception scores bound to every exact submitted record, including earlier failures.'),
        ('participation_access_assistance_and_withdrawal_audit', report['human_audit_complete'],
         'Complete the independent study/record-bound human audit, including enforced checker isolation, assistance/time capture and all current withdrawals.'),
    ]
    required = [{'id': name, 'status': 'satisfied' if complete else 'pending', 'requirement': text}
                for name, complete, text in requirements]
    session = {'requested': False, 'mechanically_ready': None,
        'provider_availability': 'not_checked', 'human_identity_authenticated': False,
        'operator_isolation': 'requires_independent_verification'}
    if args is not None and any(getattr(args, key, None) is not None for key in ('consent', 'round', 'stage')):
        session['requested'] = True
        try:
            if any(getattr(args, key, None) is None for key in ('consent', 'round', 'stage')):
                raise ValueError('Supply --consent, --round and --stage together to check a specific session')
            consent = consent_record(read_bounded(args.consent), study)
            if (directory / 'operations' / f"{consent['slot']}.lock").exists():
                raise ValueError('The selected slot has an in-flight or interrupted operation')
            plan = session_plan(args, directory, study, consent)
            session.update(mechanically_ready=True, arm=plan['assigned']['arm'], stage=args.stage,
                case_id=plan['entry']['case']['id'], onboarding_required=plan['binary'] is not None,
                binary_sha256=plan['binary_sha256'], model_revision=getattr(args, 'model_revision', None),
                consent_sha256=cross.digest(consent))
        except (ValueError, KeyError, OSError, TypeError) as error:
            session.update(mechanically_ready=False, blocker=str(error))
    return {'schema_version': 1, 'contract': 'lore.human_study_readiness', 'protocol': PROTOCOL,
        'phase': report['phase'], 'study_sha256': cross.digest(study),
        'planned_slots': report['planned_slots'], 'enrolled_participant_records': report['enrolled_participant_records'],
        'verified_human_participants': report['verified_human_participants'],
        'completed_participants': report['completed_participants'],
        'withdrawn_participant_records': report['withdrawn_participant_records'],
        'human_outcomes_status': report['human_outcomes_status'],
        'first_correct_contribution_time_status': report['first_correct_contribution_time_status'],
        'independent_learning_transfer_status': report['independent_learning_transfer_status'],
        'learning_benefit_established': False, 'fixture_only': report['fixture_only'],
        'mechanical_contracts_passed': report['mechanical_contracts_passed'],
        'inference_calls': 0, 'executed_checks': 0, 'enrollment_or_session_writes': False,
        'requirements': required, 'blocking_prerequisites': [item['id'] for item in required if item['status'] != 'satisfied'],
        'session': session, 'issues': report['issues'],
        'qualification': 'Readiness validates retained local mechanics and declarations. It does not recruit people, authenticate human identities, contact a provider, prove OS isolation or measure learning. A local session may be mechanically ready while the real study remains not run.'}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    prepare_parser = commands.add_parser('prepare')
    prepare_parser.add_argument('--output', type=Path, required=True)
    prepare_parser.add_argument('--cases', type=Path, default=DEFAULT_CASES)
    prepare_parser.add_argument('--participants', type=int, default=12)
    prepare_parser.add_argument('--seed', type=int, default=7)
    for name in ('run', 'readiness'):
        run_parser = commands.add_parser(name)
        run_parser.add_argument('--study', type=Path, required=True)
        run_parser.add_argument('--consent', type=Path, required=name == 'run')
        run_parser.add_argument('--round', type=int, choices=(1, 2), required=name == 'run')
        run_parser.add_argument('--stage', choices=STAGES, required=name == 'run')
        run_parser.add_argument('--lore-binary', type=Path)
        run_parser.add_argument('--provider', choices=('ollama', 'openai'))
        run_parser.add_argument('--model')
        run_parser.add_argument('--model-revision', help='Exact declared provider/model revision; availability remains independently verified')
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
    withdraw_parser = commands.add_parser('withdraw')
    withdraw_parser.add_argument('--study', type=Path, required=True)
    withdraw_parser.add_argument('--request', type=Path, required=True)
    args = parser.parse_args(argv)
    try:
        if args.command == 'prepare':
            result = prepare(args.output, args.cases, args.participants, args.seed)
        elif args.command == 'run':
            result = run(args)
        elif args.command == 'record':
            result = record(args)
        elif args.command == 'withdraw':
            result = withdraw(args)
        elif args.command == 'readiness':
            result = readiness(args.study, args)
        else:
            result = assess(args.study)
        print(json.dumps(result, indent=2, sort_keys=True))
        return 0 if result.get('mechanical_contracts_passed', True) else 2
    except (ValueError, OSError, KeyError, TypeError, subprocess.SubprocessError) as error:
        print(f'Human onboarding evaluation failed: {error}', file=sys.stderr)
        return 1
    except KeyboardInterrupt:
        print('Human onboarding interrupted; assess retained records before continuing. Interruption never implies completion.', file=sys.stderr)
        return 130


if __name__ == '__main__':
    raise SystemExit(main())
