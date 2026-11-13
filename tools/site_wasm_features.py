#!/usr/bin/env python3
"""Read declared WebAssembly target features for an opt-in qualification receipt.

This reads metadata, not an instruction census or a browser compatibility test.
The normal and experimental builds still need their full gameplay comparisons.
"""

from __future__ import annotations


def declared_features(module: bytes) -> dict[str, list[str]]:
    if module[:8] != b'\0asm\1\0\0\0':
        raise ValueError('not a version-one WebAssembly module')

    def u32(pos: int, end: int) -> tuple[int, int]:
        value = 0
        for shift in range(0, 35, 7):
            if pos >= end:
                raise ValueError('truncated unsigned LEB')
            byte = module[pos]
            pos += 1
            if shift == 28 and byte > 15:
                raise ValueError('unsigned LEB overflow')
            value |= (byte & 127) << shift
            if byte < 128:
                return value, pos
        raise ValueError('unsigned LEB overflow')

    def name(pos: int, end: int) -> tuple[str, int]:
        size, pos = u32(pos, end)
        if pos + size > end:
            raise ValueError('truncated feature name')
        return module[pos:pos + size].decode('utf-8'), pos + size

    result: dict[str, list[str]] = {'enabled': [], 'disabled': []}
    found = False
    pos = 8
    while pos < len(module):
        section = module[pos]
        size, body = u32(pos + 1, len(module))
        end = body + size
        if end > len(module):
            raise ValueError('truncated module section')
        pos = end
        if section != 0:
            continue
        custom, body = name(body, end)
        if custom != 'target_features':
            continue
        if found:
            raise ValueError('duplicate target_features section')
        found = True
        count, body = u32(body, end)
        seen = set()
        for _ in range(count):
            if body >= end or module[body] not in (43, 45):
                raise ValueError('invalid target feature prefix')
            key = 'enabled' if module[body] == 43 else 'disabled'
            feature, body = name(body + 1, end)
            if not feature or feature in seen:
                raise ValueError('empty or duplicate target feature')
            seen.add(feature)
            result[key].append(feature)
        if body != end:
            raise ValueError('trailing target feature bytes')
    if not found:
        raise ValueError('target_features metadata missing')
    return {key: sorted(value) for key, value in result.items()}


def qualification_features(module: bytes, experimental_simd128: bool) -> dict:
    features = declared_features(module)
    if ('simd128' in features['enabled']) != experimental_simd128:
        raise ValueError('declared SIMD differs from the requested qualification variant')
    return dict(experimental_simd128=experimental_simd128,
                rustc_extra_args=['-C', 'target-feature=+simd128'] if experimental_simd128 else [],
                compiler_scope='extra flags passed only to the final root-crate rustc invocation',
                declared_target_features=features)
