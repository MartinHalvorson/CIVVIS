"""Reject incorrect or malformed target-feature qualification receipts."""

import unittest

from site_wasm_features import declared_features, qualification_features

HEADER = b'\0asm\1\0\0\0'


def leb(value):
    data = bytearray()
    while value >= 128:
        data.append((value & 127) | 128)
        value >>= 7
    return bytes(data) + bytes([value])


def name(text):
    raw = text.encode()
    return leb(len(raw)) + raw


def custom(payload, section='target_features'):
    body = name(section) + payload
    return b'\0' + leb(len(body)) + body


class WasmFeaturesTests(unittest.TestCase):
    def test_read_positive_and_negative_features_among_other_sections(self):
        module = HEADER + custom(b'irrelevant', 'notes') + custom(
            b'\3+' + name('simd128') + b'-' + name('relaxed-simd') + b'+' + name('bulk-memory'))
        self.assertEqual(declared_features(module),
                         dict(enabled=['bulk-memory', 'simd128'], disabled=['relaxed-simd']))
        receipt = qualification_features(module, True)
        self.assertEqual(receipt['rustc_extra_args'], ['-C', 'target-feature=+simd128'])
        with self.assertRaises(ValueError):
            qualification_features(module, False)

    def test_normal_receipt_has_no_extra_compiler_flags(self):
        module = HEADER + custom(b'\1+' + name('bulk-memory'))
        self.assertEqual(qualification_features(module, False)['rustc_extra_args'], [])
        with self.assertRaises(ValueError):
            qualification_features(module, True)

    def test_invalid_and_ambiguous_metadata_is_rejected(self):
        section = custom(b'\1+' + name('simd128'))
        bad = [b'', b'\0asm\2\0\0\0', HEADER,
               HEADER + b'\0\x80', HEADER + b'\0\xff\xff\xff\xff\x1f',
               HEADER + section[:-1], HEADER + section + section,
               HEADER + custom(b'\1?' + name('simd128')),
               HEADER + custom(b'\2+' + name('simd128') + b'-' + name('simd128')),
               HEADER + custom(b'\1+' + name('')),
               HEADER + custom(b'\0trailing'), HEADER + custom(b'\1+\x01\xff')]
        for module in bad:
            with self.subTest(module=module), self.assertRaises(ValueError):
                declared_features(module)

    def test_long_sections_use_unsigned_leb_lengths(self):
        feature = 'future-' + 'x' * 150
        self.assertEqual(declared_features(HEADER + custom(b'\1+' + name(feature))),
                         dict(enabled=[feature], disabled=[]))


if __name__ == '__main__':
    unittest.main()
