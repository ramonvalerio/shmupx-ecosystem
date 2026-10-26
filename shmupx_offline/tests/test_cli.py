import contextlib
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch

from shmupx.cli import main
from shmupx.registry import ROOT, get_validator


class ValidatorTests(unittest.TestCase):
    def test_help_uses_utf8_in_windows_pipes(self):
        result = subprocess.run([sys.executable, str(ROOT / "analyze.py"), "--help"],
                                capture_output=True, encoding="utf-8", check=True)
        self.assertIn("cotação US$→R$", result.stdout)

    def test_registry_rejects_unknown_ids_and_paths(self):
        for game in ("../earthion", "Earthion", "unknown", "earthion.py"):
            with self.assertRaises(ValueError):
                get_validator(game)

    def test_listing_does_not_load_computer_vision(self):
        script = "from shmupx.cli import main; main(['--list-games']); import sys; assert 'cv2' not in sys.modules; assert 'shmupx.grok' not in sys.modules"
        result = subprocess.run([sys.executable, "-c", script], cwd=ROOT,
                                capture_output=True, text=True, check=True)
        self.assertEqual(json.loads(result.stdout)[0]["id"], "earthion")

    def test_entrypoint_works_outside_project_directory(self):
        with tempfile.TemporaryDirectory() as directory:
            result = subprocess.run([sys.executable, str(ROOT / "analyze.py"), "--list-games"],
                                    cwd=directory, capture_output=True, text=True, check=True)
        self.assertEqual(json.loads(result.stdout)[0]["version"], "1.0.0")

    def test_earthion_resources_and_legacy_import(self):
        from shmupx.hud import GameConfig
        from shmupx.pipeline import Analyzer as LegacyAnalyzer
        from shmupx.validators.earthion import Analyzer
        self.assertIs(LegacyAnalyzer, Analyzer)
        config = GameConfig(ROOT, "earthion")
        self.assertTrue(config.variants)
        self.assertEqual(len(config.digit_sets), 3)
        for digits in config.digit_sets.values():
            self.assertEqual(set(digits), set("0123456789"))

    def test_external_audit_requires_explicit_opt_in(self):
        with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit) as error:
            main(["video.mp4", "--game", "earthion", "--player", "P", "--grok-always"])
        self.assertEqual(error.exception.code, 2)

    def test_json_output_separates_logs_and_releases_video(self):
        analyzer = Mock()
        analyzer.v.duration = 60
        analyzer.v.frames_read = 30
        result = dict(hi=1234, confidence="high", continues=0, lives_lost=1,
                      game_over=False, attempts=0, observation=None, deaths=[])
        def run():
            print("progresso do script")
            return result
        analyzer.run.side_effect = run
        spec = Mock()
        spec.create.return_value = analyzer
        stdout, stderr = io.StringIO(), io.StringIO()
        with tempfile.TemporaryDirectory() as directory:
            video = Path(directory) / "video.mp4"
            video.touch()
            output = Path(directory) / "result.json"
            with patch("shmupx.cli.get_validator", return_value=spec), \
                 contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
                main([str(video), "--game", "earthion", "--player", "Jogador",
                      "--out", str(output)])
            payload = json.loads(stdout.getvalue())
            self.assertEqual(payload, json.loads(output.read_text(encoding="utf-8")))
        self.assertEqual(payload["earthion"]["hi-score"], 1234)
        self.assertEqual(payload["earthion"]["total_tokens"], None)
        self.assertIn("progresso", stderr.getvalue())
        analyzer.v.cap.release.assert_called_once()

    def test_failure_releases_video_without_success_json(self):
        analyzer = Mock()
        analyzer.run.side_effect = SystemExit("HUD não encontrado")
        spec = Mock()
        spec.create.return_value = analyzer
        output = io.StringIO()
        with tempfile.TemporaryDirectory() as directory:
            video = Path(directory) / "video.mp4"
            video.touch()
            with patch("shmupx.cli.get_validator", return_value=spec), \
                 contextlib.redirect_stdout(output), self.assertRaises(SystemExit):
                main([str(video), "--game", "earthion", "--player", "P"])
        self.assertEqual(output.getvalue(), "")
        analyzer.v.cap.release.assert_called_once()


if __name__ == "__main__":
    unittest.main()
