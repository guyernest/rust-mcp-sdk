#!/usr/bin/env python3
"""Static file server for the web-channel-client browser demo.

Serves this directory on http://127.0.0.1:8080 — the origin the bundled IdP
registers the client `redirect_uri` against (`http://127.0.0.1:8080/callback`).

Why not plain `python3 -m http.server`? The OAuth redirect comes back to
`/callback`, and a stock static server has two problems with that path:

  1. There is no file named `callback`, so it 404s before the SPA can run.
  2. Working around (1) by copying `index.html` to a file named `callback`
     makes the server send it as `application/octet-stream` (no extension), so
     the browser DOWNLOADS the file instead of rendering it.

This server maps GET `/callback` to `index.html` with `text/html`, so the
redirect lands on the app and `main.js` reads `?code=&state=` from the URL. The
browser address bar still shows `/callback?...`; only the server-side file
lookup is rewritten.

Usage:
    python3 serve.py            # serves . on 127.0.0.1:8080
    PORT=9000 python3 serve.py  # override the port
"""

import http.server
import os
import socketserver

DIRECTORY = os.path.dirname(os.path.abspath(__file__))
PORT = int(os.environ.get("PORT", "8080"))


class CallbackAwareHandler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=DIRECTORY, **kwargs)

    def do_GET(self):
        path = self.path.split("?", 1)[0]
        if path in ("/callback", "/callback/"):
            # Serve the SPA; main.js reads ?code=&state= from window.location.
            self.path = "/index.html"
        return super().do_GET()


if __name__ == "__main__":
    with socketserver.TCPServer(("127.0.0.1", PORT), CallbackAwareHandler) as httpd:
        print(f"Serving {DIRECTORY} on http://127.0.0.1:{PORT}")
        print("Open http://127.0.0.1:8080/index.html and click Login.")
        httpd.serve_forever()
