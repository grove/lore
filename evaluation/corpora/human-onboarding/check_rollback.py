#!/usr/bin/env python3
"""Independent transfer checks; never included in participant input."""
import importlib.util
import json
from pathlib import Path
import sys
sys.dont_write_bytecode = True

def main():
    path = Path(sys.argv[1]) / 'src/rollback_gate.py'
    spec = importlib.util.spec_from_file_location('candidate', path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    probes = [
        ('active_signed_production', 'correctness', ('production', True, False, 'active'), True),
        ('withdrawal_overrides_valid_signature', 'constraint', ('production', True, False, 'withdrawn'), False),
        ('fixture_exception_does_not_override_withdrawal', 'constraint', ('staging', False, True, 'withdrawn'), False),
        ('active_staging_fixture_exception', 'constraint', ('staging', False, True, 'active'), True),
        ('production_fixture_still_needs_signature', 'constraint', ('production', False, True, 'active'), False),
        ('pending_is_not_accepted', 'constraint', ('staging', True, True, 'pending'), False),
        ('unknown_scope_denied', 'constraint', ('preview', True, True, 'active'), False),
        ('unknown_authorization_denied', 'constraint', ('production', True, False, 'other'), False),
    ]
    checks=[]
    for identity, kind, args, expected in probes:
        try:
            passed = module.may_rollback(*args) is expected
        except Exception:
            passed = False
        checks.append({'id': identity, 'kind': kind, 'passed': passed})
    print(json.dumps({'schema_version':1,'checks':checks}))

if __name__ == '__main__':
    main()
