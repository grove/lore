#!/usr/bin/env python3
"""Actual different-task context/revision/grant probes; no invented coding outcomes.

This companion experiment isolates retained investigation behavior. The six-arm
coding runner measures actual patches; this probe never substitutes context
reuse for a completed coding task.
"""
from __future__ import annotations
import argparse
import json
from pathlib import Path
import random
import shutil
import subprocess
import sys
import time

import adaptive_tasks as adaptive
import benchmark as bench
import coding_tasks as coding
import cross_source as cross
import decision_tasks as decision
import shared_intelligence as shared

PROTOCOL = 'adaptive-different-task-v1'
DEFAULT = Path(__file__).resolve().parent / 'corpora/adaptive-sequences/cases.json'
ARMS = adaptive.ADAPTIVE


def load_cases(path: Path) -> dict:
    data = cross.read_json(path)
    if data.get('schema_version') != 1 or not isinstance(data.get('sequences'), list) or not data['sequences']:
        raise ValueError('Expected nonempty schema-1 different-task sequences')
    if len(data['sequences']) > 30:
        raise ValueError('At most 30 different-task sequences are allowed')
    identities = set()
    for case in data['sequences']:
        if coding.relative_file(case['id']) != case['id'] or '/' in case['id'] or case['id'] in identities:
            raise ValueError('Sequences require unique simple IDs')
        identities.add(case['id'])
        if not case.get('task_a') or not case.get('task_b') or case['task_a'] == case['task_b']:
            raise ValueError('Reuse requires two distinct tasks')
        for phase in ('related_revision', 'unrelated_revision'):
            change = case[phase]
            coding.relative_file(change['path'])
            if not isinstance(change['before'], str) or not change['before'] or not isinstance(change['after'], str) or change['before'] == change['after']:
                raise ValueError('A revision must declare an exact meaningful byte replacement')
        if case['related_revision']['path'] == case['unrelated_revision']['path']:
            raise ValueError('Related and unrelated changes must affect distinct paths')
    return data


def prepare(output: Path, manifest: Path = DEFAULT) -> dict:
    output, manifest = cross.selected_path(output), cross.selected_path(manifest)
    data = load_cases(manifest)
    if output.exists():
        raise ValueError('Use a fresh sequence output directory')
    rows = []
    for sequence in data['sequences']:
        source_cases = coding.load_cases(manifest.parent / sequence['source_cases'])
        case = next(item for item in source_cases['cases'] if item['id'] == sequence['source_case'])
        source_manifest_dir = (manifest.parent / sequence['source_cases']).resolve().parent
        destination = output / 'snapshots' / sequence['id']
        if 'base_project' in case:
            cross.prepare_project(case['base_project'], destination)
            cross.copy_snapshot(source_manifest_dir / case['overlay'], destination)
        else:
            cross.copy_snapshot(source_manifest_dir / case['source_root'], destination)
        for phase in ('related_revision', 'unrelated_revision'):
            change = sequence[phase]
            if (destination / change['path']).read_bytes().decode('utf-8').count(change['before']) != 1:
                raise ValueError('Revision precondition must match exactly once in the pinned source')
        rows.append({'sequence':sequence,'case':case,'source_root':destination.relative_to(output).as_posix(),
                     'source_manifest':cross.fingerprint(destination)})
    result = {'schema_version':1,'protocol':PROTOCOL,'phase':'preparation_only','inference_calls':0,
        'manifest_path':str(manifest),'manifest_sha256':coding.hash_file(manifest),'sequences':rows,
        'fixture_only':data.get('fixture_only') is not False,'actual_coding_outcomes':'unmeasured'}
    cross.write_json(output/'prepared.json',result)
    return result


def snapshot_source(project: Path, destination: Path) -> dict:
    state = decision.source_state(project)
    for name in state:
        target=destination/name;target.parent.mkdir(parents=True,exist_ok=True)
        shutil.copyfile(project/name,target)
    return cross.fingerprint(destination)


