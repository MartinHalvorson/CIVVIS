"""Controls for lossless splitting of actual compact probe JSON replies."""
import importlib.util
import json
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('comparison',Path(__file__).with_name('2026-10-26-production-choice-memo.compare.py'))
comparison = importlib.util.module_from_spec(spec)
spec.loader.exec_module(comparison)


class ReplyControls(unittest.TestCase):
    def test_semantic_middle_and_unicode(self):
        value = {'case':43,'samples':[{'process_cpu_ticks':1}],
                 'semantic':{'city':'Panamá','nested':[{'v':[1,2]}]},'turn':200}
        raw = (json.dumps(value,ensure_ascii=False,separators=(',',':'))+'\n').encode()
        parsed,prefix,semantic,suffix = comparison.split_reply(raw)
        self.assertEqual(parsed,value)
        self.assertEqual(json.loads(semantic),value['semantic'])
        self.assertEqual(prefix+semantic+suffix,raw)
        self.assertEqual(suffix,b',"turn":200}\n')

    def test_literal_numbers_and_escape_bytes_survive(self):
        raw = b'{"case":0,"semantic":{"negative":-0.0,"small":1e-09,"s":"a\\nb"}}\n'
        _,prefix,semantic,suffix = comparison.split_reply(raw)
        self.assertEqual(semantic,b'{"negative":-0.0,"small":1e-09,"s":"a\\nb"}')
        self.assertEqual(prefix+semantic+suffix,raw)

    def test_malformed_or_missing_payload_is_rejected(self):
        for raw in [b'{"case":0}\n',b'{"case":0,"semantic":{broken}}\n']:
            with self.subTest(raw=raw),self.assertRaises((ValueError,AssertionError)):
                comparison.split_reply(raw)


if __name__=='__main__':
    unittest.main()
