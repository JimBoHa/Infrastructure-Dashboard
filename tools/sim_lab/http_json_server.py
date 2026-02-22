#!/usr/bin/env python3
from __future__ import annotations

import os
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer
import socketserver
from pathlib import Path


class FixtureHandler(BaseHTTPRequestHandler):
    fixture_bytes: bytes = b"{}"
    fixture_name: str = "fixture.json"

    def _send_json(self) -> None:
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(self.fixture_bytes)))
        self.send_header("X-Sim-Lab", "true")
        self.end_headers()
        self.wfile.write(self.fixture_bytes)

    def do_GET(self) -> None:  # noqa: N802 - signature required by BaseHTTPRequestHandler
        if self.path in ("/", f"/{self.fixture_name}", "/healthz", "/status"):
            self._send_json()
            return
        self.send_response(404)
        self.send_header("Content-Type", "text/plain")
        self.end_headers()
        self.wfile.write(b"Not found")

    def log_message(self, format, *args):  # noqa: A003 - signature required by BaseHTTPRequestHandler
        sys.stdout.write("[sim-lab] " + format % args + "\n")


class FixtureServer(HTTPServer):
    def server_bind(self) -> None:
        socketserver.TCPServer.server_bind(self)
        host, port = self.server_address[:2]
        self.server_name = host
        self.server_port = port


def main() -> None:
    fixture_file = os.getenv("FIXTURE_FILE", "/app/fixtures/fixture.json")
    port = int(os.getenv("PORT", "9101"))

    fixture_path = Path(fixture_file)
    if not fixture_path.exists():
        raise FileNotFoundError(f"Fixture file not found: {fixture_path}")

    FixtureHandler.fixture_bytes = fixture_path.read_bytes()
    FixtureHandler.fixture_name = fixture_path.name

    server = FixtureServer(("0.0.0.0", port), FixtureHandler)
    print(f"[sim-lab] serving {fixture_path.name} on port {port}")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


if __name__ == "__main__":
    main()
