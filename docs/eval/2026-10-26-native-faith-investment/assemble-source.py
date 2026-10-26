"""Assemble exact historical compile inputs without editing a runtime checkout."""
from pathlib import Path
import argparse, hashlib, json, re, subprocess

parser = argparse.ArgumentParser()
parser.add_argument('--repository', type=Path, required=True)
parser.add_argument('--source', type=Path, required=True)
parser.add_argument('--manifest', type=Path, required=True)
args = parser.parse_args()
revision = '45eb06d2bea6bde2462e092aac6d0ed778a2c367'
args.source.mkdir(exist_ok=False)
paths = subprocess.check_output(['git', 'ls-tree', '-r', '--name-only', revision, '--',
    'src', 'data', 'Cargo.toml', 'Cargo.lock', 'build.rs', '.cargo', 'rust-toolchain.toml'],
    cwd=args.repository, text=True).splitlines()
manifest = {'revision': revision, 'scope': 'Exact tracked source/data and discovered literal compile includes', 'files': {}}

def copy_batch(names):
    result = subprocess.check_output(['git', 'cat-file', '--batch'], cwd=args.repository,
        input=''.join(f'{revision}:{name}\n' for name in names).encode())
    offset = 0
    for name in names:
        end = result.index(b'\n', offset)
        blob, kind, length = result[offset:end].decode().split()
        assert kind == 'blob', name
        length = int(length)
        data = result[end + 1:end + 1 + length]
        assert result[end + 1 + length:end + 2 + length] == b'\n'
        assert hashlib.sha1(b'blob ' + str(length).encode() + b'\0' + data).hexdigest() == blob
        offset = end + 2 + length
        destination = args.source / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(data)
        manifest['files'][name] = {'git_blob': blob, 'sha256': hashlib.sha256(data).hexdigest(), 'bytes': length}
    assert offset == len(result)

copy_batch(paths)
includes = set()
for name in paths:
    if not name.endswith('.rs'):
        continue
    code = (args.source / name).read_text()
    for relative in re.findall(r'include(?:_str|_bytes)?!\(\s*"([^"\n]+)"', code):
        resolved = (args.source / name).parent.joinpath(relative).resolve()
        path = str(resolved.relative_to(args.source.resolve()))
        if path not in manifest['files']:
            includes.add(path)
copy_batch(sorted(includes))
for name, entry in manifest['files'].items():
    assert hashlib.sha256((args.source / name).read_bytes()).hexdigest() == entry['sha256']
manifest['discovered_literal_includes'] = sorted(includes)
args.manifest.write_text(json.dumps(manifest, indent=2) + '\n')
print(json.dumps({'revision': revision, 'exact_blobs': len(manifest['files']), 'discovered_includes': len(includes)}))
