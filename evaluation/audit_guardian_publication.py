#!/usr/bin/env python3
"""Read-only content/provenance audit of the exact retained synthetic guardian bundle.

No extraction, mutation, model invocation, network access, or subprocess execution.
The only executed repository function is the reviewed literal-only fixture build()
AST, in memory, after requiring its exact reviewed SHA-256. The module-level
writer is excluded. Repository paths derive from this file, not the caller's
working directory. Historical scratch paths are compared only as captured
metadata. This is not a syscall audit.
"""
import ast,copy,hashlib,json,re,stat,zipfile
from pathlib import Path,PurePosixPath
from collections import Counter,defaultdict
repo=Path(__file__).resolve().parents[1]
# Historical path strings are checked as metadata; this location is never opened.
captured_task_root=PurePosixPath('/workspace/scratch/6f5315c44a1a')
folder=repo/'evaluation/results/guardian-debug-2026-10-10'
fixture_path=repo/'evaluation/corpora/guardian/events.json'
generator=repo/'evaluation/corpora/guardian/generate_fixture.py'
raw_fixture=fixture_path.read_bytes(); fixture=json.loads(raw_fixture)
sha=lambda b:hashlib.sha256(b).hexdigest()
canonical=lambda x:json.dumps(x,sort_keys=True,separators=(',',':')).encode()
digest=lambda x:sha(canonical(x))
summary=json.loads((folder/'summary.json').read_text())
failures=[]; counts=Counter(); all_strings=set(); string_locations=defaultdict(set); all_keys=Counter()
def check(ok,code,location=''):
 if not ok: failures.append({'check':code,'location':location})
def unique_object(pairs):
 d={}
 for k,v in pairs:
  if k in d: raise ValueError('duplicate JSON key')
  d[k]=v
 return d
def parse(raw):
 return json.loads(raw.decode('utf-8'),object_pairs_hook=unique_object,parse_constant=lambda v:(_ for _ in ()).throw(ValueError('nonfinite JSON number')))
REVIEWED_GENERATOR_SHA256='791ad222954384a16962826c79af493fb4822ba518a1919a4ff0420e3f35f9c9'
generator_bytes=generator.read_bytes()
if sha(generator_bytes)!=REVIEWED_GENERATOR_SHA256:
 print(json.dumps({'failure_count':1,'failures':[{'check':'reviewed_generator_sha256','location':'evaluation/corpora/guardian/generate_fixture.py'}]}))
 raise SystemExit(2)
# Parse and execute the exact checked bytes, never a second read of the generator.
tree=ast.parse(generator_bytes.decode('utf-8'))
fn=next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='build')
ns={}
exec(compile(ast.Module(body=[fn],type_ignores=[]),str(generator),'exec'),ns)
check((json.dumps(ns['build'](),ensure_ascii=False,indent=2)+'\n').encode()==raw_fixture,'fixture_regenerated_in_memory_identically')
check(sha(raw_fixture)==summary['events_manifest_sha256'],'fixture_hash_bound')
expected_states={}; fixture_events={}; texts=defaultdict(set); record_values=defaultdict(set); fixture_strings=set()
def getstrings(v):
 if isinstance(v,dict):
  for x in v.values(): getstrings(x)
 elif isinstance(v,list):
  for x in v:getstrings(x)
 elif isinstance(v,str):fixture_strings.add(v)
getstrings(fixture)
def fingerprint(files):
 fs={name:sha(spec['text'].encode()) for name,spec in sorted(files.items())}
 return {'sha256':digest(fs),'files_sha256':fs,'file_count':len(fs)}
for project in fixture['projects']:
 pid=project['id']; current=copy.deepcopy(project['initial_files'])
 for revision,event in [(project['initial_revision'],None)]+[(e['revision'],e) for e in project['events']]:
  if event:
   fixture_events[event['id']]=event
   for name,spec in event['files'].items():
    if spec is None:del current[name]
    else:current[name]=copy.deepcopy(spec)
  recs=[]
  for name,spec in sorted(current.items()):
   texts[(pid,name)].add(spec['text'])
   for record in spec['records']:
    recs.append(dict(record,file=name))
    for key,value in record.items():record_values[(pid,key)].add(value)
  expected_states[(pid,revision)]={'revision':revision,'source_root':f'snapshots/{pid}/{revision}','source_fingerprint':fingerprint(current),'capture_records':recs,'event':event}
