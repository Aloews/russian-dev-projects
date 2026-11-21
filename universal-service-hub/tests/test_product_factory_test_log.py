import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
from validate_product_factory_test_log import parse_report


class ProductFactoryTestLogTests(unittest.TestCase):
    def parse(self, log, workspace=None):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            report = root / "verification.log"
            report.write_text(log, encoding="utf-8")
            return parse_report(report, workspace or root)

    def test_rust_test_gate_requires_and_counts_real_runner_summary(self):
        result = self.parse(
            "## Rust tests\n$ cargo test --workspace --locked\n"
            "test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n"
            "exit_code=0\n"
        )
        self.assertEqual(result["tests_passed"], 4)
        self.assertEqual(result["errors"], [])

    def test_python_all_skipped_is_rejected(self):
        result = self.parse(
            "## Python tests\n$ python -m pytest -q\n"
            "3 skipped in 0.01s\nexit_code=0\n"
        )
        self.assertTrue(result["errors"])

    def test_zero_test_run_is_rejected(self):
        result = self.parse(
            "## Frontend tests\n$ npm test\nNo tests found, exiting with code 0\nexit_code=0\n"
        )
        self.assertTrue(result["errors"])

    def test_positive_pytest_count_is_accepted(self):
        result = self.parse(
            "## Python tests\n$ python -m pytest -q\n"
            "4 passed in 0.04s\nexit_code=0\n"
        )
        self.assertEqual(result["tests_passed"], 4)
        self.assertEqual(result["tests_skipped"], 0)
        self.assertEqual(result["errors"], [])

    def test_partially_skipped_required_gate_is_rejected(self):
        result = self.parse(
            "## Python tests\n$ python -m pytest -q\n"
            "4 passed, 1 skipped in 0.04s\nexit_code=0\n"
        )
        self.assertTrue(result["errors"])

    def test_successful_build_message_cannot_impersonate_test_count(self):
        result = self.parse(
            "## Frontend tests\n$ npm run test\n"
            "Build successful. 999 passed in deployment metadata.\nexit_code=0\n"
        )
        self.assertTrue(result["errors"])

    def test_go_json_counts_individual_tests_not_package_status(self):
        result = self.parse(
            "## Go tests\n$ go test -json ./...\n"
            '{"Action":"pass","Package":"example/pkg","Test":"TestOne"}\n'
            '{"Action":"pass","Package":"example/pkg"}\n'
            "exit_code=0\n"
        )
        self.assertEqual(result["tests_passed"], 1)
        self.assertEqual(result["errors"], [])

    def test_maven_xml_reports_are_counted(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            report = root / "verification.log"
            report.write_text(
                "## Maven tests\n$ mvn -B test\nBUILD SUCCESS\nexit_code=0\n",
                encoding="utf-8",
            )
            xml = root / "target" / "surefire-reports" / "TEST-example.xml"
            xml.parent.mkdir(parents=True)
            xml.write_text(
                '<testsuite tests="3" failures="0" errors="0" skipped="1"></testsuite>',
                encoding="utf-8",
            )
            result = parse_report(report, root)
            self.assertEqual(result["tests_passed"], 2)
            self.assertEqual(result["tests_skipped"], 1)
            self.assertEqual(result["errors"], [])

    def test_zero_exit_with_reported_failures_is_rejected(self):
        result = self.parse(
            "## Frontend tests\n$ npm test\nTests: 1 failed, 2 passed, 3 total\nexit_code=0\n"
        )
        self.assertTrue(result["errors"])

    def test_no_test_gate_is_rejected(self):
        result = self.parse("## Rust formatting\n$ cargo fmt --check\nexit_code=0\n")
        self.assertTrue(result["errors"])

    def test_test_gate_nonzero_exit_is_rejected(self):
        result = self.parse(
            "## Python tests\n$ pytest -q\n2 passed\nexit_code=1\n"
        )
        self.assertTrue(result["errors"])


if __name__ == "__main__":
    unittest.main()