def checks_for(record: dict, source: Path, arm: str, policy: dict) -> dict:
    response=record['response']
    experience='adaptive' if policy['allow_inspection'] else 'adaptive_no_inspect'
    result=shared.response_checks(response,experience,record['task'],source)
    shared.verify_relationship_sources(response,record['citation_integrity'])
    nested=shared.intelligence(response,experience)
    result.update({
        'request_bound':record['arguments']==adaptive.arguments(arm,record['task'],policy),
        'host_grants_bound':record['host_grants']==adaptive.grant_record(Path(record['project_root']),policy),
        'response_bound':record['response_sha256']==cross.digest(response),
        'source_bound':record['source_manifest']==cross.fingerprint(source),
        'registry_preserved':cross.valid_registry(record['registry_before']) and record['registry_before']==record['registry_after'],
        'source_preserved':record['sources_before']==record['sources_after'],
        'source_state_bound':{name:value['sha256'] for name,value in record['sources_before'].items()}==record['source_manifest']['files_sha256'],
        'elapsed_valid':adaptive.nonnegative(record['elapsed_seconds']),
        'budget_bound':response.get('budget',{}).get('max_tokens')==policy['max_tokens'],
        'evidence_bound':coding.resolver_complete(record['citation_integrity'],shared.response_citations(response)),
        'grant_ceiling':response['capabilities'].get('inspection')==('granted' if policy['allow_inspection'] else 'disabled_by_caller')
            and (response['capabilities'].get('hosted_egress') is not True or policy['allow_hosted'])
            and (response['capabilities'].get('checkout_egress') is not True or policy['allow_checkout_egress']),
        'no_ungranted_observations':policy['allow_inspection'] or (not nested.get('inspection',{}).get('observations')
            and nested.get('inspection',{}).get('budget',{}).get('files_read',0)==0
            and not nested.get('checkout_egress',{}).get('model_received_checkout',False)),
        'no_reuse_arm_disabled':arm!='adaptive_no_reuse' or (record['cache_before']==record['cache_after']
            and nested.get('cache_status')!='hit' and nested.get('investigation',{}).get('stop_reason')!='cache_reused'),
        'usage_bound':record['usage']==adaptive.measured_usage(response,arm),
    })
    return result


def collect(binary: str, project: Path, source: Path, task: str, arm: str, policy: dict) -> dict:
    before,sources=coding.registry_content(project),decision.source_state(project)
    cache_before=adaptive.cache_state(project)
    argv=adaptive.arguments(arm,task,policy)
    env=adaptive.environment_factory(policy)(project,'context')
    response,seconds=bench.subprocess_json(binary,project,*argv,timeout=policy['timeout'],env=env)
    citations=cross.resolve_citations(binary,project,shared.response_citations(response),policy['timeout'],env=env)
    result={'task':task,'arguments':argv,'host_grants':adaptive.grant_record(project,policy),
        'project_root':str(project.resolve(strict=True)),
        'response':response,'response_sha256':cross.digest(response),'elapsed_seconds':seconds,
        'usage':adaptive.measured_usage(response,arm),'registry_before':before,'registry_after':coding.registry_content(project),
        'sources_before':sources,'sources_after':decision.source_state(project),'source_manifest':cross.fingerprint(source),
        'cache_before':cache_before,'cache_after':adaptive.cache_state(project),'citation_integrity':citations}
    result['checks']=checks_for(result,source,arm,policy)
    if not all(result['checks'].values()):
        raise ValueError('Different-task context failed: '+', '.join(key for key,value in result['checks'].items() if not value))
    return result


def run(args: argparse.Namespace) -> dict:
    policy=adaptive.policy_from_args(args)
    prepared=prepare(args.output,args.cases)
    output=cross.selected_path(args.output)
    binary=str(args.lore_binary.resolve(strict=True))
    rng=random.Random(args.seed)
    report={'schema_version':1,'protocol':PROTOCOL,'phase':'actual_context_sequence','created_at':bench.now_utc(),
        'prepared_sha256':cross.digest(prepared),'binary_sha256':coding.hash_file(Path(binary)),
        'policy':policy,'seed':args.seed,'fixture_only':prepared['fixture_only'],'sequences':[],
        'actual_coding_outcomes':'unmeasured','qualification':'Task A/B context calls and source/grant changes, not executed coding-task outcomes.'}
    for entry in prepared['sequences']:
        sequence=entry['sequence'];base=output/'initialized'/sequence['id']
        initialization_started=time.monotonic()
        cross.copy_snapshot(output/entry['source_root'],base)
        config=coding.config_for_case(args,entry['case'],base)
        config['context']={'cache':True,'inspection':{'root':'.','enabled':False}}
        cross.write_json(base/'lore.yml',config)
        env=adaptive.environment_factory(policy)(base,'preparation')
        initialized,init_seconds=bench.subprocess_json(binary,base,'init',timeout=policy['timeout'],env=env)
        result={'sequence_id':sequence['id'],'initialization':initialized,
                'initialization_sha256':cross.digest(initialized),'initialization_seconds':init_seconds,
                'initialization_overhead_seconds':max(0.0,time.monotonic()-initialization_started-init_seconds),
                'initialization_usage':coding.usage(initialized.get('usage'),calls=cross.model_calls(initialized)),
                'initialized_manifest':cross.fingerprint(base),'arms':{}}
        order=list(ARMS);rng.shuffle(order);result['arm_order']=order
        for arm in order:
            arm_started=time.monotonic()
            project=output/'projects'/sequence['id']/arm
            shutil.copytree(base,project)
            phases=[]
            for phase in ('task_a','task_b','related_revision','unrelated_revision','narrower_grant'):
                active_policy=dict(policy)
                update=None
                if phase in ('related_revision','unrelated_revision'):
                    change=sequence[phase];path=project/change['path'];cross.reject_symlink_path(path)
                    current=path.read_bytes().decode('utf-8')
                    if current.count(change['before'])!=1:
                        raise ValueError('A revision no longer matches its pinned source')
                    path.write_bytes(current.replace(change['before'],change['after']).encode('utf-8'))
                    before_update=decision.source_state(project)
                    refreshed,seconds=bench.subprocess_json(binary,project,'update',timeout=policy['timeout'],
                        env=adaptive.environment_factory(policy)(project,'revision'))
                    if decision.source_state(project)!=before_update:
                        raise ValueError('Updating knowledge changed original source bytes')
                    update={'response':refreshed,'response_sha256':cross.digest(refreshed),'elapsed_seconds':seconds,
                            'usage':coding.usage(refreshed.get('usage'),calls=cross.model_calls(refreshed)),'change':change}
                if phase=='narrower_grant':
                    active_policy.update(allow_inspection=False,allow_hosted=False,allow_checkout_egress=False)
                source=output/'phase-sources'/sequence['id']/arm/phase
                snapshot_source(project,source)
                task=sequence['task_a'] if phase=='task_a' else sequence['task_b']
                record=collect(binary,project,source,task,arm,active_policy)
                phases.append({'phase':phase,'policy':active_policy,'update':update,
                               'source_root':source.relative_to(output).as_posix(),'context':record})
            charged=sum(item['context']['elapsed_seconds']+(item['update']['elapsed_seconds'] if item['update'] else 0) for item in phases)
            result['arms'][arm]={'phases':phases,'project_root':project.relative_to(output).as_posix(),
                'protocol_overhead_seconds':max(0.0,time.monotonic()-arm_started-charged)}
        report['sequences'].append(result)
        cross.write_json(output/'metrics.json',report)
    summary=assess(output);cross.write_json(output/'assessment.json',summary)
    return summary


