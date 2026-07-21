import json
import threading
import urllib.request

import pytest

from civvis.server import make_server


@pytest.fixture(scope="module")
def srv():
    httpd = make_server(port=0, num_players=2, width=16, height=12, seed=3,
                        max_turns=100, num_city_states=1)
    t = threading.Thread(target=httpd.serve_forever, daemon=True)
    t.start()
    yield f"http://127.0.0.1:{httpd.server_address[1]}"
    httpd.shutdown()


def get(url):
    with urllib.request.urlopen(url, timeout=10) as r:
        return json.loads(r.read())


def post(url, obj):
    req = urllib.request.Request(url, data=json.dumps(obj).encode(),
                                 headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=10) as r:
        return json.loads(r.read())


def test_index_served(srv):
    with urllib.request.urlopen(srv + "/", timeout=10) as r:
        body = r.read().decode()
    assert "<canvas" in body and "Civilization VIS" in body


def test_state_and_actions(srv):
    st = get(srv + "/state")
    assert st["turn"] >= 1
    assert st["legal_actions"]
    assert any(a["type"] == "end_turn" for a in st["legal_actions"])
    # illegal action reports error, state unchanged
    bad = post(srv + "/action", {"action": {"type": "research", "tech": "gunpowder"}})
    assert bad["error"]
    # end turn: AIs run, control returns to player 0
    st2 = post(srv + "/action", {"action": {"type": "end_turn"}})
    assert st2["error"] is None
    assert st2["current"] == 0 or st2["winner"] is not None
    assert st2["turn"] >= 2


def test_new_game(srv):
    st = post(srv + "/new", {"seed": 77})
    assert st["turn"] == 1


def test_rules_endpoint(srv):
    r = get(srv + "/rules")
    assert "gunpowder" in r["techs"]
    assert "guilds" in r["civics"]
    assert r["units"]["settler"]["sight"] == 3