check(len(expected_states)==63 and len(fixture_events)==60,'complete_fixture_inventory')
allowed_abs={str(captured_task_root/'evidence/lore-baseline'),str(captured_task_root/'evidence/lore-guardian-final')}
allowed_abs|={str(captured_task_root/f'guardian-compiled-v3/workspaces/{pid}/docs') for pid in ['payments','ledger','releases']}
expected_cfg={}
for pid in ['payments','ledger','releases']:
 cfg={'models':{'generative':{'enabled':False}},'project':{'name':f'guardian-debug-{pid}'},'output':{'state_dir':'.lore','wiki_dir':'wiki'},'sources':{'roots':[{'id':'docs','path':'docs'}],'exclude':['**/target/**','**/node_modules/**']},'imports':[],'privacy':{'local_only':True,'allow_checkout_egress':False}}
 expected_cfg[pid]=sha((json.dumps(cfg,indent=2,sort_keys=True)+'\n').encode())
model_keys={'model_calls','total_model_calls','guard_model_calls','compilation_model_calls','inference_calls'}
source_fields={'quote','statement','subject','scope','topic','topic_title','effective_at'}
sensitive=re.compile(r'(password|secret|credential|environment|environ|api.?key|access.?token|authorization|private.?key|^stdout$|^stderr$)',re.I)
actual_abs=set(); commands=Counter(); source_field_counts=Counter(); evidence_triplets=set(); current_resolvers={}
def visit(v,path=(),pid=None):
 if isinstance(v,dict):
  for k,x in v.items():
   all_keys[k]+=1
   check(not sensitive.search(k),'sensitive_payload_field','.'.join(path+(k,)))
   if k in model_keys:
    check(x==0,'nonzero_reported_model_call','.'.join(path+(k,)));counts['zero_model_count_fields']+=1
   if k in ['stdout','stderr','environment','env','credentials','api_key','password','secret']:
    check(x in (None,{},[],''),'forbidden_raw_payload','.'.join(path+(k,)))
   if k in ['imported_evidence','native_snapshots','imported_changes']:
    check(x in (None,[],{},0),'native_or_imported_payload','.'.join(path+(k,)))
   if k=='origin':check(x is None,'nonlocal_evidence_origin','.'.join(path+(k,)))
   if k in source_fields and pid is not None and isinstance(x,str):
    check(x in record_values[(pid,k)],'source_field_not_in_project_fixture','.'.join(path+(k,)));source_field_counts[k]+=1
   if k=='quote':
    check(x in fixture_strings,'quote_not_authored_fixture','.'.join(path+(k,)))
   if k=='arguments':
    check(isinstance(x,list) and all(isinstance(a,str) for a in x),'argument_shape','.'.join(path))
    command=tuple(x)
    valid=command==('baseline','save','transition','--replace') or command==('changes','--since','transition','--max-tokens','8000') or command==('guard','--since','transition','--max-tokens','8000','--no-cache','--no-inspect') or (len(command)==2 and command[0]=='evidence' and re.fullmatch(r'(?:ev|xrel|xev)_[0-9a-f]+',command[1]))
    check(valid,'unrecognized_captured_arguments','.'.join(path));commands[command[0] if command else 'empty']+=1
   visit(x,path+(k,),pid)
  if 'response_sha256' in v:
   response=v.get('response')
   check(v['response_sha256']==digest(response) if response is not None else v['response_sha256'] is None,'response_digest','.'.join(path));counts['responses_checked']+=1
  if 'excerpt' in v:
   check(pid is not None,'evidence_project_scope','.'.join(path))
   if pid is not None:
    binding=v if 'observed_path' in v else current_resolvers.get(v.get('id'),{})
    if binding is not v:
     check(bool(binding) and all(v.get(k)==binding.get(k) for k in ['id','source_revision_id','excerpt','line_start','line_end','material']),'compact_evidence_matches_bound_original','.'.join(path))
     check(v.get('source')=='docs:'+binding.get('observed_path',''),'compact_source_locator','.'.join(path));counts['compact_evidence_resolver_bindings']+=1
    name='docs/'+binding.get('observed_path','')
    quote=v['excerpt']; before=binding.get('context_before','');after=binding.get('context_after','')
    candidates=texts[(pid,name)]
    check(quote in record_values[(pid,'quote')],'evidence_quote_exact_fixture','.'.join(path))
    check(any(before+quote+after==s for s in candidates),'evidence_complete_context_fixture','.'.join(path))
    check(any('\n'.join(s.splitlines()[v['line_start']-1:v['line_end']])==quote for s in candidates),'evidence_line_span_fixture','.'.join(path))
    check(binding.get('root')=='docs' and binding.get('root_path')==str(captured_task_root/f'guardian-compiled-v3/workspaces/{pid}/docs'),'evidence_source_root','.'.join(path))
    counts['evidence_objects_checked']+=1;evidence_triplets.add((pid,name,quote,before,after))
 elif isinstance(v,list):
  for x in v:visit(x,path+('*',),pid)
 elif isinstance(v,str):
  all_strings.add(v);string_locations[v].add('.'.join(path[-4:]))
  if v.startswith('/'):
   actual_abs.add(v);check(v in allowed_abs,'unrecognized_absolute_path','.'.join(path))
  check(not re.search(r'\b(?:https?|ssh|sftp|ftp|file)://',v),'external_url_in_capture','.'.join(path))
  check('\x00' not in v,'embedded_nul','.'.join(path))
