"""Disposable exact-prefix Native response comparison; never use host orders channels."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, required=True)
    parser.add_argument('--baseline', type=Path, required=True)
    parser.add_argument('--candidate', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--selection', type=Path, default=Path(__file__).with_name('2026-10-26-production-choice-memo.native-selection.json'))
    args = parser.parse_args()
    args.out.mkdir(parents=True,exist_ok=False)
    sys.path.insert(0,str(args.repo/'tools'))
    from civ6_ladder import decider_genome
    p = args.selection
    selection = json.loads(p.read_text())
    binaries = {'baseline':args.baseline,'candidate':args.candidate,'baseline-repeat':args.baseline}
    registration = dict(selection_sha256=hashlib.sha256(p.read_bytes()).hexdigest(),
                        source_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                        binary_sha256={k:hashlib.sha256(v.read_bytes()).hexdigest() for k,v in binaries.items()})
    (args.out/'registration.json').write_text(json.dumps(registration,indent=2)+'\n')
    results = []
    for index,case in enumerate(selection['cases']):
        root = args.out/f'{index:04d}'
        root.mkdir()
        result = dict(index=index,tag=case['tag'],turn=case['turn'],complete=False,arms=[])
        try:
            data = (Path(case['directory'])/'events.jsonl').read_bytes()
            assert len(data)==case['input_bytes'] and hashlib.sha256(data).hexdigest()==case['input_sha256']
            for arm,binary in binaries.items():
                directory = root/arm
                directory.mkdir()
                (directory/'events.jsonl').write_bytes(data)
                command = selection['argv_template'].copy()
                command[0] = str(binary)
                command[command.index('--mirror')+1] = str(directory)
                command[command.index('--turn')+1] = str(case['turn'])
                assert '--serve' in command and '--fresh-board' in command
                (directory/'argv.json').write_text(json.dumps(command,indent=2)+'\n')
                done = subprocess.run(command,input=(str(case['turn'])+'\n').encode(),capture_output=True,timeout=300)
                (directory/'stdout').write_bytes(done.stdout)
                (directory/'stderr').write_bytes(done.stderr)
                genome = decider_genome(directory/'stderr')
                record = dict(arm=arm,argv=command,exit=done.returncode,genome=genome,
                              expected_genome_equal=genome==selection['expected_genome'])
                if done.returncode==0:
                    record['payload'] = json.loads(done.stdout)
                record['reasoning'] = [l for l in done.stderr.decode().splitlines() if l.startswith('[why]')]
                result['arms'].append(record)
                (root/'result.json').write_text(json.dumps(result,indent=2)+'\n')
            baseline,candidate,repeat = result['arms']
            result['candidate_payload_equal'] = baseline.get('payload')==candidate.get('payload')
            result['repeat_payload_equal'] = baseline.get('payload')==repeat.get('payload')
            result['all_genomes_equal'] = baseline['genome']==candidate['genome']==repeat['genome']
            result['all_reasoning_equal'] = baseline['reasoning']==candidate['reasoning']==repeat['reasoning']
            result['raw_stdout_equal'] = (root/'baseline/stdout').read_bytes()==(root/'candidate/stdout').read_bytes()==(root/'baseline-repeat/stdout').read_bytes()
            result['complete'] = all(a['exit']==0 and a['expected_genome_equal'] and 'payload' in a for a in result['arms']) and all(result[k] for k in ['candidate_payload_equal','repeat_payload_equal','all_genomes_equal','all_reasoning_equal','raw_stdout_equal'])
            # Original actual prefixes are already sealed in the prior input archive.
            # Keep exact source identities and every response without duplicating them.
            for arm in binaries:
                (root/arm/'events.jsonl').unlink()
        except subprocess.TimeoutExpired as error:
            for name, raw in [('stdout',error.stdout),('stderr',error.stderr)]:
                if raw is not None:
                    (root/('timeout.'+name)).write_bytes(raw)
            result['error'] = 'TimeoutExpired: '+str(error)
        except Exception as error:
            result['error'] = type(error).__name__+': '+str(error)
        (root/'result.json').write_text(json.dumps(result,indent=2)+'\n')
        results.append(result)
        print(index,case['tag'],case['turn'],result['complete'],result.get('error'),flush=True)
    summary = dict(registered_cases=len(selection['cases']),complete_cases=sum(r['complete'] for r in results),
                   incomplete=[dict(index=r['index'],error=r.get('error')) for r in results if not r['complete']])
    (args.out/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
    print(json.dumps(summary),flush=True)


if __name__=='__main__':
    main()
