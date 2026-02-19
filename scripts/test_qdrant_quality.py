import importlib.util
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

SCRIPT_PATH = Path(__file__).with_name("qdrant-quality.py")
SPEC = importlib.util.spec_from_file_location("qdrant_quality", SCRIPT_PATH)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError("Failed to load qdrant-quality.py")
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


class QdrantQualityTests(unittest.TestCase):
    def test_canonicalize_url_for_dedupe_strips_fragment_and_default_port(self):
        url = "https://example.com:443/docs/page/#section"
        normalized = MODULE.canonicalize_url_for_dedupe(url)
        self.assertEqual(normalized, "https://example.com/docs/page")

    def test_canonicalize_url_for_dedupe_trailing_slash_non_root(self):
        url = "https://example.com/docs/path/"
        normalized = MODULE.canonicalize_url_for_dedupe(url)
        self.assertEqual(normalized, "https://example.com/docs/path")

    def test_path_prefix_excluded_segment_boundary(self):
        self.assertTrue(MODULE.path_prefix_excluded("/de", "/de"))
        self.assertTrue(MODULE.path_prefix_excluded("/de/docs", "/de"))
        self.assertFalse(MODULE.path_prefix_excluded("/debug", "/de"))

    def test_parse_payload_timestamp_supports_z_suffix(self):
        payload = {"scraped_at": "2026-02-19T03:07:45.206826Z"}
        ts = MODULE.parse_payload_timestamp(payload)
        self.assertIsNotNone(ts)
        self.assertEqual(ts.tzinfo, MODULE.UTC)

    def test_confirm_destructive_action_requires_yes_in_non_interactive(self):
        with patch.object(sys.stdin, "isatty", return_value=False):
            with self.assertRaises(RuntimeError):
                MODULE.confirm_destructive_action(
                    "delete-duplicates",
                    yes=False,
                    dry_run=False,
                    target="collection 'cortex'",
                )

    def test_confirm_destructive_action_allows_yes_without_prompt(self):
        MODULE.confirm_destructive_action(
            "delete-duplicates",
            yes=True,
            dry_run=False,
            target="collection 'cortex'",
        )


if __name__ == "__main__":
    unittest.main()