rules={
 'private_key':rb'-----BEGIN (?:RSA |EC |OPENSSH |DSA |PGP )?PRIVATE KEY',
 'provider_token':rb'(?<![A-Za-z0-9])(?:sk-(?:proj-|ant-)?[A-Za-z0-9_-]{20,}|gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|xox[baprs]-[A-Za-z0-9-]{20,})',
 'aws_key':rb'(?<![A-Z0-9])(?:AKIA|ASIA)[A-Z0-9]{16}(?![A-Z0-9])',
 'jwt':rb'eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}',
 'url_password':rb'[a-z]+://[^\s/"<>:@]+:[^\s/"<>@]+@',
 'credential_assignment':rb'(?i)(?:api[_-]?key|client[_-]?secret|access[_-]?token|password|authorization)["\s]*[:=]["\s]*(?!null|false|true)[A-Za-z0-9_/-]{12,}',
 'email_address':rb'[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}'
}
entry_hashes={}; roots=['baseline','candidate','short-candidate-payments','short-baseline-payments']
with zipfile.ZipFile(folder/'captures.zip') as z:
 infos=z.infolist();check(len({i.filename for i in infos})==len(infos),'unique_archive_names')
 check(z.testzip() is None,'all_zip_member_crcs')
 check(sha((folder/'captures.zip').read_bytes())==summary['capture_archive']['sha256'],'archive_bound_to_summary')
 expected_names={'controls/existing-file.json','controls/created-file.json'}
 data={}
 for i in infos:
  name=i.filename; p=PurePosixPath(name)
  check(not p.is_absolute() and '..' not in p.parts and '\\' not in name and ':' not in name,'safe_zip_path',name)
  check(stat.S_ISREG(i.external_attr>>16) and not (i.external_attr>>16)&0o111,'regular_nonexecutable_member',name)
  check(not i.flag_bits&1 and not i.extra and not i.comment,'no_encryption_extra_or_comment',name)
  check(name.endswith('.json') and i.file_size<=500000,'bounded_json_entry',name)
  raw=z.read(name);data[name]=parse(raw);entry_hashes[name]=sha(raw)
  check(raw[:16] not in (b'SQLite format 3\x00',) and not raw.startswith((b'\x7fELF',b'MZ',b'PK\x03\x04',b'\xca\xfe\xba\xbe')),'no_binary_signature',name)
  for rule,pat in rules.items():check(re.search(pat,raw) is None,'secret_pattern_'+rule,name)
 check(not z.comment,'no_archive_comment')
 for dirname in roots:
  runname=dirname+'/run.json'; aname=dirname+'/assessment.json'
  expected_names|={runname,aname}
  run=data[runname];assessment=data[aname]
  check(entry_hashes[runname]==summary['runs'][dirname]['run_sha256'],'run_hash_bound',dirname)
  check(assessment['run_sha256']==entry_hashes[runname],'assessment_bound_to_run',dirname)
  check(run['compiler_mode']=='synthetic_explicit_records' and run['fixture_only'] and not run['held_out'],'synthetic_run_mode',dirname)
  check(not any(run['permissions'].values()) and not run['audit_requested'],'all_permissions_disabled',dirname)
  check(run['events_manifest_sha256']==sha(raw_fixture) and run['prepared_sha256']==summary['prepared_sha256'],'prepared_and_fixture_bindings',dirname)
  check(assessment['total_model_calls']==0 and not assessment['independent_runtime_audit_complete'],'reported_zero_calls_and_no_fabricated_audit',dirname)
  for project in run['prepared']['projects']:
   pid=project['id']
   check(project['source_identity']=={'kind':'synthetic_debug_fixture','version':'guardian-events-v1'},'synthetic_source_identity',dirname)
   for state in project['states']:
    check(state==expected_states[(pid,state['revision'])],'prepared_state_exact_synthetic',dirname+':'+state['revision']);counts['embedded_prepared_states_checked']+=1
  visit(run,('run',),None);visit(assessment,('assessment',),None)
  for entry in run['records']:
   name=dirname+'/'+entry['path']; expected_names.add(name)
   check(entry_hashes.get(name)==entry['sha256'],'event_hash_bound',name);counts['event_hashes_checked']+=1
   capture=data[name];pid=capture['project'];eid=capture['event_id']
   current_resolvers={entry['evidence_id']:entry['response'] for entry in capture['resolution'] if isinstance(entry.get('response'),dict)}
   check(capture['event']==fixture_events[eid],'event_payload_exact_fixture',name)
   for side in ['before','after']:
    state=expected_states[(pid,capture[side+'_revision'])]
    check(capture['expected_'+side+'_sources']==state['source_fingerprint'],'expected_state_hashes_from_synthetic',name)
   for phase in ['before','query','after_queries']:
    observed=capture['sources_'+phase];check(observed['sha256']==digest(observed['files_sha256']) and observed['file_count']==len(observed['files_sha256']),'observed_fingerprint_digest',name)
    for path,h in observed['files_sha256'].items():
     normal='docs/history.md' if path=='docs/.rsync-tmp/history.md' else path
     check(any(sha(s.encode())==h for s in texts[(pid,normal)]),'observed_file_bytes_match_synthetic_history',name+':'+path);counts['observed_source_hashes_checked']+=1
    registry=capture['registry_'+phase]
    check(registry['configuration_sha256']==expected_cfg[pid],'generated_disabled_config_hash',name);counts['disabled_configuration_hashes_checked']+=1
    check(not registry['immutable_rows']['native_snapshots'],'no_native_snapshot_inputs',name)
   visit(capture,(name,),pid)
 check(set(data)==expected_names,'all_members_accounted_for')
 for n in ['controls/existing-file.json','controls/created-file.json']:
  c=data[n];check(set(c)=={'protocol','lore_invocations','original_sha256','observations','restored','restored_sha256'},'control_schema',n)
  check(c['lore_invocations']==0 and not c['restored'] and c['restored_sha256'] is None,'control_no_lore_or_payload',n)
  check(all(set(o) in ({'elapsed_seconds','exists'},{'elapsed_seconds_after_deletion','exists'}) and o['exists'] is False for o in c['observations']),'control_boolean_observation_only',n);visit(c,('control',),None)