def assess(directory: Path) -> dict:
    directory=cross.selected_path(directory)
    report=cross.read_json(directory/'metrics.json');prepared=cross.read_json(directory/'prepared.json')
    if report.get('protocol')!=PROTOCOL or report.get('phase')!='actual_context_sequence' or report['prepared_sha256']!=cross.digest(prepared):
        raise ValueError('Sequence provenance changed')
    if coding.hash_file(Path(prepared['manifest_path']))!=prepared['manifest_sha256']:
        raise ValueError('The pinned sequence/task manifest changed')
    entries={entry['sequence']['id']:entry for entry in prepared['sequences']}
    if set(entries)!={entry['sequence_id'] for entry in report['sequences']}:
        raise ValueError('Different-task cohort is incomplete')
    issues,rows,totals=[],[],[]
    for result in report['sequences']:
        entry=entries[result['sequence_id']];sequence=entry['sequence']
        if (result['initialization_sha256']!=cross.digest(result['initialization'])
                or result['initialization_usage']!=coding.usage(result['initialization'].get('usage'),calls=cross.model_calls(result['initialization']))
                or result['initialized_manifest']!=cross.fingerprint(directory/'initialized'/sequence['id'])
                or not adaptive.nonnegative(result['initialization_seconds'])
                or not adaptive.nonnegative(result['initialization_overhead_seconds'])):
            raise ValueError('Different-task initialization or its usage changed')
        if set(result['arms'])!=set(ARMS):
            raise ValueError('Different-task reuse needs both matched cache policies')
        for arm in ARMS:
            phases=result['arms'][arm]['phases']
            project=directory/'projects'/sequence['id']/arm
            if (result['arms'][arm]['project_root']!=project.relative_to(directory).as_posix()
                    or not adaptive.nonnegative(result['arms'][arm]['protocol_overhead_seconds'])):
                raise ValueError('Sequence arm location or overhead changed')
            seconds=result['initialization_seconds']+result['initialization_overhead_seconds']+result['arms'][arm]['protocol_overhead_seconds']
            usage=[result['initialization_usage']]
            if [item['phase'] for item in phases]!=['task_a','task_b','related_revision','unrelated_revision','narrower_grant']:
                raise ValueError('Sequence phases are missing or reordered')
            expected_files=dict(entry['source_manifest']['files_sha256'])
            source_text={name:(directory/entry['source_root']/name).read_bytes().decode('utf-8') for name in expected_files}
            for item in phases:
                phase=item['phase'];policy=dict(report['policy'])
                if phase=='narrower_grant':policy.update(allow_inspection=False,allow_hosted=False,allow_checkout_egress=False)
                if phase in ('related_revision','unrelated_revision'):
                    change=sequence[phase]
                    source_text[change['path']]=source_text[change['path']].replace(change['before'],change['after'])
                    import hashlib
                    expected_files[change['path']]=hashlib.sha256(source_text[change['path']].encode('utf-8')).hexdigest()
                    if item['update']['change']!=change or item['update']['response_sha256']!=cross.digest(item['update']['response']):
                        issues.append('Source revision or refresh output changed')
                    if (item['update']['usage']!=coding.usage(item['update']['response'].get('usage'),calls=cross.model_calls(item['update']['response']))
                            or not adaptive.nonnegative(item['update']['elapsed_seconds'])):
                        issues.append('Source refresh cost changed')
                elif item['update'] is not None:
                    issues.append('An undeclared source refresh was recorded')
                record=item['context'];source=directory/coding.relative_file(item['source_root'])
                expected_task=sequence['task_a'] if phase=='task_a' else sequence['task_b']
                if (item['policy']!=policy or record['task']!=expected_task or record['source_manifest']['files_sha256']!=expected_files
                        or record['project_root']!=str(project.resolve(strict=True))):
                    issues.append(f'{sequence["id"]}/{arm}/{phase}: source, task or grant sequence changed')
                try:
                    checks=checks_for(record,source,arm,policy)
                    if not all(checks.values()):issues.append(f'{sequence["id"]}/{arm}/{phase}: context integrity failed')
                except (ValueError,KeyError,TypeError,OSError) as error:issues.append(str(error))
                nested=adaptive.core(record['response'],arm)
                seconds+=record['elapsed_seconds']+(item['update']['elapsed_seconds'] if item['update'] else 0)
                usage.append(record['usage'])
                if item['update']:usage.append(item['update']['usage'])
                rows.append({'sequence_id':sequence['id'],'arm':arm,'phase':phase,
                    'cache_status':nested.get('cache_status'),'investigation_reused':nested.get('investigation',{}).get('stop_reason')=='cache_reused',
                    'investigation_steps':len(nested.get('investigation',{}).get('steps',[])),
                    'model_calls':record['usage']['model_calls'],'context_seconds':record['elapsed_seconds'],
                    'refresh_seconds':item['update']['elapsed_seconds'] if item['update'] else 0,
                    'mode':nested.get('mode'),'usage':record['usage']})
            totals.append({'sequence_id':sequence['id'],'arm':arm,'total_seconds':seconds,
                'total_usage':coding.add_usage(usage),
                'cost_scope':'Full initialization, both tasks, both refreshes, all repeated task-B contexts, and measured protocol overhead; no amortization.'})
        for left,right in zip(*(result['arms'][arm]['phases'] for arm in ARMS)):
            if (left['context']['source_manifest']!=right['context']['source_manifest']
                    or left['context']['response']['capabilities']!=right['context']['response']['capabilities']):
                issues.append(f'{sequence["id"]}: effective sources or capabilities differ between cache arms')
    return {'schema_version':1,'protocol':PROTOCOL,'mechanical_contracts_passed':not issues,'issues':issues,
        'records':rows,'arm_totals':totals,'sequences':len(entries),'actual_coding_outcomes':'unmeasured',
        'human_outcomes':'unmeasured','fixture_only':report['fixture_only'],
        'qualification':'Exact current observations, source manifests and narrower grants are revalidated after each revision. Lack of a cache hit is not a task failure, and a cache hit is not evidence of completed work.'}


