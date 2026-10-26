"""Interleaved exact-output comparison with reconstructable raw replies and CPU controls."""
import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import statistics
import subprocess
import time


def digest(data):
    return hashlib.sha256(data).hexdigest()


def split_reply(raw):
    marker = b',"semantic":'
    start = raw.index(marker) + len(marker)
    value = json.loads(raw)
    tail = raw[start:].decode('utf-8')
    parsed, end = json.JSONDecoder().raw_decode(tail)
    semantic = tail[:end].encode('utf-8')
    prefix, suffix = raw[:start], raw[start+len(semantic):]
    assert parsed == value['semantic']
    assert prefix + semantic + suffix == raw
    return value, prefix, semantic, suffix


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--baseline', type=Path, required=True)
    parser.add_argument('--candidate', type=Path, required=True)
    parser.add_argument('--selection', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=False)
    data = json.loads(args.selection.read_text())
    binaries = {'baseline': args.baseline, 'candidate': args.candidate}
    identities = {k: digest(p.read_bytes()) for k,p in binaries.items()}
    registration = dict(selection_sha256=digest(args.selection.read_bytes()),
                        source_sha256=digest(Path(__file__).read_bytes()),
                        binaries=identities, timeout_seconds=300,
                        genome_sha256=digest(Path(data['genome_file']).read_bytes()))
    assert registration['genome_sha256'] == data['genome_sha256']
    (args.out/'registration.json').write_text(json.dumps(registration,indent=2)+'\n')
    available = []
    for index, case in enumerate(data['cases']):
        if case.get('input_file'):
            raw = Path(case['input_file']).read_bytes()
            assert len(raw) == case['input_bytes'] and digest(raw) == case['input_sha256']
            available.append(index)
    canonical = {}
    results = []
    def run(phase, round_number, index, label, binary_arm, repetitions):
        directory = args.out/f'{phase}-{round_number:02d}-{index:04d}-{label}'
        directory.mkdir()
        command = [str(binaries[binary_arm]),str(args.selection),str(index),str(repetitions)]
        record = dict(phase=phase,round=round_number,index=index,label=label,binary_arm=binary_arm,
                      argv=command,complete=False,load_before=os.getloadavg())
        before = time.monotonic()
        raw_stdout = None
        try:
            done = subprocess.run(command,capture_output=True,timeout=300)
            raw_stdout = done.stdout
            (directory/'stderr').write_bytes(done.stderr)
            record.update(exit=done.returncode,stdout_bytes=len(done.stdout),stdout_sha256=digest(done.stdout),
                          stderr_sha256=digest(done.stderr))
            if done.returncode != 0:
                (directory/'stdout-failed.gz').write_bytes(gzip.compress(done.stdout,mtime=0))
                raise ValueError('probe returned nonzero')
            value, prefix, semantic, suffix = split_reply(done.stdout)
            if index not in canonical:
                assert phase == 'qualification' and binary_arm == 'baseline'
                canonical[index] = semantic
                (args.out/f'case-{index:04d}.semantic.json.gz').write_bytes(gzip.compress(semantic,mtime=0))
            if semantic != canonical[index]:
                (directory/'stdout-different.gz').write_bytes(gzip.compress(done.stdout,mtime=0))
                raise ValueError('complete raw semantic bytes differ')
            assert value['case'] == index and len(value['samples']) == repetitions
            # Canonical bytes plus exact per-run prefix/suffix reconstruct every stdout byte.
            record.update(complete=True,samples=value['samples'],semantic_sha256=digest(semantic),
                          stdout_prefix=prefix.decode('utf-8'),stdout_suffix=suffix.decode('utf-8'))
            reconstructed = record['stdout_prefix'].encode()+canonical[index]+record['stdout_suffix'].encode()
            assert len(reconstructed) == record['stdout_bytes'] and digest(reconstructed) == record['stdout_sha256']
        except subprocess.TimeoutExpired as error:
            for name, raw in [('stdout',error.stdout),('stderr',error.stderr)]:
                if raw is not None:
                    (directory/(name+'-timeout.gz')).write_bytes(gzip.compress(raw,mtime=0))
            record['error'] = 'TimeoutExpired: '+str(error)
        except Exception as error:
            if raw_stdout is not None:
                (directory/'stdout-error.gz').write_bytes(gzip.compress(raw_stdout,mtime=0))
            record['error'] = type(error).__name__+': '+str(error)
        record.update(wall_seconds=time.monotonic()-before,load_after=os.getloadavg())
        (directory/'result.json').write_text(json.dumps(record,indent=2)+'\n')
        results.append(record)
        (args.out/'results.json').write_text(json.dumps(results,indent=2)+'\n')
        print(phase,round_number,index,label,record['complete'],record.get('error'),flush=True)
        return record
    for index in available:
        for arm in ['baseline','candidate']:
            run('qualification',0,index,arm,arm,1)
    qualified = all(r['complete'] for r in results)
    if qualified:
        for phase, rounds in [('control',data['control_rounds']),('comparison',data['comparison_rounds'])]:
            for round_number in range(rounds):
                for index in available:
                    labels = ['baseline','candidate'] if (round_number+index)%2==0 else ['candidate','baseline']
                    for label in labels:
                        run(phase,round_number,index,label,'baseline' if phase=='control' else label,data['repetitions'])
    final = dict(registered_cases=len(data['cases']),available=len(available),
                 unavailable=[i for i in range(len(data['cases'])) if i not in available],
                 qualified=qualified,complete=all(r['complete'] for r in results),
                 errors=[{k:r[k] for k in ['phase','round','index','label','error'] if k in r} for r in results if not r['complete']],
                 rounds={})
    for phase in ['control','comparison']:
        readings = []
        for number in range(data['control_rounds'] if phase=='control' else data['comparison_rounds']):
            group = [r for r in results if r['phase']==phase and r['round']==number]
            if len(group)!=2*len(available) or not all(r['complete'] for r in group):
                continue
            totals = {label:sum(s['process_cpu_ticks'] for r in group if r['label']==label for s in r['samples']) for label in ['baseline','candidate']}
            readings.append(dict(round=number,**totals,change_pct=100*(totals['candidate']/totals['baseline']-1)))
        final['rounds'][phase] = dict(readings=readings,median_change_pct=statistics.median(r['change_pct'] for r in readings) if readings else None)
    for name,path in binaries.items():
        assert digest(path.read_bytes()) == identities[name]
    (args.out/'summary.json').write_text(json.dumps(final,indent=2)+'\n')
    print(json.dumps(final,indent=2),flush=True)


if __name__=='__main__':
    main()