product_code='\n'.join(p.read_text() for p in (repo/'src').rglob('*.rs'))+'\n'+(repo/'evaluation/guardian_longitudinal.py').read_text()
generated=set()
reason='The configured model is unavailable, disabled, or disallowed; this briefing contains deterministic guidance without fresh reasoning.'
for pid in ['payments','ledger','releases']:
 task=f'Assess accepted constraints and current implementation for {pid}; recommend a concrete behavior-preserving response to consequential discrepancies'
 generated|={task,f'Start {task} with a reversible implementation that preserves the retained project rules.',f'The requested outcome for {task} is implemented without silently changing the retained rules.',f'Fresh model assessment is unavailable: {reason}'}
 for statement in record_values[(pid,'statement')]:generated.add(f'Determine whether {task} can preserve the retained rule: {statement}')
generated.add('Cross-source interpretations changed. Run '+chr(96)+'lore changes --since transition'+chr(96)+' for the included before-and-after source payloads; this advisory assesses current shared guidance. A withdrawn interpretation does not reverse a source claim, establish source agreement or verify runtime behavior.')
for kind in ['constraint','reported_outcome','proposal']:
 for life in ['accepted','completed','proposed','rejected']:
  for support in ['current_documentary_support','historical_only']:
   generated.add(f'Documented {kind} ({life}); {support}. This is source documentation, not checkout verification.')