def main(argv=None):
    parser=argparse.ArgumentParser(description=__doc__);commands=parser.add_subparsers(dest='command',required=True)
    for name in ('prepare','run'):
        command=commands.add_parser(name);command.add_argument('--cases',type=Path,default=DEFAULT);command.add_argument('--output',type=Path,required=True)
        if name=='run':
            command.add_argument('--lore-binary',type=Path,required=True);command.add_argument('--provider',choices=('ollama','openai'),required=True);command.add_argument('--model',required=True)
            for option in ('embedding-model','decision-provider','decision-model','generative-base-url','decision-base-url'):command.add_argument('--'+option)
            for option in ('allow-inspection','allow-hosted','allow-checkout-egress'):command.add_argument('--'+option,action='store_true')
            command.add_argument('--max-tokens',type=int,default=6000);command.add_argument('--timeout',type=int,default=3600);command.add_argument('--seed',type=int,default=7)
            bench.add_reasoning_options(command)
    commands.add_parser('assess').add_argument('run',type=Path)
    args=parser.parse_args(argv)
    try:
        result=prepare(args.output,args.cases) if args.command=='prepare' else run(args) if args.command=='run' else assess(args.run)
        print(json.dumps(result,indent=2,sort_keys=True));return 0 if result.get('mechanical_contracts_passed',True) else 2
    except (ValueError,OSError,KeyError,TypeError,subprocess.SubprocessError) as error:
        print(f'Different-task sequence failed: {error}',file=sys.stderr);return 1

if __name__=='__main__':raise SystemExit(main())
