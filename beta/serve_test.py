#!/usr/bin/env python3
"""Exercise the local WASM channel against a real HTTP server."""

import functools
import http.client
import pathlib
import tempfile
import threading

from serve import Handler, Server


def hit(port, path):
    connection = http.client.HTTPConnection("127.0.0.1", port, timeout=2)
    connection.request("GET", path)
    response = connection.getresponse()
    headers = {name.lower(): value for name, value in response.getheaders()}
    result = response.status, headers, response.read()
    connection.close()
    return result


def main():
    with tempfile.TemporaryDirectory() as temporary:
        root = pathlib.Path(temporary)
        # ⚠ THE LANE IS THE ROOT. `publish.sh` emits one complete lane per
        # revision — viewer at its root, /home and /download beside it — and
        # `channel_path` maps /wasm/ straight onto it. This fixture used to
        # build a `test/` subdirectory, which is where the viewer lived before
        # that move; the assertions below then read the landing page and
        # compared it to the viewer.
        (root / "index.html").write_text("wasm viewer", encoding="utf-8")
        (root / "build.json").write_text('{"commit":"latest"}', encoding="utf-8")
        (root / "civvis.wasm").write_bytes(b"wasm")
        (root / "home").mkdir()
        (root / "home" / "index.html").write_text("landing", encoding="utf-8")

        # `Handler` requires a rating host and never consults it on any route
        # this script exercises — /roster and /result are `test_serve.py`'s,
        # with a fake. Passing None keeps the two files testing different
        # halves rather than one of them re-implementing the other's stub.
        handler = functools.partial(Handler, directory=root,
                                    rating_host=None)
        with Server(("127.0.0.1", 0), handler) as server:
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            port = server.server_address[1]

            status, headers, _ = hit(port, "/wasm?game=7311")
            assert status == 301
            assert headers["location"] == "/wasm/?game=7311"
            assert headers["cache-control"] == "no-store"

            status, headers, body = hit(port, "/wasm/")
            assert status == 200
            assert body == b"wasm viewer"
            assert headers["cache-control"] == "no-store"

            status, headers, body = hit(port, "/wasm/build.json?fresh=1")
            assert status == 200
            assert body == b'{"commit":"latest"}'
            assert headers["content-type"] == "application/json"

            status, headers, body = hit(port, "/wasm/civvis.wasm")
            assert status == 200
            assert body == b"wasm"
            assert headers["content-type"] == "application/wasm"

            status, _, body = hit(port, "/home/")
            assert status == 200
            assert body == b"landing"

            server.shutdown()
            thread.join(timeout=2)

    print("the local WASM channel routes correctly.")


if __name__ == "__main__":
    main()
