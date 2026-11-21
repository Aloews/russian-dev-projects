from __future__ import annotations

import json
import unittest
import urllib.error
from unittest.mock import patch

import starimg_probe


class FakeResponse:
    def __init__(self, payload, status=200):
        self.payload = payload
        self.status = status

    def __enter__(self):
        return self

    def __exit__(self, *_args):
        return None

    def read(self):
        return json.dumps(self.payload).encode("utf-8")


class StarimgProbeTests(unittest.TestCase):
    def test_missing_key_does_not_make_network_request(self):
        with patch.object(starimg_probe.urllib.request, "urlopen") as request:
            result = starimg_probe.probe("https://ai.starimg.ru", "")
        request.assert_not_called()
        self.assertFalse(result["tool_ok"])
        self.assertEqual(result["error_type"], "missing_key")

    def test_prefers_fable_52_and_requires_exact_tool_call(self):
        catalogue = {"data": [{"id": "claude-sonnet-4.5"}, {"id": "fable-5.2"}]}
        message = {
            "content": [
                {"type": "tool_use", "name": "ping", "input": {"value": "STARIMG_TOOL_OK"}}
            ]
        }
        with patch.object(
            starimg_probe.urllib.request,
            "urlopen",
            side_effect=[FakeResponse(catalogue), FakeResponse(message)],
        ) as request:
            result = starimg_probe.probe("https://ai.starimg.ru/v1", "secret-value")
        self.assertTrue(result["tool_ok"])
        self.assertEqual(result["model"], "fable-5.2")
        self.assertEqual(result["base_url"], "https://ai.starimg.ru")
        self.assertEqual(request.call_count, 2)
        self.assertNotIn("secret-value", json.dumps(result))

    def test_falls_back_to_catalogued_claude_when_fable_is_absent(self):
        catalogue = {"data": [{"id": "claude-opus-4.1"}]}
        message = {
            "content": [
                {"type": "tool_use", "name": "ping", "input": {"value": "STARIMG_TOOL_OK"}}
            ]
        }
        with patch.object(
            starimg_probe.urllib.request,
            "urlopen",
            side_effect=[FakeResponse(catalogue), FakeResponse(message)],
        ):
            result = starimg_probe.probe("https://ai.starimg.ru", "secret-value")
        self.assertTrue(result["tool_ok"])
        self.assertEqual(result["model"], "claude-opus-4.1")

    def test_explicit_fable_model_must_exist_in_live_catalogue(self):
        with patch.object(
            starimg_probe.urllib.request,
            "urlopen",
            return_value=FakeResponse({"data": [{"id": "fable-5.1"}]}),
        ):
            result = starimg_probe.probe(
                "https://ai.starimg.ru", "secret-value", fable_model="fable-5.2"
            )
        self.assertFalse(result["tool_ok"])
        self.assertEqual(result["error_type"], "configured_fable_model_not_found")

    def test_invalid_https_origin_is_rejected_before_network(self):
        with patch.object(starimg_probe.urllib.request, "urlopen") as request:
            result = starimg_probe.probe("http://ai.starimg.ru", "secret-value")
        request.assert_not_called()
        self.assertFalse(result["tool_ok"])
        self.assertEqual(result["error_type"], "invalid_base_url")

    def test_model_id_injection_is_not_exported_to_github_output(self):
        catalogue = {"data": [{"id": "fable-5.2\n::warning::injected"}]}
        with patch.object(
            starimg_probe.urllib.request,
            "urlopen",
            return_value=FakeResponse(catalogue),
        ):
            result = starimg_probe.probe("https://ai.starimg.ru", "secret-value")
        self.assertFalse(result["tool_ok"])
        self.assertEqual(result["error_type"], "no_verified_anthropic_model")

    def test_tool_call_without_expected_input_is_rejected(self):
        catalogue = {"data": [{"id": "fable-5.2"}]}
        message = {
            "content": [
                {"type": "tool_use", "name": "ping", "input": {"value": "WRONG"}}
            ]
        }
        with patch.object(
            starimg_probe.urllib.request,
            "urlopen",
            side_effect=[FakeResponse(catalogue), FakeResponse(message)],
        ):
            result = starimg_probe.probe("https://ai.starimg.ru", "secret-value")
        self.assertFalse(result["tool_ok"])
        self.assertEqual(result["error_type"], "tool_call_not_verified")

    def test_provider_http_error_is_redacted(self):
        error = urllib.error.HTTPError(
            "https://ai.starimg.ru/v1/models",
            401,
            "unauthorized",
            {},
            None,
        )
        with patch.object(starimg_probe.urllib.request, "urlopen", side_effect=error):
            result = starimg_probe.probe("https://ai.starimg.ru", "secret-value")
        self.assertFalse(result["tool_ok"])
        self.assertEqual(result["error_type"], "provider_http_error")
        self.assertNotIn("secret-value", json.dumps(result))


if __name__ == "__main__":
    unittest.main()