generated|={f'Lifecycle is {life}; adoption must not be inferred.' for life in ['proposed','rejected']}
generated|={'created-then-deleted-external-writer-control','external-source-writer-control'}
allowed_relative={p for _,p in texts}|{p[5:] for _,p in texts if p.startswith('docs/')}
allowed_relative|={'docs:'+p[5:] for _,p in texts if p.startswith('docs/')}
allowed_relative|={s['source_root'] for s in expected_states.values()}|{f'events/{eid}/capture.json' for eid in fixture_events}
unknown=[]
for value in all_strings:
 if value in fixture_strings or value in product_code or value in generated or value in allowed_abs or value in allowed_relative or value in entry_hashes:continue
 if any(value in t for ts in texts.values() for t in ts):continue
 if re.fullmatch(r'(?:blake3:)?[0-9a-f]{16,128}|[a-z]+_[0-9a-f]{16,128}|[0-9]{4}-[0-9]{2}-[0-9]{2}(?:T| )[0-9:.]+(?:Z|\+00:00)?',value):continue
 if re.fullmatch(r'same (?:subject|topic) as ku_[0-9a-f]{32}: (?:payment retry|ledger admission|artifact admission|payments|ledger|releases)',value):continue
 unknown.append({'sha256':sha(value.encode()),'length':len(value),'locations':sorted(string_locations[value])[:3]})
result={'archive_sha256':summary['capture_archive']['sha256'],'archive_bytes':(folder/'captures.zip').stat().st_size,'member_count':len(infos),'uncompressed_bytes':sum(i.file_size for i in infos),'counts':dict(counts),'source_field_counts':dict(source_field_counts),'unique_evidence_triplets':len(evidence_triplets),'unique_strings_classified':len(all_strings)-len(unknown),'unique_strings_remaining':unknown,'absolute_paths_count':len(actual_abs),'commands':dict(commands),'failures':failures[:30],'failure_count':len(failures),'entry_digest_manifest_sha256':digest(entry_hashes),'generator_sha256':sha(generator_bytes),'fixture_sha256':sha(raw_fixture)}
# Check sidecars against archive observations without changing evidence.
expected_failures=[]
for dirname in roots:
 run=data[dirname+'/run.json']
 for entry in run['records']:
  c=data[dirname+'/'+entry['path']]
  for stage in ['sources_before','sources_query','sources_after_queries']:
   exp=c['expected_before_sources'] if stage=='sources_before' else c['expected_after_sources']
   actual=c[stage]
   if exp!=actual:
    a=actual['files_sha256'];e=exp['files_sha256']
    expected_failures.append({'run':dirname,'event_id':c['event_id'],'stage':stage,'missing_paths':sorted(set(e)-set(a)),'unexpected_paths_sha256':{k:a[k] for k in sorted(set(a)-set(e))},'changed_existing_paths':sorted(k for k in set(a)&set(e) if a[k]!=e[k])})
recorded_failures=parse((folder/'source-integrity-failures.json').read_bytes())
check(sorted(canonical(x) for x in recorded_failures)==sorted(canonical(x) for x in expected_failures),'source_failure_sidecar_exact_reconstruction')
sidecars={}
for filename in ['README.md','summary.json','source-integrity-failures.json']:
 raw=(folder/filename).read_bytes();raw.decode('utf-8')
 for rule,pat in rules.items():check(re.search(pat,raw) is None,'sidecar_secret_pattern_'+rule,filename)
 sidecars[filename]={'sha256':sha(raw),'bytes':len(raw)}
# Account for every ZIP byte: no leading/trailing or inter-member hidden payload.
import struct
archive_raw=(folder/'captures.zip').read_bytes()
position=0
for i in infos:
 check(i.header_offset==position,'zip_local_members_contiguous',i.filename)
 header=struct.unpack_from('<4s5H3I2H',archive_raw,position)
 signature,version,flags,compression,mtime,mdate,crc,compressed,size,namelen,extralen=header
 check(signature==b'PK\x03\x04' and flags==0 and extralen==0,'plain_local_zip_header',i.filename)
 check(compressed==i.compress_size and size==i.file_size and crc==i.CRC,'local_zip_header_matches_inventory',i.filename)
 position+=30+namelen+extralen+compressed
central_bytes=sum(46+len(i.filename.encode())+len(i.extra)+len(i.comment) for i in infos)
check(position+central_bytes+22==len(archive_raw) and archive_raw[-22:-18]==b'PK\x05\x06','no_hidden_zip_prefix_gap_or_trailer')
result['source_integrity_failure_rows_reconstructed']=len(expected_failures)
result['sidecar_files']=sidecars
result['archive_hidden_payload_checks_pass']=not any(x['check'].startswith(('zip_','plain_local','local_zip','no_hidden')) for x in failures)
result['failures']=failures[:30];result['failure_count']=len(failures)
result['controls_preimages_retained']=False
result['audit_script_sha256']=sha(Path(__file__).read_bytes())
print(json.dumps(result,indent=2))
if failures or unknown:
 raise SystemExit(2)



