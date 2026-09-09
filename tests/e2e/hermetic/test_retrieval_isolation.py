"""Keep the single-document retrieval double isolated from source workloads.

The default suite runs the loopback HTTP reproduction. The real composed
regression requires an existing target/debug/axon, mcporter on PATH, and the
launcher dependencies; it does not build anything. Run explicitly with:

AXON_E2E_REAL_ORDER_REGRESSION=1 RUST_MIN_STACK=8388608 \\
python3 -m unittest tests.e2e.hermetic.test_retrieval_isolation -v

It launches real providers/transports, stops at the observability boundary, and
uses the real composed finally block to verify authoritative cleanup.
"""
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import threading
import unittest
import urllib.request
from http.server import ThreadingHTTPServer
from unittest import mock

ROOT = Path(__file__).resolve().parents[3]


class RetrievalIsolationTests(unittest.TestCase):
    @unittest.skipUnless(os.environ.get("AXON_E2E_REAL_ORDER_REGRESSION") == "1",
                         "explicit real built-binary/launcher integration")
    def test_all_retrieval_transports_precede_observability_source(self):
        spec = importlib.util.spec_from_file_location(
            "composed_order_regression", ROOT / "tests/e2e/hermetic/real_composed.py")
        composed = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(composed)
        evidence = {}

        class ObservabilityBoundaryReached(Exception):
            pass

        def forwarding(surface, original, command):
            def invoke(*args, **kwargs):
                result = original(*args, **kwargs)
                if command(args) == "query":
                    evidence[surface] = result[0]
                return result
            return invoke

        def observability_boundary(*_args):
            self.assertEqual({"cli", "http", "mcp"}, set(evidence),
                             "real retrieval results must exist before source indexing")
            for surface, result in evidence.items():
                self.assertIn("amber", json.dumps(result).casefold(), surface)
            raise ObservabilityBoundaryReached()

        with mock.patch.object(composed.execute, "invoke", forwarding(
                "cli", composed.execute.invoke, lambda args: args[1][0])), \
             mock.patch.object(composed.execute, "invoke_http", forwarding(
                "http", composed.execute.invoke_http, lambda args: args[2])), \
             mock.patch.object(composed.execute, "invoke_mcp", forwarding(
                "mcp", composed.execute.invoke_mcp, lambda args: args[2])), \
             mock.patch.object(composed, "verify_observability", observability_boundary):
            with self.assertRaises(ObservabilityBoundaryReached) as caught:
                composed.main()
        self.assertFalse(getattr(caught.exception, "__notes__", []), "real teardown must succeed")

    def test_real_http_double_exposes_uuid_dependent_canary_contamination(self):
        spec = importlib.util.spec_from_file_location(
            "qdrant_isolation_fixture", ROOT / "tests/e2e/fixtures/teardown/qdrant_contract.py")
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        with tempfile.TemporaryDirectory() as directory:
            state = Path(directory) / "state.json"
            state.write_text(json.dumps({"collections": {}, "aliases": {}}))

            class Handler(module.Handler):
                state_path = state

            server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
            thread = threading.Thread(target=server.serve_forever)
            thread.start()

            def call(method, path, body):
                request = urllib.request.Request(
                    f"http://127.0.0.1:{server.server_port}/collections/proof{path}",
                    data=json.dumps(body).encode(), method=method,
                    headers={"Content-Type": "application/json"})
                with urllib.request.urlopen(request, timeout=3) as response:
                    return json.load(response)["result"]

            def first():
                return call("POST", "/points/query", {"query": [1., 0.], "limit": 1})["points"][0]["id"]

            try:
                call("PUT", "", {"vectors": {"size": 2, "distance": "Cosine"}})
                atlas = "11111111-1111-4111-8111-111111111111"
                call("PUT", "/points", {"points": [{"id": atlas, "vector": [1., 0.],
                     "payload": {"chunk_text": "Atlas emits amber"}}]})
                self.assertEqual(atlas, first())
                for identity, expected in (
                    ("00000000-0000-4000-8000-000000000001", "00000000-0000-4000-8000-000000000001"),
                    ("ffffffff-ffff-4fff-8fff-ffffffffffff", atlas),
                ):
                    call("PUT", "/points", {"points": [{"id": identity, "vector": [0., 1.],
                         "payload": {"chunk_text": "Observable beacon telemetry canary"}}]})
                    self.assertEqual(expected, first())
                    call("POST", "/points/delete", {"points": [identity]})
                    self.assertEqual(atlas, first())
            finally:
                server.shutdown()
                server.server_close()
                thread.join()
