from pathlib import Path
p=Path('evaluation/suite.py');s=p.read_text();a='    if not report.get("lore_binary_sha256"):failures.append("Binary identity missing")';assert s.count(a)==1;s=s.replace(a,a+'\n    if not report.get("provider") or not report.get("model"):failures.append("Configured model identity missing")');p.write_text(s)
p=Path('evaluation/tests/test_suite.py');s=p.read_text();a="if __name__=='__main__':unittest.main()";assert s.count(a)==1;s=s.replace(a,''' def test_missing_model_identity_and_disabled_verification_cannot_pass(self):
  with tempfile.TemporaryDirectory() as temp:
   run=completed(Path(temp)/'a');reviewed(run)
   path=run/'metrics.json';report=json.loads(path.read_text())
   report.pop('model');self.assertFalse(suite.automated_checks(report)[0])
   report['model']='fixture';report['synthesis_verification']=False
   self.assertFalse(suite.automated_checks(report)[0])
   report['synthesis_verification']=True;report['lore_binary_sha256']=None
   self.assertFalse(suite.automated_checks(report)[0])

'''+a);p.write_text(s)
