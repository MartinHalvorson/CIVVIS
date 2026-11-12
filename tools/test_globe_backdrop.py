"""Pure automatic backdrop guards plus an explicit native Canvas fixture.

Unittest discovery exercises the actual JS helpers without a browser/game.
Run `python3 tools/test_globe_backdrop.py --native-canvas` for the separate
native pixel-copy integration fixture; it starts no game or engine.
"""
from pathlib import Path
import importlib.util
import json
import re
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


class BackdropGuards(unittest.TestCase):
    def test_actual_js_cache_guards(self):
        result = subprocess.run(['node', str(ROOT / 'web/globe_backdrop_test.mjs')],
                                check=True, capture_output=True, text=True)
        self.assertIn('10 pure backdrop tests passed', result.stdout)

    def test_helper_script_loads_before_the_renderer(self):
        page = (ROOT / 'web/index.html').read_text()
        self.assertLess(page.index('<script src="/assets/app_backdrop.js">'),
                        page.index('<script src="/assets/app.js">'))


def native_pixel_fixture():
    helpers = (ROOT / 'web/assets/app_backdrop.js').read_text()
    spec = importlib.util.spec_from_file_location('backdrop_fixture_browser', ROOT / 'beta/verify.py')
    browser = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(browser)
    chrome = browser.find_chrome()
    script=r'''
    const cv=document.querySelector('canvas');let cx=cv.getContext('2d');
    let DPR=1,MAPW=137,MAPH=91;
    const cam={x:1,y:2,scale:3,rot:4,tilt:5}, state={seed:7}, PLANET={};
    let PLANET_HIT_FRAME={cells:['fixture'],inkOnly:false};
    function applyPlanetGroundTilt(y){cx.transform(1,.03,.08,.92,0,y*.02);}
    '''+helpers+r'''
    function ground(){
     cx.setTransform(DPR,0,0,DPR,0,0);cx.globalAlpha=1;cx.globalCompositeOperation='source-over';
     cx.shadowBlur=0;cx.filter='none';cx.fillStyle='#000';cx.fillRect(0,0,MAPW,MAPH);
     cx.save();applyPlanetGroundTilt(37);
     const ramp=cx.createLinearGradient(0,0,MAPW,MAPH);ramp.addColorStop(0,'#48709c');ramp.addColorStop(1,'#1e3923');
     cx.fillStyle=ramp;cx.beginPath();cx.arc(65,43,39,0,7);cx.fill();
     cx.save();cx.beginPath();cx.ellipse(63,45,35,19,.15,0,7);cx.clip();
     cx.globalAlpha=.47;cx.fillStyle='#9a7b34';cx.fillRect(26,22,70,39);cx.restore();
     cx.shadowColor='#b7ffdb';cx.shadowBlur=3;cx.shadowOffsetY=2;
     cx.strokeStyle='#ffd47b';cx.lineWidth=1.7;cx.setLineDash([3,2]);cx.lineDashOffset=1.3;
     cx.beginPath();cx.moveTo(14,24);cx.lineTo(111,57);cx.stroke();
     cx.shadowBlur=0;cx.shadowOffsetY=0;cx.fillStyle='#e3dab8';cx.strokeStyle='#333';
     cx.font='13px sans-serif';cx.textAlign='center';cx.textBaseline='middle';cx.globalAlpha=.76;
    }
    function overlay(){cx.fillText('test',65,44);cx.beginPath();cx.arc(79,51,9,0,7);cx.fill();cx.stroke();cx.restore();}
    const rows=[];
    for(const ratio of [1,1.5,2,3]){
     DPR=ratio;cv.width=Math.round(MAPW*DPR);cv.height=Math.round(MAPH*DPR);
     ground();savePlanetBackdrop(planetBackdropView(0,0),[]);overlay();
     const expected=cx.getImageData(0,0,cv.width,cv.height).data;
     // Replay has different entry styles, as it does after a foreground paint.
     cx.globalAlpha=.21;cx.fillStyle='#f00';cx.shadowBlur=12;cx.lineWidth=7;cx.setLineDash([8,1]);
     paintPlanetBackdrop(planetBackdrop,37);overlay();
     const actual=cx.getImageData(0,0,cv.width,cv.height).data;
     let differing=0,maxDelta=0;for(let i=0;i<expected.length;i++)if(actual[i]!==expected[i]){differing++;maxDelta=Math.max(maxDelta,Math.abs(actual[i]-expected[i]));}
     rows.push({DPR,channels:actual.length,differing,maxDelta,hits:planetBackdrop.hits});
    }
    document.querySelector('output').textContent=JSON.stringify({rows});
    '''

    with tempfile.TemporaryDirectory(prefix='civvis-pure-canvas-') as tmp:
        directory = Path(tmp)
        page = directory / 'fixture.html'
        page.write_text('<!doctype html><meta charset="utf-8"><canvas></canvas>'
                        '<output></output><script>' + script + '</script>')
        observed = subprocess.run(
            [chrome, '--headless=new', '--disable-gpu', '--no-first-run', '--no-default-browser-check',
             '--user-data-dir=' + str(directory / 'fresh-profile'), '--dump-dom', page.as_uri()],
            capture_output=True, text=True, timeout=40, check=True)
        match = re.search(r'<output>(.*?)</output>', observed.stdout, re.S)
        if not match:
            raise AssertionError('native fixture did not report canvas pixels')
        result = json.loads(match.group(1))
        if (len(result['rows']) != 4
                or not all(row['differing'] == 0 and row['hits'] == 1 for row in result['rows'])):
            raise AssertionError('native replay changed pixel channels: ' + json.dumps(result))
        result['chrome'] = subprocess.check_output([chrome, '--version'], text=True).strip()
        result['scope'] = 'Native headless Canvas fixture only; no engine, game, user profile or timing claim.'
        return result


if __name__ == '__main__':
    if sys.argv[1:] == ['--native-canvas']:
        print(json.dumps(native_pixel_fixture(), indent=2))
    else:
        unittest.main()
