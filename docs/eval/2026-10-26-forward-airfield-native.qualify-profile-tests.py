"""Qualification must preserve registry order and reject every other change."""
import copy,importlib.util,pathlib,unittest
p=pathlib.Path(__file__).with_name('2026-10-26-forward-airfield-native.qualify-profile.py')
spec=importlib.util.spec_from_file_location('qualification',p);module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
GENE='candidate'

class ProfileControls(unittest.TestCase):
    def setUp(self):
        self.base={'treatments':['war','air'],'forced':['zeta','alpha'],'source':'actual','lane':None}
        self.on=copy.deepcopy(self.base)
        self.on['treatments'].insert(1,GENE);self.on['forced'].append(GENE)
    def check(self,on=None,off=None,repeated=None):
        module.validate_genomes(self.base,off if off is not None else self.base,on if on is not None else self.on,repeated if repeated is not None else self.base,GENE)
    def test_registry_order_accepts_only_the_added_candidate(self):
        before=copy.deepcopy(self.on);self.check();self.assertEqual(self.on,before)
    def test_sorting_or_reordering_existing_tags_is_rejected(self):
        value=copy.deepcopy(self.on);value['treatments'].sort()
        with self.assertRaises(ValueError):self.check(on=value)
    def test_duplicate_candidate_or_another_new_flag_is_rejected(self):
        for tag in [GENE,'another']:
            value=copy.deepcopy(self.on);value['forced'].append(tag)
            with self.subTest(tag=tag),self.assertRaises(ValueError):self.check(on=value)
    def test_metadata_change_is_rejected(self):
        value=copy.deepcopy(self.on);value['source']='different'
        with self.assertRaises(ValueError):self.check(on=value)
    def test_off_and_repeat_each_must_reproduce_actual(self):
        value=copy.deepcopy(self.base);value['forced'].reverse()
        for key in ['off','repeated']:
            with self.subTest(key=key),self.assertRaises(ValueError):self.check(**{key:value})

if __name__=='__main__':unittest.main()
