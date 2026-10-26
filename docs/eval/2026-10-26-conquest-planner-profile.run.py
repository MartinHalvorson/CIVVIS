"""Run every fixed board and the four preregistered disposable sample processes."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--probe', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=False)
    selection = Path(__file__).with_name('2026-10-26-conquest-planner-profile.selection.json')
    data = json.loads(selection.read_text())
    for case in data['cases']:
        if case.get('input_file'):
            raw = Path(case['input_file']).read_bytes()
            assert len(raw) == case['input_bytes'] and hashlib.sha256(raw).hexdigest() == case['input_sha256']
    registration = dict(selection_sha256=hashlib.sha256(selection.read_bytes()).hexdigest(),
                        source_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                        probe_sha256=hashlib.sha256(args.probe.read_bytes()).hexdigest(),
                        genome_sha256=hashlib.sha256(Path(data['genome_file']).read_bytes()).hexdigest(),
                        baseline_timeout_seconds=120, profile_timeout_seconds=300,
                        sampler_seconds=1, sampler_interval_milliseconds=1)
    assert registration['genome_sha256'] == data['genome_sha256']
    (args.out/'registration.json').write_text(json.dumps(registration, indent=2)+'\n')
    results = []
    tasks = [('baseline', i, data['measurement_repetitions']) for i in range(len(data['cases']))]
    tasks.extend(('profile', i, data['profile_repetitions']) for i in data['profile_indices'])
    for phase, index, repetitions in tasks:
        case = data['cases'][index]
        result = dict(phase=phase,index=index,repetitions=repetitions,input_status=case['status'],complete=False)
        results.append(result)
        if not case.get('input_file'):
            result['unavailable'] = True
            print(phase,index,'unavailable',flush=True)
            continue
        directory = args.out/f'{phase}-{index:04d}'
        directory.mkdir()
        command = [str(args.probe),str(selection),str(index),str(repetitions)]
        result['argv'] = command
        before = time.monotonic()
        process = None
        try:
            with (directory/'stdout').open('wb') as stdout, (directory/'stderr').open('wb') as stderr:
                process = subprocess.Popen(command,stdout=stdout,stderr=stderr)
                result['pid'] = process.pid
                (directory/'pid.json').write_text(json.dumps(result,indent=2)+'\n')
                if phase=='profile':
                    sampler = ['/usr/bin/sample',str(process.pid),'1','1','-file',str(directory/'sample.txt')]
                    sampled = subprocess.run(sampler,capture_output=True,timeout=20)
                    (directory/'sampler.stdout').write_bytes(sampled.stdout)
                    (directory/'sampler.stderr').write_bytes(sampled.stderr)
                    result.update(sampler_argv=sampler,sampler_exit=sampled.returncode)
                try:
                    result['exit'] = process.wait(timeout=300 if phase=='profile' else 120)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
                    raise
            output = json.loads((directory/'stdout').read_bytes())
            assert result['exit']==0 and output['case']==index
            result.update(complete=True,samples=output['samples'])
        except Exception as error:
            result['error'] = type(error).__name__+': '+str(error)
        finally:
            if process is not None and process.poll() is None:
                process.kill()
                process.wait()
        result['wall_seconds'] = time.monotonic()-before
        (directory/'result.json').write_text(json.dumps(result,indent=2)+'\n')
        print(phase,index,result['complete'],result.get('samples'),result.get('error'),flush=True)
        (args.out/'results.json').write_text(json.dumps(results,indent=2)+'\n')
    (args.out/'results.json').write_text(json.dumps(results,indent=2)+'\n')


if __name__=='__main__':
    main()
